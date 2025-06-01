//! 窗口计算模块
//!
//! 提供各种窗口类型的计算支持

use std::collections::{HashMap, VecDeque};
use serde::{Deserialize, Serialize};
use crate::error::{EnterpriseError, EnterpriseResult};

/// 窗口计算器
pub struct WindowCalculator {
    /// 窗口配置
    config: WindowConfig,
    /// 活跃窗口
    active_windows: HashMap<String, WindowInstance>,
    /// 水印管理器
    watermark_manager: WatermarkManager,
}

/// 窗口配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// 窗口类型
    pub window_type: WindowType,
    /// 窗口大小 (毫秒)
    pub size_ms: u64,
    /// 滑动间隔 (毫秒)
    pub slide_ms: Option<u64>,
    /// 会话间隔 (毫秒)
    pub session_gap_ms: Option<u64>,
    /// 水印延迟 (毫秒)
    pub watermark_delay_ms: u64,
    /// 最大延迟容忍 (毫秒)
    pub max_lateness_ms: u64,
}

/// 窗口类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WindowType {
    /// 滚动窗口 (Tumbling)
    Tumbling,
    /// 滑动窗口 (Sliding)
    Sliding,
    /// 会话窗口 (Session)
    Session,
    /// 全局窗口 (Global)
    Global,
}

/// 窗口实例
pub struct WindowInstance {
    /// 窗口ID
    pub id: String,
    /// 窗口类型
    pub window_type: WindowType,
    /// 开始时间 (毫秒)
    pub start_time: i64,
    /// 结束时间 (毫秒)
    pub end_time: i64,
    /// 窗口数据
    pub records: VecDeque<WindowRecord>,
    /// 窗口状态
    pub state: WindowState,
    /// 最后活跃时间
    pub last_activity: i64,
}

/// 窗口记录
#[derive(Debug, Clone)]
pub struct WindowRecord {
    /// 记录数据
    pub data: serde_json::Value,
    /// 事件时间
    pub event_time: i64,
    /// 处理时间
    pub processing_time: i64,
}

/// 窗口状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowState {
    /// 活跃状态
    Active,
    /// 等待触发
    Pending,
    /// 已触发
    Triggered,
    /// 已过期
    Expired,
}

/// 水印管理器
pub struct WatermarkManager {
    /// 当前水印
    current_watermark: i64,
    /// 水印延迟
    delay_ms: u64,
    /// 水印历史
    watermark_history: VecDeque<WatermarkEvent>,
}

/// 水印事件
#[derive(Debug, Clone)]
pub struct WatermarkEvent {
    /// 水印时间
    pub watermark: i64,
    /// 生成时间
    pub generated_at: i64,
}

impl WindowCalculator {
    /// 创建新的窗口计算器
    pub fn new(config: WindowConfig) -> Self {
        Self {
            watermark_manager: WatermarkManager::new(config.watermark_delay_ms),
            config,
            active_windows: HashMap::new(),
        }
    }

    /// 处理新记录
    pub fn process_record(&mut self, record: serde_json::Value, event_time: i64) -> EnterpriseResult<Vec<String>> {
        let window_record = WindowRecord {
            data: record,
            event_time,
            processing_time: chrono::Utc::now().timestamp_millis(),
        };

        // 确定记录所属的窗口
        let window_ids = self.assign_to_windows(&window_record)?;

        // 添加记录到相应窗口
        for window_id in &window_ids {
            self.add_record_to_window(window_id, window_record.clone())?;
        }

        // 更新水印
        self.watermark_manager.update_watermark(event_time);

        // 检查并触发到期的窗口
        let triggered_windows = self.check_and_trigger_windows()?;

        Ok(triggered_windows)
    }

    /// 分配记录到窗口
    fn assign_to_windows(&self, record: &WindowRecord) -> EnterpriseResult<Vec<String>> {
        let mut window_ids = Vec::new();

        match self.config.window_type {
            WindowType::Tumbling => {
                let window_id = self.calculate_tumbling_window_id(record.event_time);
                window_ids.push(window_id);
            }
            WindowType::Sliding => {
                let window_ids_for_sliding = self.calculate_sliding_window_ids(record.event_time);
                window_ids.extend(window_ids_for_sliding);
            }
            WindowType::Session => {
                let window_id = self.calculate_session_window_id(record.event_time)?;
                window_ids.push(window_id);
            }
            WindowType::Global => {
                window_ids.push("global".to_string());
            }
        }

        Ok(window_ids)
    }

