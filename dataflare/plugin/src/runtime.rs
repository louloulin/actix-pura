//! 插件运行时管理器
//!
//! 统一的插件运行时，支持原生和WASM插件

use crate::core::{DataFlarePlugin, PluginRecord, PluginResult, PluginInfo, PluginType, Result, PluginError, BatchPlugin, BatchStats};
use crate::wasm_pool::{WasmInstancePool, WasmPoolConfig};
use crate::memory_pool::{MemoryPool, MemoryPoolConfig};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{info, debug, warn, error};
use serde::{Deserialize, Serialize};

/// 插件运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRuntimeConfig {
    /// 最大并发插件数
    pub max_concurrent_plugins: usize,
    /// 插件执行超时
    pub execution_timeout: Duration,
    /// 是否启用性能监控
    pub enable_metrics: bool,
    /// 是否启用调试模式
    pub debug_mode: bool,
    /// 内存限制（字节）
    pub memory_limit: usize,
    /// 批处理大小
    pub default_batch_size: usize,
    /// WASM池配置
    pub wasm_pool_config: WasmPoolConfig,
    /// 内存池配置
    pub memory_pool_config: MemoryPoolConfig,
    /// 是否启用批处理优化
    pub enable_batch_optimization: bool,
    /// 批处理并行度
    pub batch_parallelism: usize,
}

impl Default for PluginRuntimeConfig {
    fn default() -> Self {
        Self {
            max_concurrent_plugins: 100,
            execution_timeout: Duration::from_secs(30),
            enable_metrics: true,
            debug_mode: false,
            memory_limit: 64 * 1024 * 1024, // 64MB
            default_batch_size: 1000,
            wasm_pool_config: WasmPoolConfig::default(),
            memory_pool_config: MemoryPoolConfig::default(),
            enable_batch_optimization: true,
            batch_parallelism: 4,
        }
    }
}

/// 插件注册信息
#[derive(Debug, Clone)]
pub struct PluginRegistration {
    /// 插件ID
    pub plugin_id: String,
    /// 插件信息
    pub info: PluginInfo,
    /// 注册时间
    pub registered_at: std::time::SystemTime,
    /// 是否启用
    pub enabled: bool,
    /// 使用计数
    pub usage_count: u64,
    /// 最后使用时间
    pub last_used: Option<std::time::SystemTime>,
}

/// 插件性能指标
#[derive(Debug, Clone, Default)]
pub struct PluginMetrics {
    /// 总调用次数
    pub total_calls: u64,
    /// 成功调用次数
    pub successful_calls: u64,
    /// 失败调用次数
    pub failed_calls: u64,
    /// 总执行时间
    pub total_execution_time: Duration,
    /// 平均执行时间
    pub average_execution_time: Duration,
    /// 最小执行时间
    pub min_execution_time: Duration,
    /// 最大执行时间
    pub max_execution_time: Duration,
    /// 处理的记录数
    pub records_processed: u64,
    /// 处理的字节数
    pub bytes_processed: u64,
}

impl PluginMetrics {
    /// 记录一次调用
    pub fn record_call(&mut self, execution_time: Duration, success: bool, bytes: usize) {
        self.total_calls += 1;
        if success {
            self.successful_calls += 1;
            self.records_processed += 1;
            self.bytes_processed += bytes as u64;
        } else {
            self.failed_calls += 1;
        }

        self.total_execution_time += execution_time;
        self.average_execution_time = self.total_execution_time / self.total_calls as u32;

        if self.total_calls == 1 {
            self.min_execution_time = execution_time;
            self.max_execution_time = execution_time;
        } else {
            if execution_time < self.min_execution_time {
                self.min_execution_time = execution_time;
            }
            if execution_time > self.max_execution_time {
                self.max_execution_time = execution_time;
            }
        }
    }

    /// 获取成功率
    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            (self.successful_calls as f64) / (self.total_calls as f64) * 100.0
        }
    }

    /// 获取吞吐量（记录/秒）
    pub fn throughput(&self) -> f64 {
        if self.total_execution_time.is_zero() {
            0.0
        } else {
            (self.records_processed as f64) / self.total_execution_time.as_secs_f64()
        }
    }
}

