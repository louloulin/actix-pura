//! DataFlare Enterprise 结果类型模块
//!
//! 提供企业级操作结果和状态管理

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::{EnterpriseError, EnterpriseResult};

/// 处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult<T> {
    /// 处理是否成功
    pub success: bool,
    /// 处理结果数据
    pub data: Option<T>,
    /// 错误信息
    pub error: Option<EnterpriseError>,
    /// 处理时间 (毫秒)
    pub processing_time_ms: u64,
    /// 处理的记录数
    pub records_processed: usize,
    /// 处理开始时间
    pub start_time: i64,
    /// 处理结束时间
    pub end_time: i64,
    /// 额外的元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

impl<T> ProcessingResult<T> {
    /// 创建成功结果
    pub fn success(data: T, processing_time_ms: u64, records_processed: usize) -> Self {
        let now = chrono::Utc::now().timestamp_nanos();
        Self {
            success: true,
            data: Some(data),
            error: None,
            processing_time_ms,
            records_processed,
            start_time: now - (processing_time_ms as i64 * 1_000_000),
            end_time: now,
            metadata: HashMap::new(),
        }
    }

    /// 创建失败结果
    pub fn failure(error: EnterpriseError, processing_time_ms: u64) -> Self {
        let now = chrono::Utc::now().timestamp_nanos();
        Self {
            success: false,
            data: None,
            error: Some(error),
            processing_time_ms,
            records_processed: 0,
            start_time: now - (processing_time_ms as i64 * 1_000_000),
            end_time: now,
            metadata: HashMap::new(),
        }
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// 获取吞吐量 (记录/秒)
    pub fn throughput(&self) -> f64 {
        if self.processing_time_ms == 0 {
            return 0.0;
        }
        (self.records_processed as f64) / (self.processing_time_ms as f64 / 1000.0)
    }

    /// 转换为 Result 类型
    pub fn into_result(self) -> EnterpriseResult<T> {
        match self.success {
            true => Ok(self.data.unwrap()),
            false => Err(self.error.unwrap()),
        }
    }
}

impl<T> From<EnterpriseResult<T>> for ProcessingResult<T> {
    fn from(result: EnterpriseResult<T>) -> Self {
        match result {
            Ok(data) => ProcessingResult::success(data, 0, 1),
            Err(error) => ProcessingResult::failure(error, 0),
        }
    }
}

/// 任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// 任务ID
    pub task_id: String,
    /// 任务状态
    pub status: TaskStatus,
    /// 任务结果数据
    pub result_data: Option<serde_json::Value>,
    /// 错误信息
    pub error: Option<EnterpriseError>,
    /// 任务开始时间
    pub start_time: i64,
    /// 任务结束时间
    pub end_time: Option<i64>,
    /// 执行节点ID
    pub node_id: Option<String>,
    /// 任务优先级
    pub priority: TaskPriority,
    /// 资源使用情况
    pub resource_usage: ResourceUsage,
    /// 任务元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    /// 等待中
    Pending,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
    /// 超时
    Timeout,
}

/// 任务优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    /// 低优先级
    Low,
    /// 普通优先级
    Normal,
    /// 高优先级
    High,
    /// 紧急优先级
    Urgent,
}

/// 资源使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU 使用率 (0.0 - 1.0)
    pub cpu_usage: f64,
    /// 内存使用量 (字节)
    pub memory_usage: u64,
    /// 网络输入 (字节)
    pub network_in: u64,
    /// 网络输出 (字节)
    pub network_out: u64,
    /// 磁盘读取 (字节)
    pub disk_read: u64,
    /// 磁盘写入 (字节)
    pub disk_write: u64,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0,
            network_in: 0,
            network_out: 0,
            disk_read: 0,
            disk_write: 0,
        }
    }
}

impl TaskResult {
    /// 创建新的任务结果
    pub fn new(task_id: impl Into<String>, priority: TaskPriority) -> Self {
        Self {
            task_id: task_id.into(),
            status: TaskStatus::Pending,
            result_data: None,
            error: None,
            start_time: chrono::Utc::now().timestamp_nanos(),
            end_time: None,
            node_id: None,
            priority,
            resource_usage: ResourceUsage::default(),
            metadata: HashMap::new(),
        }
    }

    /// 标记任务开始
    pub fn start(&mut self, node_id: Option<String>) {
        self.status = TaskStatus::Running;
        self.start_time = chrono::Utc::now().timestamp_nanos();
        self.node_id = node_id;
    }

    /// 标记任务完成
    pub fn complete(&mut self, result_data: Option<serde_json::Value>) {
        self.status = TaskStatus::Completed;
        self.end_time = Some(chrono::Utc::now().timestamp_nanos());
        self.result_data = result_data;
    }

    /// 标记任务失败
    pub fn fail(&mut self, error: EnterpriseError) {
        self.status = TaskStatus::Failed;
        self.end_time = Some(chrono::Utc::now().timestamp_nanos());
        self.error = Some(error);
    }