    /// 计算滚动窗口ID
    fn calculate_tumbling_window_id(&self, event_time: i64) -> String {
        let window_start = (event_time / self.config.size_ms as i64) * self.config.size_ms as i64;
        let window_end = window_start + self.config.size_ms as i64;
        format!("tumbling_{}_{}", window_start, window_end)
    }

    /// 计算滑动窗口ID列表
    fn calculate_sliding_window_ids(&self, event_time: i64) -> Vec<String> {
        let mut window_ids = Vec::new();
        let slide_ms = self.config.slide_ms.unwrap_or(self.config.size_ms);

        // 计算记录可能属于的所有滑动窗口
        let num_windows = (self.config.size_ms / slide_ms) as i64;

        for i in 0..num_windows {
            let window_start = event_time - (event_time % slide_ms as i64) - (i * slide_ms as i64);
            let window_end = window_start + self.config.size_ms as i64;

            if event_time >= window_start && event_time < window_end {
                let window_id = format!("sliding_{}_{}", window_start, window_end);
                window_ids.push(window_id);
            }
        }

        window_ids
    }

    /// 计算会话窗口ID
    fn calculate_session_window_id(&self, event_time: i64) -> EnterpriseResult<String> {
        let session_gap = self.config.session_gap_ms
            .ok_or_else(|| EnterpriseError::configuration("会话窗口需要配置会话间隔"))?;

        // 查找现有的会话窗口
        for (window_id, window) in &self.active_windows {
            if window.window_type == WindowType::Session {
                let gap = event_time - window.last_activity;
                if gap <= session_gap as i64 {
                    return Ok(window_id.clone());
                }
            }
        }

        // 创建新的会话窗口
        let window_id = format!("session_{}_{}", event_time, uuid::Uuid::new_v4());
        Ok(window_id)
    }

    /// 添加记录到窗口
    fn add_record_to_window(&mut self, window_id: &str, record: WindowRecord) -> EnterpriseResult<()> {
        let window = self.active_windows.entry(window_id.to_string())
            .or_insert_with(|| self.create_window_instance(window_id, &record));

        window.records.push_back(record.clone());
        window.last_activity = record.event_time;

        Ok(())
    }

    /// 创建窗口实例
    fn create_window_instance(&self, window_id: &str, first_record: &WindowRecord) -> WindowInstance {
        let (start_time, end_time) = match self.config.window_type {
            WindowType::Tumbling => {
                let start = (first_record.event_time / self.config.size_ms as i64) * self.config.size_ms as i64;
                (start, start + self.config.size_ms as i64)
            }
            WindowType::Sliding => {
                // 从窗口ID中解析时间
                let parts: Vec<&str> = window_id.split('_').collect();
                if parts.len() >= 3 {
                    let start = parts[1].parse().unwrap_or(first_record.event_time);
                    let end = parts[2].parse().unwrap_or(start + self.config.size_ms as i64);
                    (start, end)
                } else {
                    (first_record.event_time, first_record.event_time + self.config.size_ms as i64)
                }
            }
            WindowType::Session => {
                (first_record.event_time, i64::MAX) // 会话窗口的结束时间是动态的
            }
            WindowType::Global => {
                (0, i64::MAX)
            }
        };

        WindowInstance {
            id: window_id.to_string(),
            window_type: self.config.window_type.clone(),
            start_time,
            end_time,
            records: VecDeque::new(),
            state: WindowState::Active,
            last_activity: first_record.event_time,
        }
    }

    /// 检查并触发到期的窗口
    fn check_and_trigger_windows(&mut self) -> EnterpriseResult<Vec<String>> {
        let mut triggered_windows = Vec::new();
        let current_watermark = self.watermark_manager.get_current_watermark();

        let mut windows_to_remove = Vec::new();

        for (window_id, window) in &mut self.active_windows {
            if self.should_trigger_window(window, current_watermark) {
                window.state = WindowState::Triggered;
                triggered_windows.push(window_id.clone());

                // 标记为待删除（如果不是全局窗口）
                if window.window_type != WindowType::Global {
                    windows_to_remove.push(window_id.clone());
                }
            }
        }

        // 移除已触发的窗口
        for window_id in windows_to_remove {
            self.active_windows.remove(&window_id);
        }

        Ok(triggered_windows)
    }