/// 插件运行时
pub struct PluginRuntime {
    /// 运行时配置
    config: PluginRuntimeConfig,
    /// 原生插件注册表
    native_plugins: Arc<RwLock<HashMap<String, Box<dyn DataFlarePlugin>>>>,
    /// WASM插件实例池
    wasm_pools: Arc<RwLock<HashMap<String, WasmInstancePool>>>,
    /// 内存池
    memory_pool: Arc<MemoryPool>,
    /// 插件注册信息
    registrations: Arc<RwLock<HashMap<String, PluginRegistration>>>,
    /// 插件性能指标
    metrics: Arc<RwLock<HashMap<String, PluginMetrics>>>,
    /// 批处理统计
    batch_stats: Arc<RwLock<HashMap<String, BatchStats>>>,
    /// 配置管理器
    config_manager: PluginConfigManager,
}



/// 插件配置管理器
#[derive(Debug)]
pub struct PluginConfigManager {
    configs: HashMap<String, HashMap<String, String>>,
}

impl PluginConfigManager {
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    pub fn set_config(&mut self, plugin_id: &str, config: HashMap<String, String>) {
        self.configs.insert(plugin_id.to_string(), config);
    }

    pub fn get_config(&self, plugin_id: &str) -> Option<&HashMap<String, String>> {
        self.configs.get(plugin_id)
    }
}

impl PluginRuntime {
    /// 创建新的插件运行时
    pub fn new(config: PluginRuntimeConfig) -> Self {
        info!("创建插件运行时，配置: {:?}", config);

        // 创建内存池
        let memory_pool = Arc::new(MemoryPool::new(config.memory_pool_config.clone()));

        // 预热内存池
        memory_pool.warmup();

        Self {
            config,
            native_plugins: Arc::new(RwLock::new(HashMap::new())),
            wasm_pools: Arc::new(RwLock::new(HashMap::new())),
            memory_pool,
            registrations: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            batch_stats: Arc::new(RwLock::new(HashMap::new())),
            config_manager: PluginConfigManager::new(),
        }
    }

    /// 使用默认配置创建运行时
    pub fn default() -> Self {
        Self::new(PluginRuntimeConfig::default())
    }

    /// 注册原生插件
    pub fn register_native_plugin(
        &mut self,
        plugin_id: String,
        mut plugin: Box<dyn DataFlarePlugin>,
    ) -> Result<()> {
        info!("注册原生插件: {}", plugin_id);

        // 初始化插件
        if let Some(config) = self.config_manager.get_config(&plugin_id) {
            plugin.initialize(config)?;
        }

        let info = plugin.info();

        // 存储插件
        {
            let mut plugins = self.native_plugins.write()
                .map_err(|_| PluginError::Execution("获取插件锁失败".to_string()))?;
            plugins.insert(plugin_id.clone(), plugin);
        }

        // 存储注册信息
        {
            let mut registrations = self.registrations.write()
                .map_err(|_| PluginError::Execution("获取注册信息锁失败".to_string()))?;

            let registration = PluginRegistration {
                plugin_id: plugin_id.clone(),
                info,
                registered_at: std::time::SystemTime::now(),
                enabled: true,
                usage_count: 0,
                last_used: None,
            };

            registrations.insert(plugin_id.clone(), registration);
        }

        // 初始化指标
        {
            let mut metrics = self.metrics.write()
                .map_err(|_| PluginError::Execution("获取指标锁失败".to_string()))?;
            metrics.insert(plugin_id.clone(), PluginMetrics::default());
        }

        info!("原生插件注册成功: {}", plugin_id);
        Ok(())
    }

    /// 卸载插件
    pub fn unregister_plugin(&mut self, plugin_id: &str) -> Result<()> {
        info!("卸载插件: {}", plugin_id);

        // 从原生插件中移除
        {
            let mut plugins = self.native_plugins.write()
                .map_err(|_| PluginError::Execution("获取插件锁失败".to_string()))?;

            if let Some(mut plugin) = plugins.remove(plugin_id) {
                // 调用插件清理方法
                if let Err(e) = plugin.finalize() {
                    warn!("插件清理失败: {}: {}", plugin_id, e);
                }
            }
        }

        // 移除注册信息
        {
            let mut registrations = self.registrations.write()
                .map_err(|_| PluginError::Execution("获取注册信息锁失败".to_string()))?;
            registrations.remove(plugin_id);
        }

        // 移除指标
        {
            let mut metrics = self.metrics.write()
                .map_err(|_| PluginError::Execution("获取指标锁失败".to_string()))?;
            metrics.remove(plugin_id);
        }

        info!("插件卸载成功: {}", plugin_id);
        Ok(())
    }