    /// 标记任务取消
    pub fn cancel(&mut self) {
        self.status = TaskStatus::Cancelled;
        self.end_time = Some(chrono::Utc::now().timestamp_nanos());
    }

    /// 标记任务超时
    pub fn timeout(&mut self) {
        self.status = TaskStatus::Timeout;
        self.end_time = Some(chrono::Utc::now().timestamp_nanos());
    }

    /// 更新资源使用情况
    pub fn update_resource_usage(&mut self, usage: ResourceUsage) {
        self.resource_usage = usage;
    }

    /// 添加元数据
    pub fn add_metadata(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.metadata.insert(key.into(), value);
    }

    /// 获取执行时间 (毫秒)
    pub fn execution_time_ms(&self) -> Option<u64> {
        self.end_time.map(|end| {
            ((end - self.start_time) / 1_000_000) as u64
        })
    }

    /// 判断任务是否完成 (成功或失败)
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Timeout
        )
    }

    /// 判断任务是否成功
    pub fn is_successful(&self) -> bool {
        self.status == TaskStatus::Completed
    }
}

/// 批量处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult<T> {
    /// 批次ID
    pub batch_id: String,
    /// 总记录数
    pub total_records: usize,
    /// 成功处理的记录数
    pub successful_records: usize,
    /// 失败的记录数
    pub failed_records: usize,
    /// 处理结果列表
    pub results: Vec<ProcessingResult<T>>,
    /// 批次开始时间
    pub start_time: i64,
    /// 批次结束时间
    pub end_time: i64,
    /// 总处理时间 (毫秒)
    pub total_processing_time_ms: u64,
}

impl<T> BatchResult<T> {
    /// 创建新的批量结果
    pub fn new(batch_id: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp_nanos();
        Self {
            batch_id: batch_id.into(),
            total_records: 0,
            successful_records: 0,
            failed_records: 0,
            results: Vec::new(),
            start_time: now,
            end_time: now,
            total_processing_time_ms: 0,
        }
    }

    /// 添加处理结果
    pub fn add_result(&mut self, result: ProcessingResult<T>) {
        self.total_records += 1;
        if result.success {
            self.successful_records += 1;
        } else {
            self.failed_records += 1;
        }
        self.total_processing_time_ms += result.processing_time_ms;
        self.results.push(result);
    }

    /// 完成批次处理
    pub fn finish(&mut self) {
        self.end_time = chrono::Utc::now().timestamp_nanos();
    }

    /// 获取成功率
    pub fn success_rate(&self) -> f64 {
        if self.total_records == 0 {
            return 0.0;
        }
        (self.successful_records as f64) / (self.total_records as f64)
    }

    /// 获取平均处理时间 (毫秒)
    pub fn average_processing_time_ms(&self) -> f64 {
        if self.total_records == 0 {
            return 0.0;
        }
        (self.total_processing_time_ms as f64) / (self.total_records as f64)
    }

    /// 获取总吞吐量 (记录/秒)
    pub fn throughput(&self) -> f64 {
        let duration_ms = ((self.end_time - self.start_time) / 1_000_000) as u64;
        if duration_ms == 0 {
            return 0.0;
        }
        (self.total_records as f64) / (duration_ms as f64 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::EnterpriseError;

    #[test]
    fn test_processing_result_success() {
        let result = ProcessingResult::success("test_data", 100, 10);
        assert!(result.success);
        assert_eq!(result.data, Some("test_data"));
        assert_eq!(result.processing_time_ms, 100);
        assert_eq!(result.records_processed, 10);
        assert_eq!(result.throughput(), 100.0); // 10 records / 0.1 seconds
    }

    #[test]
    fn test_processing_result_failure() {
        let error = EnterpriseError::configuration("test error");
        let result: ProcessingResult<String> = ProcessingResult::failure(error, 50);
        assert!(!result.success);
        assert!(result.data.is_none());
        assert!(result.error.is_some());
        assert_eq!(result.processing_time_ms, 50);
    }

    #[test]
    fn test_task_result_lifecycle() {
        let mut task = TaskResult::new("task-001", TaskPriority::High);
        assert_eq!(task.status, TaskStatus::Pending);

        task.start(Some("node-001".to_string()));
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.node_id, Some("node-001".to_string()));

        task.complete(Some(serde_json::json!({"result": "success"})));
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.is_finished());
        assert!(task.is_successful());
    }

    #[test]
    fn test_batch_result() {
        let mut batch = BatchResult::new("batch-001");
        
        batch.add_result(ProcessingResult::success("data1", 10, 1));
        batch.add_result(ProcessingResult::success("data2", 20, 1));
        batch.add_result(ProcessingResult::failure(
            EnterpriseError::configuration("error"), 
            15
        ));

        batch.finish();

        assert_eq!(batch.total_records, 3);
        assert_eq!(batch.successful_records, 2);
        assert_eq!(batch.failed_records, 1);
        assert_eq!(batch.success_rate(), 2.0 / 3.0);
        assert_eq!(batch.average_processing_time_ms(), 15.0); // (10+20+15)/3
    }
}