    /// 判断是否应该触发窗口
    fn should_trigger_window(&self, window: &WindowInstance, current_watermark: i64) -> bool {
        match window.window_type {
            WindowType::Tumbling | WindowType::Sliding => {
                current_watermark >= window.end_time
            }
            WindowType::Session => {
                let session_gap = self.config.session_gap_ms.unwrap_or(30000);
                current_watermark - window.last_activity >= session_gap as i64
            }
            WindowType::Global => false, // 全局窗口不自动触发
        }
    }

    /// 获取窗口数据
    pub fn get_window_data(&self, window_id: &str) -> Option<&VecDeque<WindowRecord>> {
        self.active_windows.get(window_id).map(|w| &w.records)
    }

    /// 手动触发窗口
    pub fn trigger_window(&mut self, window_id: &str) -> EnterpriseResult<bool> {
        if let Some(window) = self.active_windows.get_mut(window_id) {
            window.state = WindowState::Triggered;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 获取活跃窗口统计
    pub fn get_window_stats(&self) -> WindowStats {
        let mut stats = WindowStats::default();

        for window in self.active_windows.values() {
            stats.total_windows += 1;
            stats.total_records += window.records.len();

            match window.state {
                WindowState::Active => stats.active_windows += 1,
                WindowState::Pending => stats.pending_windows += 1,
                WindowState::Triggered => stats.triggered_windows += 1,
                WindowState::Expired => stats.expired_windows += 1,
            }
        }

        stats.current_watermark = self.watermark_manager.get_current_watermark();
        stats
    }
}

impl WatermarkManager {
    /// 创建新的水印管理器
    pub fn new(delay_ms: u64) -> Self {
        Self {
            current_watermark: 0,
            delay_ms,
            watermark_history: VecDeque::with_capacity(1000),
        }
    }

    /// 更新水印
    pub fn update_watermark(&mut self, event_time: i64) {
        let new_watermark = event_time - self.delay_ms as i64;

        if new_watermark > self.current_watermark {
            self.current_watermark = new_watermark;

            let event = WatermarkEvent {
                watermark: new_watermark,
                generated_at: chrono::Utc::now().timestamp_millis(),
            };

            self.watermark_history.push_back(event);

            // 保持历史记录大小
            if self.watermark_history.len() > 1000 {
                self.watermark_history.pop_front();
            }
        }
    }

    /// 获取当前水印
    pub fn get_current_watermark(&self) -> i64 {
        self.current_watermark
    }
}

/// 窗口统计信息
#[derive(Debug, Default, Clone)]
pub struct WindowStats {
    /// 总窗口数
    pub total_windows: usize,
    /// 活跃窗口数
    pub active_windows: usize,
    /// 等待窗口数
    pub pending_windows: usize,
    /// 已触发窗口数
    pub triggered_windows: usize,
    /// 已过期窗口数
    pub expired_windows: usize,
    /// 总记录数
    pub total_records: usize,
    /// 当前水印
    pub current_watermark: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tumbling_window() {
        let config = WindowConfig {
            window_type: WindowType::Tumbling,
            size_ms: 5000,
            slide_ms: None,
            session_gap_ms: None,
            watermark_delay_ms: 1000,
            max_lateness_ms: 5000,
        };

        let mut calculator = WindowCalculator::new(config);

        let record = json!({"value": 100});
        let result = calculator.process_record(record, 10000).unwrap();

        assert!(!result.is_empty());
    }

    #[test]
    fn test_sliding_window() {
        let config = WindowConfig {
            window_type: WindowType::Sliding,
            size_ms: 10000,
            slide_ms: Some(5000),
            session_gap_ms: None,
            watermark_delay_ms: 1000,
            max_lateness_ms: 5000,
        };

        let mut calculator = WindowCalculator::new(config);

        let record = json!({"value": 200});
        let result = calculator.process_record(record, 15000).unwrap();

        // 滑动窗口可能产生多个窗口
        assert!(!result.is_empty());
    }

    #[test]
    fn test_watermark_management() {
        let mut watermark_manager = WatermarkManager::new(1000);

        watermark_manager.update_watermark(5000);
        assert_eq!(watermark_manager.get_current_watermark(), 4000);

        watermark_manager.update_watermark(3000); // 不应该更新，因为比当前水印小
        assert_eq!(watermark_manager.get_current_watermark(), 4000);

        watermark_manager.update_watermark(7000);
        assert_eq!(watermark_manager.get_current_watermark(), 6000);
    }
}
