//! 背压控制系统
//! 
//! 提供背压控制机制以防止系统过载，确保处理稳定性

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use log::{debug, info, warn};

/// 背压信用模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditMode {
    /// 固定信用额度
    Fixed,
    /// 自适应信用额度（根据队列长度和处理时间动态调整）
    Adaptive,
    /// 基于时间的信用额度（根据处理速率调整）
    TimeBased,
}

/// 背压控制器配置
#[derive(Debug, Clone)]
pub struct CreditConfig {
    /// 基础信用额度（每次授予的记录数）
    pub base_credits: usize,
    
    /// 最小信用额度
    pub min_credits: usize,
    
    /// 最大信用额度
    pub max_credits: usize,
    
    /// 信用额度恢复间隔（毫秒）
    pub credit_interval_ms: u64,
    
    /// 背压模式
    pub mode: CreditMode,
    
    /// 目标队列大小（用于自适应模式）
    pub target_queue_size: usize,
    
    /// 目标处理时间（毫秒，用于自适应模式）
    pub target_processing_time_ms: u64,
}

impl Default for CreditConfig {
    fn default() -> Self {
        Self {
            base_credits: 1000,
            min_credits: 100,
            max_credits: 10000,
            credit_interval_ms: 50,
            mode: CreditMode::Adaptive,
            target_queue_size: 5000,
            target_processing_time_ms: 100,
        }
    }
}

/// 背压控制器指标
#[derive(Debug, Clone)]
pub struct BackpressureMetrics {
    /// 授予的信用总额
    pub total_credits_granted: usize,
    
    /// 信用额度请求总数
    pub total_requests: usize,
    
    /// 被拒绝的请求总数（由于背压）
    pub rejected_requests: usize,
    
    /// 平均信用额度大小
    pub average_credit_size: usize,
    
    /// 当前背压因子 (0.0-1.0)，值越高表示背压越大
    pub current_pressure: f64,
    
    /// 上次度量时间
    pub last_measured: Instant,
}

impl BackpressureMetrics {
    /// 创建新的背压度量
    pub fn new() -> Self {
        Self {
            total_credits_granted: 0,
            total_requests: 0,
            rejected_requests: 0,
            average_credit_size: 0,
            current_pressure: 0.0,
            last_measured: Instant::now(),
        }
    }
    
    /// 重置度量
    pub fn reset(&mut self) {
        self.total_credits_granted = 0;
        self.total_requests = 0;
        self.rejected_requests = 0;
        self.average_credit_size = 0;
        self.current_pressure = 0.0;
        self.last_measured = Instant::now();
    }
}

/// 背压控制器
pub struct BackpressureController {
    /// 控制器配置
    config: CreditConfig,
    
    /// 可用信用额度
    available_credits: usize,
    
    /// 上次信用恢复时间
    last_credit_refresh: Instant,
    
    /// 度量数据
    metrics: BackpressureMetrics,
    
    /// 当前队列大小
    current_queue_size: usize,
    
    /// 等待中的请求计数
    waiting_requests: usize,
}

impl BackpressureController {
    /// 创建新的背压控制器
    pub fn new(config: CreditConfig) -> Self {
        Self {
            available_credits: config.base_credits,
            config,
            last_credit_refresh: Instant::now(),
            metrics: BackpressureMetrics::new(),
            current_queue_size: 0,
            waiting_requests: 0,
        }
    }
    
    /// 使用默认配置创建背压控制器
    pub fn default() -> Self {
        Self::new(CreditConfig::default())
    }
    
    /// 请求信用额度（返回批大小）
    pub fn request_credits(&mut self, requested: usize) -> Option<usize> {
        self.metrics.total_requests += 1;
        
        // 检查是否需要刷新信用额度
        self.refresh_credits_if_needed();
        
        // 增加等待请求计数
        self.waiting_requests += 1;
        
        // 如果没有足够的信用额度，返回 None
        if self.available_credits == 0 {
            self.metrics.rejected_requests += 1;
            debug!("背压控制：拒绝请求，没有可用信用额度");
            return None;
        }
        
        // 计算可以授予的信用额度
        let granted = requested.min(self.available_credits);
        
        // 更新可用信用额度
        self.available_credits -= granted;
        self.metrics.total_credits_granted += granted;
        
        // 更新平均信用额度
        if self.metrics.total_requests > 0 {
            self.metrics.average_credit_size = self.metrics.total_credits_granted / self.metrics.total_requests;
        }
        
        // 减少等待请求计数
        self.waiting_requests -= 1;
        
        Some(granted)
    }
    
