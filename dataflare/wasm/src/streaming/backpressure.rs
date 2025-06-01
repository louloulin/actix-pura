//! 智能背压控制器
//!
//! 提供自适应流量控制和负载管理，借鉴 Fluvio 的背压设计

use std::time::{Duration, Instant};
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};

/// 背压控制器
pub struct BackpressureController {
    /// 背压阈值 (0.0 - 1.0)
    threshold: f64,
    /// 最大并发数
    max_concurrent: usize,
    /// 当前负载
    current_load: f64,
    /// 负载历史 (最近100个样本)
    load_history: VecDeque<LoadSample>,
    /// 背压策略
    strategy: BackpressureStrategy,
    /// 控制状态
    state: BackpressureState,
    /// 统计信息
    stats: BackpressureStats,
}

/// 负载样本
#[derive(Debug, Clone)]
struct LoadSample {
    /// 负载值
    load: f64,
    /// 时间戳
    timestamp: Instant,
}

/// 背压策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackpressureStrategy {
    /// 线性降速
    LinearSlowdown { factor: f64 },
    /// 指数退避
    ExponentialBackoff { base: f64, max_delay: Duration },
    /// 自适应控制
    AdaptiveControl { target_latency: Duration },
    /// 基于令牌桶的控制
    TokenBucket { tokens_per_second: f64, bucket_size: usize },
}

/// 背压状态
#[derive(Debug, Clone, PartialEq, Eq)]
enum BackpressureState {
    /// 正常状态
    Normal,
    /// 轻微背压
    LightBackpressure,
    /// 中等背压
    ModerateBackpressure,
    /// 严重背压
    SevereBackpressure,
}

/// 背压统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackpressureStats {
    /// 背压触发次数
    pub trigger_count: u64,
    /// 总背压时间 (毫秒)
    pub total_backpressure_time_ms: u64,
    /// 平均负载
    pub average_load: f64,
    /// 峰值负载
    pub peak_load: f64,
    /// 最后触发时间
    pub last_trigger_time: Option<i64>,
    /// 当前状态
    pub current_state: String,
}

impl Default for BackpressureStats {
    fn default() -> Self {
        Self {
            trigger_count: 0,
            total_backpressure_time_ms: 0,
            average_load: 0.0,
            peak_load: 0.0,
            last_trigger_time: None,
            current_state: "Normal".to_string(),
        }
    }
}

impl BackpressureController {
    /// 创建新的背压控制器
    pub fn new(threshold: f64, max_concurrent: usize) -> Self {
        Self {
            threshold,
            max_concurrent,
            current_load: 0.0,
            load_history: VecDeque::with_capacity(100),
            strategy: BackpressureStrategy::AdaptiveControl {
                target_latency: Duration::from_millis(10),
            },
            state: BackpressureState::Normal,
            stats: BackpressureStats::default(),
        }
    }

    /// 设置背压策略
    pub fn set_strategy(&mut self, strategy: BackpressureStrategy) {
        self.strategy = strategy;
    }

    /// 更新当前负载
    pub fn update_load(&mut self, load: f64) {
        self.current_load = load;
        
        // 添加负载样本
        let sample = LoadSample {
            load,
            timestamp: Instant::now(),
        };
        self.load_history.push_back(sample);
        
        // 保持历史记录大小
        if self.load_history.len() > 100 {
            self.load_history.pop_front();
        }

        // 更新统计信息
        self.update_stats(load);
        
        // 更新状态
        self.update_state();
    }

    /// 判断是否应该应用背压
    pub fn should_apply_backpressure(&mut self, current_load: f64) -> bool {
        self.update_load(current_load);
        
        match self.state {
            BackpressureState::Normal => false,
            _ => {
                self.stats.trigger_count += 1;
                self.stats.last_trigger_time = Some(chrono::Utc::now().timestamp_nanos());
                true
            }
        }
    }

    /// 计算背压延迟
    pub fn calculate_delay(&self) -> Duration {
        match &self.strategy {
            BackpressureStrategy::LinearSlowdown { factor } => {
                let delay_ms = (self.current_load - self.threshold) * factor * 1000.0;
                Duration::from_millis(delay_ms.max(0.0) as u64)
            }
            BackpressureStrategy::ExponentialBackoff { base, max_delay } => {
                let multiplier = (self.current_load / self.threshold).powf(*base);
                let delay_ms = (multiplier * 10.0) as u64;
                Duration::from_millis(delay_ms.min(max_delay.as_millis() as u64))
            }
            BackpressureStrategy::AdaptiveControl { target_latency } => {
                // 基于目标延迟的自适应控制
                let load_factor = self.current_load / self.threshold;
                let delay_ms = target_latency.as_millis() as f64 * load_factor;
                Duration::from_millis(delay_ms as u64)
            }
            BackpressureStrategy::TokenBucket { tokens_per_second, .. } => {
                // 令牌桶算法的延迟计算
                let delay_ms = 1000.0 / tokens_per_second;
                Duration::from_millis(delay_ms as u64)
            }
        }
    }

