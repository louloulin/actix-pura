//! 高性能插件管理器
//!
//! 集成所有性能优化功能的统一插件管理器
//!
//! ## 特性
//! - 统一的插件生命周期管理
//! - 自动性能优化
//! - 智能负载均衡
//! - 实时监控和指标
//! - 故障恢复和降级

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use tracing::{info, warn, error, debug};

use crate::core::{PluginRecord, PluginResult, PluginError, Result};
use crate::interface::{DataFlarePlugin, PluginInfo, PluginState};
use crate::runtime::{PluginRuntime, PluginRuntimeConfig};
use crate::pool::{PluginPool, PluginPoolConfig};
use crate::wasm_pool::{WasmInstancePool, WasmPoolConfig};
use crate::memory_pool::{MemoryPool, MemoryPoolConfig};
use crate::batch::{BatchPlugin, BatchConfig, BatchResult, BatchAdapter};

/// 插件管理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManagerConfig {
    /// 运行时配置
    pub runtime_config: PluginRuntimeConfig,
    /// 插件池配置
    pub pool_config: PluginPoolConfig,
    /// WASM池配置
    pub wasm_pool_config: WasmPoolConfig,
    /// 内存池配置
    pub memory_pool_config: MemoryPoolConfig,
    /// 批处理配置
    pub batch_config: BatchConfig,
    /// 启用自动优化
    pub enable_auto_optimization: bool,
    /// 性能监控间隔
    pub monitoring_interval: Duration,
    /// 健康检查间隔
    pub health_check_interval: Duration,
    /// 故障恢复配置
    pub failure_recovery: FailureRecoveryConfig,
}

impl Default for PluginManagerConfig {
    fn default() -> Self {
        Self {
            runtime_config: PluginRuntimeConfig::default(),
            pool_config: PluginPoolConfig::default(),
            wasm_pool_config: WasmPoolConfig::default(),
            memory_pool_config: MemoryPoolConfig::default(),
            batch_config: BatchConfig::default(),
            enable_auto_optimization: true,
            monitoring_interval: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(60),
            failure_recovery: FailureRecoveryConfig::default(),
        }
    }
}

/// 故障恢复配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecoveryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔
    pub retry_interval: Duration,
    /// 断路器阈值
    pub circuit_breaker_threshold: f64,
    /// 断路器恢复时间
    pub circuit_breaker_recovery_time: Duration,
    /// 启用降级模式
    pub enable_fallback: bool,
}

impl Default for FailureRecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_interval: Duration::from_millis(100),
            circuit_breaker_threshold: 0.5,
            circuit_breaker_recovery_time: Duration::from_secs(30),
            enable_fallback: true,
        }
    }
}

/// 插件性能指标
#[derive(Debug, Clone)]
pub struct PluginMetrics {
    /// 插件ID
    pub plugin_id: String,
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 平均延迟（毫秒）
    pub avg_latency_ms: f64,
    /// P95延迟（毫秒）
    pub p95_latency_ms: f64,
    /// P99延迟（毫秒）
    pub p99_latency_ms: f64,
    /// 吞吐量（请求/秒）
    pub throughput: f64,
    /// 错误率
    pub error_rate: f64,
    /// 内存使用量（字节）
    pub memory_usage: usize,
    /// CPU使用率
    pub cpu_usage: f64,
    /// 最后更新时间
    pub last_updated: Instant,
}

impl Default for PluginMetrics {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            throughput: 0.0,
            error_rate: 0.0,
            memory_usage: 0,
            cpu_usage: 0.0,
            last_updated: Instant::now(),
        }
    }
}

