use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use actix::prelude::*;
use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DeliveryGuarantee, DiscoveryMethod,
    MessageEnvelope, MessageType, NodeRole, SerializationFormat, AnyMessage,
};
use serde::{Deserialize, Serialize};

// 定义测试消息类型
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "String")]
struct NetworkTestMessage {
    content: String,
}

// 测试Actor接收网络消息
struct NetworkTestActor;

impl Actor for NetworkTestActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("NetworkTestActor started");
    }
}

impl Handler<NetworkTestMessage> for NetworkTestActor {
    type Result = String;
    
    fn handle(&mut self, msg: NetworkTestMessage, _ctx: &mut Self::Context) -> Self::Result {
        println!("NetworkTestActor received: {}", msg.content);
        format!("Received: {}", msg.content)
    }
}

impl Handler<AnyMessage> for NetworkTestActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessage, _ctx: &mut Self::Context) {
        if let Some(test_msg) = msg.downcast::<NetworkTestMessage>() {
            println!("NetworkTestActor received boxed message: {}", test_msg.content);
        } else {
            println!("NetworkTestActor received unknown message type");
        }
    }
}

#[actix_rt::test]
async fn test_network_transport() {
    // 创建两个节点配置，使用不同的端口
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9501);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9502);
    
    let config1 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node1_addr)
        .cluster_name("test-network-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![node2_addr.to_string()],
        })
        .serialization_format(SerializationFormat::Bincode)
        .build()
        .expect("Failed to create node1 config");
    
    let config2 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node2_addr)
        .cluster_name("test-network-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![node1_addr.to_string()],
        })
        .serialization_format(SerializationFormat::Bincode)
        .build()
        .expect("Failed to create node2 config");
    
    // 创建两个集群节点
    let mut node1 = ClusterSystem::new("network-node1", config1);
    let mut node2 = ClusterSystem::new("network-node2", config2);
    
    // 获取节点ID
    let node1_id = node1.local_node().id.clone();
    let node2_id = node2.local_node().id.clone();
    
    println!("Starting nodes...");
    
    // 启动两个节点
    let _node1_actor = node1.start().await.expect("Failed to start node1");
    let _node2_actor = node2.start().await.expect("Failed to start node2");
    
    println!("Nodes started, waiting for discovery...");
    
    // 等待节点发现彼此
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 在节点2上创建一个测试Actor
    let test_actor = NetworkTestActor.start();
    let actor_path = "network_test_actor";
    
    println!("Registering test actor on node2...");
    
    // 注册测试Actor
    node2.register(actor_path, test_actor).await.expect("Failed to register test actor");
    
    // 等待Actor注册
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    println!("Sending test message from node1 to node2...");
    
    // 从节点1向节点2上的Actor发送消息
    let test_message = NetworkTestMessage {
        content: "Hello from network transport test".to_string(),
    };
    
    match node1.send_remote(&node2_id, actor_path, test_message, DeliveryGuarantee::AtLeastOnce).await {
        Ok(_) => println!("Message sent successfully"),
        Err(e) => println!("Failed to send message: {:?}", e),
    }
    
    // 等待消息处理
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    println!("Network transport test completed");
} 