use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DeliveryGuarantee, DiscoveryMethod,
    MessageEnvelope, MessageType, NodeRole, SerializationFormat, AnyMessage,
    compression::{CompressionAlgorithm, CompressionConfig, CompressionLevel},
    node::NodeId,
    transport::P2PTransport,
    testing,
};
use serde::{Deserialize, Serialize};
use log::info;

// 定义一个大型消息，包含5MB的数据
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "()")]
struct LargeMessage {
    data: Vec<u8>,           // 5MB的数据
    sender: String,          // 发送者标识
    timestamp: u64,          // 发送时间戳（毫秒）
}

// 定义一个测试Actor
struct TestActor;

impl Actor for TestActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("TestActor started");
    }
}

// 处理LargeMessage
impl Handler<LargeMessage> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: LargeMessage, _ctx: &mut Self::Context) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let duration = now - msg.timestamp;
        
        println!("Received message from {}: {} bytes, took {} ms", 
            msg.sender, 
            msg.data.len(), 
            duration
        );
    }
}

// 处理任意消息
impl Handler<AnyMessage> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessage, _ctx: &mut Self::Context) {
        if let Some(large_msg) = msg.downcast::<LargeMessage>() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let duration = now - large_msg.timestamp;
            
            println!("Received AnyMessage from {}: {} bytes, took {} ms", 
                large_msg.sender, 
                large_msg.data.len(), 
                duration
            );
        }
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    
    // 创建两个节点配置
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8701);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8702);
    
    // 节点1：启用压缩功能
    let config1 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node1_addr)
        .cluster_name("compression-test-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .serialization_format(SerializationFormat::Bincode)
        .with_compression_config(CompressionConfig {
            enabled: true,
            algorithm: CompressionAlgorithm::Gzip,
            level: CompressionLevel::Default,
            min_size_threshold: 1024, // 对大于1KB的消息进行压缩
        })
        .build()
        .expect("Failed to create compression node config");
    
    // 节点2：不启用压缩功能
    let config2 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node2_addr)
        .cluster_name("compression-test-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .serialization_format(SerializationFormat::Bincode)
        .build()
        .expect("Failed to create regular node config");
    
    // 创建集群系统
    let system1 = ClusterSystem::new(config1);
    let system2 = ClusterSystem::new(config2);
    
    // 获取节点ID和信息
    let node1_id = system1.local_node().id.clone();
    let node2_id = system2.local_node().id.clone();
    let node1_info = system1.local_node().clone();
    let node2_info = system2.local_node().clone();
    
    println!("Node 1 (with compression) ID: {}", node1_id);
    println!("Node 2 (without compression) ID: {}", node2_id);
    
    // 启动集群系统
    let mut system1 = system1;
    let mut system2 = system2;
    system1.start().await.expect("Failed to start node1");
    system2.start().await.expect("Failed to start node2");
    
    // 等待节点启动
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // 手动添加对等节点信息
    {
        // 获取transport锁
        let transport1 = system1.transport.as_ref().unwrap();
        let transport2 = system2.transport.as_ref().unwrap();
        
        // 解锁后获取可变引用
        let mut transport1_guard = transport1.lock().await;
        let mut transport2_guard = transport2.lock().await;
        
        // 使用testing模块的add_peer_for_test方法添加对方节点
        testing::add_peer_for_test(&mut transport1_guard, node2_id.clone(), node2_info.clone());
        testing::add_peer_for_test(&mut transport2_guard, node1_id.clone(), node1_info.clone());
        
        println!("Peers manually added to each node");
        
        // 确认对方节点已添加
        let node1_peers = transport1_guard.peers_lock_for_testing();
        let node2_peers = transport2_guard.peers_lock_for_testing();
        println!("Node 1 peers: {}", node1_peers.keys().map(|id| id.to_string()).collect::<Vec<_>>().join(", "));
        println!("Node 2 peers: {}", node2_peers.keys().map(|id| id.to_string()).collect::<Vec<_>>().join(", "));
    }
    
    // 在节点上注册Actor
    let test_actor1 = TestActor.start();
    let test_actor2 = TestActor.start();
    system1.register("test_actor", test_actor1).await.expect("Failed to register actor on node1");
    system2.register("test_actor", test_actor2).await.expect("Failed to register actor on node2");
    
    println!("Test actors registered on both nodes");
    
    // 等待Actor注册完成
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // 创建一个大型消息，5MB数据
    let large_data = vec![0u8; 5 * 1024 * 1024]; // 5MB的数据
    
    // 测试从压缩节点到普通节点的消息发送
    println!("\n--- Test 1: Sending large message from compression-enabled node to regular node ---");
    let msg1 = LargeMessage {
        data: large_data.clone(),
        sender: "Compression Node".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };
    
    match system1.send_remote(&node2_id, "test_actor", msg1, DeliveryGuarantee::AtLeastOnce).await {
        Ok(_) => println!("Message sent from compression node to regular node"),
        Err(e) => println!("Failed to send message: {:?}", e),
    }
    
    // 等待消息处理
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 测试从普通节点到压缩节点的消息发送
    println!("\n--- Test 2: Sending large message from regular node to compression-enabled node ---");
    let msg2 = LargeMessage {
        data: large_data.clone(),
        sender: "Regular Node".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };
    
    match system2.send_remote(&node1_id, "test_actor", msg2, DeliveryGuarantee::AtLeastOnce).await {
        Ok(_) => println!("Message sent from regular node to compression node"),
        Err(e) => println!("Failed to send message: {:?}", e),
    }
    
    // 等待消息处理
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    println!("Compression test completed");
    
    Ok(())
} 