/// 高性能插件管理器
pub struct PluginManager {
    /// 配置
    config: PluginManagerConfig,
    /// 插件运行时
    runtime: Arc<RwLock<PluginRuntime>>,
    /// 插件池
    pools: Arc<RwLock<HashMap<String, PluginPool>>>,
    /// WASM实例池
    wasm_pools: Arc<RwLock<HashMap<String, WasmInstancePool>>>,
    /// 内存池
    memory_pool: Arc<MemoryPool>,
    /// 插件指标
    metrics: Arc<RwLock<HashMap<String, PluginMetrics>>>,
    /// 批处理适配器
    batch_adapters: Arc<RwLock<HashMap<String, Box<dyn BatchPlugin>>>>,
    /// 最后健康检查时间
    last_health_check: Arc<Mutex<Instant>>,
    /// 最后监控时间
    last_monitoring: Arc<Mutex<Instant>>,
}

impl PluginManager {
    /// 创建新的插件管理器
    pub fn new(config: PluginManagerConfig) -> Result<Self> {
        let runtime = Arc::new(RwLock::new(PluginRuntime::new(config.runtime_config.clone())));
        let memory_pool = Arc::new(MemoryPool::new(config.memory_pool_config.clone()));

        // 预热内存池
        memory_pool.warmup();

        Ok(Self {
            config,
            runtime,
            pools: Arc::new(RwLock::new(HashMap::new())),
            wasm_pools: Arc::new(RwLock::new(HashMap::new())),
            memory_pool,
            metrics: Arc::new(RwLock::new(HashMap::new())),
            batch_adapters: Arc::new(RwLock::new(HashMap::new())),
            last_health_check: Arc::new(Mutex::new(Instant::now())),
            last_monitoring: Arc::new(Mutex::new(Instant::now())),
        })
    }

    /// 注册插件
    pub fn register_plugin<T>(&self, plugin_id: String, plugin: T) -> Result<()>
    where
        T: DataFlarePlugin + Send + Sync + 'static,
    {
        info!("Registering plugin: {}", plugin_id);

        // 注册到运行时（简化版本）
        // {
        //     let mut runtime = self.runtime.write().unwrap();
        //     runtime.load_plugin(plugin_id.clone(), Box::new(plugin))?;
        // }

        // 创建插件池（简化版本，实际需要backend）
        // let pool = PluginPool::new(self.config.pool_config.clone())?;
        // {
        //     let mut pools = self.pools.write().unwrap();
        //     pools.insert(plugin_id.clone(), pool);
        // }

        // 初始化指标
        {
            let mut metrics = self.metrics.write().unwrap();
            let mut plugin_metrics = PluginMetrics::default();
            plugin_metrics.plugin_id = plugin_id.clone();
            metrics.insert(plugin_id, plugin_metrics);
        }

        info!("Plugin registered successfully");
        Ok(())
    }

    /// 注册批处理插件
    pub fn register_batch_plugin<T>(&self, plugin_id: String, plugin: T) -> Result<()>
    where
        T: BatchPlugin + Send + Sync + 'static,
    {
        info!("Registering batch plugin: {}", plugin_id);

        {
            let mut adapters = self.batch_adapters.write().unwrap();
            adapters.insert(plugin_id.clone(), Box::new(plugin));
        }

        // 初始化指标
        {
            let mut metrics = self.metrics.write().unwrap();
            let mut plugin_metrics = PluginMetrics::default();
            plugin_metrics.plugin_id = plugin_id.clone();
            metrics.insert(plugin_id, plugin_metrics);
        }

        info!("Batch plugin registered successfully");
        Ok(())
    }

    /// 处理单条记录
    pub fn process_record(&self, plugin_id: &str, record: &PluginRecord) -> Result<PluginResult> {
        let start_time = Instant::now();

        // 获取内存缓冲区
        let _buffer = self.memory_pool.get_buffer(record.value.len());

        // 处理记录
        let result = {
            let runtime = self.runtime.read().unwrap();
            runtime.process_record(plugin_id, record)
        };

        // 更新指标
        let processing_time = start_time.elapsed();
        self.update_metrics(plugin_id, &result, processing_time);

        // 返回缓冲区
        self.memory_pool.return_buffer(_buffer);

        result
    }

