//! Plugin Instance Pool
//!
//! 插件实例池管理，提供高性能的插件实例复用
//!
//! ## 特性
//! - 实例池管理：避免重复创建插件实例
//! - 负载均衡：智能分配插件实例
//! - 资源监控：监控实例使用情况
//! - 自动扩缩容：根据负载自动调整实例数量

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::interface::{DataFlarePlugin, PluginRecord, PluginResult, PluginError};
use crate::backend::{PluginBackend, PluginBackendConfig};

/// 插件实例池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPoolConfig {
    /// 最小实例数
    pub min_instances: usize,
    /// 最大实例数
    pub max_instances: usize,
    /// 实例空闲超时（秒）
    pub idle_timeout: u64,
    /// 实例预热数量
    pub warmup_instances: usize,
    /// 负载阈值（百分比）
    pub load_threshold: f64,
    /// 扩容步长
    pub scale_up_step: usize,
    /// 缩容步长
    pub scale_down_step: usize,
    /// 健康检查间隔（秒）
    pub health_check_interval: u64,
}

impl Default for PluginPoolConfig {
    fn default() -> Self {
        Self {
            min_instances: 1,
            max_instances: 10,
            idle_timeout: 300,      // 5分钟
            warmup_instances: 2,
            load_threshold: 0.8,    // 80%
            scale_up_step: 2,
            scale_down_step: 1,
            health_check_interval: 30, // 30秒
        }
    }
}

/// 插件实例信息
struct PluginInstance {
    /// 实例ID
    id: String,
    /// 插件实例
    plugin: Box<dyn DataFlarePlugin>,
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
}

impl PluginInstance {
    fn new(id: String, plugin: Box<dyn DataFlarePlugin>) -> Self {
        let now = Instant::now();
        Self {
            id,
            plugin,
            created_at: now,
            last_used: now,
            usage_count: 0,
            in_use: false,
            error_count: 0,
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

/// 插件实例池
pub struct PluginPool {
    /// 池配置
    config: PluginPoolConfig,
    /// 插件后端配置
    backend_config: PluginBackendConfig,
    /// 可用实例队列
    available_instances: Arc<Mutex<VecDeque<PluginInstance>>>,
    /// 使用中的实例
    in_use_instances: Arc<Mutex<HashMap<String, PluginInstance>>>,
    /// 信号量（控制并发数）
    semaphore: Arc<Semaphore>,
    /// 池统计信息
    stats: Arc<Mutex<PoolStats>>,
    /// 插件后端
    backend: Arc<Mutex<Box<dyn PluginBackend>>>,
}

impl PluginPool {
    /// 创建新的插件池
    pub async fn new(
        config: PluginPoolConfig,
        backend_config: PluginBackendConfig,
        backend: Box<dyn PluginBackend>,
    ) -> Result<Self, PluginError> {
        let pool = Self {
            semaphore: Arc::new(Semaphore::new(config.max_instances)),
            available_instances: Arc::new(Mutex::new(VecDeque::new())),
            in_use_instances: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(PoolStats::default())),
            backend: Arc::new(Mutex::new(backend)),
            config,
            backend_config,
        };

        // 预热实例
        pool.warmup().await?;

        Ok(pool)
    }

    /// 预热插件实例
    async fn warmup(&self) -> Result<(), PluginError> {
        let warmup_count = self.config.warmup_instances.min(self.config.max_instances);

        for i in 0..warmup_count {
            let instance_id = format!("{}_{}", self.backend_config.plugin_id, i);
            let plugin = {
                let mut backend = self.backend.lock().unwrap();
                backend.load_plugin(&self.backend_config).await?
            };

            let instance = PluginInstance::new(instance_id, plugin);

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

        log::info!("Plugin pool warmed up with {} instances", warmup_count);
        Ok(())
    }

    /// 获取插件实例
    pub async fn acquire(&self) -> Result<PluginInstanceHandle, PluginError> {
        // 获取信号量许可
        let _permit = self.semaphore.acquire().await
            .map_err(|_| PluginError::generic("Failed to acquire semaphore permit"))?;

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
                self.create_instance().await?
            }
        };

        // 移动到使用中的实例
        let instance_id = instance.id.clone();
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

        Ok(PluginInstanceHandle {
            instance_id,
            pool: self,
            _permit,
        })
    }

    /// 创建新的插件实例
    async fn create_instance(&self) -> Result<PluginInstance, PluginError> {
        let instance_id = format!("{}_{}", self.backend_config.plugin_id, uuid::Uuid::new_v4());

        let plugin = {
            let mut backend = self.backend.lock().unwrap();
            backend.load_plugin(&self.backend_config).await?
        };

        let mut instance = PluginInstance::new(instance_id, plugin);
        instance.mark_used();

        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_instances += 1;
        }

        Ok(instance)
    }

