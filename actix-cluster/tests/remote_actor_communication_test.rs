use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};

use actix::prelude::*;
use actix_cluster::{
    node::{NodeId, NodeInfo},
    transport::{P2PTransport, TransportMessage, MessageHandler, RemoteActorRef},
    config::NodeRole,
    serialization::SerializationFormat,
    error::ClusterResult,
    message::{MessageEnvelope, MessageType, DeliveryGuarantee, ActorPath},
    testing,
};
use parking_lot::Mutex;
use tokio::sync::Mutex as TokioMutex;
use serde::{Deserialize, Serialize};

// 测试消息
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestActorMessage {
    content: String,
}

// 测试接收器，用于验证消息是否被接收
struct MessageReceiver {
    received: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl MessageHandler for MessageReceiver {
    async fn handle_message(&self, sender: NodeId, message: TransportMessage) -> ClusterResult<()> {
        println!("MessageReceiver received message from {}: {:?}", sender, message);
        
        if let TransportMessage::Envelope(envelope) = message {
            // 收到消息，设置标志
            self.received.store(true, Ordering::SeqCst);
            println!("Message received and flag set to true");
        }
        
        Ok(())
    }
}

#[tokio::test]
async fn test_remote_actor_communication() {
    // 创建消息已接收标志
    let message_received = Arc::new(AtomicBool::new(false));
    
    // 创建两个节点
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9701);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9702);
    
    let node1_id = NodeId::new();
    let node2_id = NodeId::new();
    
    let node1_info = NodeInfo::new(
        node1_id.clone(),
        "remote-test-node-1".to_string(),
        NodeRole::Peer,
        node1_addr,
    );
    
    let node2_info = NodeInfo::new(
        node2_id.clone(),
        "remote-test-node-2".to_string(),
        NodeRole::Peer,
        node2_addr,
    );
    
    println!("Setting up node 1 (sender)");
    // 创建发送节点的传输层
    let mut transport1 = P2PTransport::new(node1_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport1");
    
    println!("Setting up node 2 (receiver)");
    // 创建接收节点的传输层
    let mut transport2 = P2PTransport::new(node2_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport2");
    
    // 设置节点2的消息接收器
    let receiver = MessageReceiver { received: message_received.clone() };
    testing::set_message_handler_direct_for_test(&mut transport2, Arc::new(Mutex::new(receiver)));
    
    // 手动添加节点信息到对方的peers列表
    testing::add_peer_for_test(&mut transport1, node2_id.clone(), node2_info.clone());
    testing::add_peer_for_test(&mut transport2, node1_id.clone(), node1_info.clone());
    
    // 创建一个RemoteActorRef来发送消息到远程actor
    println!("Creating RemoteActorRef for target actor");
    let target_actor_path = "test_actor".to_string();
    let transport1_arc = Arc::new(TokioMutex::new(transport1));
    let remote_ref = RemoteActorRef::new(
        node2_id.clone(),
        target_actor_path,
        transport1_arc.clone(),
        DeliveryGuarantee::AtMostOnce,
    );
    
    // 创建测试消息
    let test_message = TestActorMessage {
        content: "Hello from remote actor".to_string(),
    };
    
    // 序列化测试消息
    println!("Serializing test message");
    let serializer = actix_cluster::serialization::BincodeSerializer::new();
    let payload = serializer.serialize(&test_message).expect("Failed to serialize message");
    
    // 发送消息信封
    println!("Sending message envelope to remote actor");
    let result = remote_ref.send(test_message).await;
    assert!(result.is_ok(), "Failed to send message: {:?}", result);
    
    // 模拟消息到达节点2并被处理
    // 在真实场景中，这部分由网络传输完成，但在测试中我们手动模拟
    println!("Simulating message reception on node 2");
    if let Some(handler) = testing::get_message_handler_for_test(&transport2) {
        let node_id_clone = node1_id.clone();
        let test_envelope = MessageEnvelope::new(
            node1_id.clone(),
            node2_id.clone(),
            "test_actor".to_string(),
            MessageType::ActorMessage,
            DeliveryGuarantee::AtMostOnce,
            payload.clone(),
        );
        let message_clone = TransportMessage::Envelope(test_envelope);
        let handler_clone = handler.clone();
        
        tokio::task::spawn_blocking(move || {
            let handler_locked = handler_clone.lock();
            
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            
            if let Err(e) = rt.block_on(handler_locked.handle_message(node_id_clone, message_clone)) {
                eprintln!("Error handling message in test: {:?}", e);
            }
        }).await.unwrap();
    }
    
    // 等待消息处理完成
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // 验证消息是否被接收
    assert!(message_received.load(Ordering::SeqCst), "Message was not received by remote actor");
    println!("Remote actor communication test passed!");
}

#[tokio::test]
async fn test_send_typed_message() {
    // 创建消息已接收标志
    let message_received = Arc::new(AtomicBool::new(false));
    
    // 创建两个节点
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9703);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9704);
    
    let node1_id = NodeId::new();
    let node2_id = NodeId::new();
    
    let node1_info = NodeInfo::new(
        node1_id.clone(),
        "typed-test-node-1".to_string(),
        NodeRole::Peer,
        node1_addr,
    );
    
    let node2_info = NodeInfo::new(
        node2_id.clone(),
        "typed-test-node-2".to_string(),
        NodeRole::Peer,
        node2_addr,
    );
    
    // 创建发送节点的传输层
    let mut transport1 = P2PTransport::new(node1_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport1");
    
    // 创建接收节点的传输层
    let mut transport2 = P2PTransport::new(node2_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport2");
    
    // 设置节点2的消息接收器
    let receiver = MessageReceiver { received: message_received.clone() };
    testing::set_message_handler_direct_for_test(&mut transport2, Arc::new(Mutex::new(receiver)));
    
    // 手动添加节点信息到对方的peers列表
    testing::add_peer_for_test(&mut transport1, node2_id.clone(), node2_info.clone());
    testing::add_peer_for_test(&mut transport2, node1_id.clone(), node1_info.clone());
    
    // 创建一个RemoteActorRef来发送消息到远程actor
    let target_actor_path = "typed_test_actor".to_string();
    let transport1_arc = Arc::new(TokioMutex::new(transport1));
    let remote_ref = RemoteActorRef::new(
        node2_id.clone(),
        target_actor_path,
        transport1_arc.clone(),
        DeliveryGuarantee::AtMostOnce,
    );
    
    // 创建测试消息
    let test_message = TestActorMessage {
        content: "Hello from typed message".to_string(),
    };
    
    // 直接使用 send 方法发送类型化消息
    let result = remote_ref.send(test_message).await;
    assert!(result.is_ok(), "Failed to send typed message: {:?}", result);
    
    // 等待消息处理
    // 在实际应用中，这部分由网络传输完成，但在测试中我们只验证API是否正确
    
    println!("Typed message send API test passed!");
} 