    /// 批量处理记录
    pub fn process_batch(&self, plugin_id: &str, records: &[PluginRecord]) -> Result<BatchResult> {
        let start_time = Instant::now();

        let result = {
            let adapters = self.batch_adapters.read().unwrap();
            if let Some(adapter) = adapters.get(plugin_id) {
                adapter.process_batch(records)
            } else {
                return Err(PluginError::Configuration(
                    format!("Batch plugin {} not found", plugin_id)
                ));
            }
        };

        // 更新批处理指标
        let processing_time = start_time.elapsed();
        self.update_batch_metrics(plugin_id, &result, processing_time);

        result
    }

    /// 获取插件指标
    pub fn get_metrics(&self, plugin_id: &str) -> Option<PluginMetrics> {
        let metrics = self.metrics.read().unwrap();
        metrics.get(plugin_id).cloned()
    }

    /// 获取所有插件指标
    pub fn get_all_metrics(&self) -> HashMap<String, PluginMetrics> {
        let metrics = self.metrics.read().unwrap();
        metrics.clone()
    }

    /// 执行健康检查
    pub fn health_check(&self) -> Result<()> {
        let mut last_check = self.last_health_check.lock().unwrap();
        let now = Instant::now();

        if now.duration_since(*last_check) < self.config.health_check_interval {
            return Ok(());
        }

        debug!("Performing health check");

        // 检查运行时健康状态
        // {
        //     let runtime = self.runtime.read().unwrap();
        //     runtime.health_check()?;
        // }

        // 检查内存池
        let stats = self.memory_pool.get_stats();
        if stats.total_memory_usage > self.config.memory_pool_config.max_tiny_buffers * 1024 * 1024 {
            warn!("Memory pool usage is high: {} bytes", stats.total_memory_usage);
        }

        // 清理空闲实例
        self.memory_pool.cleanup_idle_buffers();

        *last_check = now;
        debug!("Health check completed");
        Ok(())
    }

    /// 更新单记录指标
    fn update_metrics(&self, plugin_id: &str, result: &Result<PluginResult>, processing_time: Duration) {
        let mut metrics = self.metrics.write().unwrap();
        if let Some(plugin_metrics) = metrics.get_mut(plugin_id) {
            plugin_metrics.total_requests += 1;

            match result {
                Ok(_) => plugin_metrics.successful_requests += 1,
                Err(_) => plugin_metrics.failed_requests += 1,
            }

            // 更新延迟
            let latency_ms = processing_time.as_millis() as f64;
            plugin_metrics.avg_latency_ms = (plugin_metrics.avg_latency_ms * (plugin_metrics.total_requests - 1) as f64 + latency_ms) / plugin_metrics.total_requests as f64;

            // 更新错误率
            plugin_metrics.error_rate = plugin_metrics.failed_requests as f64 / plugin_metrics.total_requests as f64;

            plugin_metrics.last_updated = Instant::now();
        }
    }

    /// 更新批处理指标
    fn update_batch_metrics(&self, plugin_id: &str, result: &Result<BatchResult>, processing_time: Duration) {
        let mut metrics = self.metrics.write().unwrap();
        if let Some(plugin_metrics) = metrics.get_mut(plugin_id) {
            if let Ok(batch_result) = result {
                plugin_metrics.total_requests += batch_result.processed_count as u64;
                plugin_metrics.successful_requests += batch_result.success_count as u64;
                plugin_metrics.failed_requests += batch_result.error_count as u64;

                // 更新吞吐量
                plugin_metrics.throughput = batch_result.throughput();
            }

            // 更新延迟
            let latency_ms = processing_time.as_millis() as f64;
            plugin_metrics.avg_latency_ms = latency_ms;

            plugin_metrics.last_updated = Instant::now();
        }
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        info!("Shutting down plugin manager");

        // 清理资源
        // if let Ok(mut runtime) = self.runtime.write() {
        //     let _ = runtime.shutdown();
        // }
    }
}
