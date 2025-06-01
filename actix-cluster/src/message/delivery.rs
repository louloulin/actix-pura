use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use lru::LruCache;

use crate::node::NodeId;

/// 消息传递保障级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryGuarantee {
    /// 最多一次交付 - 无保证，可能丢失
    AtMostOnce,
    
    /// 至少一次交付 - 保证交付，但可能重复
    AtLeastOnce,
    
    /// 精确一次交付 - 保证交付且不重复
    ExactlyOnce,
    
    /// 顺序交付 - 保证消息按发送顺序交付
    Ordered,
    
    /// 因果交付 - 保证因果关系消息顺序
    Causal,
    
    /// 事务性消息 - 所有目标节点都收到或都不收到
    Transactional,
}

impl Default for DeliveryGuarantee {
    fn default() -> Self {
        Self::AtLeastOnce
    }
}

/// 消息确认状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckStatus {
    /// 等待确认
    Pending,
    
    /// 已成功交付
    Delivered,
    
    /// 交付失败
    Failed(String),
    
    /// 确认超时
    Expired,
}

/// 消息确认信息
#[derive(Debug, Clone)]
pub struct MessageAcknowledgement {
    /// 消息唯一标识
    pub message_id: String,
    
    /// 发送节点ID
    pub sender: NodeId,
    
    /// 接收节点ID
    pub receiver: NodeId,
    
    /// 消息时间戳
    pub timestamp: Instant,
    
    /// 确认状态
    pub status: AckStatus,
}

impl MessageAcknowledgement {
    /// 创建新的消息确认
    pub fn new(message_id: String, sender: NodeId, receiver: NodeId) -> Self {
        Self {
            message_id,
            sender,
            receiver,
            timestamp: Instant::now(),
            status: AckStatus::Pending,
        }
    }
    
    /// 标记为已交付
    pub fn mark_delivered(&mut self) {
        self.status = AckStatus::Delivered;
    }
    
    /// 标记为失败
    pub fn mark_failed(&mut self, reason: String) {
        self.status = AckStatus::Failed(reason);
    }
    
    /// 标记为超时
    pub fn mark_expired(&mut self) {
        self.status = AckStatus::Expired;
    }
    
    /// 检查是否已经过期
    pub fn is_expired(&self, timeout: Duration) -> bool {
        match self.status {
            AckStatus::Pending => Instant::now().duration_since(self.timestamp) > timeout,
            _ => false,
        }
    }
}

/// 消息去重器
pub struct MessageDeduplicator {
    /// 已处理消息缓存，键为消息ID，值为处理时间
    seen_messages: LruCache<String, Instant>,
    
    /// 消息过期时间
    expiry_time: Duration,
}

impl MessageDeduplicator {
    /// 创建新的消息去重器
    pub fn new(capacity: usize, expiry_time: Duration) -> Self {
        let capacity = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(100).unwrap());
        Self {
            seen_messages: LruCache::new(capacity),
            expiry_time,
        }
    }
    
    /// 检查消息是否是重复的
    pub fn is_duplicate(&mut self, message_id: &str) -> bool {
        if let Some(time) = self.seen_messages.get(message_id) {
            Instant::now().duration_since(*time) < self.expiry_time
        } else {
            self.seen_messages.put(message_id.to_string(), Instant::now());
            false
        }
    }
    
    /// 清理过期的消息记录
    pub fn cleanup(&mut self) {
        let now = Instant::now();
        let expired_keys: Vec<String> = self.seen_messages
            .iter()
            .filter(|(_, &time)| now.duration_since(time) >= self.expiry_time)
            .map(|(key, _)| key.clone())
            .collect();
        
        for key in expired_keys {
            self.seen_messages.pop(&key);
        }
    }
    
    /// 获取已记录的消息数量
    pub fn len(&self) -> usize {
        self.seen_messages.len()
    }
    
    /// 检查去重器是否为空
    pub fn is_empty(&self) -> bool {
        self.seen_messages.is_empty()
    }
    
    /// 手动添加消息ID
    pub fn add_message(&mut self, message_id: &str) {
        self.seen_messages.put(message_id.to_string(), Instant::now());
    }
    
    /// 重置去重器
    pub fn clear(&mut self) {
        self.seen_messages.clear();
    }
} 