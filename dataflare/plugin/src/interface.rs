//! Plugin Interface Layer
//!
//! 第1层：统一的插件接口定义
//!
//! 这一层定义了所有插件必须实现的核心接口，包括：
//! - DataFlarePlugin trait：统一的插件接口
//! - PluginRecord<'a>：零拷贝数据结构
//! - 插件生命周期管理
//! - 标准化的错误处理

use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};

/// 统一的DataFlare插件接口
///
/// 这是新架构的核心接口，所有插件都必须实现这个trait
///
/// # 设计原则
/// - 零拷贝：使用引用避免不必要的数据拷贝
/// - 同步执行：简化接口，提升性能
/// - 类型安全：利用Rust类型系统保证安全性
/// - 统一接口：所有插件类型使用相同的接口
pub trait DataFlarePlugin: Send + Sync {
    /// 插件名称
    fn name(&self) -> &str;

    /// 插件版本
    fn version(&self) -> &str;

    /// 插件类型
    fn plugin_type(&self) -> PluginType;

    /// 插件描述
    fn description(&self) -> &str {
        "DataFlare Plugin"
    }

    /// 插件作者
    fn author(&self) -> Option<&str> {
        None
    }

    /// 初始化插件
    ///
    /// # 参数
    /// - `config`: 插件配置参数
    ///
    /// # 返回
    /// - `Ok(())`: 初始化成功
    /// - `Err(PluginError)`: 初始化失败
    fn initialize(&mut self, config: &HashMap<String, serde_json::Value>) -> Result<(), PluginError> {
        let _ = config;
        Ok(())
    }

    /// 处理单条记录
    ///
    /// 这是插件的核心方法，处理单条数据记录
    ///
    /// # 参数
    /// - `record`: 输入记录（零拷贝引用）
    ///
    /// # 返回
    /// - `Ok(PluginResult)`: 处理成功
    /// - `Err(PluginError)`: 处理失败
    fn process(&self, record: &PluginRecord) -> Result<PluginResult, PluginError>;

    /// 批处理记录（可选实现）
    ///
    /// 默认实现会逐个调用process方法，插件可以重写此方法以提供优化的批处理
    ///
    /// # 参数
    /// - `records`: 输入记录批次
    ///
    /// # 返回
    /// - 处理结果列表
    fn process_batch(&self, records: &[PluginRecord]) -> Vec<Result<PluginResult, PluginError>> {
        records.iter().map(|record| self.process(record)).collect()
    }

    /// 获取插件状态
    fn get_state(&self) -> PluginState {
        PluginState::Ready
    }

    /// 清理资源
    fn cleanup(&mut self) -> Result<(), PluginError> {
        Ok(())
    }

    /// 获取插件信息
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: self.name().to_string(),
            version: self.version().to_string(),
            plugin_type: self.plugin_type(),
            description: self.description().to_string(),
            author: self.author().map(|s| s.to_string()),
            api_version: crate::core::API_VERSION.to_string(),
        }
    }
}

/// 零拷贝插件记录
///
/// 这是新架构的核心数据结构，使用生命周期参数避免数据拷贝
///
/// # 生命周期
/// - `'a`: 数据的生命周期，确保在记录使用期间数据有效
#[derive(Debug, Clone)]
pub struct PluginRecord<'a> {
    /// 记录值（零拷贝引用）
    pub value: &'a [u8],
    /// 记录元数据（零拷贝引用）
    pub metadata: &'a HashMap<String, String>,
    /// 记录时间戳（纳秒）
    pub timestamp: i64,
    /// 分区ID
    pub partition: u32,
    /// 记录偏移量
    pub offset: u64,
    /// 记录键（可选，零拷贝引用）
    pub key: Option<&'a [u8]>,
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
            key: None,
        }
    }

    /// 设置记录键
    pub fn with_key(mut self, key: &'a [u8]) -> Self {
        self.key = Some(key);
        self
    }

    /// 获取记录大小（字节）
    pub fn size(&self) -> usize {
        self.value.len() + self.key.map_or(0, |k| k.len())
    }

    /// 检查是否有键
    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    /// 获取元数据值
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

