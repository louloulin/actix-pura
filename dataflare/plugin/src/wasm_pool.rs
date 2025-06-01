//! WASM实例池管理
//!
//! 高性能的WASM实例池，支持实例复用、编译缓存和内存管理
//!
//! 此模块仅在启用 `wasm` feature 时可用

use std::time::{Duration, Instant};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use serde::{Serialize, Deserialize};
use crate::core::{PluginError, Result};

/// WASM实例池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPoolConfig {
    /// 最大实例数
    pub max_instances: usize,
    /// 最小实例数
    pub min_instances: usize,
    /// 实例空闲超时时间
    pub idle_timeout: Duration,
    /// 实例预热时间
    pub warmup_timeout: Duration,
    /// 编译缓存大小
    pub compilation_cache_size: usize,
    /// 内存限制（字节）
    pub memory_limit: usize,
    /// 执行超时时间
    pub execution_timeout: Duration,
    /// 启用JIT编译
    pub enable_jit: bool,
    /// 启用多线程
    pub enable_threads: bool,
    /// 启用SIMD
    pub enable_simd: bool,
    /// 编译优化级别
    pub optimization_level: OptimizationLevel,
    /// 预热实例数
    pub warmup_instances: usize,
    /// 健康检查间隔
    pub health_check_interval: Duration,
}

/// 编译优化级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OptimizationLevel {
    /// 无优化（最快编译）
    None,
    /// 速度优化
    Speed,
    /// 大小优化
    Size,
    /// 速度和大小平衡
    SpeedAndSize,
}

impl Default for WasmPoolConfig {
    fn default() -> Self {
        Self {
            max_instances: 50,
            min_instances: 5,
            idle_timeout: Duration::from_secs(300), // 5分钟
            warmup_timeout: Duration::from_secs(10),
            compilation_cache_size: 200,
            memory_limit: 128 * 1024 * 1024, // 128MB
            execution_timeout: Duration::from_secs(30),
            enable_jit: true,
            enable_threads: false,
            enable_simd: true,
            optimization_level: OptimizationLevel::Speed,
            warmup_instances: 10,
            health_check_interval: Duration::from_secs(60),
        }
    }
}

/// WASM实例信息
#[derive(Debug)]
struct WasmInstance {
    /// 实例ID
    id: String,
    /// 创建时间
    created_at: Instant,
    /// 最后使用时间
    last_used: Instant,
    /// 使用次数
    usage_count: u64,
    /// 是否正在使用
    in_use: bool,
    /// 错误次数
    error_count: u64,
    /// 内存使用量（字节）
    memory_usage: usize,
    /// 编译时间（毫秒）
    compilation_time: u64,
}

impl WasmInstance {
    fn new(id: String) -> Self {
        let now = Instant::now();
        Self {
            id,
            created_at: now,
            last_used: now,
            usage_count: 0,
            in_use: false,
            error_count: 0,
            memory_usage: 0,
            compilation_time: 0,
        }
    }

    fn mark_used(&mut self) {
        self.last_used = Instant::now();
        self.usage_count += 1;
        self.in_use = true;
    }

    fn mark_released(&mut self) {
        self.in_use = false;
    }

    fn mark_error(&mut self) {
        self.error_count += 1;
    }

    fn is_idle(&self, timeout: Duration) -> bool {
        !self.in_use && self.last_used.elapsed() > timeout
    }

    fn is_healthy(&self, max_errors: u64) -> bool {
        self.error_count < max_errors
    }
}

/// 编译缓存条目
#[derive(Debug, Clone)]
struct CompilationCacheEntry {
    /// 编译后的模块（简化表示）
    compiled_module: Vec<u8>,
    /// 编译时间
    compilation_time: Duration,
    /// 创建时间
    created_at: Instant,
    /// 访问次数
    access_count: u64,
    /// 最后访问时间
    last_accessed: Instant,
    /// 模块大小
    module_size: usize,
}