    /// 更新队列大小和处理时间
    pub fn update_stats(&mut self, queue_size: usize, processing_time: Duration) {
        self.current_queue_size = queue_size;
        
        // 根据模式调整背压
        match self.config.mode {
            CreditMode::Fixed => {
                // 固定模式下不调整
            },
            CreditMode::Adaptive => {
                // 自适应模式：根据队列大小和处理时间调整
                self.adjust_adaptive_credits(queue_size, processing_time);
            },
            CreditMode::TimeBased => {
                // 基于时间模式：根据处理时间调整
                self.adjust_time_based_credits(processing_time);
            },
        }
    }
    
    /// 根据队列大小和处理时间自适应调整信用额度
    fn adjust_adaptive_credits(&mut self, queue_size: usize, processing_time: Duration) {
        // 计算队列压力因子 (0.0-1.0+)
        let queue_pressure = queue_size as f64 / self.config.target_queue_size as f64;
        
        // 计算时间压力因子 (0.0-1.0+)
        let time_pressure = processing_time.as_millis() as f64 / self.config.target_processing_time_ms as f64;
        
        // 结合两个压力因子
        let pressure_factor = (queue_pressure + time_pressure) / 2.0;
        
        // 更新度量
        self.metrics.current_pressure = pressure_factor;
        
        // 根据压力因子调整基础信用额度
        if pressure_factor > 1.2 {
            // 高压力：大幅减少信用额度
            let new_base = (self.config.base_credits as f64 / pressure_factor).round() as usize;
            self.config.base_credits = new_base.max(self.config.min_credits);
            info!("背压增加：队列大小={}, 处理时间={}ms, 调整基础信用额度为 {}", 
                queue_size, processing_time.as_millis(), self.config.base_credits);
        } else if pressure_factor < 0.5 {
            // 低压力：适度增加信用额度
            let new_base = (self.config.base_credits as f64 * 1.1).round() as usize;
            self.config.base_credits = new_base.min(self.config.max_credits);
            info!("背压减轻：队列大小={}, 处理时间={}ms, 调整基础信用额度为 {}", 
                queue_size, processing_time.as_millis(), self.config.base_credits);
        }
    }
    
    /// 根据处理时间调整信用额度
    fn adjust_time_based_credits(&mut self, processing_time: Duration) {
        // 计算时间压力因子
        let time_pressure = processing_time.as_millis() as f64 / self.config.target_processing_time_ms as f64;
        
        // 更新度量
        self.metrics.current_pressure = time_pressure;
        
        // 根据时间压力调整
        if time_pressure > 1.2 {
            // 处理时间过长：减少信用额度
            let new_base = (self.config.base_credits as f64 / time_pressure).round() as usize;
            self.config.base_credits = new_base.max(self.config.min_credits);
            debug!("基于时间的背压增加：处理时间={}ms, 调整基础信用额度为 {}", 
                processing_time.as_millis(), self.config.base_credits);
        } else if time_pressure < 0.5 {
            // 处理时间短：增加信用额度
            let new_base = (self.config.base_credits as f64 * 1.1).round() as usize;
            self.config.base_credits = new_base.min(self.config.max_credits);
            debug!("基于时间的背压减轻：处理时间={}ms, 调整基础信用额度为 {}", 
                processing_time.as_millis(), self.config.base_credits);
        }
    }
    
    /// 检查并刷新信用额度
    fn refresh_credits_if_needed(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_credit_refresh);
        
        // 如果自上次刷新以来已经过去了足够的时间，重置信用额度
        if elapsed.as_millis() >= self.config.credit_interval_ms as u128 {
            let refresh_amount = self.config.base_credits;
            self.available_credits = self.available_credits.saturating_add(refresh_amount)
                .min(self.config.max_credits);
                
            debug!("刷新信用额度：添加 {} 个信用额度，当前可用 {}", 
                refresh_amount, self.available_credits);
                
            self.last_credit_refresh = now;
        }
    }
    
    /// 获取当前可用信用额度
    pub fn available_credits(&self) -> usize {
        self.available_credits
    }
    
    /// 获取度量
    pub fn metrics(&self) -> &BackpressureMetrics {
        &self.metrics
    }
    
    /// 获取当前背压（0.0-1.0，值越高表示背压越大）
    pub fn current_pressure(&self) -> f64 {
        self.metrics.current_pressure
    }
    
    /// 获取等待请求数
    pub fn waiting_requests(&self) -> usize {
        self.waiting_requests
    }
    
    /// 重置控制器
    pub fn reset(&mut self) {
        self.available_credits = self.config.base_credits;
        self.last_credit_refresh = Instant::now();
        self.metrics.reset();
        self.current_queue_size = 0;
        self.waiting_requests = 0;
    }
}

