//! 内存池管理
//!
//! 高性能的内存池，减少内存分配和释放开销

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// 内存块大小类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferSize {
    /// 小缓冲区 (< 1KB)
    Small,
    /// 中等缓冲区 (1KB - 64KB)
    Medium,
    /// 大缓冲区 (> 64KB)
    Large,
}

impl BufferSize {
    /// 根据大小确定缓冲区类别
    pub fn from_size(size: usize) -> Self {
        if size < 1024 {
            BufferSize::Small
        } else if size < 64 * 1024 {
            BufferSize::Medium
        } else {
            BufferSize::Large
        }
    }

    /// 获取该类别的标准大小
    pub fn standard_size(self) -> usize {
        match self {
            BufferSize::Small => 1024,      // 1KB
            BufferSize::Medium => 64 * 1024, // 64KB
            BufferSize::Large => 1024 * 1024, // 1MB
        }
    }

    /// 获取该类别的最大缓存数量
    pub fn max_cached(self) -> usize {
        match self {
            BufferSize::Small => 1000,
            BufferSize::Medium => 100,
            BufferSize::Large => 10,
        }
    }
}

/// 内存块包装器
pub struct MemoryBuffer {
    /// 实际数据
    data: Vec<u8>,
    /// 创建时间
    created_at: Instant,
    /// 最后使用时间
    last_used: Instant,
    /// 使用次数
    usage_count: u64,
    /// 缓冲区类别
    size_category: BufferSize,
}

impl MemoryBuffer {
    /// 创建新的内存缓冲区
    pub fn new(size: usize, category: BufferSize) -> Self {
        let now = Instant::now();
        Self {
            data: vec![0; size],
            created_at: now,
            last_used: now,
            usage_count: 0,
            size_category: category,
        }
    }

    /// 获取数据的可变引用
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        self.last_used = Instant::now();
        self.usage_count += 1;
        &mut self.data
    }

    /// 获取数据的不可变引用
    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }

    /// 重置缓冲区
    pub fn reset(&mut self) {
        self.data.clear();
        self.last_used = Instant::now();
    }

    /// 调整缓冲区大小
    pub fn resize(&mut self, new_size: usize) {
        self.data.resize(new_size, 0);
        self.last_used = Instant::now();
    }

    /// 检查是否空闲超时
    pub fn is_idle(&self, timeout: Duration) -> bool {
        self.last_used.elapsed() > timeout
    }

    /// 获取缓冲区年龄
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// 获取使用次数
    pub fn usage_count(&self) -> u64 {
        self.usage_count
    }

    /// 获取大小类别
    pub fn size_category(&self) -> BufferSize {
        self.size_category
    }
}

/// 内存池配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryPoolConfig {
    /// 小缓冲区最大数量
    pub max_small_buffers: usize,
    /// 中等缓冲区最大数量
    pub max_medium_buffers: usize,
    /// 大缓冲区最大数量
    pub max_large_buffers: usize,
    /// 缓冲区空闲超时时间
    pub idle_timeout: Duration,
    /// 清理间隔
    pub cleanup_interval: Duration,
    /// 是否启用统计
    pub enable_stats: bool,
}

impl Default for MemoryPoolConfig {
    fn default() -> Self {
        Self {
            max_small_buffers: 1000,
            max_medium_buffers: 100,
            max_large_buffers: 10,
            idle_timeout: Duration::from_secs(300), // 5分钟
            cleanup_interval: Duration::from_secs(60), // 1分钟
            enable_stats: true,
        }
    }
}

/// 内存池统计信息
#[derive(Debug, Default, Clone)]
pub struct MemoryPoolStats {
    /// 总分配次数
    pub total_allocations: u64,
    /// 总释放次数
    pub total_deallocations: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
    /// 当前缓存的缓冲区数量
    pub cached_buffers: usize,
    /// 总内存使用量（字节）
    pub total_memory_usage: usize,
    /// 平均缓冲区使用次数
    pub avg_buffer_usage: f64,
}

/// 内存池
pub struct MemoryPool {
    /// 配置
    config: MemoryPoolConfig,
    /// 小缓冲区池
    small_buffers: Arc<Mutex<VecDeque<MemoryBuffer>>>,
    /// 中等缓冲区池
    medium_buffers: Arc<Mutex<VecDeque<MemoryBuffer>>>,
    /// 大缓冲区池
    large_buffers: Arc<Mutex<VecDeque<MemoryBuffer>>>,
    /// 统计信息
    stats: Arc<Mutex<MemoryPoolStats>>,
    /// 最后清理时间
    last_cleanup: Arc<Mutex<Instant>>,
}

