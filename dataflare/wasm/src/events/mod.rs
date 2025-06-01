//! DataFlare Enterprise 事件处理模块
//!
//! 提供企业级事件驱动处理系统

pub mod router;
pub mod store;
pub mod replay;
pub mod processor;

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::error::{EnterpriseError, EnterpriseResult};

pub use router::EventRouter;
pub use store::EventStore;
pub use replay::EventReplayer;
pub use processor::EnterpriseEventProcessor;

/// 企业级事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnterpriseEvent {
    /// 事件ID
    pub id: String,
    /// 事件类型
    pub event_type: String,
    /// 事件数据
    pub data: serde_json::Value,
    /// 事件元数据
    pub metadata: HashMap<String, String>,
    /// 时间戳 (纳秒)
    pub timestamp: i64,
    /// 来源
    pub source: String,
    /// 版本
    pub version: String,
    /// 追踪ID
    pub trace_id: Option<String>,
    /// 相关事件ID
    pub correlation_id: Option<String>,
    /// 事件优先级
    pub priority: EventPriority,
    /// 事件状态
    pub status: EventStatus,
}

/// 事件优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    /// 低优先级
    Low,
    /// 普通优先级
    Normal,
    /// 高优先级
    High,
    /// 紧急优先级
    Critical,
}

/// 事件状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventStatus {
    /// 待处理
    Pending,
    /// 处理中
    Processing,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已跳过
    Skipped,
}

/// 事件处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventProcessingResult {
    /// 事件ID
    pub event_id: String,
    /// 处理状态
    pub status: EventStatus,
    /// 处理时间 (毫秒)
    pub processing_time_ms: u64,
    /// 输出事件
    pub output_events: Vec<EnterpriseEvent>,
    /// 错误信息
    pub error: Option<String>,
    /// 处理器ID
    pub processor_id: String,
    /// 处理时间戳
    pub processed_at: i64,
}

/// 事件处理器接口
#[async_trait::async_trait]
pub trait EventProcessor: Send + Sync {
    /// 处理事件
    async fn process_event(&mut self, event: EnterpriseEvent) -> EnterpriseResult<EventProcessingResult>;

    /// 批量处理事件
    async fn process_events(&mut self, events: Vec<EnterpriseEvent>) -> EnterpriseResult<Vec<EventProcessingResult>>;

    /// 获取处理器ID
    fn get_processor_id(&self) -> String;

    /// 获取支持的事件类型
    fn get_supported_event_types(&self) -> Vec<String>;
}

/// 事件路由规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRoutingRule {
    /// 规则ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 事件类型匹配
    pub event_type_pattern: String,
    /// 来源匹配
    pub source_pattern: Option<String>,
    /// 元数据匹配条件
    pub metadata_conditions: HashMap<String, String>,
    /// 目标处理器
    pub target_processors: Vec<String>,
    /// 规则优先级
    pub priority: i32,
    /// 是否启用
    pub enabled: bool,
}

/// 事件存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStoreConfig {
    /// 存储类型
    pub store_type: EventStoreType,
    /// 连接字符串
    pub connection_string: String,
    /// 批量写入大小
    pub batch_size: usize,
    /// 写入超时 (毫秒)
    pub write_timeout_ms: u64,
    /// 保留期 (天)
    pub retention_days: u32,
    /// 是否启用压缩
    pub enable_compression: bool,
}

/// 事件存储类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventStoreType {
    /// 内存存储
    Memory,
    /// 文件存储
    File { path: String },
    /// 数据库存储
    Database { db_type: String },
    /// 对象存储
    ObjectStorage { bucket: String },
}

/// 事件重放配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventReplayConfig {
    /// 开始时间
    pub start_time: i64,
    /// 结束时间
    pub end_time: Option<i64>,
    /// 事件类型过滤
    pub event_type_filter: Option<Vec<String>>,
    /// 来源过滤
    pub source_filter: Option<Vec<String>>,
    /// 重放速度倍数
    pub speed_multiplier: f64,
    /// 是否保持原始时间间隔
    pub preserve_timing: bool,
}