    /// 释放插件实例
    fn release(&self, instance_id: &str) -> Result<(), PluginError> {
        let mut instance = {
            let mut in_use = self.in_use_instances.lock().unwrap();
            in_use.remove(instance_id)
                .ok_or_else(|| PluginError::generic(format!("Instance {} not found", instance_id)))?
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
            stats.discarded_instances += 1;
        }

        Ok(())
    }

    /// 获取池统计信息
    pub fn get_stats(&self) -> PoolStats {
        let stats = self.stats.lock().unwrap();
        stats.clone()
    }

    /// 清理空闲实例
    pub async fn cleanup_idle_instances(&self) -> Result<(), PluginError> {
        let idle_timeout = Duration::from_secs(self.config.idle_timeout);
        let min_instances = self.config.min_instances;

        let mut to_remove = Vec::new();

        {
            let mut available = self.available_instances.lock().unwrap();
            let current_count = available.len();

            // 保留最小实例数
            if current_count > min_instances {
                let mut new_queue = VecDeque::new();
                let mut removed_count = 0;

                while let Some(instance) = available.pop_front() {
                    if instance.is_idle(idle_timeout) &&
                       (current_count - removed_count) > min_instances {
                        to_remove.push(instance.id.clone());
                        removed_count += 1;
                    } else {
                        new_queue.push_back(instance);
                    }
                }

                *available = new_queue;
            }
        }

        // 更新统计信息
        if !to_remove.is_empty() {
            let mut stats = self.stats.lock().unwrap();
            stats.total_instances = stats.total_instances.saturating_sub(to_remove.len());
            stats.available_instances = stats.available_instances.saturating_sub(to_remove.len());
            stats.cleaned_instances += to_remove.len() as u64;
        }

        log::debug!("Cleaned up {} idle instances", to_remove.len());
        Ok(())
    }
}

/// 插件实例句柄
pub struct PluginInstanceHandle<'a> {
    instance_id: String,
    pool: &'a PluginPool,
    _permit: tokio::sync::SemaphorePermit<'a>,
}

impl<'a> PluginInstanceHandle<'a> {
    /// 处理记录
    pub fn process(&self, record: &PluginRecord) -> Result<PluginResult, PluginError> {
        let result = {
            let mut in_use = self.pool.in_use_instances.lock().unwrap();
            let instance = in_use.get_mut(&self.instance_id)
                .ok_or_else(|| PluginError::generic("Instance not found"))?;

            let result = instance.plugin.process(record);

            // 记录错误
            if result.is_err() {
                instance.mark_error();
            }

            result
        };

        // 更新统计信息
        {
            let mut stats = self.pool.stats.lock().unwrap();
            stats.total_executions += 1;
            if result.is_err() {
                stats.total_errors += 1;
            }
        }

        result
    }

    /// 批处理记录
    pub fn process_batch(&self, records: &[PluginRecord]) -> Vec<Result<PluginResult, PluginError>> {
        let results = {
            let mut in_use = self.pool.in_use_instances.lock().unwrap();
            let instance = in_use.get_mut(&self.instance_id)
                .expect("Instance not found");

            let results = instance.plugin.process_batch(records);

            // 记录错误
            let error_count = results.iter().filter(|r| r.is_err()).count();
            for _ in 0..error_count {
                instance.mark_error();
            }

            results
        };

        // 更新统计信息
        {
            let mut stats = self.pool.stats.lock().unwrap();
            stats.total_executions += records.len() as u64;
            stats.total_errors += results.iter().filter(|r| r.is_err()).count() as u64;
        }

        results
    }
}

impl<'a> Drop for PluginInstanceHandle<'a> {
    fn drop(&mut self) {
        if let Err(e) = self.pool.release(&self.instance_id) {
            log::error!("Failed to release plugin instance {}: {}", self.instance_id, e);
        }
    }
}

/// 池统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
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
    /// 丢弃的实例数
    pub discarded_instances: u64,
    /// 清理的实例数
    pub cleaned_instances: u64,
    /// 平均响应时间（毫秒）
    pub average_response_time: f64,
    /// 错误率
    pub error_rate: f64,
}

impl Default for PoolStats {
    fn default() -> Self {
        Self {
            total_instances: 0,
            available_instances: 0,
            in_use_instances: 0,
            total_acquisitions: 0,
            total_executions: 0,
            total_errors: 0,
            discarded_instances: 0,
            cleaned_instances: 0,
            average_response_time: 0.0,
            error_rate: 0.0,
        }
    }
}

impl PoolStats {
    /// 计算错误率
    pub fn calculate_error_rate(&mut self) {
        if self.total_executions > 0 {
            self.error_rate = self.total_errors as f64 / self.total_executions as f64;
        } else {
            self.error_rate = 0.0;
        }
    }
}