impl MemoryPool {
    /// 创建新的内存池
    pub fn new(config: MemoryPoolConfig) -> Self {
        info!("创建内存池，配置: {:?}", config);

        Self {
            config,
            small_buffers: Arc::new(Mutex::new(VecDeque::new())),
            medium_buffers: Arc::new(Mutex::new(VecDeque::new())),
            large_buffers: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(Mutex::new(MemoryPoolStats::default())),
            last_cleanup: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 获取缓冲区
    pub fn get_buffer(&self, size: usize) -> MemoryBuffer {
        let category = BufferSize::from_size(size);
        let standard_size = category.standard_size().max(size);

        // 尝试从池中获取
        if let Some(mut buffer) = self.try_get_from_pool(category) {
            // 调整大小如果需要
            if buffer.data().len() < size {
                buffer.resize(size);
            } else {
                buffer.reset();
            }

            // 更新统计
            if self.config.enable_stats {
                if let Ok(mut stats) = self.stats.lock() {
                    stats.cache_hits += 1;
                    stats.total_allocations += 1;
                }
            }

            debug!("从池中获取缓冲区，类别: {:?}, 大小: {}", category, size);
            return buffer;
        }

        // 创建新缓冲区
        let buffer = MemoryBuffer::new(standard_size, category);

        // 更新统计
        if self.config.enable_stats {
            if let Ok(mut stats) = self.stats.lock() {
                stats.cache_misses += 1;
                stats.total_allocations += 1;
                stats.total_memory_usage += standard_size;
            }
        }

        debug!("创建新缓冲区，类别: {:?}, 大小: {}", category, standard_size);
        buffer
    }

    /// 归还缓冲区
    pub fn return_buffer(&self, buffer: MemoryBuffer) {
        let category = buffer.size_category();

        // 检查是否应该缓存
        if self.should_cache_buffer(&buffer) {
            if let Some(pool) = self.get_pool_for_category(category) {
                if let Ok(mut pool_guard) = pool.lock() {
                    let max_size = match category {
                        BufferSize::Small => self.config.max_small_buffers,
                        BufferSize::Medium => self.config.max_medium_buffers,
                        BufferSize::Large => self.config.max_large_buffers,
                    };

                    if pool_guard.len() < max_size {
                        pool_guard.push_back(buffer);
                        debug!("归还缓冲区到池，类别: {:?}", category);

                        // 更新统计
                        if self.config.enable_stats {
                            if let Ok(mut stats) = self.stats.lock() {
                                stats.cached_buffers += 1;
                            }
                        }
                        return;
                    }
                }
            }
        }

        // 不缓存，直接丢弃
        debug!("丢弃缓冲区，类别: {:?}", category);

        // 更新统计
        if self.config.enable_stats {
            if let Ok(mut stats) = self.stats.lock() {
                stats.total_deallocations += 1;
                stats.total_memory_usage = stats.total_memory_usage.saturating_sub(buffer.data().len());
            }
        }
    }

    /// 从池中尝试获取缓冲区
    fn try_get_from_pool(&self, category: BufferSize) -> Option<MemoryBuffer> {
        if let Some(pool) = self.get_pool_for_category(category) {
            if let Ok(mut pool_guard) = pool.lock() {
                if let Some(buffer) = pool_guard.pop_front() {
                    // 更新统计
                    if self.config.enable_stats {
                        if let Ok(mut stats) = self.stats.lock() {
                            stats.cached_buffers = stats.cached_buffers.saturating_sub(1);
                        }
                    }
                    return Some(buffer);
                }
            }
        }
        None
    }

    /// 获取指定类别的池
    fn get_pool_for_category(&self, category: BufferSize) -> Option<&Arc<Mutex<VecDeque<MemoryBuffer>>>> {
        match category {
            BufferSize::Small => Some(&self.small_buffers),
            BufferSize::Medium => Some(&self.medium_buffers),
            BufferSize::Large => Some(&self.large_buffers),
        }
    }

    /// 检查是否应该缓存缓冲区
    fn should_cache_buffer(&self, buffer: &MemoryBuffer) -> bool {
        // 不缓存过期的缓冲区
        if buffer.is_idle(self.config.idle_timeout) {
            return false;
        }

        // 不缓存使用次数过多的缓冲区（可能有内存碎片）
        if buffer.usage_count() > 1000 {
            return false;
        }

        // 不缓存过大的缓冲区
        if buffer.data().len() > 10 * 1024 * 1024 { // 10MB
            return false;
        }

        true
    }

    /// 清理空闲缓冲区
    pub fn cleanup_idle_buffers(&self) -> usize {
        let mut total_cleaned = 0;

        // 检查是否需要清理
        {
            let last_cleanup = self.last_cleanup.lock().unwrap();
            if last_cleanup.elapsed() < self.config.cleanup_interval {
                return 0;
            }
        }

        // 清理各个池
        total_cleaned += self.cleanup_pool(&self.small_buffers, "small");
        total_cleaned += self.cleanup_pool(&self.medium_buffers, "medium");
        total_cleaned += self.cleanup_pool(&self.large_buffers, "large");

        // 更新最后清理时间
        {
            let mut last_cleanup = self.last_cleanup.lock().unwrap();
            *last_cleanup = Instant::now();
        }

        if total_cleaned > 0 {
            info!("清理了 {} 个空闲缓冲区", total_cleaned);
        }

        total_cleaned
    }

    /// 清理指定池
    fn cleanup_pool(&self, pool: &Arc<Mutex<VecDeque<MemoryBuffer>>>, pool_name: &str) -> usize {
        let mut cleaned = 0;

        if let Ok(mut pool_guard) = pool.lock() {
            let mut to_remove = Vec::new();

            for (index, buffer) in pool_guard.iter().enumerate() {
                if buffer.is_idle(self.config.idle_timeout) {
                    to_remove.push(index);
                }
            }

            // 从后往前移除
            for &index in to_remove.iter().rev() {
                if let Some(buffer) = pool_guard.remove(index) {
                    cleaned += 1;

                    // 更新统计
                    if self.config.enable_stats {
                        if let Ok(mut stats) = self.stats.lock() {
                            stats.total_deallocations += 1;
                            stats.cached_buffers = stats.cached_buffers.saturating_sub(1);
                            stats.total_memory_usage = stats.total_memory_usage.saturating_sub(buffer.data().len());
                        }
                    }
                }
            }
        }

        if cleaned > 0 {
            debug!("清理了 {} 个 {} 缓冲区", cleaned, pool_name);
        }

        cleaned
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> MemoryPoolStats {
        if let Ok(stats) = self.stats.lock() {
            stats.clone()
        } else {
            MemoryPoolStats::default()
        }
    }

    /// 预热内存池
    pub fn warmup(&self) {
        info!("预热内存池");

        // 预创建一些缓冲区
        let small_count = self.config.max_small_buffers / 10;
        let medium_count = self.config.max_medium_buffers / 10;
        let large_count = self.config.max_large_buffers / 10;

        // 预热小缓冲区
        if let Ok(mut pool) = self.small_buffers.lock() {
            for _ in 0..small_count {
                let buffer = MemoryBuffer::new(BufferSize::Small.standard_size(), BufferSize::Small);
                pool.push_back(buffer);
            }
        }

        // 预热中等缓冲区
        if let Ok(mut pool) = self.medium_buffers.lock() {
            for _ in 0..medium_count {
                let buffer = MemoryBuffer::new(BufferSize::Medium.standard_size(), BufferSize::Medium);
                pool.push_back(buffer);
            }
        }

        // 预热大缓冲区
        if let Ok(mut pool) = self.large_buffers.lock() {
            for _ in 0..large_count {
                let buffer = MemoryBuffer::new(BufferSize::Large.standard_size(), BufferSize::Large);
                pool.push_back(buffer);
            }
        }

        // 更新统计
        if self.config.enable_stats {
            if let Ok(mut stats) = self.stats.lock() {
                let total_created = small_count + medium_count + large_count;
                stats.cached_buffers += total_created;
                stats.total_memory_usage += small_count * BufferSize::Small.standard_size() +
                                           medium_count * BufferSize::Medium.standard_size() +
                                           large_count * BufferSize::Large.standard_size();
            }
        }

        info!("内存池预热完成，创建了 {} 个缓冲区", small_count + medium_count + large_count);
    }

    /// 获取池大小信息
    pub fn get_pool_sizes(&self) -> (usize, usize, usize) {
        let small_size = self.small_buffers.lock().map(|p| p.len()).unwrap_or(0);
        let medium_size = self.medium_buffers.lock().map(|p| p.len()).unwrap_or(0);
        let large_size = self.large_buffers.lock().map(|p| p.len()).unwrap_or(0);

        (small_size, medium_size, large_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_size_categorization() {
        assert_eq!(BufferSize::from_size(512), BufferSize::Small);
        assert_eq!(BufferSize::from_size(2048), BufferSize::Medium);
        assert_eq!(BufferSize::from_size(128 * 1024), BufferSize::Large);
    }

    #[test]
    fn test_memory_pool_creation() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config);

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 0);
        assert_eq!(stats.cached_buffers, 0);
    }

    #[test]
    fn test_buffer_allocation_and_return() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config);

        // 获取缓冲区
        let buffer = pool.get_buffer(1024);
        assert_eq!(buffer.data().len(), 1024);
        assert_eq!(buffer.size_category(), BufferSize::Medium);

        // 归还缓冲区
        pool.return_buffer(buffer);

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.cached_buffers, 1);
    }

    #[test]
    fn test_buffer_reuse() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config);

        // 第一次分配
        let buffer1 = pool.get_buffer(1024);
        pool.return_buffer(buffer1);

        // 第二次分配应该复用
        let buffer2 = pool.get_buffer(1024);
        pool.return_buffer(buffer2);

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 2);
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 1);
    }

    #[test]
    fn test_pool_warmup() {
        let config = MemoryPoolConfig::default();
        let pool = MemoryPool::new(config);

        pool.warmup();

        let (small, medium, large) = pool.get_pool_sizes();
        assert!(small > 0);
        assert!(medium > 0);
        assert!(large > 0);

        let stats = pool.get_stats();
        assert!(stats.cached_buffers > 0);
        assert!(stats.total_memory_usage > 0);
    }
}
