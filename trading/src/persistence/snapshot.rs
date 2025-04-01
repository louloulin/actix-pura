use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use log::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use tokio::sync::RwLock;

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Snapshot not found: {0}")]
    NotFound(String),
    
    #[error("Snapshot error: {0}")]
    Other(String),
}

/// 快照元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// 快照ID
    pub id: String,
    /// 快照创建时间
    pub created_at: DateTime<Utc>,
    /// 快照描述
    pub description: Option<String>,
    /// 快照版本
    pub version: String,
    /// 节点ID
    pub node_id: String,
}

/// 系统状态快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    /// 快照元数据
    pub metadata: SnapshotMetadata,
    /// 快照数据 (序列化后的JSON)
    pub data: String,
}

impl SystemSnapshot {
    /// 创建新的系统快照
    pub fn new<T: Serialize>(
        node_id: impl Into<String>,
        description: Option<String>,
        version: impl Into<String>,
        data: &T,
    ) -> Result<Self, SnapshotError> {
        let data_json = serde_json::to_string(data)
            .map_err(|e| SnapshotError::Other(format!("Failed to serialize snapshot data: {}", e)))?;
            
        let metadata = SnapshotMetadata {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            description,
            version: version.into(),
            node_id: node_id.into(),
        };
        
        Ok(Self {
            metadata,
            data: data_json,
        })
    }
    
    /// 从快照恢复数据
    pub fn restore<T: for<'de> Deserialize<'de>>(&self) -> Result<T, SnapshotError> {
        let data = serde_json::from_str(&self.data)?;
        Ok(data)
    }
}

/// 快照提供者接口
#[async_trait::async_trait]
pub trait SnapshotProvider: Send + Sync {
    /// 保存系统快照
    async fn save_snapshot(&self, snapshot: SystemSnapshot) -> Result<(), SnapshotError>;
    
    /// 加载指定ID的快照
    async fn load_snapshot(&self, snapshot_id: &str) -> Result<SystemSnapshot, SnapshotError>;
    
    /// 获取最新的快照
    async fn get_latest_snapshot(&self) -> Result<Option<SystemSnapshot>, SnapshotError>;
    
    /// 列出所有快照
    async fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>, SnapshotError>;
    
    /// 删除快照
    async fn delete_snapshot(&self, snapshot_id: &str) -> Result<(), SnapshotError>;
}

/// 文件系统快照实现
pub struct FileSystemSnapshot {
    snapshot_dir: PathBuf,
    cache: Arc<RwLock<HashMap<String, SystemSnapshot>>>,
}

