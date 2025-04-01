use chrono::Utc;
use tokio::test;

use crate::persistence::log::{LogProvider, InMemoryLog, LogEntry, LogResult};

#[test]
async fn test_in_memory_log_write_query() {
    let log_provider = InMemoryLog::new();
    
    // 写入成功日志
    let success_entry = LogEntry::success(
        "create_order",
        Some("order1".to_string()),
        "Created new order"
    );
    log_provider.write_log(success_entry.clone()).await.unwrap();
    
    // 写入失败日志
    let failure_entry = LogEntry::failure(
        "cancel_order",
        Some("order2".to_string()),
        "Order not found"
    );
    log_provider.write_log(failure_entry.clone()).await.unwrap();
    
    // 查询所有日志
    let all_logs = log_provider.query_logs(None, None, None, None, None).await.unwrap();
    assert_eq!(all_logs.len(), 2, "应该有2条日志");
    
    // 按操作类型查询
    let create_logs = log_provider.query_logs(
        Some("create_order".to_string()),
        None, None, None, None
    ).await.unwrap();
    assert_eq!(create_logs.len(), 1, "应该有1条创建订单日志");
    assert_eq!(create_logs[0].operation_type, "create_order");
    assert_eq!(create_logs[0].result, LogResult::Success);
    
    // 按目标ID查询
    let order2_logs = log_provider.query_logs(
        None,
        Some("order2".to_string()),
        None, None, None
    ).await.unwrap();
    assert_eq!(order2_logs.len(), 1, "应该有1条关于order2的日志");
    assert_eq!(order2_logs[0].target_id.as_ref().unwrap(), "order2");
    assert_eq!(order2_logs[0].result, LogResult::Failure);
}

#[test]
async fn test_log_entry_creation() {
    // 测试成功日志创建
    let success_entry = LogEntry::success(
        "test_operation",
        Some("target123".to_string()),
        "Test details"
    );
    
    assert_eq!(success_entry.operation_type, "test_operation");
    assert_eq!(success_entry.target_id.as_ref().unwrap(), "target123");
    assert_eq!(success_entry.details, "Test details");
    assert_eq!(success_entry.result, LogResult::Success);
    
    // 测试失败日志创建
    let failure_entry = LogEntry::failure(
        "test_operation",
        None,
        "Error details"
    );
    
    assert_eq!(failure_entry.operation_type, "test_operation");
    assert!(failure_entry.target_id.is_none());
    assert_eq!(failure_entry.details, "Error details");
    assert_eq!(failure_entry.result, LogResult::Failure);
}

#[test]
async fn test_log_query_with_time_filter() {
    let log_provider = InMemoryLog::new();
    
    // 创建具有不同时间戳的日志
    let mut entries = Vec::new();
    
    // 等待一小段时间，确保时间戳不同
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    let start_time = Utc::now();
    
    // 创建三个日志条目
    for i in 0..3 {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let entry = LogEntry::success(
            format!("operation_{}", i),
            Some(format!("target_{}", i)),
            format!("Details {}", i)
        );
        log_provider.write_log(entry.clone()).await.unwrap();
        entries.push(entry);
    }
    
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    let end_time = Utc::now();
    
    // 创建更多日志条目
    for i in 3..5 {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let entry = LogEntry::success(
            format!("operation_{}", i),
            Some(format!("target_{}", i)),
            format!("Details {}", i)
        );
        log_provider.write_log(entry.clone()).await.unwrap();
        entries.push(entry);
    }
    
    // 按时间范围查询
    let time_filtered_logs = log_provider.query_logs(
        None, None,
        Some(start_time), Some(end_time),
        None
    ).await.unwrap();
    
    assert_eq!(time_filtered_logs.len(), 3, "应该有3条在时间范围内的日志");
    
    for log in &time_filtered_logs {
        assert!(log.timestamp >= start_time, "日志时间应该晚于或等于开始时间");
        assert!(log.timestamp <= end_time, "日志时间应该早于或等于结束时间");
    }
    
    // 测试限制结果数量
    let limited_logs = log_provider.query_logs(
        None, None, None, None,
        Some(2)
    ).await.unwrap();
    
    assert_eq!(limited_logs.len(), 2, "应该限制为2条日志");
} 