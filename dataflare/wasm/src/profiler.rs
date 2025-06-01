//! WASM插件性能分析器
//!
//! 提供详细的性能分析和监控功能

use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use log::{info, debug};

/// 性能分析器
#[derive(Debug)]
pub struct WasmProfiler {
    /// 是否启用分析
    enabled: bool,
    /// 开始时间
    start_time: Option<Instant>,
    /// 函数调用统计
    function_calls: HashMap<String, FunctionStats>,
    /// 内存使用统计
    memory_stats: MemoryStats,
    /// 执行时间统计
    execution_stats: ExecutionStats,
}

/// 函数调用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionStats {
    /// 调用次数
    pub call_count: u64,
    /// 总执行时间
    pub total_time: Duration,
    /// 平均执行时间
    pub avg_time: Duration,
    /// 最小执行时间
    pub min_time: Duration,
    /// 最大执行时间
    pub max_time: Duration,
    /// 错误次数
    pub error_count: u64,
}

/// 内存使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// 峰值内存使用
    pub peak_memory: u64,
    /// 当前内存使用
    pub current_memory: u64,
    /// 平均内存使用
    pub avg_memory: u64,
    /// 内存分配次数
    pub allocation_count: u64,
    /// 内存释放次数
    pub deallocation_count: u64,
}

/// 执行时间统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// 总执行时间
    pub total_time: Duration,
    /// 插件加载时间
    pub load_time: Duration,
    /// 初始化时间
    pub init_time: Duration,
    /// 数据处理时间
    pub processing_time: Duration,
    /// 清理时间
    pub cleanup_time: Duration,
}

/// 性能报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// 分析持续时间
    pub duration: Duration,
    /// 函数调用统计
    pub function_stats: HashMap<String, FunctionStats>,
    /// 内存统计
    pub memory_stats: MemoryStats,
    /// 执行统计
    pub execution_stats: ExecutionStats,
    /// 吞吐量 (操作/秒)
    pub throughput: f64,
    /// 平均延迟
    pub avg_latency: Duration,
    /// 错误率
    pub error_rate: f64,
}

impl Default for WasmProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmProfiler {
    /// 创建新的性能分析器
    pub fn new() -> Self {
        Self {
            enabled: false,
            start_time: None,
            function_calls: HashMap::new(),
            memory_stats: MemoryStats::default(),
            execution_stats: ExecutionStats::default(),
        }
    }

    /// 启用性能分析
    pub fn enable(&mut self) {
        self.enabled = true;
        info!("WASM性能分析器已启用");
    }

    /// 禁用性能分析
    pub fn disable(&mut self) {
        self.enabled = false;
        info!("WASM性能分析器已禁用");
    }

    /// 开始分析
    pub fn start_profiling(&mut self) {
        if !self.enabled {
            return;
        }

        self.start_time = Some(Instant::now());
        self.function_calls.clear();
        self.memory_stats = MemoryStats::default();
        self.execution_stats = ExecutionStats::default();
        
        debug!("开始WASM性能分析");
    }

    /// 停止分析
    pub fn stop_profiling(&mut self) -> Option<PerformanceReport> {
        if !self.enabled || self.start_time.is_none() {
            return None;
        }

        let duration = self.start_time.unwrap().elapsed();
        let report = self.generate_report(duration);
        
        debug!("停止WASM性能分析，持续时间: {:?}", duration);
        Some(report)
    }

    /// 记录函数调用
    pub fn record_function_call(&mut self, function_name: &str, execution_time: Duration, success: bool) {
        if !self.enabled {
            return;
        }

        let stats = self.function_calls.entry(function_name.to_string()).or_insert_with(|| {
            FunctionStats {
                call_count: 0,
                total_time: Duration::ZERO,
                avg_time: Duration::ZERO,
                min_time: Duration::MAX,
                max_time: Duration::ZERO,
                error_count: 0,
            }
        });

        stats.call_count += 1;
        stats.total_time += execution_time;
        stats.avg_time = stats.total_time / stats.call_count as u32;
        stats.min_time = stats.min_time.min(execution_time);
        stats.max_time = stats.max_time.max(execution_time);

        if !success {
            stats.error_count += 1;
        }

        debug!("记录函数调用: {} 执行时间: {:?} 成功: {}", function_name, execution_time, success);
    }

    /// 记录内存使用
    pub fn record_memory_usage(&mut self, current_memory: u64) {
        if !self.enabled {
            return;
        }

        self.memory_stats.current_memory = current_memory;
        self.memory_stats.peak_memory = self.memory_stats.peak_memory.max(current_memory);
        
        // 简单的平均值计算
        if self.memory_stats.allocation_count > 0 {
            self.memory_stats.avg_memory = 
                (self.memory_stats.avg_memory * self.memory_stats.allocation_count + current_memory) 
                / (self.memory_stats.allocation_count + 1);
        } else {
            self.memory_stats.avg_memory = current_memory;
        }
        
        self.memory_stats.allocation_count += 1;
    }

    /// 记录执行阶段时间
    pub fn record_execution_phase(&mut self, phase: ExecutionPhase, duration: Duration) {
        if !self.enabled {
            return;
        }

        match phase {
            ExecutionPhase::Load => self.execution_stats.load_time += duration,
            ExecutionPhase::Init => self.execution_stats.init_time += duration,
            ExecutionPhase::Processing => self.execution_stats.processing_time += duration,
            ExecutionPhase::Cleanup => self.execution_stats.cleanup_time += duration,
        }

        self.execution_stats.total_time += duration;
        debug!("记录执行阶段: {:?} 时间: {:?}", phase, duration);
    }