    /// 获取当前状态
    pub fn get_state(&self) -> String {
        format!("{:?}", self.state)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> BackpressureStats {
        self.stats.clone()
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = BackpressureStats::default();
        self.stats.current_state = self.get_state();
    }

    /// 获取平均负载
    pub fn get_average_load(&self) -> f64 {
        if self.load_history.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.load_history.iter().map(|s| s.load).sum();
        sum / self.load_history.len() as f64
    }

    /// 获取负载趋势
    pub fn get_load_trend(&self) -> LoadTrend {
        if self.load_history.len() < 10 {
            return LoadTrend::Stable;
        }

        let recent_samples: Vec<f64> = self.load_history
            .iter()
            .rev()
            .take(10)
            .map(|s| s.load)
            .collect();

        let first_half: f64 = recent_samples[5..].iter().sum::<f64>() / 5.0;
        let second_half: f64 = recent_samples[..5].iter().sum::<f64>() / 5.0;

        let diff = second_half - first_half;
        
        if diff > 0.1 {
            LoadTrend::Increasing
        } else if diff < -0.1 {
            LoadTrend::Decreasing
        } else {
            LoadTrend::Stable
        }
    }

    /// 预测未来负载
    pub fn predict_future_load(&self, seconds_ahead: f64) -> f64 {
        if self.load_history.len() < 5 {
            return self.current_load;
        }

        // 简单的线性预测
        let recent_samples: Vec<(f64, f64)> = self.load_history
            .iter()
            .rev()
            .take(10)
            .enumerate()
            .map(|(i, sample)| (i as f64, sample.load))
            .collect();

        // 计算线性回归
        let n = recent_samples.len() as f64;
        let sum_x: f64 = recent_samples.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = recent_samples.iter().map(|(_, y)| y).sum();
        let sum_xy: f64 = recent_samples.iter().map(|(x, y)| x * y).sum();
        let sum_x2: f64 = recent_samples.iter().map(|(x, _)| x * x).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
        let intercept = (sum_y - slope * sum_x) / n;

        // 预测未来值
        let future_x = n + seconds_ahead;
        let predicted_load = slope * future_x + intercept;

        // 限制在合理范围内
        predicted_load.max(0.0).min(2.0)
    }

    /// 更新统计信息
    fn update_stats(&mut self, load: f64) {
        // 更新平均负载
        self.stats.average_load = self.get_average_load();
        
        // 更新峰值负载
        if load > self.stats.peak_load {
            self.stats.peak_load = load;
        }
        
        // 更新当前状态
        self.stats.current_state = self.get_state();
    }

    /// 更新背压状态
    fn update_state(&mut self) {
        let new_state = if self.current_load < self.threshold {
            BackpressureState::Normal
        } else if self.current_load < self.threshold * 1.2 {
            BackpressureState::LightBackpressure
        } else if self.current_load < self.threshold * 1.5 {
            BackpressureState::ModerateBackpressure
        } else {
            BackpressureState::SevereBackpressure
        };

        if new_state != self.state {
            log::info!("背压状态变更: {:?} -> {:?}", self.state, new_state);
            self.state = new_state;
        }
    }
}

/// 负载趋势
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadTrend {
    /// 增长中
    Increasing,
    /// 下降中
    Decreasing,
    /// 稳定
    Stable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backpressure_controller_creation() {
        let controller = BackpressureController::new(0.8, 1000);
        assert_eq!(controller.threshold, 0.8);
        assert_eq!(controller.max_concurrent, 1000);
        assert_eq!(controller.current_load, 0.0);
    }

    #[test]
    fn test_should_apply_backpressure() {
        let mut controller = BackpressureController::new(0.8, 1000);
        
        // 正常负载，不应该应用背压
        assert!(!controller.should_apply_backpressure(0.5));
        
        // 高负载，应该应用背压
        assert!(controller.should_apply_backpressure(0.9));
    }

    #[test]
    fn test_load_trend_detection() {
        let mut controller = BackpressureController::new(0.8, 1000);
        
        // 添加递增的负载样本
        for i in 0..15 {
            controller.update_load(i as f64 * 0.1);
        }
        
        let trend = controller.get_load_trend();
        assert_eq!(trend, LoadTrend::Increasing);
    }

    #[test]
    fn test_delay_calculation() {
        let mut controller = BackpressureController::new(0.8, 1000);
        controller.set_strategy(BackpressureStrategy::LinearSlowdown { factor: 1.0 });
        controller.update_load(0.9);
        
        let delay = controller.calculate_delay();
        assert!(delay.as_millis() > 0);
    }

    #[test]
    fn test_stats_tracking() {
        let mut controller = BackpressureController::new(0.8, 1000);
        
        // 触发背压
        controller.should_apply_backpressure(0.9);
        
        let stats = controller.get_stats();
        assert_eq!(stats.trigger_count, 1);
        assert!(stats.last_trigger_time.is_some());
    }

    #[test]
    fn test_future_load_prediction() {
        let mut controller = BackpressureController::new(0.8, 1000);
        
        // 添加一些负载样本
        for i in 0..10 {
            controller.update_load(0.5 + i as f64 * 0.05);
        }
        
        let predicted = controller.predict_future_load(5.0);
        assert!(predicted > 0.0);
        assert!(predicted < 2.0);
    }
}