/// WASM实例池统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPoolStats {
    /// 总实例数
    pub total_instances: usize,
    /// 可用实例数
    pub available_instances: usize,
    /// 使用中实例数
    pub in_use_instances: usize,
    /// 总获取次数
    pub total_acquisitions: u64,
    /// 总执行次数
    pub total_executions: u64,
    /// 总错误次数
    pub total_errors: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
    /// 平均编译时间（毫秒）
    pub average_compilation_time: f64,
    /// 平均执行时间（毫秒）
    pub average_execution_time: f64,
    /// 内存使用量（字节）
    pub memory_usage: usize,
    /// 错误率
    pub error_rate: f64,
    /// 缓存命中率
    pub cache_hit_rate: f64,
}

impl Default for WasmPoolStats {
    fn default() -> Self {
        Self {
            total_instances: 0,
            available_instances: 0,
            in_use_instances: 0,
            total_acquisitions: 0,
            total_executions: 0,
            total_errors: 0,
            cache_hits: 0,
            cache_misses: 0,
            average_compilation_time: 0.0,
            average_execution_time: 0.0,
            memory_usage: 0,
            error_rate: 0.0,
            cache_hit_rate: 0.0,
        }
    }
}

/// WASM实例池（增强版）
pub struct WasmInstancePool {
    /// 池配置
    config: WasmPoolConfig,
    /// 可用实例队列
    available_instances: Arc<Mutex<VecDeque<WasmInstance>>>,
    /// 使用中的实例
    in_use_instances: Arc<Mutex<HashMap<String, WasmInstance>>>,
    /// 编译缓存
    compilation_cache: Arc<RwLock<HashMap<String, CompilationCacheEntry>>>,
    /// 池统计信息
    stats: Arc<Mutex<WasmPoolStats>>,
}

impl WasmInstancePool {
    /// 创建新的实例池
    pub fn new(config: WasmPoolConfig) -> Result<Self> {
        #[cfg(not(feature = "wasm"))]
        {
            let _ = config;
            Err(PluginError::Configuration(
                "WASM功能未启用，请启用 'wasm' feature".to_string()
            ))
        }

        #[cfg(feature = "wasm")]
        {
            let pool = Self {
                config,
                available_instances: Arc::new(Mutex::new(VecDeque::new())),
                in_use_instances: Arc::new(Mutex::new(HashMap::new())),
                compilation_cache: Arc::new(RwLock::new(HashMap::new())),
                stats: Arc::new(Mutex::new(WasmPoolStats::default())),
            };

            // 预热实例
            pool.warmup_instances()?;

            Ok(pool)
        }
    }

    /// 预热实例
    #[cfg(feature = "wasm")]
    fn warmup_instances(&self) -> Result<()> {
        let warmup_count = self.config.warmup_instances.min(self.config.max_instances);

        for i in 0..warmup_count {
            let instance_id = format!("wasm_instance_{}", i);
            let instance = WasmInstance::new(instance_id);

            {
                let mut available = self.available_instances.lock().unwrap();
                available.push_back(instance);
            }

            {
                let mut stats = self.stats.lock().unwrap();
                stats.total_instances += 1;
                stats.available_instances += 1;
            }
        }

        Ok(())
    }

    /// 获取实例
    pub fn acquire_instance(&self) -> Result<String> {
        #[cfg(not(feature = "wasm"))]
        {
            Err(PluginError::Configuration(
                "WASM功能未启用".to_string()
            ))
        }

        #[cfg(feature = "wasm")]
        {
            // 尝试从可用实例中获取
            let instance = {
                let mut available = self.available_instances.lock().unwrap();
                available.pop_front()
            };

            let mut instance = match instance {
                Some(mut inst) => {
                    inst.mark_used();
                    inst
                }
                None => {
                    // 创建新实例
                    self.create_new_instance()?
                }
            };

            let instance_id = instance.id.clone();

            // 移动到使用中的实例
            {
                let mut in_use = self.in_use_instances.lock().unwrap();
                in_use.insert(instance_id.clone(), instance);
            }

            // 更新统计信息
            {
                let mut stats = self.stats.lock().unwrap();
                stats.available_instances = stats.available_instances.saturating_sub(1);
                stats.in_use_instances += 1;
                stats.total_acquisitions += 1;
            }

            Ok(instance_id)
        }
    }

    /// 创建新实例
    #[cfg(feature = "wasm")]
    fn create_new_instance(&self) -> Result<WasmInstance> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(0);