    /// 生成性能报告
    fn generate_report(&self, duration: Duration) -> PerformanceReport {
        let total_calls: u64 = self.function_calls.values().map(|s| s.call_count).sum();
        let total_errors: u64 = self.function_calls.values().map(|s| s.error_count).sum();
        
        let throughput = if duration.as_secs_f64() > 0.0 {
            total_calls as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        let avg_latency = if total_calls > 0 {
            self.execution_stats.total_time / total_calls as u32
        } else {
            Duration::ZERO
        };

        let error_rate = if total_calls > 0 {
            total_errors as f64 / total_calls as f64
        } else {
            0.0
        };

        PerformanceReport {
            duration,
            function_stats: self.function_calls.clone(),
            memory_stats: self.memory_stats.clone(),
            execution_stats: self.execution_stats.clone(),
            throughput,
            avg_latency,
            error_rate,
        }
    }

    /// 获取当前统计信息
    pub fn get_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        
        stats.insert("enabled".to_string(), serde_json::Value::Bool(self.enabled));
        stats.insert("function_calls".to_string(), serde_json::to_value(&self.function_calls).unwrap_or_default());
        stats.insert("memory_stats".to_string(), serde_json::to_value(&self.memory_stats).unwrap_or_default());
        stats.insert("execution_stats".to_string(), serde_json::to_value(&self.execution_stats).unwrap_or_default());
        
        stats
    }

    /// 重置统计信息
    pub fn reset(&mut self) {
        self.function_calls.clear();
        self.memory_stats = MemoryStats::default();
        self.execution_stats = ExecutionStats::default();
        self.start_time = None;
        
        debug!("重置WASM性能分析器统计信息");
    }
}

/// 执行阶段
#[derive(Debug, Clone, Copy)]
pub enum ExecutionPhase {
    /// 加载阶段
    Load,
    /// 初始化阶段
    Init,
    /// 处理阶段
    Processing,
    /// 清理阶段
    Cleanup,
}

impl Default for FunctionStats {
    fn default() -> Self {
        Self {
            call_count: 0,
            total_time: Duration::ZERO,
            avg_time: Duration::ZERO,
            min_time: Duration::MAX,
            max_time: Duration::ZERO,
            error_count: 0,
        }
    }
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            peak_memory: 0,
            current_memory: 0,
            avg_memory: 0,
            allocation_count: 0,
            deallocation_count: 0,
        }
    }
}

impl Default for ExecutionStats {
    fn default() -> Self {
        Self {
            total_time: Duration::ZERO,
            load_time: Duration::ZERO,
            init_time: Duration::ZERO,
            processing_time: Duration::ZERO,
            cleanup_time: Duration::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_profiler_creation() {
        let profiler = WasmProfiler::new();
        assert!(!profiler.enabled);
        assert!(profiler.start_time.is_none());
    }

    #[test]
    fn test_profiler_enable_disable() {
        let mut profiler = WasmProfiler::new();
        
        profiler.enable();
        assert!(profiler.enabled);
        
        profiler.disable();
        assert!(!profiler.enabled);
    }

    #[test]
    fn test_function_call_recording() {
        let mut profiler = WasmProfiler::new();
        profiler.enable();
        
        let duration = Duration::from_millis(100);
        profiler.record_function_call("test_function", duration, true);
        
        let stats = profiler.function_calls.get("test_function").unwrap();
        assert_eq!(stats.call_count, 1);
        assert_eq!(stats.total_time, duration);
        assert_eq!(stats.error_count, 0);
    }

    #[test]
    fn test_memory_recording() {
        let mut profiler = WasmProfiler::new();
        profiler.enable();
        
        profiler.record_memory_usage(1024);
        profiler.record_memory_usage(2048);
        
        assert_eq!(profiler.memory_stats.peak_memory, 2048);
        assert_eq!(profiler.memory_stats.current_memory, 2048);
        assert_eq!(profiler.memory_stats.allocation_count, 2);
    }

    #[test]
    fn test_execution_phase_recording() {
        let mut profiler = WasmProfiler::new();
        profiler.enable();
        
        let duration = Duration::from_millis(50);
        profiler.record_execution_phase(ExecutionPhase::Load, duration);
        profiler.record_execution_phase(ExecutionPhase::Processing, duration);
        
        assert_eq!(profiler.execution_stats.load_time, duration);
        assert_eq!(profiler.execution_stats.processing_time, duration);
        assert_eq!(profiler.execution_stats.total_time, duration * 2);
    }

    #[test]
    fn test_profiling_session() {
        let mut profiler = WasmProfiler::new();
        profiler.enable();
        profiler.start_profiling();
        
        // 模拟一些操作
        thread::sleep(Duration::from_millis(10));
        profiler.record_function_call("test", Duration::from_millis(5), true);
        
        let report = profiler.stop_profiling().unwrap();
        assert!(report.duration >= Duration::from_millis(10));
        assert_eq!(report.function_stats.len(), 1);
    }

    #[test]
    fn test_profiler_reset() {
        let mut profiler = WasmProfiler::new();
        profiler.enable();
        
        profiler.record_function_call("test", Duration::from_millis(100), true);
        profiler.record_memory_usage(1024);
        
        assert!(!profiler.function_calls.is_empty());
        assert!(profiler.memory_stats.allocation_count > 0);
        
        profiler.reset();
        
        assert!(profiler.function_calls.is_empty());
        assert_eq!(profiler.memory_stats.allocation_count, 0);
    }
}
