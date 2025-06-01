//! WASM插件监控指标系统
//!
//! 提供全面的监控指标收集和报告功能

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use log::{info, debug, warn};

/// 指标收集器
#[derive(Debug)]
pub struct MetricsCollector {
    /// 指标存储
    metrics: Arc<RwLock<HashMap<String, MetricValue>>>,
    /// 计数器
    counters: Arc<RwLock<HashMap<String, u64>>>,
    /// 直方图
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
    /// 仪表盘
    gauges: Arc<RwLock<HashMap<String, f64>>>,
    /// 开始时间
    start_time: Instant,
}

/// 指标值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    /// 计数器值
    Counter(u64),
    /// 仪表盘值
    Gauge(f64),
    /// 直方图值
    Histogram(HistogramData),
    /// 时间序列值
    TimeSeries(Vec<TimePoint>),
}

/// 直方图数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramData {
    /// 样本数量
    pub count: u64,
    /// 总和
    pub sum: f64,
    /// 桶
    pub buckets: Vec<Bucket>,
}

/// 直方图桶
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bucket {
    /// 上界
    pub upper_bound: f64,
    /// 累计计数
    pub cumulative_count: u64,
}

/// 时间点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimePoint {
    /// 时间戳
    pub timestamp: u64,
    /// 值
    pub value: f64,
}

/// 直方图
#[derive(Debug, Clone)]
pub struct Histogram {
    /// 桶边界
    buckets: Vec<f64>,
    /// 桶计数
    counts: Vec<u64>,
    /// 总计数
    total_count: u64,
    /// 总和
    sum: f64,
}

/// 系统指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// CPU使用率
    pub cpu_usage: f64,
    /// 内存使用量
    pub memory_usage: u64,
    /// 内存使用率
    pub memory_usage_percent: f64,
    /// 活跃插件数量
    pub active_plugins: u64,
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 平均响应时间
    pub avg_response_time: Duration,
    /// 吞吐量 (请求/秒)
    pub throughput: f64,
}