/// 共享背压控制器，可在多个 Actor 之间共享
pub struct SharedBackpressureController {
    /// 内部控制器
    inner: Arc<Mutex<BackpressureController>>,
}

impl SharedBackpressureController {
    /// 创建新的共享背压控制器
    pub fn new(config: CreditConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BackpressureController::new(config))),
        }
    }
    
    /// 使用默认配置创建共享背压控制器
    pub fn default() -> Self {
        Self::new(CreditConfig::default())
    }
    
    /// 请求信用额度
    pub fn request_credits(&self, requested: usize) -> Option<usize> {
        let mut controller = self.inner.lock().unwrap();
        controller.request_credits(requested)
    }
    
    /// 更新队列统计信息
    pub fn update_stats(&self, queue_size: usize, processing_time: Duration) {
        let mut controller = self.inner.lock().unwrap();
        controller.update_stats(queue_size, processing_time);
    }
    
    /// 获取当前背压
    pub fn current_pressure(&self) -> f64 {
        let controller = self.inner.lock().unwrap();
        controller.current_pressure()
    }
    
    /// 重置控制器
    pub fn reset(&self) {
        let mut controller = self.inner.lock().unwrap();
        controller.reset();
    }
}

/// 创建共享背压控制器
pub fn create_shared_controller(config: CreditConfig) -> SharedBackpressureController {
    SharedBackpressureController::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_backpressure_controller_credits() {
        let mut controller = BackpressureController::default();
        
        // 尝试请求小于可用信用额度的数量
        let granted = controller.request_credits(500);
        assert_eq!(granted, Some(500));
        
        // 确认信用额度减少了
        assert_eq!(controller.available_credits(), 500);
        
        // 尝试请求大于可用信用额度的数量
        let granted = controller.request_credits(1000);
        assert_eq!(granted, Some(500));
        
        // 确认所有信用额度都被使用
        assert_eq!(controller.available_credits(), 0);
        
        // 尝试请求更多信用额度
        let granted = controller.request_credits(100);
        assert_eq!(granted, None);
    }
    
    #[test]
    fn test_backpressure_controller_refresh() {
        let config = CreditConfig {
            base_credits: 1000,
            credit_interval_ms: 10, // 设置较短的刷新间隔以便测试
            ..Default::default()
        };
        
        let mut controller = BackpressureController::new(config);
        
        // 使用所有信用额度
        controller.request_credits(1000);
        assert_eq!(controller.available_credits(), 0);
        
        // 等待刷新间隔
        std::thread::sleep(Duration::from_millis(20));
        
        // 再次请求信用额度，此时应该已经刷新
        let granted = controller.request_credits(500);
        assert_eq!(granted, Some(500));
    }
    
    #[test]
    fn test_backpressure_adaptive_mode() {
        let config = CreditConfig {
            base_credits: 1000,
            mode: CreditMode::Adaptive,
            target_queue_size: 1000,
            target_processing_time_ms: 50,
            ..Default::default()
        };
        
        let mut controller = BackpressureController::new(config);
        
        // 模拟高负载场景
        controller.update_stats(2000, Duration::from_millis(100));
        
        // 验证背压增加（基础信用额度减少）
        assert!(controller.config.base_credits < 1000);
        
        // 模拟低负载场景
        controller.reset();
        controller.update_stats(200, Duration::from_millis(10));
        
        // 验证背压减少（基础信用额度增加）
        assert!(controller.config.base_credits > 1000);
    }
    
    #[test]
    fn test_shared_backpressure_controller() {
        let shared = SharedBackpressureController::default();
        
        // 从共享控制器请求信用额度
        let granted = shared.request_credits(500);
        assert_eq!(granted, Some(500));
        
        // 更新统计信息
        shared.update_stats(2000, Duration::from_millis(100));
        
        // 获取当前背压
        let pressure = shared.current_pressure();
        assert!(pressure > 0.0);
    }
} 