impl FileSystemSnapshot {
    /// 创建新的文件系统快照提供者
    pub fn new<P: AsRef<Path>>(snapshot_dir: P) -> Result<Self, SnapshotError> {
        let snapshot_dir = snapshot_dir.as_ref().to_path_buf();
        
        // 确保快照目录存在
        fs::create_dir_all(&snapshot_dir)?;
        
        Ok(Self {
            snapshot_dir,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// 获取快照文件路径
    fn get_snapshot_path(&self, snapshot_id: &str) -> PathBuf {
        self.snapshot_dir.join(format!("{}.snap", snapshot_id))
    }
    
    /// 获取元数据文件路径
    fn get_metadata_path(&self) -> PathBuf {
        self.snapshot_dir.join("metadata.json")
    }
    
    /// 加载快照元数据
    async fn load_metadata(&self) -> Result<Vec<SnapshotMetadata>, SnapshotError> {
        let metadata_path = self.get_metadata_path();
        
        if !metadata_path.exists() {
            return Ok(Vec::new());
        }
        
        let mut file = File::open(metadata_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        
        let metadata: Vec<SnapshotMetadata> = serde_json::from_str(&content)?;
        Ok(metadata)
    }
    
    /// 保存快照元数据
    async fn save_metadata(&self, metadata: &[SnapshotMetadata]) -> Result<(), SnapshotError> {
        let metadata_path = self.get_metadata_path();
        let serialized = serde_json::to_string_pretty(metadata)?;
        
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(metadata_path)?;
            
        file.write_all(serialized.as_bytes())?;
        file.flush()?;
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl SnapshotProvider for FileSystemSnapshot {
    async fn save_snapshot(&self, snapshot: SystemSnapshot) -> Result<(), SnapshotError> {
        let snapshot_path = self.get_snapshot_path(&snapshot.metadata.id);
        let serialized = bincode::serialize(&snapshot)?;
        
        // 写入快照文件
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&snapshot_path)?;
            
        file.write_all(&serialized)?;
        file.flush()?;
        
        // 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(snapshot.metadata.id.clone(), snapshot.clone());
        }
        
        // 更新元数据
        let mut metadata = self.load_metadata().await?;
        metadata.push(snapshot.metadata.clone());
        
        // 按时间排序
        metadata.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        // 保存元数据
        self.save_metadata(&metadata).await?;
        
        debug!("Saved snapshot: {}", snapshot.metadata.id);
        Ok(())
    }
    
    async fn load_snapshot(&self, snapshot_id: &str) -> Result<SystemSnapshot, SnapshotError> {
        // 先检查缓存
        {
            let cache = self.cache.read().await;
            if let Some(snapshot) = cache.get(snapshot_id) {
                return Ok(snapshot.clone());
            }
        }
        
        // 从文件加载
        let snapshot_path = self.get_snapshot_path(snapshot_id);
        
        if !snapshot_path.exists() {
            return Err(SnapshotError::NotFound(format!("Snapshot {} not found", snapshot_id)));
        }
        
        let mut file = File::open(snapshot_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        
        let snapshot: SystemSnapshot = bincode::deserialize(&buffer)?;
        
        // 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(snapshot_id.to_string(), snapshot.clone());
        }
        
        Ok(snapshot)
    }
    
    async fn get_latest_snapshot(&self) -> Result<Option<SystemSnapshot>, SnapshotError> {
        let metadata = self.load_metadata().await?;
        
        if metadata.is_empty() {
            return Ok(None);
        }
        
        // 元数据已经按时间排序，第一个是最新的
        let latest_id = &metadata[0].id;
        let snapshot = self.load_snapshot(latest_id).await?;
        
        Ok(Some(snapshot))
    }
    
    async fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>, SnapshotError> {
        self.load_metadata().await
    }
    
    async fn delete_snapshot(&self, snapshot_id: &str) -> Result<(), SnapshotError> {
        let snapshot_path = self.get_snapshot_path(snapshot_id);
        
        // 从缓存中删除
        {
            let mut cache = self.cache.write().await;
            cache.remove(snapshot_id);
        }
        
        // 从文件系统删除
        if snapshot_path.exists() {
            fs::remove_file(snapshot_path)?;
        } else {
            warn!("Attempt to delete non-existent snapshot: {}", snapshot_id);
        }
        
        // 更新元数据
        let mut metadata = self.load_metadata().await?;
        metadata.retain(|m| m.id != snapshot_id);
        self.save_metadata(&metadata).await?;
        
        debug!("Deleted snapshot: {}", snapshot_id);
        Ok(())
    }
}

/// 内存快照实现（用于测试）
pub struct InMemorySnapshot {
    snapshots: Arc<RwLock<HashMap<String, SystemSnapshot>>>,
    metadata: Arc<RwLock<Vec<SnapshotMetadata>>>,
}

impl InMemorySnapshot {
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl SnapshotProvider for InMemorySnapshot {
    async fn save_snapshot(&self, snapshot: SystemSnapshot) -> Result<(), SnapshotError> {
        let mut snapshots = self.snapshots.write().await;
        let mut metadata = self.metadata.write().await;
        
        snapshots.insert(snapshot.metadata.id.clone(), snapshot.clone());
        metadata.push(snapshot.metadata);
        
        // 按时间排序
        metadata.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        Ok(())
    }
    
    async fn load_snapshot(&self, snapshot_id: &str) -> Result<SystemSnapshot, SnapshotError> {
        let snapshots = self.snapshots.read().await;
        
        if let Some(snapshot) = snapshots.get(snapshot_id) {
            Ok(snapshot.clone())
        } else {
            Err(SnapshotError::NotFound(format!("Snapshot {} not found", snapshot_id)))
        }
    }
    
    async fn get_latest_snapshot(&self) -> Result<Option<SystemSnapshot>, SnapshotError> {
        let metadata = self.metadata.read().await;
        
        if metadata.is_empty() {
            return Ok(None);
        }
        
        let latest_id = &metadata[0].id;
        let snapshots = self.snapshots.read().await;
        
        if let Some(snapshot) = snapshots.get(latest_id) {
            Ok(Some(snapshot.clone()))
        } else {
            Err(SnapshotError::Other(format!("Inconsistent state: metadata references snapshot {} which doesn't exist", latest_id)))
        }
    }
    
    async fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>, SnapshotError> {
        let metadata = self.metadata.read().await;
        Ok(metadata.clone())
    }
    
    async fn delete_snapshot(&self, snapshot_id: &str) -> Result<(), SnapshotError> {
        let mut snapshots = self.snapshots.write().await;
        let mut metadata = self.metadata.write().await;
        
        snapshots.remove(snapshot_id);
        metadata.retain(|m| m.id != snapshot_id);
        
        Ok(())
    }
} 