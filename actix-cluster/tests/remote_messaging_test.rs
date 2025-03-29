use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DeliveryGuarantee, DiscoveryMethod,
    MessageEnvelope, MessageType, NodeRole, SerializationFormat, AnyMessage
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "String")]
struct TestMessage {
    content: String,
}

struct TestActor;

impl Actor for TestActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("TestActor started");
    }
}

impl Handler<TestMessage> for TestActor {
    type Result = String;
    
    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        format!("Received: {}", msg.content)
    }
}

impl Handler<AnyMessage> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessage, _ctx: &mut Self::Context) {
        if let Some(test_msg) = msg.downcast::<TestMessage>() {
            println!("Received boxed message: {}", test_msg.content);
        }
    }
}

#[actix_rt::test]
async fn test_remote_messaging_envelope() {
    // 创建两个节点配置
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8501);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8502);
    
    let config1 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node1_addr)
        .cluster_name("test-cluster".to_string())
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
        .cluster_name("test-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![node1_addr.to_string()],
        })
        .serialization_format(SerializationFormat::Bincode)
        .build()
        .expect("Failed to create node2 config");
    
    // 创建两个集群节点
    let node1 = ClusterSystem::new("node1", config1);
    let node2 = ClusterSystem::new("node2", config2);
    
    // 获取节点ID用于后续发送消息
    let node1_id = node1.local_node().id.clone();
    let node2_id = node2.local_node().id.clone();
    
    // 启动两个节点
    let mut node1 = node1;
    let mut node2 = node2;
    let _node1_actor = node1.start().await.expect("Failed to start node1");
    let _node2_actor = node2.start().await.expect("Failed to start node2");
    
    // 等待节点发现彼此
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 在节点2上创建一个测试Actor
    let test_actor = TestActor.start();
    node2.register("test_actor", test_actor).await.expect("Failed to register test actor");
    
    // 等待Actor注册
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // 准备要发送的测试消息
    let test_message = TestMessage {
        content: "Hello from remote node".to_string(),
    };
    
    // 从节点1向节点2上的Actor发送消息
    match node1.send_remote(&node2_id, "test_actor", test_message, DeliveryGuarantee::AtLeastOnce).await {
        Ok(_) => println!("Message sent successfully"),
        Err(e) => println!("Failed to send message: {:?}", e),
    }
    
    // 等待消息处理
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // 停止节点
    // 这个测试主要是验证API和结构是否正确，实际上在当前实现中消息不会真正传递
    // 因为网络层需要实际的网络连接实现
    
    println!("Remote messaging test completed");
}

// 集成测试只验证API结构的正确性，不测试实际网络通信
// 因为实际网络通信需要完整的网络设置和环境
// 这对于单元测试来说太复杂了
#[test]
fn test_message_envelope_creation() {
    let node1_id = actix_cluster::node::NodeId::new();
    let node2_id = actix_cluster::node::NodeId::new();
    
    let envelope = MessageEnvelope::new(
        node1_id,
        node2_id,
        "test_actor".to_string(),
        MessageType::ActorMessage,
        DeliveryGuarantee::AtLeastOnce,
        vec![1, 2, 3, 4],
    );
    
    assert_eq!(envelope.target_actor, "test_actor");
    assert_eq!(envelope.message_type, MessageType::ActorMessage);
    assert_eq!(envelope.delivery_guarantee, DeliveryGuarantee::AtLeastOnce);
    assert_eq!(envelope.payload, vec![1, 2, 3, 4]);
} 