//! 流式聚合引擎
//!
//! 提供实时窗口计算和聚合分析能力

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::error::{EnterpriseError, EnterpriseResult};

/// 流式聚合器
pub struct StreamAggregator {
    /// 聚合配置
    config: AggregationConfig,
    /// 窗口管理器
    window_manager: WindowManager,
    /// 聚合状态
    aggregation_state: HashMap<String, AggregationState>,
}

/// 聚合配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationConfig {
    /// 窗口大小 (毫秒)
    pub window_size_ms: u64,
    /// 滑动间隔 (毫秒)
    pub slide_interval_ms: u64,
    /// 聚合函数
    pub aggregation_functions: Vec<AggregationFunction>,
    /// 分组字段
    pub group_by_fields: Vec<String>,
}

/// 聚合函数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationFunction {
    /// 计数
    Count,
    /// 求和
    Sum { field: String },
    /// 平均值
    Average { field: String },
    /// 最小值
    Min { field: String },
    /// 最大值
    Max { field: String },
    /// 自定义聚合
    Custom { name: String, expression: String },
}

/// 窗口管理器
pub struct WindowManager {
    /// 活跃窗口
    active_windows: HashMap<String, Window>,
    /// 窗口配置
    config: WindowConfig,
}

/// 窗口配置
#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// 窗口类型
    pub window_type: WindowType,
    /// 窗口大小
    pub size_ms: u64,
    /// 滑动间隔
    pub slide_ms: u64,
}

/// 窗口类型
#[derive(Debug, Clone)]
pub enum WindowType {
    /// 滚动窗口
    Tumbling,
    /// 滑动窗口
    Sliding,
    /// 会话窗口
    Session { gap_ms: u64 },
}

/// 窗口
pub struct Window {
    /// 窗口ID
    pub id: String,
    /// 开始时间
    pub start_time: i64,
    /// 结束时间
    pub end_time: i64,
    /// 窗口数据
    pub data: Vec<serde_json::Value>,
    /// 聚合结果
    pub aggregation_results: HashMap<String, f64>,
}

/// 聚合状态
#[derive(Debug, Clone)]
pub struct AggregationState {
    /// 当前值
    pub current_value: f64,
    /// 记录数
    pub count: u64,
    /// 最后更新时间
    pub last_updated: i64,
}

impl StreamAggregator {
    /// 创建新的流式聚合器
    pub fn new(config: AggregationConfig) -> Self {
        let window_config = WindowConfig {
            window_type: WindowType::Sliding,
            size_ms: config.window_size_ms,
            slide_ms: config.slide_interval_ms,
        };

        Self {
            config,
            window_manager: WindowManager::new(window_config),
            aggregation_state: HashMap::new(),
        }
    }

    /// 处理新记录
    pub fn process_record(&mut self, record: &serde_json::Value) -> EnterpriseResult<Option<HashMap<String, f64>>> {
        // 确定记录所属的窗口
        let timestamp = self.extract_timestamp(record)?;
        let window_id = self.window_manager.get_window_id(timestamp);

        // 添加记录到窗口
        self.window_manager.add_record_to_window(&window_id, record.clone())?;

        // 检查是否有窗口需要触发聚合
        if self.window_manager.should_trigger_aggregation(&window_id) {
            let window = self.window_manager.get_window(&window_id)
                .ok_or_else(|| EnterpriseError::stream_processing("窗口不存在", "WINDOW_NOT_FOUND"))?;
            
            return Ok(Some(self.compute_aggregations(window)?));
        }

        Ok(None)
    }

    /// 计算聚合结果
    fn compute_aggregations(&self, window: &Window) -> EnterpriseResult<HashMap<String, f64>> {
        let mut results = HashMap::new();

        for function in &self.config.aggregation_functions {
            let result = match function {
                AggregationFunction::Count => window.data.len() as f64,
                AggregationFunction::Sum { field } => {
                    self.compute_sum(&window.data, field)?
                }
                AggregationFunction::Average { field } => {
                    self.compute_average(&window.data, field)?
                }
                AggregationFunction::Min { field } => {
                    self.compute_min(&window.data, field)?
                }
                AggregationFunction::Max { field } => {
                    self.compute_max(&window.data, field)?
                }
                AggregationFunction::Custom { name, expression } => {
                    self.compute_custom(&window.data, name, expression)?
                }
            };

            let function_name = self.get_function_name(function);
            results.insert(function_name, result);
        }

        Ok(results)
    }

