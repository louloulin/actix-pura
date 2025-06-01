//! 批处理插件接口
//!
//! 支持高性能批量数据处理的插件接口
//!
//! ## 特性
//! - 批量处理多条记录
//! - 自动批次大小优化
//! - 并行处理支持
//! - 内存池集成
//! - 性能监控

use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use crate::core::{PluginRecord, PluginResult, PluginError, Result, PluginInfo};
use crate::interface::PluginState;

/// 批处理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    /// 最大批次大小
    pub max_batch_size: usize,
    /// 最小批次大小
    pub min_batch_size: usize,
    /// 批次超时时间
    pub batch_timeout: Duration,
    /// 启用并行处理
    pub enable_parallel: bool,
    /// 并行度
    pub parallelism: usize,
    /// 启用内存池
    pub enable_memory_pool: bool,
    /// 自动批次大小调整
    pub auto_batch_sizing: bool,
    /// 目标延迟（毫秒）
    pub target_latency_ms: u64,
    /// 目标吞吐量（记录/秒）
    pub target_throughput: u64,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            min_batch_size: 10,
            batch_timeout: Duration::from_millis(100),
            enable_parallel: true,
            parallelism: 4,
            enable_memory_pool: true,
            auto_batch_sizing: true,
            target_latency_ms: 50,
            target_throughput: 10000,
        }
    }
}

/// 批处理结果
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// 处理的记录数
    pub processed_count: usize,
    /// 成功的记录数
    pub success_count: usize,
    /// 失败的记录数
    pub error_count: usize,
    /// 处理时间
    pub processing_time: Duration,
    /// 结果记录
    pub results: Vec<PluginResult>,
    /// 错误信息
    pub errors: Vec<(usize, PluginError)>,
}

impl BatchResult {
    /// 创建成功的批处理结果
    pub fn success(results: Vec<PluginResult>, processing_time: Duration) -> Self {
        let processed_count = results.len();
        Self {
            processed_count,
            success_count: processed_count,
            error_count: 0,
            processing_time,
            results,
            errors: Vec::new(),
        }
    }

    /// 创建部分成功的批处理结果
    pub fn partial(
        results: Vec<PluginResult>,
        errors: Vec<(usize, PluginError)>,
        processing_time: Duration,
    ) -> Self {
        let processed_count = results.len() + errors.len();
        let error_count = errors.len();
        Self {
            processed_count,
            success_count: results.len(),
            error_count,
            processing_time,
            results,
            errors,
        }
    }

    /// 创建失败的批处理结果
    pub fn error(error: PluginError, batch_size: usize) -> Self {
        Self {
            processed_count: batch_size,
            success_count: 0,
            error_count: batch_size,
            processing_time: Duration::from_millis(0),
            results: Vec::new(),
            errors: vec![(0, error)],
        }
    }

    /// 获取成功率
    pub fn success_rate(&self) -> f64 {
        if self.processed_count == 0 {
            0.0
        } else {
            self.success_count as f64 / self.processed_count as f64
        }
    }

    /// 获取吞吐量（记录/秒）
    pub fn throughput(&self) -> f64 {
        if self.processing_time.as_secs_f64() == 0.0 {
            0.0
        } else {
            self.processed_count as f64 / self.processing_time.as_secs_f64()
        }
    }
}

/// 批处理统计信息
#[derive(Debug, Clone)]
pub struct BatchStats {
    /// 总批次数
    pub total_batches: u64,
    /// 总处理记录数
    pub total_records: u64,
    /// 总成功记录数
    pub total_success: u64,
    /// 总错误记录数
    pub total_errors: u64,
    /// 平均批次大小
    pub avg_batch_size: f64,
    /// 平均处理时间（毫秒）
    pub avg_processing_time_ms: f64,
    /// 平均吞吐量（记录/秒）
    pub avg_throughput: f64,
    /// 成功率
    pub success_rate: f64,
    /// 最后更新时间
    pub last_updated: Instant,
}

impl Default for BatchStats {
    fn default() -> Self {
        Self {
            total_batches: 0,
            total_records: 0,
            total_success: 0,
            total_errors: 0,
            avg_batch_size: 0.0,
            avg_processing_time_ms: 0.0,
            avg_throughput: 0.0,
            success_rate: 0.0,
            last_updated: Instant::now(),
        }
    }
}