/// 插件指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetrics {
    /// 插件ID
    pub plugin_id: String,
    /// 加载时间
    pub load_time: Duration,
    /// 执行次数
    pub execution_count: u64,
    /// 成功次数
    pub success_count: u64,
    /// 失败次数
    pub error_count: u64,
    /// 平均执行时间
    pub avg_execution_time: Duration,
    /// 内存使用量
    pub memory_usage: u64,
    /// 最后执行时间
    pub last_execution: Option<SystemTime>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// 创建新的指标收集器
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            counters: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    /// 增加计数器
    pub fn increment_counter(&self, name: &str, value: u64) {
        if let Ok(mut counters) = self.counters.write() {
            *counters.entry(name.to_string()).or_insert(0) += value;
            debug!("计数器 {} 增加 {}", name, value);
        }
    }

    /// 设置仪表盘值
    pub fn set_gauge(&self, name: &str, value: f64) {
        if let Ok(mut gauges) = self.gauges.write() {
            gauges.insert(name.to_string(), value);
            debug!("仪表盘 {} 设置为 {}", name, value);
        }
    }

    /// 记录直方图值
    pub fn record_histogram(&self, name: &str, value: f64) {
        if let Ok(mut histograms) = self.histograms.write() {
            let histogram = histograms.entry(name.to_string()).or_insert_with(|| {
                Histogram::new(vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0])
            });
            histogram.observe(value);
            debug!("直方图 {} 记录值 {}", name, value);
        }
    }

    /// 记录执行时间
    pub fn record_execution_time(&self, name: &str, duration: Duration) {
        let millis = duration.as_secs_f64() * 1000.0;
        self.record_histogram(&format!("{}_duration_ms", name), millis);
    }

    /// 获取计数器值
    pub fn get_counter(&self, name: &str) -> Option<u64> {
        self.counters.read().ok()?.get(name).copied()
    }

    /// 获取仪表盘值
    pub fn get_gauge(&self, name: &str) -> Option<f64> {
        self.gauges.read().ok()?.get(name).copied()
    }

    /// 获取所有指标
    pub fn get_all_metrics(&self) -> HashMap<String, MetricValue> {
        let mut all_metrics = HashMap::new();

        // 添加计数器
        if let Ok(counters) = self.counters.read() {
            for (name, value) in counters.iter() {
                all_metrics.insert(name.clone(), MetricValue::Counter(*value));
            }
        }

        // 添加仪表盘
        if let Ok(gauges) = self.gauges.read() {
            for (name, value) in gauges.iter() {
                all_metrics.insert(name.clone(), MetricValue::Gauge(*value));
            }
        }

        // 添加直方图
        if let Ok(histograms) = self.histograms.read() {
            for (name, histogram) in histograms.iter() {
                all_metrics.insert(name.clone(), MetricValue::Histogram(histogram.to_data()));
            }
        }

        all_metrics
    }

    /// 获取系统指标
    pub fn get_system_metrics(&self) -> SystemMetrics {
        let total_requests = self.get_counter("total_requests").unwrap_or(0);
        let successful_requests = self.get_counter("successful_requests").unwrap_or(0);
        let failed_requests = self.get_counter("failed_requests").unwrap_or(0);
        
        let uptime = self.start_time.elapsed();
        let throughput = if uptime.as_secs_f64() > 0.0 {
            total_requests as f64 / uptime.as_secs_f64()
        } else {
            0.0
        };

        SystemMetrics {
            cpu_usage: self.get_gauge("cpu_usage").unwrap_or(0.0),
            memory_usage: self.get_gauge("memory_usage").unwrap_or(0.0) as u64,
            memory_usage_percent: self.get_gauge("memory_usage_percent").unwrap_or(0.0),
            active_plugins: self.get_counter("active_plugins").unwrap_or(0),
            total_requests,
            successful_requests,
            failed_requests,
            avg_response_time: Duration::from_millis(
                self.get_gauge("avg_response_time_ms").unwrap_or(0.0) as u64
            ),
            throughput,
        }
    }

    /// 记录插件指标
    pub fn record_plugin_metrics(&self, plugin_id: &str, metrics: &PluginMetrics) {
        let prefix = format!("plugin.{}", plugin_id);
        
        self.increment_counter(&format!("{}.execution_count", prefix), metrics.execution_count);
        self.increment_counter(&format!("{}.success_count", prefix), metrics.success_count);
        self.increment_counter(&format!("{}.error_count", prefix), metrics.error_count);
        
        self.set_gauge(&format!("{}.memory_usage", prefix), metrics.memory_usage as f64);
        self.record_execution_time(&format!("{}.load", prefix), metrics.load_time);
        self.record_execution_time(&format!("{}.execution", prefix), metrics.avg_execution_time);
        
        info!("记录插件 {} 的指标", plugin_id);
    }

    /// 导出Prometheus格式指标
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // 导出计数器
        if let Ok(counters) = self.counters.read() {
            for (name, value) in counters.iter() {
                output.push_str(&format!("# TYPE {} counter\n", name));
                output.push_str(&format!("{} {}\n", name, value));
            }
        }

        // 导出仪表盘
        if let Ok(gauges) = self.gauges.read() {
            for (name, value) in gauges.iter() {
                output.push_str(&format!("# TYPE {} gauge\n", name));
                output.push_str(&format!("{} {}\n", name, value));
            }
        }

        // 导出直方图
        if let Ok(histograms) = self.histograms.read() {
            for (name, histogram) in histograms.iter() {
                output.push_str(&format!("# TYPE {} histogram\n", name));
                let data = histogram.to_data();
                
                for bucket in &data.buckets {
                    output.push_str(&format!(
                        "{}_bucket{{le=\"{}\"}} {}\n", 
                        name, bucket.upper_bound, bucket.cumulative_count
                    ));
                }
                
                output.push_str(&format!("{}_bucket{{le=\"+Inf\"}} {}\n", name, data.count));
                output.push_str(&format!("{}_sum {}\n", name, data.sum));
                output.push_str(&format!("{}_count {}\n", name, data.count));
            }
        }

        output
    }

    /// 重置所有指标
    pub fn reset(&self) {
        if let Ok(mut counters) = self.counters.write() {
            counters.clear();
        }
        if let Ok(mut gauges) = self.gauges.write() {
            gauges.clear();
        }
        if let Ok(mut histograms) = self.histograms.write() {
            histograms.clear();
        }
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.clear();
        }
        
        info!("重置所有监控指标");
    }
}