        let instance_id = format!("wasm_instance_{}", INSTANCE_COUNTER.fetch_add(1, Ordering::SeqCst));
        let mut instance = WasmInstance::new(instance_id);
        instance.mark_used();

        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_instances += 1;
        }

        Ok(instance)
    }

    /// 释放实例
    pub fn release_instance(&self, instance_id: &str) -> Result<()> {
        #[cfg(not(feature = "wasm"))]
        {
            let _ = instance_id;
            Err(PluginError::Configuration(
                "WASM功能未启用".to_string()
            ))
        }

        #[cfg(feature = "wasm")]
        {
            let mut instance = {
                let mut in_use = self.in_use_instances.lock().unwrap();
                in_use.remove(instance_id)
                    .ok_or_else(|| PluginError::Configuration(format!("Instance {} not found", instance_id)))?
            };

            instance.mark_released();

            // 检查实例健康状态
            if instance.is_healthy(10) { // 最多10个错误
                // 返回到可用实例池
                let mut available = self.available_instances.lock().unwrap();
                available.push_back(instance);

                let mut stats = self.stats.lock().unwrap();
                stats.available_instances += 1;
                stats.in_use_instances = stats.in_use_instances.saturating_sub(1);
            } else {
                // 丢弃不健康的实例
                let mut stats = self.stats.lock().unwrap();
                stats.total_instances = stats.total_instances.saturating_sub(1);
                stats.in_use_instances = stats.in_use_instances.saturating_sub(1);
            }

            Ok(())
        }
    }

    /// 清理空闲实例
    pub fn cleanup_idle_instances(&self) -> Result<usize> {
        #[cfg(not(feature = "wasm"))]
        {
            Err(PluginError::Configuration(
                "WASM功能未启用".to_string()
            ))
        }

        #[cfg(feature = "wasm")]
        {
            let idle_timeout = self.config.idle_timeout;
            let min_instances = self.config.min_instances;

            let mut cleaned_count = 0;

            {
                let mut available = self.available_instances.lock().unwrap();
                let current_count = available.len();

                // 保留最小实例数
                if current_count > min_instances {
                    let mut new_queue = VecDeque::new();

                    while let Some(instance) = available.pop_front() {
                        if instance.is_idle(idle_timeout) &&
                           (current_count - cleaned_count) > min_instances {
                            cleaned_count += 1;
                        } else {
                            new_queue.push_back(instance);
                        }
                    }

                    *available = new_queue;
                }
            }

            // 更新统计信息
            if cleaned_count > 0 {
                let mut stats = self.stats.lock().unwrap();
                stats.total_instances = stats.total_instances.saturating_sub(cleaned_count);
                stats.available_instances = stats.available_instances.saturating_sub(cleaned_count);
            }

            Ok(cleaned_count)
        }
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> WasmPoolStats {
        let stats = self.stats.lock().unwrap();
        let mut stats_copy = stats.clone();

        // 计算错误率和缓存命中率
        if stats_copy.total_executions > 0 {
            stats_copy.error_rate = stats_copy.total_errors as f64 / stats_copy.total_executions as f64;
        }

        let total_cache_requests = stats_copy.cache_hits + stats_copy.cache_misses;
        if total_cache_requests > 0 {
            stats_copy.cache_hit_rate = stats_copy.cache_hits as f64 / total_cache_requests as f64;
        }

        stats_copy
    }

    /// 获取编译缓存
    pub fn get_compiled_module(&self, module_hash: &str) -> Option<Vec<u8>> {
        #[cfg(not(feature = "wasm"))]
        {
            let _ = module_hash;
            None
        }

        #[cfg(feature = "wasm")]
        {
            let cache = self.compilation_cache.read().unwrap();
            if let Some(entry) = cache.get(module_hash) {
                // 更新访问统计
                drop(cache);
                let mut cache_write = self.compilation_cache.write().unwrap();
                if let Some(entry) = cache_write.get_mut(module_hash) {
                    entry.access_count += 1;
                    entry.last_accessed = Instant::now();
                }

                let mut stats = self.stats.lock().unwrap();
                stats.cache_hits += 1;

                cache_write.get(module_hash).map(|e| e.compiled_module.clone())
            } else {
                let mut stats = self.stats.lock().unwrap();
                stats.cache_misses += 1;
                None
            }
        }
    }

    /// 缓存编译后的模块
    pub fn cache_compiled_module(&self, module_hash: String, compiled_module: Vec<u8>, compilation_time: Duration) -> Result<()> {
        #[cfg(not(feature = "wasm"))]
        {
            let _ = (module_hash, compiled_module, compilation_time);
            Err(PluginError::Configuration(
                "WASM功能未启用".to_string()
            ))
        }

        #[cfg(feature = "wasm")]
        {
            let mut cache = self.compilation_cache.write().unwrap();

            // 检查缓存大小限制
            if cache.len() >= self.config.compilation_cache_size {
                // 移除最旧的条目
                if let Some((oldest_key, _)) = cache.iter()
                    .min_by_key(|(_, entry)| entry.last_accessed)
                    .map(|(k, v)| (k.clone(), v.clone())) {
                    cache.remove(&oldest_key);
                }
            }

            let entry = CompilationCacheEntry {
                module_size: compiled_module.len(),
                compiled_module,
                compilation_time,
                created_at: Instant::now(),
                access_count: 0,
                last_accessed: Instant::now(),
            };

            cache.insert(module_hash, entry);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_pool_config() {
        let config = WasmPoolConfig::default();
        assert_eq!(config.max_instances, 50);
        assert_eq!(config.min_instances, 5);
        assert!(config.enable_jit);
        assert!(config.enable_simd);
        assert!(!config.enable_threads);
        assert_eq!(config.warmup_instances, 10);
    }

    #[test]
    fn test_optimization_level() {
        let config = WasmPoolConfig::default();
        matches!(config.optimization_level, OptimizationLevel::Speed);
    }

    #[test]
    fn test_wasm_pool_creation_without_feature() {
        let config = WasmPoolConfig::default();
        let result = WasmInstancePool::new(config);

        #[cfg(not(feature = "wasm"))]
        assert!(result.is_err());

        #[cfg(feature = "wasm")]
        assert!(result.is_ok());
    }

    #[test]
    fn test_wasm_instance() {
        let instance = WasmInstance::new("test_instance".to_string());
        assert_eq!(instance.id, "test_instance");
        assert_eq!(instance.usage_count, 0);
        assert!(!instance.in_use);
        assert!(instance.is_healthy(10));
    }

    #[test]
    fn test_wasm_instance_lifecycle() {
        let mut instance = WasmInstance::new("test_instance".to_string());

        // 标记为使用中
        instance.mark_used();
        assert!(instance.in_use);
        assert_eq!(instance.usage_count, 1);

        // 标记为释放
        instance.mark_released();
        assert!(!instance.in_use);

        // 标记错误
        instance.mark_error();
        assert_eq!(instance.error_count, 1);
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_wasm_pool_stats() {
        let config = WasmPoolConfig::default();
        let pool = WasmInstancePool::new(config).unwrap();
        let stats = pool.get_stats();

        assert_eq!(stats.total_instances, 10); // warmup_instances
        assert_eq!(stats.available_instances, 10);
        assert_eq!(stats.in_use_instances, 0);
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_instance_acquisition_and_release() {
        let config = WasmPoolConfig::default();
        let pool = WasmInstancePool::new(config).unwrap();

        // 获取实例
        let instance_id = pool.acquire_instance().unwrap();
        let stats = pool.get_stats();
        assert_eq!(stats.available_instances, 9);
        assert_eq!(stats.in_use_instances, 1);

        // 释放实例
        pool.release_instance(&instance_id).unwrap();
        let stats = pool.get_stats();
        assert_eq!(stats.available_instances, 10);
        assert_eq!(stats.in_use_instances, 0);
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_compilation_cache() {
        let config = WasmPoolConfig::default();
        let pool = WasmInstancePool::new(config).unwrap();

        let module_hash = "test_hash".to_string();
        let compiled_module = vec![1, 2, 3, 4];
        let compilation_time = Duration::from_millis(100);

        // 缓存模块
        pool.cache_compiled_module(module_hash.clone(), compiled_module.clone(), compilation_time).unwrap();

        // 获取缓存的模块
        let cached_module = pool.get_compiled_module(&module_hash);
        assert_eq!(cached_module, Some(compiled_module));

        // 检查统计信息
        let stats = pool.get_stats();
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 0);
    }
}
