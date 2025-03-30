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
        println!("ENTRY: NetworkMessageReceiver::handle_message called from {}", sender);
        println!("NetworkMessageReceiver received message type: {:?}", std::any::TypeId::of::<TransportMessage>());
        println!("Message details: {:?}", message);
        
        match &message {
            TransportMessage::Envelope(envelope) => {
                println!("Message is an envelope with type: {:?}", envelope.message_type);
                println!("Envelope details: sender={}, target={}, message_id={}, payload_size={}", 
                        envelope.sender_node, envelope.target_node, envelope.message_id, envelope.payload.len());
                
                if envelope.message_type == MessageType::ActorMessage {
                    // Try to deserialize the payload to see what's inside
                    println!("Envelope has ActorMessage type, attempting to deserialize payload");
                    let serializer = actix_cluster::serialization::BincodeSerializer::new();
                    match serializer.deserialize::<NetworkTestMessage>(&envelope.payload) {
                        Ok(test_message) => {
                            println!("Successfully deserialized test message: {:?}", test_message);
                        }
                        Err(e) => {
                            println!("Failed to deserialize payload as NetworkTestMessage: {:?}", e);
                            if envelope.payload.len() > 0 {
                                let preview_len = std::cmp::min(envelope.payload.len(), 16);
                                println!("Payload first {} bytes: {:?}", preview_len, &envelope.payload[0..preview_len]);
                            }
                        }
                    }
                    
                    println!("Setting received flag to true");
                    self.received.store(true, Ordering::SeqCst);
                    println!("Received envelope message with payload size: {}", envelope.payload.len());
                } else {
                    println!("Envelope message type is not ActorMessage, but: {:?}", envelope.message_type);
                }
            }
            other => {
                println!("Message is not an envelope but: {:?}", other);
            }
        }
        
        println!("NetworkMessageReceiver::handle_message completed successfully");
        Ok(())
    }
}

#[tokio::test]
async fn test_network_communication() {
    // Use a LocalSet to properly handle spawn_local
    let local = tokio::task::LocalSet::new();
    
    local.run_until(async {
        println!("======= Starting network communication test =======");
        
        // 创建消息接收标志
        let message_received = Arc::new(AtomicBool::new(false));
        
        // 使用不同的端口，避免与其他测试冲突
        let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9901);
        let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9902);
        
        let node1_id = NodeId::new();
        let node2_id = NodeId::new();
        
        println!("Creating node info");
        println!("Node 1 ID: {}, address: {}", node1_id, node1_addr);
        println!("Node 2 ID: {}, address: {}", node2_id, node2_addr);
        
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
        println!("Waiting for node 2 to initialize...");
        tokio::task::yield_now().await; // Ensure tasks get a chance to start
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        println!("Starting transport on node 1 (sender)");
        // 启动传输层1
        transport1.init().await.expect("Failed to initialize transport1");
        
        // Allow tasks to get scheduled
        tokio::task::yield_now().await;
        
        // 手动将节点2添加为节点1的对等节点
        println!("Adding node 2 as peer to node 1");
        transport1.add_peer(node2_id.clone(), node2_info.clone());
        
        // Verify node 2 is in the peers list of node 1
        {
            let peers = transport1.get_peers_lock();
            let peers_guard = peers.lock();
            println!("Node 1 peers: {:?}", peers_guard.keys().collect::<Vec<_>>());
            assert!(peers_guard.contains_key(&node2_id), "Node 2 should be in the peers list of node 1");
        }
        
        println!("Establishing connection from node 1 to node 2");
        // 连接到节点2
        match transport1.connect_to_peer(node2_addr).await {
            Ok(_) => println!("Successfully connected to node 2"),
            Err(e) => panic!("Failed to connect to node 2: {:?}", e),
        }
        
        // 给一些时间建立连接和任务调度
        println!("Waiting for connection to establish...");
        for _ in 0..5 {
            tokio::task::yield_now().await; // Ensure tasks get a chance to run
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        
        // Verify connection exists indirectly - we can check by getting the peer list
        let peer_list = transport1.get_peer_list();
        println!("Node 1 peers after connection: {:?}", peer_list);
        assert!(peer_list.contains(&node2_id), "Node 2 should be in the peer list of node 1");
        
        // 创建测试消息
        let test_message = NetworkTestMessage {
            id: 1,
            content: "Test network communication".to_string(),
        };
        
        // 序列化消息
        println!("Serializing test message");
        let serializer = actix_cluster::serialization::BincodeSerializer::new();
        let payload = serializer.serialize(&test_message).expect("Failed to serialize message");
        
        // 创建消息信封
        println!("Creating message envelope");
        println!("Creating envelope from node 1 to node 2");
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
        match transport1.send_envelope(envelope).await {
            Ok(_) => println!("Successfully sent envelope to node 2"),
            Err(e) => panic!("Failed to send envelope: {:?}", e),
        }
        
        // Ensure tasks get a chance to run
        tokio::task::yield_now().await;
        
        // 等待消息处理
        println!("Waiting for message to be processed...");
        for i in 0..30 {
            // Allow tasks to be scheduled
            tokio::task::yield_now().await;
            
            // 检查消息是否收到
            if message_received.load(Ordering::SeqCst) {
                println!("Message received after {} attempts", i + 1);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            if (i + 1) % 10 == 0 {
                println!("Still waiting for message... ({} attempts)", i + 1);
            }
        }
        
        // 验证消息是否被接收
        assert!(message_received.load(Ordering::SeqCst), "Message was not received by node 2");
        println!("Network communication test passed!");
    }).await;
}