impl Histogram {
    /// 创建新的直方图
    pub fn new(buckets: Vec<f64>) -> Self {
        let mut sorted_buckets = buckets;
        sorted_buckets.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        Self {
            buckets: sorted_buckets.clone(),
            counts: vec![0; sorted_buckets.len()],
            total_count: 0,
            sum: 0.0,
        }
    }

    /// 观察一个值
    pub fn observe(&mut self, value: f64) {
        self.sum += value;
        self.total_count += 1;

        for (i, &bucket) in self.buckets.iter().enumerate() {
            if value <= bucket {
                self.counts[i] += 1;
            }
        }
    }

    /// 转换为数据格式
    pub fn to_data(&self) -> HistogramData {
        let mut cumulative_count = 0;
        let buckets = self.buckets.iter().zip(self.counts.iter()).map(|(&upper_bound, &count)| {
            cumulative_count += count;
            Bucket {
                upper_bound,
                cumulative_count,
            }
        }).collect();

        HistogramData {
            count: self.total_count,
            sum: self.sum,
            buckets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        assert!(collector.get_counter("test").is_none());
        assert!(collector.get_gauge("test").is_none());
    }

    #[test]
    fn test_counter_operations() {
        let collector = MetricsCollector::new();
        
        collector.increment_counter("test_counter", 5);
        collector.increment_counter("test_counter", 3);
        
        assert_eq!(collector.get_counter("test_counter"), Some(8));
    }

    #[test]
    fn test_gauge_operations() {
        let collector = MetricsCollector::new();
        
        collector.set_gauge("test_gauge", 42.5);
        assert_eq!(collector.get_gauge("test_gauge"), Some(42.5));
        
        collector.set_gauge("test_gauge", 100.0);
        assert_eq!(collector.get_gauge("test_gauge"), Some(100.0));
    }

    #[test]
    fn test_histogram_operations() {
        let collector = MetricsCollector::new();
        
        collector.record_histogram("test_histogram", 1.5);
        collector.record_histogram("test_histogram", 2.5);
        collector.record_histogram("test_histogram", 0.5);
        
        let metrics = collector.get_all_metrics();
        assert!(metrics.contains_key("test_histogram"));
    }

    #[test]
    fn test_execution_time_recording() {
        let collector = MetricsCollector::new();
        
        let duration = Duration::from_millis(150);
        collector.record_execution_time("test_operation", duration);
        
        let metrics = collector.get_all_metrics();
        assert!(metrics.contains_key("test_operation_duration_ms"));
    }

    #[test]
    fn test_system_metrics() {
        let collector = MetricsCollector::new();
        
        collector.increment_counter("total_requests", 100);
        collector.increment_counter("successful_requests", 95);
        collector.increment_counter("failed_requests", 5);
        collector.set_gauge("cpu_usage", 75.5);
        
        let system_metrics = collector.get_system_metrics();
        assert_eq!(system_metrics.total_requests, 100);
        assert_eq!(system_metrics.successful_requests, 95);
        assert_eq!(system_metrics.failed_requests, 5);
        assert_eq!(system_metrics.cpu_usage, 75.5);
    }

    #[test]
    fn test_prometheus_export() {
        let collector = MetricsCollector::new();
        
        collector.increment_counter("test_counter", 42);
        collector.set_gauge("test_gauge", 3.14);
        
        let prometheus_output = collector.export_prometheus();
        assert!(prometheus_output.contains("test_counter 42"));
        assert!(prometheus_output.contains("test_gauge 3.14"));
    }

    #[test]
    fn test_histogram_creation() {
        let mut histogram = Histogram::new(vec![1.0, 5.0, 10.0]);
        
        histogram.observe(0.5);
        histogram.observe(3.0);
        histogram.observe(7.0);
        histogram.observe(15.0);
        
        let data = histogram.to_data();
        assert_eq!(data.count, 4);
        assert_eq!(data.sum, 25.5);
        assert_eq!(data.buckets.len(), 3);
    }
}