/// 插件处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginResult {
    /// 处理成功，返回新的数据
    Success {
        /// 处理后的数据
        data: Vec<u8>,
        /// 更新的元数据
        metadata: Option<HashMap<String, String>>,
    },
    /// 过滤掉此记录
    Filtered,
    /// 跳过处理，保持原样
    Skip,
    /// 处理失败
    Error {
        /// 错误消息
        message: String,
        /// 错误代码
        code: Option<String>,
    },
}

impl PluginResult {
    /// 创建成功结果
    pub fn success(data: Vec<u8>) -> Self {
        Self::Success { data, metadata: None }
    }

    /// 创建带元数据的成功结果
    pub fn success_with_metadata(data: Vec<u8>, metadata: HashMap<String, String>) -> Self {
        Self::Success { data: data, metadata: Some(metadata) }
    }

    /// 创建过滤结果
    pub fn filtered() -> Self {
        Self::Filtered
    }

    /// 创建跳过结果
    pub fn skip() -> Self {
        Self::Skip
    }

    /// 创建错误结果
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            code: None,
        }
    }

    /// 创建带错误代码的错误结果
    pub fn error_with_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            code: Some(code.into()),
        }
    }

    /// 检查是否成功
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// 检查是否被过滤
    pub fn is_filtered(&self) -> bool {
        matches!(self, Self::Filtered)
    }

    /// 检查是否跳过
    pub fn is_skip(&self) -> bool {
        matches!(self, Self::Skip)
    }

    /// 检查是否错误
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

/// 插件类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginType {
    /// 数据源插件
    Source,
    /// 数据目标插件
    Destination,
    /// 数据处理插件
    Processor,
    /// 数据转换插件
    Transformer,
    /// 数据过滤插件
    Filter,
    /// 数据聚合插件
    Aggregator,
    /// 自定义插件
    Custom(String),
}

impl std::fmt::Display for PluginType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source => write!(f, "source"),
            Self::Destination => write!(f, "destination"),
            Self::Processor => write!(f, "processor"),
            Self::Transformer => write!(f, "transformer"),
            Self::Filter => write!(f, "filter"),
            Self::Aggregator => write!(f, "aggregator"),
            Self::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// 插件状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// 未初始化
    Uninitialized,
    /// 初始化中
    Initializing,
    /// 就绪状态
    Ready,
    /// 运行中
    Running,
    /// 错误状态
    Error,
    /// 已停止
    Stopped,
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
    pub description: String,
    /// 插件作者
    pub author: Option<String>,
    /// API版本
    pub api_version: String,
}

/// 插件错误类型
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum PluginError {
    /// 初始化错误
    #[error("Plugin initialization failed: {message}")]
    Initialization { message: String },

    /// 配置错误
    #[error("Plugin configuration error: {message}")]
    Configuration { message: String },

    /// 处理错误
    #[error("Plugin processing error: {message}")]
    Processing { message: String },

    /// 序列化错误
    #[error("Serialization error: {message}")]
    Serialization { message: String },

    /// 超时错误
    #[error("Plugin timeout after {timeout:?}")]
    Timeout { timeout: Duration },

    /// 内存错误
    #[error("Memory error: {message}")]
    Memory { message: String },

    /// 通用错误
    #[error("Plugin error: {message}")]
    Generic { message: String },
}

impl PluginError {
    /// 创建初始化错误
    pub fn initialization(message: impl Into<String>) -> Self {
        Self::Initialization { message: message.into() }
    }

    /// 创建配置错误
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration { message: message.into() }
    }

    /// 创建处理错误
    pub fn processing(message: impl Into<String>) -> Self {
        Self::Processing { message: message.into() }
    }

    /// 创建序列化错误
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization { message: message.into() }
    }

    /// 创建超时错误
    pub fn timeout(timeout: Duration) -> Self {
        Self::Timeout { timeout }
    }

    /// 创建内存错误
    pub fn memory(message: impl Into<String>) -> Self {
        Self::Memory { message: message.into() }
    }

    /// 创建通用错误
    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic { message: message.into() }
    }
}