#[tokio::test]
async fn test_bidirectional_communication() {
    // Use a LocalSet to properly handle spawn_local
    let local = tokio::task::LocalSet::new();
    
    local.run_until(async {
        // 创建消息接收标志
        let node1_received = Arc::new(AtomicBool::new(false));
        let node2_received = Arc::new(AtomicBool::new(false));
        
        // 使用不同的端口，避免与其他测试冲突
        let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9903);
        let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9904);
        
        let node1_id = NodeId::new();
        let node2_id = NodeId::new();
        
        println!("Creating node info for bidirectional test");
        println!("Node 1 ID: {}, address: {}", node1_id, node1_addr);
        println!("Node 2 ID: {}, address: {}", node2_id, node2_addr);
        
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
        println!("Waiting for transports to initialize...");
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // 手动添加对等节点信息
        println!("Adding peer information to both nodes");
        transport1.add_peer(node2_id.clone(), node2_info.clone());
        transport2.add_peer(node1_id.clone(), node1_info.clone());
        
        // Verify peers lists
        {
            let peers1 = transport1.get_peers_lock();
            let peers1_guard = peers1.lock();
            println!("Node 1 peers: {:?}", peers1_guard.keys().collect::<Vec<_>>());
            assert!(peers1_guard.contains_key(&node2_id), "Node 2 should be in the peers list of node 1");
            
            let peers2 = transport2.get_peers_lock();
            let peers2_guard = peers2.lock();
            println!("Node 2 peers: {:?}", peers2_guard.keys().collect::<Vec<_>>());
            assert!(peers2_guard.contains_key(&node1_id), "Node 1 should be in the peers list of node 2");
        }
        
        println!("Establishing connections between nodes");
        // 建立连接 from node 1 to node 2
        match transport1.connect_to_peer(node2_addr).await {
            Ok(_) => println!("Successfully connected from node 1 to node 2"),
            Err(e) => panic!("Failed to connect from node 1 to node 2: {:?}", e),
        }
        
        // 给一些时间建立连接
        println!("Waiting for connections to establish...");
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // Check peer lists after connection
        println!("Node 1 peers after connection: {:?}", transport1.get_peer_list());
        println!("Node 2 peers after connection: {:?}", transport2.get_peer_list());
        
        // 创建测试消息从节点1到节点2
        let message1to2 = NetworkTestMessage {
            id: 2,
            content: "Message from node 1 to node 2".to_string(),
        };
        
        // 序列化消息
        println!("Serializing message from node 1 to node 2");
        let serializer = actix_cluster::serialization::BincodeSerializer::new();
        let payload1to2 = serializer.serialize(&message1to2).expect("Failed to serialize message1to2");
        
        // 创建消息信封
        println!("Creating envelope from node 1 to node 2");
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
        match transport1.send_envelope(envelope1to2).await {
            Ok(_) => println!("Successfully sent envelope from node 1 to node 2"),
            Err(e) => panic!("Failed to send envelope1to2: {:?}", e),
        }
        
        // 等待消息处理
        println!("Waiting for node 2 to receive message...");
        for i in 0..30 {
            if node2_received.load(Ordering::SeqCst) {
                println!("Node 2 received message after {} attempts", i + 1);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            if (i + 1) % 10 == 0 {
                println!("Still waiting for node 2 to receive message... ({} attempts)", i + 1);
            }
        }
        
        // 创建测试消息从节点2到节点1
        let message2to1 = NetworkTestMessage {
            id: 3,
            content: "Message from node 2 to node 1".to_string(),
        };
        
        // 序列化消息
        println!("Serializing message from node 2 to node 1");
        let payload2to1 = serializer.serialize(&message2to1).expect("Failed to serialize message2to1");
        
        // 创建消息信封
        println!("Creating envelope from node 2 to node 1");
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
        match transport2.send_envelope(envelope2to1).await {
            Ok(_) => println!("Successfully sent envelope from node 2 to node 1"),
            Err(e) => panic!("Failed to send envelope2to1: {:?}", e),
        }
        
        // 等待消息处理
        println!("Waiting for node 1 to receive message...");
        for i in 0..30 {
            if node1_received.load(Ordering::SeqCst) {
                println!("Node 1 received message after {} attempts", i + 1);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            if (i + 1) % 10 == 0 {
                println!("Still waiting for node 1 to receive message... ({} attempts)", i + 1);
            }
        }
        
        // 验证两个节点都收到了消息
        assert!(node2_received.load(Ordering::SeqCst), "Message was not received by node 2");
        assert!(node1_received.load(Ordering::SeqCst), "Message was not received by node 1");
        
        println!("Bidirectional communication test passed!");
    }).await;
} 