impl EnterpriseEvent {
    /// 创建新的企业级事件
    pub fn new(
        event_type: impl Into<String>,
        data: serde_json::Value,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: event_type.into(),
            data,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp_nanos(),
            source: source.into(),
            version: "1.0".to_string(),
            trace_id: None,
            correlation_id: None,
            priority: EventPriority::Normal,
            status: EventStatus::Pending,
        }
    }

    /// 设置追踪ID
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// 设置相关事件ID
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 设置版本
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// 更新状态
    pub fn update_status(&mut self, status: EventStatus) {
        self.status = status;
    }

    /// 获取事件大小 (字节)
    pub fn size(&self) -> usize {
        serde_json::to_vec(self).map(|v| v.len()).unwrap_or(0)
    }

    /// 验证事件
    pub fn validate(&self) -> EnterpriseResult<()> {
        if self.id.is_empty() {
            return Err(EnterpriseError::validation("事件ID不能为空", Some("id".to_string())));
        }

        if self.event_type.is_empty() {
            return Err(EnterpriseError::validation("事件类型不能为空", Some("event_type".to_string())));
        }

        if self.source.is_empty() {
            return Err(EnterpriseError::validation("事件来源不能为空", Some("source".to_string())));
        }

        if self.timestamp <= 0 {
            return Err(EnterpriseError::validation("事件时间戳无效", Some("timestamp".to_string())));
        }

        Ok(())
    }

    /// 克隆事件并生成新ID
    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = uuid::Uuid::new_v4().to_string();
        cloned.timestamp = chrono::Utc::now().timestamp_nanos();
        cloned.status = EventStatus::Pending;
        cloned
    }
}

impl EventProcessingResult {
    /// 创建成功结果
    pub fn success(
        event_id: impl Into<String>,
        processing_time_ms: u64,
        processor_id: impl Into<String>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            status: EventStatus::Completed,
            processing_time_ms,
            output_events: Vec::new(),
            error: None,
            processor_id: processor_id.into(),
            processed_at: chrono::Utc::now().timestamp_nanos(),
        }
    }

    /// 创建失败结果
    pub fn failure(
        event_id: impl Into<String>,
        processing_time_ms: u64,
        processor_id: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            status: EventStatus::Failed,
            processing_time_ms,
            output_events: Vec::new(),
            error: Some(error.into()),
            processor_id: processor_id.into(),
            processed_at: chrono::Utc::now().timestamp_nanos(),
        }
    }

    /// 添加输出事件
    pub fn with_output_events(mut self, events: Vec<EnterpriseEvent>) -> Self {
        self.output_events = events;
        self
    }
}

impl EventRoutingRule {
    /// 创建新的路由规则
    pub fn new(
        name: impl Into<String>,
        event_type_pattern: impl Into<String>,
        target_processors: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            event_type_pattern: event_type_pattern.into(),
            source_pattern: None,
            metadata_conditions: HashMap::new(),
            target_processors,
            priority: 0,
            enabled: true,
        }
    }

    /// 检查事件是否匹配规则
    pub fn matches(&self, event: &EnterpriseEvent) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查事件类型匹配
        if !self.matches_pattern(&event.event_type, &self.event_type_pattern) {
            return false;
        }

        // 检查来源匹配
        if let Some(source_pattern) = &self.source_pattern {
            if !self.matches_pattern(&event.source, source_pattern) {
                return false;
            }
        }

        // 检查元数据条件
        for (key, expected_value) in &self.metadata_conditions {
            if let Some(actual_value) = event.metadata.get(key) {
                if !self.matches_pattern(actual_value, expected_value) {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    /// 模式匹配 (支持简单的通配符)
    fn matches_pattern(&self, value: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') {
            // 简单的通配符匹配
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return value.starts_with(prefix) && value.ends_with(suffix);
            }
        }

        value == pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_enterprise_event_creation() {
        let event = EnterpriseEvent::new(
            "user.created",
            json!({"user_id": 123, "name": "Alice"}),
            "user-service"
        )
        .with_trace_id("trace-123")
        .with_priority(EventPriority::High)
        .with_metadata("region", "us-east-1");

        assert_eq!(event.event_type, "user.created");
        assert_eq!(event.source, "user-service");
        assert_eq!(event.priority, EventPriority::High);
        assert_eq!(event.trace_id, Some("trace-123".to_string()));
        assert_eq!(event.metadata.get("region"), Some(&"us-east-1".to_string()));
    }

    #[test]
    fn test_event_validation() {
        let mut event = EnterpriseEvent::new("test.event", json!({}), "test-source");
        assert!(event.validate().is_ok());

        event.id = String::new();
        assert!(event.validate().is_err());
    }

    #[test]
    fn test_routing_rule_matching() {
        let rule = EventRoutingRule::new(
            "user events",
            "user.*",
            vec!["user-processor".to_string()]
        );

        let event1 = EnterpriseEvent::new("user.created", json!({}), "test");
        let event2 = EnterpriseEvent::new("order.created", json!({}), "test");

        assert!(rule.matches(&event1));
        assert!(!rule.matches(&event2));
    }

    #[test]
    fn test_event_processing_result() {
        let result = EventProcessingResult::success("event-123", 50, "processor-1")
            .with_output_events(vec![
                EnterpriseEvent::new("output.event", json!({}), "processor-1")
            ]);

        assert_eq!(result.status, EventStatus::Completed);
        assert_eq!(result.processing_time_ms, 50);
        assert_eq!(result.output_events.len(), 1);
    }
}