    /// 处理记录
    pub fn process_record(
        &self,
        plugin_id: &str,
        record: &PluginRecord,
    ) -> Result<PluginResult> {
        let start_time = Instant::now();

        debug!("处理记录，插件: {}, 数据大小: {} bytes", plugin_id, record.size());

        // 检查插件是否存在且启用
        {
            let registrations = self.registrations.read()
                .map_err(|_| PluginError::Execution("获取注册信息锁失败".to_string()))?;

            if let Some(registration) = registrations.get(plugin_id) {
                if !registration.enabled {
                    return Err(PluginError::Execution(format!("插件已禁用: {}", plugin_id)));
                }
            } else {
                return Err(PluginError::Execution(format!("插件不存在: {}", plugin_id)));
            }
        }

        // 执行插件处理
        let result = {
            let plugins = self.native_plugins.read()
                .map_err(|_| PluginError::Execution("获取插件锁失败".to_string()))?;

            if let Some(plugin) = plugins.get(plugin_id) {
                plugin.process(record)
            } else {
                return Err(PluginError::Execution(format!("原生插件不存在: {}", plugin_id)));
            }
        };

        let execution_time = start_time.elapsed();
        let success = result.is_ok();

        // 更新指标
        if self.config.enable_metrics {
            if let Ok(mut metrics) = self.metrics.write() {
                if let Some(plugin_metrics) = metrics.get_mut(plugin_id) {
                    plugin_metrics.record_call(execution_time, success, record.size());
                }
            }

            // 更新使用统计
            if let Ok(mut registrations) = self.registrations.write() {
                if let Some(registration) = registrations.get_mut(plugin_id) {
                    registration.usage_count += 1;
                    registration.last_used = Some(std::time::SystemTime::now());
                }
            }
        }

        if self.config.debug_mode {
            debug!(
                "插件处理完成: {}, 耗时: {:?}, 成功: {}",
                plugin_id, execution_time, success
            );
        }

        result
    }

    /// 批处理记录
    pub fn process_batch(
        &self,
        plugin_id: &str,
        records: &[PluginRecord],
    ) -> Vec<Result<PluginResult>> {
        debug!("批处理记录，插件: {}, 记录数: {}", plugin_id, records.len());

        let start_time = Instant::now();

        let plugins = match self.native_plugins.read() {
            Ok(plugins) => plugins,
            Err(_) => {
                let error = PluginError::Execution("获取插件锁失败".to_string());
                return (0..records.len()).map(|_| Err(error.clone())).collect();
            }
        };

        let results = if let Some(plugin) = plugins.get(plugin_id) {
            // 使用标准批处理接口
            if self.config.enable_batch_optimization && plugin.supports_batch() {
                // 使用插件的批处理方法
                plugin.process_batch(records)
            } else {
                // 逐个处理记录
                records.iter().map(|record| plugin.process(record)).collect()
            }
        } else {
            let error = PluginError::Execution(format!("插件不存在: {}", plugin_id));
            (0..records.len()).map(|_| Err(error.clone())).collect()
        };

        let processing_time = start_time.elapsed();

        // 更新批处理统计
        if self.config.enable_metrics {
            self.update_batch_stats(plugin_id, records.len(), processing_time, &results);
        }

        results
    }

    /// 更新批处理统计
    fn update_batch_stats(
        &self,
        plugin_id: &str,
        record_count: usize,
        processing_time: Duration,
        results: &[Result<PluginResult>],
    ) {
        if let Ok(mut stats_map) = self.batch_stats.write() {
            let stats = stats_map.entry(plugin_id.to_string()).or_insert_with(BatchStats::default);

            stats.total_batches += 1;
            stats.total_records += record_count as u64;
            stats.avg_batch_size = stats.total_records as f64 / stats.total_batches as f64;

            let processing_time_ms = processing_time.as_millis() as f64;
            stats.avg_processing_time_ms =
                (stats.avg_processing_time_ms * (stats.total_batches - 1) as f64 + processing_time_ms)
                / stats.total_batches as f64;

            if processing_time.as_secs_f64() > 0.0 {
                stats.throughput = record_count as f64 / processing_time.as_secs_f64();
            }

            let error_count = results.iter().filter(|r| r.is_err()).count();
            let total_processed = stats.total_records as f64;
            if total_processed > 0.0 {
                stats.error_rate = error_count as f64 / total_processed * 100.0;
            }
        }
    }

