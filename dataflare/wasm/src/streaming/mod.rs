//! DataFlare Enterprise 流处理模块
//!
//! 提供企业级高性能流处理能力，借鉴 Fluvio 的设计理念

pub mod processor;
pub mod backpressure;
pub mod zero_copy;
pub mod aggregation;
pub mod windowing;

use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use dataflare_plugin::{PluginRuntime, PluginRecord};
use crate::error::{EnterpriseError, EnterpriseResult};

pub use processor::EnterpriseStreamProcessor;
pub use backpressure::BackpressureController;
pub use zero_copy::ZeroCopyStreamProcessor;

/// 流处理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamProcessorConfig {
    /// 最大并发流数
    pub max_concurrent_streams: usize,
    /// 缓冲区大小
    pub buffer_size: usize,
    /// 批处理大小
    pub batch_size: usize,
    /// 处理超时
    pub processing_timeout: Duration,
    /// 背压阈值 (0.0 - 1.0)
    pub backpressure_threshold: f64,
    /// 检查点间隔
    pub checkpoint_interval: Duration,
    /// 是否启用零拷贝优化
    pub enable_zero_copy: bool,
    /// 是否启用流式聚合
    pub enable_aggregation: bool,
    /// 窗口大小 (毫秒)
    pub window_size_ms: u64,
    /// 水印延迟 (毫秒)
    pub watermark_delay_ms: u64,
}

impl Default for StreamProcessorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_streams: 1000,
            buffer_size: 64 * 1024,
            batch_size: 1000,
            processing_timeout: Duration::from_secs(10),
            backpressure_threshold: 0.8,
            checkpoint_interval: Duration::from_secs(60),
            enable_zero_copy: true,
            enable_aggregation: true,
            window_size_ms: 5000,
            watermark_delay_ms: 1000,
        }
    }
}

/// 流处理指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetrics {
    /// 处理的记录总数
    pub records_processed: u64,
    /// 处理的字节总数
    pub bytes_processed: u64,
    /// 当前吞吐量 (记录/秒)
    pub current_throughput: f64,
    /// 平均延迟 (毫秒)
    pub average_latency_ms: f64,
    /// P99 延迟 (毫秒)
    pub p99_latency_ms: f64,
    /// 错误计数
    pub error_count: u64,
    /// 背压事件计数
    pub backpressure_events: u64,
    /// 活跃流数量
    pub active_streams: usize,
    /// 缓冲区使用率
    pub buffer_utilization: f64,
    /// 最后更新时间
    pub last_updated: i64,
}

impl Default for StreamMetrics {
    fn default() -> Self {
        Self {
            records_processed: 0,
            bytes_processed: 0,
            current_throughput: 0.0,
            average_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            error_count: 0,
            backpressure_events: 0,
            active_streams: 0,
            buffer_utilization: 0.0,
            last_updated: chrono::Utc::now().timestamp_nanos(),
        }
    }
}

/// 流数据记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamRecord {
    /// 记录ID
    pub id: String,
    /// 记录数据
    pub data: Vec<u8>,
    /// 时间戳 (纳秒)
    pub timestamp: i64,
    /// 分区ID
    pub partition: u32,
    /// 偏移量
    pub offset: u64,
    /// 记录键
    pub key: Option<Vec<u8>>,
    /// 元数据
    pub metadata: std::collections::HashMap<String, String>,
}

impl StreamRecord {
    /// 创建新的流记录
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            data,
            timestamp: chrono::Utc::now().timestamp_nanos(),
            partition: 0,
            offset: 0,
            key: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// 设置分区和偏移量
    pub fn with_partition_offset(mut self, partition: u32, offset: u64) -> Self {
        self.partition = partition;
        self.offset = offset;
        self
    }

    /// 设置记录键
    pub fn with_key(mut self, key: Vec<u8>) -> Self {
        self.key = Some(key);
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// 转换为 PluginRecord (用于与 @dataflare/plugin 集成)
    pub fn to_plugin_record(&self) -> PluginRecord {
        PluginRecord {
            value: &self.data,
            metadata: &self.metadata,
            timestamp: self.timestamp,
            partition: self.partition,
            offset: self.offset,
            key: self.key.as_deref(),
        }
    }

    /// 从 PluginRecord 创建 StreamRecord
    pub fn from_plugin_record(record: &PluginRecord) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            data: record.value.to_vec(),
            timestamp: record.timestamp,
            partition: record.partition,
            offset: record.offset,
            key: record.key.map(|k| k.to_vec()),
            metadata: record.metadata.clone(),
        }
    }

    /// 获取记录大小 (字节)
    pub fn size(&self) -> usize {
        self.data.len() +
        self.key.as_ref().map_or(0, |k| k.len()) +
        self.metadata.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>()
    }
}

