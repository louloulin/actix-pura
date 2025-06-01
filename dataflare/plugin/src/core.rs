//! DataFlare 插件系统核心接口
//!
//! 新的3层架构：Plugin Interface → Plugin Runtime → Native/WASM Backend
//! 统一的插件接口，支持零拷贝数据处理

use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};

/// 插件类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginType {
    /// 数据源插件
    Source,
    /// 数据目标插件
    Sink,
    /// 过滤器插件
    Filter,
    /// 转换器插件
    Transform,
    /// 聚合器插件
    Aggregate,
    /// 处理器插件
    Processor,
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginType::Source => write!(f, "source"),
            PluginType::Sink => write!(f, "sink"),
            PluginType::Filter => write!(f, "filter"),
            PluginType::Transform => write!(f, "transform"),
            PluginType::Aggregate => write!(f, "aggregate"),
            PluginType::Processor => write!(f, "processor"),
        }
    }
}

/// 零拷贝插件记录结构
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PluginRecord<'a> {
    /// 记录值（零拷贝引用）
    pub value: &'a [u8],
    /// 元数据（零拷贝引用）
    pub metadata: &'a HashMap<String, String>,
    /// 时间戳
    pub timestamp: i64,
    /// 分区ID
    pub partition: u32,
    /// 记录偏移量
    pub offset: u64,
}

impl<'a> PluginRecord<'a> {
    /// 创建新的插件记录
    pub fn new(
        value: &'a [u8],
        metadata: &'a HashMap<String, String>,
        timestamp: i64,
        partition: u32,
        offset: u64,
    ) -> Self {
        Self {
            value,
            metadata,
            timestamp,
            partition,
            offset,
        }
    }

    /// 获取记录值作为字符串
    pub fn value_as_str(&self) -> std::result::Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.value)
    }

    /// 获取元数据值
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// 获取记录大小
    pub fn size(&self) -> usize {
        self.value.len()
    }
}

/// 插件处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginResult {
    /// 过滤结果（保留/丢弃）
    Filtered(bool),
    /// 转换结果（新数据）
    Transformed(Vec<u8>),
    /// 聚合结果（累积数据）
    Aggregated(Vec<u8>),
    /// 处理结果（可能的多个输出）
    Processed(Vec<Vec<u8>>),
    /// 跳过处理
    Skip,
    /// 错误结果
    Error(String),
}

impl PluginResult {
    /// 创建成功的转换结果
    pub fn success(data: Vec<u8>) -> Self {
        PluginResult::Transformed(data)
    }

    /// 创建过滤结果
    pub fn filtered() -> Self {
        PluginResult::Filtered(false)
    }

    /// 检查结果是否应该保留
    pub fn should_keep(&self) -> bool {
        match self {
            PluginResult::Filtered(keep) => *keep,
            PluginResult::Transformed(_) => true,
            PluginResult::Aggregated(_) => true,
            PluginResult::Processed(outputs) => !outputs.is_empty(),
            PluginResult::Skip => false,
            PluginResult::Error(_) => false,
        }
    }

    /// 获取输出数据
    pub fn get_output(&self) -> Option<Vec<Vec<u8>>> {
        match self {
            PluginResult::Transformed(data) => Some(vec![data.clone()]),
            PluginResult::Aggregated(data) => Some(vec![data.clone()]),
            PluginResult::Processed(outputs) => Some(outputs.clone()),
            _ => None,
        }
    }

    /// 检查是否为错误结果
    pub fn is_error(&self) -> bool {
        matches!(self, PluginResult::Error(_))
    }

    /// 获取错误信息
    pub fn error_message(&self) -> Option<&str> {
        match self {
            PluginResult::Error(msg) => Some(msg),
            _ => None,
        }
    }
}

/// 插件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// 插件名称
    pub name: String,
    /// 插件版本
    pub version: String,
    /// 插件类型
    pub plugin_type: PluginType,
    /// 插件描述
    pub description: Option<String>,
    /// 插件作者
    pub author: Option<String>,
    /// 支持的API版本
    pub api_version: String,
}

impl PluginInfo {
    /// 创建新的插件信息
    pub fn new(name: String, version: String, plugin_type: PluginType) -> Self {
        Self {
            name,
            version,
            plugin_type,
            description: None,
            author: None,
            api_version: "1.0.0".to_string(),
        }
    }

    /// 设置描述
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// 设置作者
    pub fn with_author(mut self, author: String) -> Self {
        self.author = Some(author);
        self
    }

    /// 检查API版本兼容性
    pub fn is_compatible(&self, required_version: &str) -> bool {
        // 简单的版本兼容性检查
        self.api_version == required_version
    }
}

/// 插件错误类型
#[derive(Debug, Clone, thiserror::Error)]
pub enum PluginError {
    #[error("插件初始化失败: {0}")]
    Initialization(String),

    #[error("插件执行失败: {0}")]
    Execution(String),

    #[error("插件配置错误: {0}")]
    Configuration(String),

    #[error("数据序列化错误: {0}")]
    Serialization(String),

    #[error("不支持的操作: {0}")]
    UnsupportedOperation(String),

    #[error("插件超时: {0:?}")]
    Timeout(Duration),

    #[error("内存限制超出: {0} bytes")]
    MemoryLimit(usize),

    #[error("IO错误: {0}")]
    Io(String),
}

/// 插件结果类型别名
pub type Result<T> = std::result::Result<T, PluginError>;

impl From<std::io::Error> for PluginError {
    fn from(err: std::io::Error) -> Self {
        PluginError::Io(err.to_string())
    }
}

