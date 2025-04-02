use std::fs::{self, File, OpenOptions};
use std::io::{self, Write, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use log::{error, debug};
use serde::{Serialize, Deserialize};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Log error: {0}")]
    Other(String),
}

/// 日志条目，记录系统操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 日志ID
    pub id: String,
    /// 日志时间戳
    pub timestamp: DateTime<Utc>,
    /// 操作类型
    pub operation_type: String,
    /// 操作对象ID
    pub target_id: Option<String>,
    /// 操作详情
    pub details: String,
    /// 操作结果
    pub result: LogResult,
}

/// 日志操作结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogResult {
    Success,
    Failure,
}

impl LogEntry {
    /// 创建新的成功日志条目
    pub fn success(operation_type: impl Into<String>, target_id: Option<String>, details: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            operation_type: operation_type.into(),
            target_id,
            details: details.into(),
            result: LogResult::Success,
        }
    }
    
    /// 创建新的失败日志条目
    pub fn failure(operation_type: impl Into<String>, target_id: Option<String>, details: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            operation_type: operation_type.into(),
            target_id,
            details: details.into(),
            result: LogResult::Failure,
        }
    }
}

/// 日志接口，定义日志操作
#[async_trait::async_trait]
pub trait LogProvider: Send + Sync {
    /// 写入日志条目
    async fn write_log(&self, entry: LogEntry) -> Result<(), LogError>;
    
    /// 查询日志条目
    async fn query_logs(&self, 
        operation_type: Option<String>, 
        target_id: Option<String>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<LogEntry>, LogError>;
}

/// 文件系统日志实现
pub struct FileSystemLog {
    log_file: Arc<Mutex<File>>,
    log_dir: PathBuf,
}

impl FileSystemLog {
    /// 创建新的文件系统日志
    pub fn new<P: AsRef<Path>>(log_dir: P) -> Result<Self, LogError> {
        let log_dir = log_dir.as_ref().to_path_buf();
        
        // 确保日志目录存在
        fs::create_dir_all(&log_dir)?;
        
        let log_file_path = log_dir.join("system.log");
        
        // 打开或创建日志文件
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)?;
            
        Ok(Self {
            log_file: Arc::new(Mutex::new(log_file)),
            log_dir,
        })
    }
    
    /// 加载所有日志条目
    async fn load_all_logs(&self) -> Result<Vec<LogEntry>, LogError> {
        let log_file_path = self.log_dir.join("system.log");
        
        if !log_file_path.exists() {
            return Ok(Vec::new());
        }
        
        let mut file = File::open(&log_file_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        
        let entries = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| {
                match serde_json::from_str::<LogEntry>(line) {
                    Ok(entry) => Some(entry),
                    Err(e) => {
                        error!("Failed to parse log entry: {}", e);
                        None
                    }
                }
            })
            .collect();
            
        Ok(entries)
    }
}

#[async_trait::async_trait]
impl LogProvider for FileSystemLog {
    async fn write_log(&self, entry: LogEntry) -> Result<(), LogError> {
        let serialized = serde_json::to_string(&entry)
            .map_err(|e| LogError::Other(format!("JSON serialization error: {}", e)))?;
            
        let mut file = self.log_file.lock().await;
        writeln!(file, "{}", serialized)?;
        file.flush()?;
        
        debug!("Wrote log entry: {} - {}", entry.operation_type, entry.id);
        Ok(())
    }
    
    async fn query_logs(&self, 
        operation_type: Option<String>, 
        target_id: Option<String>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<LogEntry>, LogError> {
        let all_logs = self.load_all_logs().await?;
        
        // 应用筛选条件
        let filtered_logs: Vec<LogEntry> = all_logs
            .into_iter()
            .filter(|entry| {
                operation_type.as_ref()
                    .map_or(true, |op_type| entry.operation_type == *op_type)
            })
            .filter(|entry| {
                target_id.as_ref()
                    .map_or(true, |id| entry.target_id.as_ref().map_or(false, |tid| tid == id))
            })
            .filter(|entry| {
                start_time.map_or(true, |time| entry.timestamp >= time)
            })
            .filter(|entry| {
                end_time.map_or(true, |time| entry.timestamp <= time)
            })
            .collect();
            
        // 应用限制
        let limited_logs = if let Some(limit) = limit {
            filtered_logs.into_iter().take(limit).collect()
        } else {
            filtered_logs
        };
        
        Ok(limited_logs)
    }
}

/// 内存日志实现（用于测试）
pub struct InMemoryLog {
    entries: Arc<Mutex<Vec<LogEntry>>>,
}

impl InMemoryLog {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl LogProvider for InMemoryLog {
    async fn write_log(&self, entry: LogEntry) -> Result<(), LogError> {
        let mut entries = self.entries.lock().await;
        entries.push(entry);
        Ok(())
    }
    
    async fn query_logs(&self, 
        operation_type: Option<String>, 
        target_id: Option<String>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<LogEntry>, LogError> {
        let entries = self.entries.lock().await;
        
        let filtered_logs: Vec<LogEntry> = entries
            .iter()
            .filter(|entry| {
                operation_type.as_ref()
                    .map_or(true, |op_type| entry.operation_type == *op_type)
            })
            .filter(|entry| {
                target_id.as_ref()
                    .map_or(true, |id| entry.target_id.as_ref().map_or(false, |tid| tid == id))
            })
            .filter(|entry| {
                start_time.map_or(true, |time| entry.timestamp >= time)
            })
            .filter(|entry| {
                end_time.map_or(true, |time| entry.timestamp <= time)
            })
            .cloned()
            .collect();
            
        let limited_logs = if let Some(limit) = limit {
            filtered_logs.into_iter().take(limit).collect()
        } else {
            filtered_logs
        };
        
        Ok(limited_logs)
    }
} 