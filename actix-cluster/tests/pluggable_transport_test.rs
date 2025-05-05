use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use actix::prelude::*;
use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DeliveryGuarantee, DiscoveryMethod,
    MessageEnvelope, MessageType, NodeRole, SerializationFormat, AnyMessage,
    TransportType, TransportConfig, create_transport,
};
use serde::{Deserialize, Serialize};

// 定义测试消息类型
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "String")]
struct PluggableTransportTestMessage {
    content: String,
}

// 测试Actor接收网络消息
struct PluggableTransportTestActor;

impl Actor for PluggableTransportTestActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("PluggableTransportTestActor started");
    }
}

impl Handler<PluggableTransportTestMessage> for PluggableTransportTestActor {
    type Result = String;

    fn handle(&mut self, msg: PluggableTransportTestMessage, _ctx: &mut Self::Context) -> Self::Result {
        println!("PluggableTransportTestActor received: {}", msg.content);
        format!("Received: {}", msg.content)
    }
}

impl Handler<AnyMessage> for PluggableTransportTestActor {
    type Result = ();

    fn handle(&mut self, msg: AnyMessage, _ctx: &mut Self::Context) {
        if let Some(test_msg) = msg.downcast::<PluggableTransportTestMessage>() {
            println!("PluggableTransportTestActor received boxed message: {}", test_msg.content);
        } else {
            println!("PluggableTransportTestActor received unknown message type");
        }
    }
}

#[actix_rt::test]
async fn test_pluggable_transport() {
    // 创建两个节点配置，使用不同的端口
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9601);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9602);

    let config1 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node1_addr)
        .cluster_name("test-pluggable-transport-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![node2_addr.to_string()],
        })
        .serialization_format(SerializationFormat::Bincode)
        .transport_type(TransportType::TCP) // 使用TCP传输
        .build()
        .expect("Failed to create node1 config");

    let config2 = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node2_addr)
        .cluster_name("test-pluggable-transport-cluster".to_string())
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .discovery(DiscoveryMethod::Static {
            seed_nodes: vec![node1_addr.to_string()],
        })
        .serialization_format(SerializationFormat::Bincode)
        .transport_type(TransportType::TCP) // 使用TCP传输
        .build()
        .expect("Failed to create node2 config");

    // 创建两个集群节点
    let mut node1 = ClusterSystem::new(config1);
    let mut node2 = ClusterSystem::new(config2);

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
    let test_actor = PluggableTransportTestActor.start();
    let actor_path = "pluggable_transport_test_actor";

    println!("Registering test actor on node2...");

    // 注册测试Actor
    node2.register(actor_path, test_actor).await.expect("Failed to register test actor");

    // 等待Actor注册
    tokio::time::sleep(Duration::from_secs(1)).await;

    println!("Sending test message from node1 to node2...");

    // 从节点1向节点2上的Actor发送消息
    let test_message = PluggableTransportTestMessage {
        content: "Hello from pluggable transport test".to_string(),
    };

    match node1.send_remote(&node2_id, actor_path, test_message, DeliveryGuarantee::AtLeastOnce).await {
        Ok(_) => println!("Message sent successfully"),
        Err(e) => println!("Failed to send message: {:?}", e),
    }

    // 等待消息处理
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("Pluggable transport test completed");
}

// 这个测试会尝试使用不支持的传输类型，应该会返回错误
#[actix_rt::test]
async fn test_unsupported_transport() {
    let node_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9603);
    
    // 创建一个使用不支持的传输类型的配置
    let config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(node_addr)
        .cluster_name("test-unsupported-transport".to_string())
        .transport_type(TransportType::UDP) // 使用尚未实现的UDP传输
        .build()
        .expect("Failed to create config");
    
    // 创建节点信息
    let node_info = config.build_node_info();
    
    // 创建传输配置
    let transport_config = TransportConfig::new(
        TransportType::UDP,
        node_addr,
    );
    
    // 尝试创建传输实例，应该会返回错误
    let transport_result = create_transport(transport_config, node_info);
    
    // 验证返回了预期的错误
    assert!(transport_result.is_err());
    match transport_result {
        Err(e) => {
            println!("Got expected error: {:?}", e);
            assert!(format!("{:?}", e).contains("UnsupportedTransport"));
        },
        Ok(_) => panic!("Expected error but got success"),
    }
}