/// 统一的DataFlare插件接口
///
/// 所有插件必须实现此trait，支持同步处理以获得最佳性能
pub trait DataFlarePlugin: Send + Sync {
    /// 处理单个记录
    ///
    /// 这是核心方法，使用零拷贝引用以获得最佳性能
    fn process(&self, record: &PluginRecord) -> Result<PluginResult>;

    /// 获取插件名称
    fn name(&self) -> &str;

    /// 获取插件版本
    fn version(&self) -> &str;

    /// 获取插件类型
    fn plugin_type(&self) -> PluginType;

    /// 获取插件信息
    fn info(&self) -> PluginInfo {
        PluginInfo::new(
            self.name().to_string(),
            self.version().to_string(),
            self.plugin_type(),
        )
    }

    /// 初始化插件
    ///
    /// 在插件加载时调用一次，默认实现不做任何操作
    fn initialize(&mut self, _config: &HashMap<String, String>) -> Result<()> {
        Ok(())
    }

    /// 清理插件
    ///
    /// 在插件卸载时调用，默认实现不做任何操作
    fn finalize(&mut self) -> Result<()> {
        Ok(())
    }

    /// 检查是否支持批处理
    ///
    /// 默认返回false，支持批处理的插件应该重写此方法
    fn supports_batch(&self) -> bool {
        false
    }

    /// 批处理记录
    ///
    /// 默认实现逐个处理记录，支持批处理的插件可以重写此方法
    fn process_batch(&self, records: &[PluginRecord]) -> Vec<Result<PluginResult>> {
        records.iter().map(|record| self.process(record)).collect()
    }

    /// 获取插件配置模式
    ///
    /// 返回插件支持的配置参数描述
    fn config_schema(&self) -> Option<serde_json::Value> {
        None
    }

    /// 验证配置
    ///
    /// 验证给定的配置是否有效
    fn validate_config(&self, _config: &HashMap<String, String>) -> Result<()> {
        Ok(())
    }
}

/// 批处理插件接口
///
/// 为支持高效批处理的插件提供专门的接口
pub trait BatchPlugin: DataFlarePlugin {
    /// 批处理记录
    ///
    /// 高效的批处理实现，应该比逐个处理更快
    fn process_batch_optimized(&self, records: &[PluginRecord]) -> Vec<Result<PluginResult>>;

    /// 获取最大批处理大小
    fn max_batch_size(&self) -> usize {
        1000
    }

    /// 获取最佳批处理大小
    fn optimal_batch_size(&self) -> usize {
        100
    }

    /// 获取批处理超时时间
    fn batch_timeout(&self) -> Duration {
        Duration::from_millis(100)
    }

    /// 检查是否支持并行批处理
    fn supports_parallel_batch(&self) -> bool {
        false
    }

    /// 并行批处理记录
    ///
    /// 将批次分割为多个子批次并行处理
    fn process_batch_parallel(&self, records: &[PluginRecord], parallelism: usize) -> Vec<Result<PluginResult>> {
        if !self.supports_parallel_batch() || parallelism <= 1 {
            return self.process_batch_optimized(records);
        }

        let chunk_size = (records.len() + parallelism - 1) / parallelism;
        let chunks: Vec<&[PluginRecord]> = records.chunks(chunk_size).collect();

        // 简单的顺序处理（实际实现中可以使用线程池）
        let mut results = Vec::new();
        for chunk in chunks {
            let mut chunk_results = self.process_batch_optimized(chunk);
            results.append(&mut chunk_results);
        }

        results
    }

    /// 获取批处理统计信息
    fn batch_stats(&self) -> BatchStats {
        BatchStats::default()
    }
}

/// 批处理统计信息
#[derive(Debug, Default, Clone)]
pub struct BatchStats {
    /// 总批次数
    pub total_batches: u64,
    /// 总记录数
    pub total_records: u64,
    /// 平均批次大小
    pub avg_batch_size: f64,
    /// 平均处理时间（毫秒）
    pub avg_processing_time_ms: f64,
    /// 吞吐量（记录/秒）
    pub throughput: f64,
    /// 错误率
    pub error_rate: f64,
}

/// API版本常量
pub const API_VERSION: &str = "1.0.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_record() {
        let value = b"test data";
        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());

        let record = PluginRecord::new(value, &metadata, 1234567890, 0, 100);

        assert_eq!(record.value, b"test data");
        assert_eq!(record.value_as_str().unwrap(), "test data");
        assert_eq!(record.get_metadata("key1"), Some(&"value1".to_string()));
        assert_eq!(record.size(), 9);
        assert_eq!(record.timestamp, 1234567890);
        assert_eq!(record.partition, 0);
        assert_eq!(record.offset, 100);
    }

    #[test]
    fn test_plugin_result() {
        let result = PluginResult::Filtered(true);
        assert!(result.should_keep());
        assert!(!result.is_error());

        let result = PluginResult::Transformed(b"transformed".to_vec());
        assert!(result.should_keep());
        assert_eq!(result.get_output().unwrap(), vec![b"transformed".to_vec()]);

        let result = PluginResult::Error("test error".to_string());
        assert!(!result.should_keep());
        assert!(result.is_error());
        assert_eq!(result.error_message(), Some("test error"));
    }

    #[test]
    fn test_plugin_info() {
        let info = PluginInfo::new(
            "test_plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Filter,
        )
        .with_description("Test plugin".to_string())
        .with_author("Test Author".to_string());

        assert_eq!(info.name, "test_plugin");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.plugin_type, PluginType::Filter);
        assert_eq!(info.description, Some("Test plugin".to_string()));
        assert_eq!(info.author, Some("Test Author".to_string()));
        assert!(info.is_compatible("1.0.0"));
        assert!(!info.is_compatible("2.0.0"));
    }
}
