use std::time::{Duration, Instant};
use std::thread::sleep;

use crate::node::NodeId;
use crate::message::{DeliveryGuarantee, MessageAcknowledgement, MessageDeduplicator, AckStatus};

#[test]
fn test_delivery_guarantee_default() {
    let default_guarantee = DeliveryGuarantee::default();
    assert_eq!(default_guarantee, DeliveryGuarantee::AtLeastOnce);
}

#[test]
fn test_message_acknowledgement() {
    let sender = NodeId::new();
    let receiver = NodeId::new();
    let message_id = "test-message-1".to_string();
    
    let mut ack = MessageAcknowledgement::new(message_id.clone(), sender.clone(), receiver.clone());
    
    // 检查初始状态
    assert_eq!(ack.message_id, message_id);
    assert_eq!(ack.sender, sender);
    assert_eq!(ack.receiver, receiver);
    assert!(matches!(ack.status, AckStatus::Pending));
    
    // 测试状态转换
    ack.mark_delivered();
    assert!(matches!(ack.status, AckStatus::Delivered));
    
    ack.mark_failed("网络错误".to_string());
    assert!(matches!(ack.status, AckStatus::Failed(_)));
    if let AckStatus::Failed(reason) = &ack.status {
        assert_eq!(reason, "网络错误");
    }
    
    ack.mark_expired();
    assert!(matches!(ack.status, AckStatus::Expired));
    
    // 测试过期检测
    let mut pending_ack = MessageAcknowledgement::new("test-message-2".to_string(), sender, receiver);
    assert!(!pending_ack.is_expired(Duration::from_secs(10)));
    
    // 使用一个很短的超时来测试过期
    assert!(pending_ack.is_expired(Duration::from_nanos(1)));
}

#[test]
fn test_message_deduplicator() {
    // 创建一个容量为10，过期时间为100毫秒的去重器
    let mut deduplicator = MessageDeduplicator::new(10, Duration::from_millis(100));
    
    // 初始状态
    assert_eq!(deduplicator.len(), 0);
    assert!(deduplicator.is_empty());
    
    // 第一次见到消息
    assert!(!deduplicator.is_duplicate("msg1"));
    assert_eq!(deduplicator.len(), 1);
    
    // 重复消息
    assert!(deduplicator.is_duplicate("msg1"));
    assert_eq!(deduplicator.len(), 1);
    
    // 不同的消息
    assert!(!deduplicator.is_duplicate("msg2"));
    assert_eq!(deduplicator.len(), 2);
    
    // 手动添加消息
    deduplicator.add_message("msg3");
    assert_eq!(deduplicator.len(), 3);
    assert!(deduplicator.is_duplicate("msg3"));
    
    // 测试过期
    sleep(Duration::from_millis(150));
    assert!(!deduplicator.is_duplicate("msg1")); // 已过期，视为新消息
    assert_eq!(deduplicator.len(), 3); // 加入新的msg1
    
    // 清理过期消息
    deduplicator.cleanup();
    assert_eq!(deduplicator.len(), 1); // 只剩下新加入的msg1
    
    // 清空去重器
    deduplicator.clear();
    assert_eq!(deduplicator.len(), 0);
    assert!(deduplicator.is_empty());
    
    // 测试容量限制
    for i in 0..15 {
        deduplicator.add_message(&format!("capacity-test-{}", i));
    }
    assert_eq!(deduplicator.len(), 10); // 容量限制为10
    
    // 确认最早的消息被淘汰
    assert!(!deduplicator.is_duplicate("capacity-test-0"));
    assert!(deduplicator.is_duplicate("capacity-test-14"));
}

#[test]
fn test_different_delivery_guarantees() {
    // 测试不同保障级别的特性
    let at_most_once = DeliveryGuarantee::AtMostOnce;
    let at_least_once = DeliveryGuarantee::AtLeastOnce;
    let exactly_once = DeliveryGuarantee::ExactlyOnce;
    let ordered = DeliveryGuarantee::Ordered;
    let causal = DeliveryGuarantee::Causal;
    let transactional = DeliveryGuarantee::Transactional;
    
    // 确保它们是不同的枚举值
    assert_ne!(at_most_once, at_least_once);
    assert_ne!(at_least_once, exactly_once);
    assert_ne!(exactly_once, ordered);
    assert_ne!(ordered, causal);
    assert_ne!(causal, transactional);
} 