    /// 获取插件信息
    pub fn get_plugin_info(&self, plugin_id: &str) -> Option<PluginInfo> {
        let registrations = self.registrations.read().ok()?;
        registrations.get(plugin_id).map(|reg| reg.info.clone())
    }

    /// 列出所有插件
    pub fn list_plugins(&self) -> Vec<PluginRegistration> {
        let registrations = match self.registrations.read() {
            Ok(guard) => guard,
            Err(_) => {
                warn!("获取注册信息锁失败");
                return Vec::new();
            }
        };

        registrations.values().cloned().collect()
    }

    /// 获取插件指标
    pub fn get_plugin_metrics(&self, plugin_id: &str) -> Option<PluginMetrics> {
        let metrics = self.metrics.read().ok()?;
        metrics.get(plugin_id).cloned()
    }

    /// 获取所有插件指标
    pub fn get_all_metrics(&self) -> HashMap<String, PluginMetrics> {
        let metrics = match self.metrics.read() {
            Ok(guard) => guard,
            Err(_) => {
                warn!("获取指标锁失败");
                return HashMap::new();
            }
        };

        metrics.clone()
    }

    /// 启用/禁用插件
    pub fn set_plugin_enabled(&mut self, plugin_id: &str, enabled: bool) -> Result<()> {
        let mut registrations = self.registrations.write()
            .map_err(|_| PluginError::Execution("获取注册信息锁失败".to_string()))?;

        if let Some(registration) = registrations.get_mut(plugin_id) {
            registration.enabled = enabled;
            info!("插件 {} 状态更新为: {}", plugin_id, if enabled { "启用" } else { "禁用" });
            Ok(())
        } else {
            Err(PluginError::Execution(format!("插件不存在: {}", plugin_id)))
        }
    }

    /// 重置插件指标
    pub fn reset_plugin_metrics(&mut self, plugin_id: &str) -> Result<()> {
        let mut metrics = self.metrics.write()
            .map_err(|_| PluginError::Execution("获取指标锁失败".to_string()))?;

        if metrics.contains_key(plugin_id) {
            metrics.insert(plugin_id.to_string(), PluginMetrics::default());
            info!("插件指标已重置: {}", plugin_id);
            Ok(())
        } else {
            Err(PluginError::Execution(format!("插件不存在: {}", plugin_id)))
        }
    }

    /// 获取批处理统计
    pub fn get_batch_stats(&self, plugin_id: &str) -> Option<BatchStats> {
        let stats = self.batch_stats.read().ok()?;
        stats.get(plugin_id).cloned()
    }

    /// 获取所有批处理统计
    pub fn get_all_batch_stats(&self) -> HashMap<String, BatchStats> {
        let stats = match self.batch_stats.read() {
            Ok(guard) => guard,
            Err(_) => {
                warn!("获取批处理统计锁失败");
                return HashMap::new();
            }
        };

        stats.clone()
    }

    /// 获取内存池统计
    pub fn get_memory_pool_stats(&self) -> crate::memory_pool::MemoryPoolStats {
        self.memory_pool.get_stats()
    }

    /// 清理空闲资源
    pub fn cleanup_idle_resources(&self) -> Result<()> {
        // 清理内存池
        let cleaned_buffers = self.memory_pool.cleanup_idle_buffers();

        // 清理WASM实例池
        let mut total_cleaned_instances = 0;
        #[cfg(feature = "wasm")]
        {
            if let Ok(pools) = self.wasm_pools.read() {
                for (plugin_id, pool) in pools.iter() {
                    match pool.cleanup_idle_instances() {
                        Ok(cleaned) => {
                            total_cleaned_instances += cleaned;
                            if cleaned > 0 {
                                debug!("清理了插件 {} 的 {} 个空闲WASM实例", plugin_id, cleaned);
                            }
                        }
                        Err(e) => {
                            warn!("清理插件 {} 的WASM实例失败: {}", plugin_id, e);
                        }
                    }
                }
            }
        }

        info!("资源清理完成：清理了 {} 个缓冲区，{} 个WASM实例",
              cleaned_buffers, total_cleaned_instances);

        Ok(())
    }