/// 流处理状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StreamProcessorState {
    /// 未初始化
    Uninitialized,
    /// 正在初始化
    Initializing,
    /// 运行中
    Running,
    /// 暂停中
    Paused,
    /// 正在停止
    Stopping,
    /// 已停止
    Stopped,
    /// 错误状态
    Error,
}

/// 流处理事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    /// 记录到达
    RecordArrived { record: StreamRecord },
    /// 批次完成
    BatchCompleted { batch_id: String, record_count: usize },
    /// 背压触发
    BackpressureTriggered { threshold: f64, current_load: f64 },
    /// 检查点创建
    CheckpointCreated { checkpoint_id: String, offset: u64 },
    /// 错误发生
    ErrorOccurred { error: EnterpriseError },
    /// 状态变更
    StateChanged { old_state: StreamProcessorState, new_state: StreamProcessorState },
}

/// 流处理器接口
#[async_trait::async_trait]
pub trait StreamProcessor: Send + Sync {
    /// 启动流处理器
    async fn start(&mut self) -> EnterpriseResult<()>;

    /// 停止流处理器
    async fn stop(&mut self) -> EnterpriseResult<()>;

    /// 暂停流处理器
    async fn pause(&mut self) -> EnterpriseResult<()>;

    /// 恢复流处理器
    async fn resume(&mut self) -> EnterpriseResult<()>;

    /// 处理单个记录
    async fn process_record(&mut self, record: StreamRecord) -> EnterpriseResult<StreamRecord>;

    /// 处理记录批次
    async fn process_batch(&mut self, records: Vec<StreamRecord>) -> EnterpriseResult<Vec<StreamRecord>>;

    /// 获取当前状态
    fn get_state(&self) -> StreamProcessorState;

    /// 获取处理指标
    async fn get_metrics(&self) -> EnterpriseResult<StreamMetrics>;

    /// 创建检查点
    async fn create_checkpoint(&mut self) -> EnterpriseResult<String>;

    /// 从检查点恢复
    async fn restore_from_checkpoint(&mut self, checkpoint_id: &str) -> EnterpriseResult<()>;
}

/// 流处理器工厂
pub struct StreamProcessorFactory;

impl StreamProcessorFactory {
    /// 创建企业级流处理器
    pub async fn create_enterprise_processor(
        config: StreamProcessorConfig,
        plugin_runtime: Arc<PluginRuntime>,
    ) -> EnterpriseResult<Box<dyn StreamProcessor>> {
        let processor = EnterpriseStreamProcessor::new(config, plugin_runtime).await?;
        Ok(Box::new(processor))
    }

    /// 创建零拷贝流处理器
    pub async fn create_zero_copy_processor(
        config: StreamProcessorConfig,
        plugin_runtime: Arc<PluginRuntime>,
    ) -> EnterpriseResult<Box<dyn StreamProcessor>> {
        let processor = ZeroCopyStreamProcessor::new(config, plugin_runtime).await?;
        Ok(Box::new(processor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_record_creation() {
        let data = b"test data".to_vec();
        let record = StreamRecord::new(data.clone())
            .with_partition_offset(1, 100)
            .with_key(b"test_key".to_vec())
            .with_metadata("source".to_string(), "test".to_string());

        assert_eq!(record.data, data);
        assert_eq!(record.partition, 1);
        assert_eq!(record.offset, 100);
        assert_eq!(record.key, Some(b"test_key".to_vec()));
        assert_eq!(record.metadata.get("source"), Some(&"test".to_string()));
    }

    #[test]
    fn test_stream_processor_config_default() {
        let config = StreamProcessorConfig::default();
        assert_eq!(config.max_concurrent_streams, 1000);
        assert_eq!(config.batch_size, 1000);
        assert!(config.enable_zero_copy);
        assert!(config.enable_aggregation);
    }

    #[test]
    fn test_stream_metrics_default() {
        let metrics = StreamMetrics::default();
        assert_eq!(metrics.records_processed, 0);
        assert_eq!(metrics.error_count, 0);
        assert_eq!(metrics.active_streams, 0);
    }

    #[test]
    fn test_plugin_record_conversion() {
        let stream_record = StreamRecord::new(b"test".to_vec())
            .with_partition_offset(1, 100);

        let plugin_record = stream_record.to_plugin_record();
        assert_eq!(plugin_record.value, b"test");
        assert_eq!(plugin_record.partition, 1);
        assert_eq!(plugin_record.offset, 100);

        let converted_back = StreamRecord::from_plugin_record(&plugin_record);
        assert_eq!(converted_back.data, stream_record.data);
        assert_eq!(converted_back.partition, stream_record.partition);
        assert_eq!(converted_back.offset, stream_record.offset);
    }
}
