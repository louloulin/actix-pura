//! 定义分布式消息信封和路由。
//! 
//! 此模块实现了用于远程消息传递的信封结构，
//! 允许消息在集群节点之间传递。

use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use actix;

use crate::node::{NodeId, NodeInfo};
use crate::error::ClusterResult;

/// 表示消息的传递保证级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryGuarantee {
    /// 尽力而为传递，可能会丢失消息（最高性能）
    AtMostOnce,
    /// 确保消息至少被传递一次，可能会重复
    AtLeastOnce,
    /// 确保消息恰好传递一次（最高可靠性，但性能较低）
    ExactlyOnce,
}

/// 消息信封的类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// 标准Actor消息
    ActorMessage,
    /// 系统控制消息
    SystemControl,
    /// 服务发现相关消息
    Discovery,
    /// Ping消息 - 用于检测节点活跃度
    Ping,
    /// Pong消息 - 回应Ping
    Pong,
}

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
}

/// Actor路径，用于标识集群中的Actor
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorPath {
    /// 包含Actor的节点ID
    pub node_id: NodeId,
    /// Actor在节点内的路径/名称
    pub path: String,
}

impl ActorPath {
    /// 创建新的Actor路径
    pub fn new(node_id: NodeId, path: String) -> Self {
        Self { node_id, path }
    }
    
    /// 创建本地Actor路径，使用特殊的NodeId::local()
    pub fn local(path: String) -> Self {
        Self {
            node_id: NodeId::local(),
            path,
        }
    }
    
    /// 检查路径是否指向本地节点
    pub fn is_local(&self, local_node: &NodeId) -> bool {
        &self.node_id == local_node || self.node_id.is_local()
    }
}

/// 通用消息包装，使我们能在Actor系统中传递任意类型的消息
#[derive(Debug)]
pub struct AnyMessage(pub Box<dyn std::any::Any + Send>);

impl actix::Message for AnyMessage {
    type Result = ();
}

impl AnyMessage {
    /// 创建新的泛型消息
    pub fn new<T: 'static + Send>(inner: T) -> Self {
        AnyMessage(Box::new(inner))
    }
    
    /// 尝试从包装中取出特定类型的消息
    pub fn downcast<T: 'static>(&self) -> Option<&T> {
        self.0.downcast_ref::<T>()
    }
    
    /// 尝试从包装中取出特定类型的消息（可变版本）
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.0.downcast_mut::<T>()
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
    }
    
    #[test]
    fn test_actor_path() {
        let node_id = NodeId::new();
        let path = "user/greeter".to_string();
        
        let actor_path = ActorPath::new(node_id.clone(), path.clone());
        
        assert_eq!(actor_path.node_id, node_id);
        assert_eq!(actor_path.path, path);
        
        // 测试本地路径
        let local_path = ActorPath::local(path.clone());
        assert_eq!(local_path.path, path);
        
        // 本地路径应该使用NodeId::local()
        let local_node_id = NodeId::local();
        assert_eq!(local_path.node_id, local_node_id);
        
        // 测试是否为本地
        assert!(!actor_path.is_local(&local_node_id));
        // 使用本地节点ID创建的路径应该识别为本地
        assert!(local_path.is_local(&local_node_id));
        // 在任何节点上，本地路径也应该被识别为本地
        assert!(local_path.is_local(&NodeId::new()));
    }
} 