use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, MutexGuard};

use crate::node::{NodeId, NodeInfo};
use crate::transport::{MessageHandler, P2PTransport};
use crate::message::MessageEnvelope;
use crate::error::ClusterError;

/// 测试帮助函数：为测试添加对等节点
pub fn add_peer_for_test(transport: &mut P2PTransport, node_id: NodeId, node_info: NodeInfo) {
    let mut peers = transport.peers_lock_for_testing();
    peers.insert(node_id, node_info);
}

/// 测试帮助函数：获取对等节点锁
pub fn get_peers_lock_for_test(transport: &P2PTransport) -> MutexGuard<'_, HashMap<NodeId, NodeInfo>> {
    transport.peers_lock_for_testing()
}

/// 测试帮助函数：获取对等节点列表
pub fn get_peer_list_for_test(transport: &P2PTransport) -> Vec<NodeInfo> {
    let peers = transport.peers_lock_for_testing();
    peers.values().cloned().collect()
}

/// 测试帮助函数：直接设置消息处理器
pub fn set_message_handler_direct_for_test(transport: &mut P2PTransport, handler: Arc<Mutex<dyn MessageHandler>>) {
    transport.message_handler_for_testing(handler);
}

/// 测试帮助函数：获取消息处理器
pub fn get_message_handler_for_test(transport: &P2PTransport) -> Option<Arc<Mutex<dyn MessageHandler>>> {
    transport.get_message_handler_for_testing()
}

/// 测试帮助函数：发送消息信封
pub async fn send_envelope_for_test(transport: &mut P2PTransport, envelope: MessageEnvelope) -> Result<(), ClusterError> {
    transport.send_envelope(envelope).await
}

/// 测试帮助函数：直接发送消息信封到目标节点，不依赖连接状态
pub async fn send_envelope_direct_for_test(transport: &mut P2PTransport, target_node: NodeId, envelope: MessageEnvelope) -> Result<(), ClusterError> {
    transport.send_envelope_direct_for_test(target_node, envelope).await
} 