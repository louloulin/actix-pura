use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};
use actix;

use crate::node::NodeId;
use crate::message::DeliveryGuarantee;
use crate::message::MessageType;

/// 消息信封，包含用于路由和处理远程消息的元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    /// 唯一消息标识符
    pub message_id: Uuid,
    /// 消息发送者节点ID
    pub sender_node: NodeId,
    /// 目标节点ID
    pub target_node: NodeId,
    /// 目标Actor路径（名称）
    pub target_actor: String,
    /// 发送时间戳（毫秒）
    pub timestamp: u64,
    /// 消息类型
    pub message_type: MessageType,
    /// 传递保证
    pub delivery_guarantee: DeliveryGuarantee,
    /// 序列化的消息内容
    pub payload: Vec<u8>,
    /// 指示消息是否已压缩
    pub compressed: bool,
}

impl actix::Message for MessageEnvelope {
    type Result = ();
}

impl MessageEnvelope {
    /// 创建新的消息信封
    pub fn new(
        sender_node: NodeId,
        target_node: NodeId,
        target_actor: String,
        message_type: MessageType,
        delivery_guarantee: DeliveryGuarantee,
        payload: Vec<u8>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        Self {
            message_id: Uuid::new_v4(),
            sender_node,
            target_node,
            target_actor,
            timestamp: now,
            message_type,
            delivery_guarantee,
            payload,
            compressed: false,
        }
    }
    
    /// 创建此消息的确认消息
    pub fn create_ack(&self) -> Self {
        Self::new(
            self.target_node.clone(),
            self.sender_node.clone(),
            "system".to_string(),
            MessageType::Pong,
            DeliveryGuarantee::AtMostOnce,
            Vec::new(),
        )
    }
    
    /// 创建ping消息
    pub fn create_ping(from_node: NodeId, to_node: NodeId) -> Self {
        Self::new(
            from_node,
            to_node,
            "system".to_string(),
            MessageType::Ping,
            DeliveryGuarantee::AtMostOnce,
            Vec::new(),
        )
    }
    
    /// 使用压缩功能创建一个消息信封
    pub fn new_compressed(
        sender_node: NodeId,
        target_node: NodeId,
        target_actor: String,
        message_type: MessageType,
        delivery_guarantee: DeliveryGuarantee,
        payload: Vec<u8>,
    ) -> Self {
        let mut envelope = Self::new(
            sender_node,
            target_node,
            target_actor,
            message_type,
            delivery_guarantee,
            payload,
        );
        envelope.compressed = true;
        envelope
    }
    
    /// 检查消息是否已压缩
    pub fn is_compressed(&self) -> bool {
        self.compressed
    }
    
    /// 设置压缩标志
    pub fn set_compressed(&mut self, compressed: bool) {
        self.compressed = compressed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_envelope_creation() {
        let sender = NodeId::default();
        let target = NodeId::new();
        let target_actor = "test_actor".to_string();
        let payload = vec![1, 2, 3, 4];
        
        let envelope = MessageEnvelope::new(
            sender.clone(),
            target.clone(),
            target_actor.clone(),
            MessageType::ActorMessage,
            DeliveryGuarantee::AtLeastOnce,
            payload.clone(),
        );
        
        assert_eq!(envelope.sender_node, sender);
        assert_eq!(envelope.target_node, target);
        assert_eq!(envelope.target_actor, target_actor);
        assert_eq!(envelope.message_type, MessageType::ActorMessage);
        assert_eq!(envelope.delivery_guarantee, DeliveryGuarantee::AtLeastOnce);
        assert_eq!(envelope.payload, payload);
        assert_eq!(envelope.compressed, false);
    }
    
    #[test]
    fn test_create_acknowledgement() {
        let sender = NodeId::default();
        let target = NodeId::new();
        let target_actor = "test_actor".to_string();
        let payload = vec![1, 2, 3, 4];
        
        let envelope = MessageEnvelope::new(
            sender.clone(),
            target.clone(),
            target_actor,
            MessageType::ActorMessage,
            DeliveryGuarantee::AtLeastOnce,
            payload,
        );
        
        let ack = envelope.create_ack();
        
        assert_eq!(ack.sender_node, target);
        assert_eq!(ack.target_node, sender);
        assert_eq!(ack.message_type, MessageType::Pong);
        assert_eq!(ack.delivery_guarantee, DeliveryGuarantee::AtMostOnce);
        assert_eq!(ack.compressed, false);
    }
    
    #[test]
    fn test_message_compressed_flag() {
        let sender = NodeId::default();
        let target = NodeId::new();
        let target_actor = "test_actor".to_string();
        let payload = vec![1, 2, 3, 4];
        
        let envelope = MessageEnvelope::new_compressed(
            sender.clone(),
            target.clone(),
            target_actor.clone(),
            MessageType::ActorMessage,
            DeliveryGuarantee::AtLeastOnce,
            payload.clone(),
        );
        
        assert_eq!(envelope.is_compressed(), true);
        
        let mut normal_envelope = MessageEnvelope::new(
            sender.clone(),
            target.clone(),
            target_actor.clone(),
            MessageType::ActorMessage,
            DeliveryGuarantee::AtLeastOnce,
            payload.clone(),
        );
        
        assert_eq!(normal_envelope.is_compressed(), false);
        
        normal_envelope.set_compressed(true);
        assert_eq!(normal_envelope.is_compressed(), true);
    }
} 