//! 定义分布式消息信封和路由。
//! 
//! 此模块实现了用于远程消息传递的信封结构，
//! 允许消息在集群节点之间传递。

use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use actix;
use std::fmt;

use crate::node::{NodeId, NodeInfo};
use crate::error::ClusterResult;

mod envelope;
mod delivery;
mod handler;

pub use envelope::MessageEnvelope;
pub use delivery::*;
pub use handler::MessageEnvelopeHandler;

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

impl std::fmt::Display for ActorPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.node_id, self.path)
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