    /// 获取运行时统计摘要
    pub fn get_runtime_summary(&self) -> RuntimeSummary {
        let plugin_count = self.list_plugins().len();
        let memory_stats = self.get_memory_pool_stats();
        let all_metrics = self.get_all_metrics();
        let all_batch_stats = self.get_all_batch_stats();

        let total_calls: u64 = all_metrics.values().map(|m| m.total_calls).sum();
        let total_records: u64 = all_batch_stats.values().map(|s| s.total_records).sum();
        let avg_throughput: f64 = all_batch_stats.values()
            .map(|s| s.throughput)
            .filter(|&t| t > 0.0)
            .sum::<f64>() / all_batch_stats.len().max(1) as f64;

        RuntimeSummary {
            plugin_count,
            total_calls,
            total_records,
            memory_usage: memory_stats.total_memory_usage,
            cached_buffers: memory_stats.cached_buffers,
            avg_throughput,
            uptime: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default(),
        }
    }
}

/// 运行时统计摘要
#[derive(Debug, Clone)]
pub struct RuntimeSummary {
    /// 插件数量
    pub plugin_count: usize,
    /// 总调用次数
    pub total_calls: u64,
    /// 总处理记录数
    pub total_records: u64,
    /// 内存使用量（字节）
    pub memory_usage: usize,
    /// 缓存的缓冲区数量
    pub cached_buffers: usize,
    /// 平均吞吐量（记录/秒）
    pub avg_throughput: f64,
    /// 运行时间
    pub uptime: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PluginType;

    // 测试插件实现
    struct TestPlugin {
        name: String,
        version: String,
    }

    impl TestPlugin {
        fn new(name: &str, version: &str) -> Self {
            Self {
                name: name.to_string(),
                version: version.to_string(),
            }
        }
    }

    impl DataFlarePlugin for TestPlugin {
        fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
            // 简单的测试逻辑：如果数据包含"keep"则保留
            let data = record.value_as_str().map_err(|e| {
                PluginError::Execution(format!("UTF-8解析失败: {}", e))
            })?;

            Ok(PluginResult::Filtered(data.contains("keep")))
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn version(&self) -> &str {
            &self.version
        }

        fn plugin_type(&self) -> PluginType {
            PluginType::Filter
        }
    }

    #[test]
    fn test_plugin_runtime_creation() {
        let runtime = PluginRuntime::default();
        assert_eq!(runtime.config.max_concurrent_plugins, 100);
        assert_eq!(runtime.config.default_batch_size, 1000);
    }

    #[test]
    fn test_plugin_registration() {
        let mut runtime = PluginRuntime::default();
        let plugin = Box::new(TestPlugin::new("test_filter", "1.0.0"));

        let result = runtime.register_native_plugin("test_filter".to_string(), plugin);
        assert!(result.is_ok());

        let plugins = runtime.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].plugin_id, "test_filter");
    }

    #[test]
    fn test_plugin_processing() {
        let mut runtime = PluginRuntime::default();
        let plugin = Box::new(TestPlugin::new("test_filter", "1.0.0"));

        runtime.register_native_plugin("test_filter".to_string(), plugin).unwrap();

        let value = b"keep this record";
        let metadata = HashMap::new();
        let record = PluginRecord::new(value, &metadata, 1234567890, 0, 100);

        let result = runtime.process_record("test_filter", &record);
        assert!(result.is_ok());

        match result.unwrap() {
            PluginResult::Filtered(keep) => assert!(keep),
            _ => panic!("Expected filtered result"),
        }
    }

    #[test]
    fn test_plugin_metrics() {
        let mut runtime = PluginRuntime::default();
        let plugin = Box::new(TestPlugin::new("test_filter", "1.0.0"));

        runtime.register_native_plugin("test_filter".to_string(), plugin).unwrap();

        let value = b"keep this record";
        let metadata = HashMap::new();
        let record = PluginRecord::new(value, &metadata, 1234567890, 0, 100);

        // 处理几条记录
        for _ in 0..5 {
            let _ = runtime.process_record("test_filter", &record);
        }

        let metrics = runtime.get_plugin_metrics("test_filter").unwrap();
        assert_eq!(metrics.total_calls, 5);
        assert_eq!(metrics.successful_calls, 5);
        assert_eq!(metrics.records_processed, 5);
        assert_eq!(metrics.success_rate(), 100.0);
    }
}