    /// 提取时间戳
    fn extract_timestamp(&self, record: &serde_json::Value) -> EnterpriseResult<i64> {
        record.get("timestamp")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| EnterpriseError::validation("记录缺少时间戳字段", Some("timestamp".to_string())))
    }

    /// 计算求和
    fn compute_sum(&self, data: &[serde_json::Value], field: &str) -> EnterpriseResult<f64> {
        let sum = data.iter()
            .filter_map(|record| record.get(field)?.as_f64())
            .sum();
        Ok(sum)
    }

    /// 计算平均值
    fn compute_average(&self, data: &[serde_json::Value], field: &str) -> EnterpriseResult<f64> {
        let values: Vec<f64> = data.iter()
            .filter_map(|record| record.get(field)?.as_f64())
            .collect();
        
        if values.is_empty() {
            return Ok(0.0);
        }

        Ok(values.iter().sum::<f64>() / values.len() as f64)
    }

    /// 计算最小值
    fn compute_min(&self, data: &[serde_json::Value], field: &str) -> EnterpriseResult<f64> {
        data.iter()
            .filter_map(|record| record.get(field)?.as_f64())
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .ok_or_else(|| EnterpriseError::validation("无法计算最小值", Some(field.to_string())))
    }

    /// 计算最大值
    fn compute_max(&self, data: &[serde_json::Value], field: &str) -> EnterpriseResult<f64> {
        data.iter()
            .filter_map(|record| record.get(field)?.as_f64())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .ok_or_else(|| EnterpriseError::validation("无法计算最大值", Some(field.to_string())))
    }

    /// 计算自定义聚合
    fn compute_custom(&self, _data: &[serde_json::Value], _name: &str, _expression: &str) -> EnterpriseResult<f64> {
        // TODO: 实现自定义表达式计算
        Ok(0.0)
    }

    /// 获取函数名称
    fn get_function_name(&self, function: &AggregationFunction) -> String {
        match function {
            AggregationFunction::Count => "count".to_string(),
            AggregationFunction::Sum { field } => format!("sum_{}", field),
            AggregationFunction::Average { field } => format!("avg_{}", field),
            AggregationFunction::Min { field } => format!("min_{}", field),
            AggregationFunction::Max { field } => format!("max_{}", field),
            AggregationFunction::Custom { name, .. } => name.clone(),
        }
    }
}

impl WindowManager {
    /// 创建新的窗口管理器
    pub fn new(config: WindowConfig) -> Self {
        Self {
            active_windows: HashMap::new(),
            config,
        }
    }

    /// 获取窗口ID
    pub fn get_window_id(&self, timestamp: i64) -> String {
        match self.config.window_type {
            WindowType::Tumbling => {
                let window_start = (timestamp / self.config.size_ms as i64) * self.config.size_ms as i64;
                format!("tumbling_{}_{}", window_start, window_start + self.config.size_ms as i64)
            }
            WindowType::Sliding => {
                let window_start = timestamp - (timestamp % self.config.slide_ms as i64);
                format!("sliding_{}_{}", window_start, window_start + self.config.size_ms as i64)
            }
            WindowType::Session { gap_ms } => {
                format!("session_{}_{}", timestamp, gap_ms)
            }
        }
    }

    /// 添加记录到窗口
    pub fn add_record_to_window(&mut self, window_id: &str, record: serde_json::Value) -> EnterpriseResult<()> {
        let window = self.active_windows.entry(window_id.to_string())
            .or_insert_with(|| self.create_window(window_id));
        
        window.data.push(record);
        Ok(())
    }

    /// 检查是否应该触发聚合
    pub fn should_trigger_aggregation(&self, _window_id: &str) -> bool {
        // TODO: 实现触发逻辑
        true
    }

    /// 获取窗口
    pub fn get_window(&self, window_id: &str) -> Option<&Window> {
        self.active_windows.get(window_id)
    }

    /// 创建窗口
    fn create_window(&self, window_id: &str) -> Window {
        let now = chrono::Utc::now().timestamp_nanos();
        Window {
            id: window_id.to_string(),
            start_time: now,
            end_time: now + self.config.size_ms as i64 * 1_000_000,
            data: Vec::new(),
            aggregation_results: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_stream_aggregator_creation() {
        let config = AggregationConfig {
            window_size_ms: 5000,
            slide_interval_ms: 1000,
            aggregation_functions: vec![AggregationFunction::Count],
            group_by_fields: vec![],
        };

        let aggregator = StreamAggregator::new(config);
        assert_eq!(aggregator.config.window_size_ms, 5000);
    }

    #[test]
    fn test_aggregation_functions() {
        let config = AggregationConfig {
            window_size_ms: 5000,
            slide_interval_ms: 1000,
            aggregation_functions: vec![
                AggregationFunction::Count,
                AggregationFunction::Sum { field: "value".to_string() },
                AggregationFunction::Average { field: "value".to_string() },
            ],
            group_by_fields: vec![],
        };

        let aggregator = StreamAggregator::new(config);
        
        let data = vec![
            json!({"value": 10.0}),
            json!({"value": 20.0}),
            json!({"value": 30.0}),
        ];

        let sum = aggregator.compute_sum(&data, "value").unwrap();
        assert_eq!(sum, 60.0);

        let avg = aggregator.compute_average(&data, "value").unwrap();
        assert_eq!(avg, 20.0);
    }
}
