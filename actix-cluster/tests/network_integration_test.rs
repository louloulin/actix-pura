use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use actix::prelude::*;
use actix_cluster::{
    node::{NodeId, NodeInfo},
    transport::{P2PTransport, TransportMessage, MessageHandler},
    config::NodeRole,
    serialization::SerializationFormat,
    error::ClusterResult,
    message::{MessageEnvelope, MessageType, DeliveryGuarantee, ActorPath},
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

// 测试消息
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkTestMessage {
    id: u32,
    content: String,
}

// 消息接收处理器
struct NetworkMessageReceiver {
    received: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl MessageHandler for NetworkMessageReceiver {
    async fn handle_message(&mut self, sender: NodeId, message: TransportMessage) -> ClusterResult<()> {
        println!("NetworkMessageReceiver received message from {}: {:?}", sender, message);
        
        if let TransportMessage::Envelope(envelope) = &message {
            if envelope.message_type == MessageType::ActorMessage {
                self.received.store(true, Ordering::SeqCst);
                println!("Received envelope message with payload size: {}", envelope.payload.len());
            }
        }
        
        Ok(())
    }
}

#[tokio::test]
async fn test_network_communication() {
    // 创建消息接收标志
    let message_received = Arc::new(AtomicBool::new(false));
    
    // 使用不同的端口，避免与其他测试冲突
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8901);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8902);
    
    let node1_id = NodeId::new();
    let node2_id = NodeId::new();
    
    println!("Creating node info");
    let node1_info = NodeInfo::new(
        node1_id.clone(),
        "network-test-node-1".to_string(),
        NodeRole::Peer,
        node1_addr,
    );
    
    let node2_info = NodeInfo::new(
        node2_id.clone(),
        "network-test-node-2".to_string(),
        NodeRole::Peer,
        node2_addr,
    );
    
    println!("Creating transports");
    // 创建传输层
    let mut transport1 = P2PTransport::new(node1_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport1");
    
    let mut transport2 = P2PTransport::new(node2_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport2");
    
    // 在传输层2上设置消息处理器
    let receiver = NetworkMessageReceiver { received: message_received.clone() };
    transport2.set_message_handler_direct(Arc::new(Mutex::new(receiver)));
    
    println!("Starting transport on node 2 (receiver)");
    // 启动传输层2
    transport2.init().await.expect("Failed to initialize transport2");
    
    // 给节点2一些时间启动
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    println!("Starting transport on node 1 (sender)");
    // 启动传输层1
    transport1.init().await.expect("Failed to initialize transport1");
    
    // 手动将节点2添加为节点1的对等节点
    transport1.add_peer(node2_id.clone(), node2_info.clone());
    
    println!("Establishing connection from node 1 to node 2");
    // 连接到节点2
    transport1.connect_to_peer(node2_addr).await.expect("Failed to connect to node2");
    
    // 给一些时间建立连接
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // 创建测试消息
    let test_message = NetworkTestMessage {
        id: 1,
        content: "Test network communication".to_string(),
    };
    
    // 序列化消息
    let serializer = actix_cluster::serialization::BincodeSerializer::new();
    let payload = serializer.serialize(&test_message).expect("Failed to serialize message");
    
    // 创建消息信封
    let envelope = MessageEnvelope::new(
        node1_id.clone(),
        node2_id.clone(),
        "network_test_actor".to_string(),
        MessageType::ActorMessage,
        DeliveryGuarantee::AtMostOnce,
        payload,
    );
    
    println!("Sending message from node 1 to node 2");
    // 发送消息到节点2
    transport1.send_envelope(envelope).await.expect("Failed to send envelope");
    
    // 等待消息处理
    for _ in 0..20 {
        // 检查消息是否收到
        if message_received.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // 验证消息是否被接收
    assert!(message_received.load(Ordering::SeqCst), "Message was not received by node 2");
    println!("Network communication test passed!");
}

#[tokio::test]
async fn test_bidirectional_communication() {
    // 创建消息接收标志
    let node1_received = Arc::new(AtomicBool::new(false));
    let node2_received = Arc::new(AtomicBool::new(false));
    
    // 使用不同的端口，避免与其他测试冲突
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8903);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8904);
    
    let node1_id = NodeId::new();
    let node2_id = NodeId::new();
    
    println!("Creating node info for bidirectional test");
    let node1_info = NodeInfo::new(
        node1_id.clone(),
        "bidir-test-node-1".to_string(),
        NodeRole::Peer,
        node1_addr,
    );
    
    let node2_info = NodeInfo::new(
        node2_id.clone(),
        "bidir-test-node-2".to_string(),
        NodeRole::Peer,
        node2_addr,
    );
    
    // 创建消息处理器
    let receiver1 = NetworkMessageReceiver { received: node1_received.clone() };
    let receiver2 = NetworkMessageReceiver { received: node2_received.clone() };
    
    println!("Creating transports for bidirectional test");
    // 创建传输层
    let mut transport1 = P2PTransport::new(node1_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport1");
    
    let mut transport2 = P2PTransport::new(node2_info.clone(), SerializationFormat::Bincode)
        .expect("Failed to create transport2");
    
    // 设置消息处理器
    transport1.set_message_handler_direct(Arc::new(Mutex::new(receiver1)));
    transport2.set_message_handler_direct(Arc::new(Mutex::new(receiver2)));
    
    println!("Starting both transports");
    // 启动传输层
    transport1.init().await.expect("Failed to initialize transport1");
    transport2.init().await.expect("Failed to initialize transport2");
    
    // 给一些时间启动
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // 手动添加对等节点信息
    transport1.add_peer(node2_id.clone(), node2_info.clone());
    transport2.add_peer(node1_id.clone(), node1_info.clone());
    
    println!("Establishing connections between nodes");
    // 建立连接
    transport1.connect_to_peer(node2_addr).await.expect("Failed to connect node1 to node2");
    
    // 给一些时间建立连接
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // 创建测试消息从节点1到节点2
    let message1to2 = NetworkTestMessage {
        id: 2,
        content: "Message from node 1 to node 2".to_string(),
    };
    
    // 序列化消息
    let serializer = actix_cluster::serialization::BincodeSerializer::new();
    let payload1to2 = serializer.serialize(&message1to2).expect("Failed to serialize message1to2");
    
    // 创建消息信封
    let envelope1to2 = MessageEnvelope::new(
        node1_id.clone(),
        node2_id.clone(),
        "bidir_test_actor".to_string(),
        MessageType::ActorMessage,
        DeliveryGuarantee::AtMostOnce,
        payload1to2,
    );
    
    println!("Sending message from node 1 to node 2");
    // 发送消息到节点2
    transport1.send_envelope(envelope1to2).await.expect("Failed to send envelope1to2");
    
    // 等待消息处理
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // 创建测试消息从节点2到节点1
    let message2to1 = NetworkTestMessage {
        id: 3,
        content: "Message from node 2 to node 1".to_string(),
    };
    
    // 序列化消息
    let payload2to1 = serializer.serialize(&message2to1).expect("Failed to serialize message2to1");
    
    // 创建消息信封
    let envelope2to1 = MessageEnvelope::new(
        node2_id.clone(),
        node1_id.clone(),
        "bidir_test_actor".to_string(),
        MessageType::ActorMessage,
        DeliveryGuarantee::AtMostOnce,
        payload2to1,
    );
    
    println!("Sending message from node 2 to node 1");
    // 发送消息到节点1
    transport2.send_envelope(envelope2to1).await.expect("Failed to send envelope2to1");
    
    // 等待消息处理
    for _ in 0..20 {
        if node1_received.load(Ordering::SeqCst) && node2_received.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // 验证消息是否双向接收
    assert!(node2_received.load(Ordering::SeqCst), "Message was not received by node 2");
    assert!(node1_received.load(Ordering::SeqCst), "Message was not received by node 1");
    
    println!("Bidirectional communication test passed!");
} 