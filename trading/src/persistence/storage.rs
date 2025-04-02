use std::path::{Path, PathBuf};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::collections::HashMap;

use log::{warn, error, debug};
use serde::{Serialize, Deserialize};
use bincode;
use tokio::sync::RwLock;


#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Item not found: {0}")]
    NotFound(String),
    
    #[error("Storage error: {0}")]
    Other(String),
}

/// 存储类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageType {
    Order,
    Account,
    Trade,
}

impl StorageType {
    fn as_str(&self) -> &'static str {
        match self {
            StorageType::Order => "orders",
            StorageType::Account => "accounts",
            StorageType::Trade => "trades",
        }
    }
}

/// 存储接口，定义数据持久化操作
#[async_trait::async_trait]
pub trait StorageProvider: Send + Sync {
    async fn save<T: Serialize + Send + Sync>(&self, storage_type: StorageType, id: &str, item: &T) -> Result<(), StorageError>;
    async fn load<T: for<'de> Deserialize<'de> + Send + Sync>(&self, storage_type: StorageType, id: &str) -> Result<T, StorageError>;
    async fn delete(&self, storage_type: StorageType, id: &str) -> Result<(), StorageError>;
    async fn list<T: for<'de> Deserialize<'de> + Send + Sync>(&self, storage_type: StorageType) -> Result<Vec<T>, StorageError>;
}

/// 文件系统存储实现
pub struct FileSystemStorage {
    base_dir: PathBuf,
    cache: Arc<RwLock<HashMap<(StorageType, String), Vec<u8>>>>,
}

impl FileSystemStorage {
    /// 创建新的文件系统存储
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self, StorageError> {
        let base_dir = base_dir.as_ref().to_path_buf();
        
        // 确保基础目录存在
        fs::create_dir_all(&base_dir)?;
        
        // 确保各类型存储目录存在
        fs::create_dir_all(base_dir.join(StorageType::Order.as_str()))?;
        fs::create_dir_all(base_dir.join(StorageType::Account.as_str()))?;
        fs::create_dir_all(base_dir.join(StorageType::Trade.as_str()))?;
        
        Ok(Self {
            base_dir,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// 获取存储项的文件路径
    fn get_file_path(&self, storage_type: StorageType, id: &str) -> PathBuf {
        self.base_dir.join(storage_type.as_str()).join(format!("{}.bin", id))
    }
}

#[async_trait::async_trait]
impl StorageProvider for FileSystemStorage {
    async fn save<T: Serialize + Send + Sync>(&self, storage_type: StorageType, id: &str, item: &T) -> Result<(), StorageError> {
        let serialized = bincode::serialize(item)?;
        let file_path = self.get_file_path(storage_type, id);
        
        // 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert((storage_type, id.to_string()), serialized.clone());
        }
        
        // 写入文件
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)?;
            
        file.write_all(&serialized)?;
        file.flush()?;
        
        debug!("Saved {} with id {} to storage", storage_type.as_str(), id);
        Ok(())
    }
    
    async fn load<T: for<'de> Deserialize<'de> + Send + Sync>(&self, storage_type: StorageType, id: &str) -> Result<T, StorageError> {
        // 先检查缓存
        {
            let cache = self.cache.read().await;
            if let Some(data) = cache.get(&(storage_type, id.to_string())) {
                return Ok(bincode::deserialize(data)?);
            }
        }
        
        // 从文件加载
        let file_path = self.get_file_path(storage_type, id);
        if !file_path.exists() {
            return Err(StorageError::NotFound(format!("{} with id {} not found", storage_type.as_str(), id)));
        }
        
        let mut file = File::open(file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        
        // 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert((storage_type, id.to_string()), buffer.clone());
        }
        
        let item = bincode::deserialize(&buffer)?;
        Ok(item)
    }
    
    async fn delete(&self, storage_type: StorageType, id: &str) -> Result<(), StorageError> {
        let file_path = self.get_file_path(storage_type, id);
        
        // 从缓存中删除
        {
            let mut cache = self.cache.write().await;
            cache.remove(&(storage_type, id.to_string()));
        }
        
        // 删除文件
        if file_path.exists() {
            fs::remove_file(file_path)?;
            debug!("Deleted {} with id {} from storage", storage_type.as_str(), id);
        } else {
            warn!("Attempt to delete non-existent {} with id {}", storage_type.as_str(), id);
        }
        
        Ok(())
    }
    
    async fn list<T: for<'de> Deserialize<'de> + Send + Sync>(&self, storage_type: StorageType) -> Result<Vec<T>, StorageError> {
        let dir_path = self.base_dir.join(storage_type.as_str());
        let mut items = Vec::new();
        
        if !dir_path.exists() {
            return Ok(items);
        }
        
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "bin") {
                let mut file = File::open(&path)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;
                
                let item: T = bincode::deserialize(&buffer)?;
                items.push(item);
            }
        }
        
        Ok(items)
    }
}

/// 内存存储实现（仅用于测试）
pub struct InMemoryStorage {
    data: Arc<RwLock<HashMap<(StorageType, String), Vec<u8>>>>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl StorageProvider for InMemoryStorage {
    async fn save<T: Serialize + Send + Sync>(&self, storage_type: StorageType, id: &str, item: &T) -> Result<(), StorageError> {
        let serialized = bincode::serialize(item)?;
        let mut data = self.data.write().await;
        data.insert((storage_type, id.to_string()), serialized);
        Ok(())
    }
    
    async fn load<T: for<'de> Deserialize<'de> + Send + Sync>(&self, storage_type: StorageType, id: &str) -> Result<T, StorageError> {
        let data = self.data.read().await;
        if let Some(bytes) = data.get(&(storage_type, id.to_string())) {
            Ok(bincode::deserialize(bytes)?)
        } else {
            Err(StorageError::NotFound(format!("{} with id {} not found", storage_type.as_str(), id)))
        }
    }
    
    async fn delete(&self, storage_type: StorageType, id: &str) -> Result<(), StorageError> {
        let mut data = self.data.write().await;
        data.remove(&(storage_type, id.to_string()));
        Ok(())
    }
    
    async fn list<T: for<'de> Deserialize<'de> + Send + Sync>(&self, storage_type: StorageType) -> Result<Vec<T>, StorageError> {
        let data = self.data.read().await;
        let mut items = Vec::new();
        
        for ((type_, _), bytes) in data.iter().filter(|((t, _), _)| *t == storage_type) {
            items.push(bincode::deserialize(bytes)?);
        }
        
        Ok(items)
    }
} 