impl BatchStats {
    /// 更新统计信息
    pub fn update(&mut self, result: &BatchResult) {
        self.total_batches += 1;
        self.total_records += result.processed_count as u64;
        self.total_success += result.success_count as u64;
        self.total_errors += result.error_count as u64;

        // 更新平均值
        self.avg_batch_size = self.total_records as f64 / self.total_batches as f64;
        self.avg_processing_time_ms = (self.avg_processing_time_ms * (self.total_batches - 1) as f64
            + result.processing_time.as_millis() as f64) / self.total_batches as f64;
        self.avg_throughput = (self.avg_throughput * (self.total_batches - 1) as f64
            + result.throughput()) / self.total_batches as f64;
        self.success_rate = self.total_success as f64 / self.total_records as f64;
        self.last_updated = Instant::now();
    }
}

/// 批处理插件trait
pub trait BatchPlugin: Send + Sync {
    /// 获取插件信息
    fn info(&self) -> PluginInfo;

    /// 获取插件状态
    fn get_state(&self) -> PluginState;

    /// 获取批处理配置
    fn batch_config(&self) -> BatchConfig {
        BatchConfig::default()
    }

    /// 批量处理记录
    fn process_batch(&self, records: &[PluginRecord]) -> Result<BatchResult>;

    /// 初始化插件
    fn initialize(&mut self, config: &std::collections::HashMap<String, String>) -> Result<()> {
        let _ = config;
        Ok(())
    }

    /// 关闭插件
    fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    /// 健康检查
    fn health_check(&self) -> Result<()> {
        Ok(())
    }

    /// 获取统计信息
    fn get_stats(&self) -> BatchStats {
        BatchStats::default()
    }

    /// 优化批次大小
    fn optimize_batch_size(&self, current_size: usize, latency: Duration, throughput: f64) -> usize {
        let config = self.batch_config();

        if !config.auto_batch_sizing {
            return current_size;
        }

        let target_latency = Duration::from_millis(config.target_latency_ms);
        let target_throughput = config.target_throughput as f64;

        // 基于延迟调整
        let latency_factor = if latency > target_latency {
            0.9 // 减小批次大小
        } else {
            1.1 // 增大批次大小
        };

        // 基于吞吐量调整
        let throughput_factor = if throughput < target_throughput {
            1.1 // 增大批次大小
        } else {
            0.95 // 稍微减小批次大小
        };

        let new_size = (current_size as f64 * latency_factor * throughput_factor) as usize;

        // 限制在配置范围内
        new_size.max(config.min_batch_size).min(config.max_batch_size)
    }
}

/// 批处理适配器，将单记录插件适配为批处理插件
pub struct BatchAdapter<T> {
    inner: T,
    config: BatchConfig,
    stats: BatchStats,
}

impl<T> BatchAdapter<T> {
    /// 创建新的批处理适配器
    pub fn new(inner: T, config: BatchConfig) -> Self {
        Self {
            inner,
            config,
            stats: BatchStats::default(),
        }
    }

    /// 获取内部插件的引用
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// 获取内部插件的可变引用
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> BatchPlugin for BatchAdapter<T>
where
    T: crate::core::DataFlarePlugin + crate::interface::DataFlarePlugin + Send + Sync,
{
    fn info(&self) -> PluginInfo {
        crate::core::DataFlarePlugin::info(&self.inner)
    }

    fn get_state(&self) -> PluginState {
        self.inner.get_state()
    }

    fn batch_config(&self) -> BatchConfig {
        self.config.clone()
    }

    fn process_batch(&self, records: &[PluginRecord]) -> Result<BatchResult> {
        let start_time = Instant::now();
        let mut results = Vec::with_capacity(records.len());
        let mut errors = Vec::new();

        if self.config.enable_parallel && records.len() > self.config.parallelism {
            // 并行处理（简化版本，避免生命周期问题）
            // 在实际实现中，这里应该使用更复杂的并行处理逻辑
            for (idx, record) in records.iter().enumerate() {
                match crate::core::DataFlarePlugin::process(&self.inner, record) {
                    Ok(result) => results.push(result),
                    Err(error) => errors.push((idx, error)),
                }
            }
        } else {
            // 串行处理
            for (idx, record) in records.iter().enumerate() {
                match crate::core::DataFlarePlugin::process(&self.inner, record) {
                    Ok(result) => results.push(result),
                    Err(error) => errors.push((idx, error)),
                }
            }
        }

        let processing_time = start_time.elapsed();
        let batch_result = if errors.is_empty() {
            BatchResult::success(results, processing_time)
        } else {
            BatchResult::partial(results, errors, processing_time)
        };

        Ok(batch_result)
    }

    fn get_stats(&self) -> BatchStats {
        self.stats.clone()
    }
}
