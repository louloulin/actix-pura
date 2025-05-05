use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use actix_cluster::{
    NodeId, NodeInfo, NodeRole, SerializationFormat,
    TransportConfig, TransportType, create_transport,
};
use actix_cluster::transport::TransportMessage;
use actix_cluster::message::MessageEnvelopeHandler;
use actix_cluster::registry::ActorRegistry;
use actix_cluster::node::NodeStatus;

#[actix_rt::test]
async fn test_transport_trait_creation() {
    // 创建一个本地节点信息
    let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9701);
    let local_node = NodeInfo {
        id: NodeId::new(),
        name: "test-node".to_string(),
        addr: local_addr,
        role: NodeRole::Peer,
        status: NodeStatus::Joining,
        joined_at: None,
        capabilities: Vec::new(),
        load: 0,
        metadata: serde_json::Map::new(),
    };

    // 创建传输配置
    let transport_config = TransportConfig::new(
        TransportType::TCP,
        local_addr,
    )
    .with_serialization_format(SerializationFormat::Bincode);

    // 创建传输实例
    let transport_result = create_transport(transport_config, local_node.clone());

    // 验证传输实例创建成功
    assert!(transport_result.is_ok());

    let mut transport = transport_result.unwrap();

    // 验证传输类型
    assert_eq!(transport.transport_type(), TransportType::TCP);

    // 验证本地节点信息
    assert_eq!(transport.local_node().id, local_node.id);

    // 初始化传输
    let init_result = transport.init().await;
    assert!(init_result.is_ok());

    // 验证传输已启动
    assert!(transport.is_started());

    // 验证只有本地节点在连接列表中
    assert_eq!(transport.get_peers().len(), 1);
}

#[actix_rt::test]
async fn test_transport_trait_unsupported() {
    // 创建一个本地节点信息
    let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9702);
    let local_node = NodeInfo {
        id: NodeId::new(),
        name: "test-node".to_string(),
        addr: local_addr,
        role: NodeRole::Peer,
        status: NodeStatus::Joining,
        joined_at: None,
        capabilities: Vec::new(),
        load: 0,
        metadata: serde_json::Map::new(),
    };

    // 创建传输配置，使用不支持的传输类型
    let transport_config = TransportConfig::new(
        TransportType::UDP,
        local_addr,
    )
    .with_serialization_format(SerializationFormat::Bincode);

    // 创建传输实例
    let transport_result = create_transport(transport_config, local_node.clone());

    // 验证传输实例创建失败，返回UnsupportedTransport错误
    assert!(transport_result.is_err());
    match transport_result {
        Err(e) => {
            let error_string = format!("{:?}", e);
            assert!(error_string.contains("UnsupportedTransport"));
        },
        Ok(_) => panic!("Expected error but got success"),
    }
}

#[actix_rt::test]
async fn test_transport_trait_connection() {
    // 创建两个节点
    let node1_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9703);
    let node2_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9704);

    let node1 = NodeInfo {
        id: NodeId::new(),
        name: "node1".to_string(),
        addr: node1_addr,
        role: NodeRole::Peer,
        status: NodeStatus::Joining,
        joined_at: None,
        capabilities: Vec::new(),
        load: 0,
        metadata: serde_json::Map::new(),
    };

    let node2 = NodeInfo {
        id: NodeId::new(),
        name: "node2".to_string(),
        addr: node2_addr,
        role: NodeRole::Peer,
        status: NodeStatus::Joining,
        joined_at: None,
        capabilities: Vec::new(),
        load: 0,
        metadata: serde_json::Map::new(),
    };

    // 创建两个传输实例
    let transport1_config = TransportConfig::new(
        TransportType::TCP,
        node1_addr,
    )
    .with_serialization_format(SerializationFormat::Bincode);

    let transport2_config = TransportConfig::new(
        TransportType::TCP,
        node2_addr,
    )
    .with_serialization_format(SerializationFormat::Bincode);

    let mut transport1 = create_transport(transport1_config, node1.clone()).unwrap();
    let mut transport2 = create_transport(transport2_config, node2.clone()).unwrap();

    // 初始化传输
    transport1.init().await.unwrap();
    transport2.init().await.unwrap();

    // 创建消息处理器
    let handler1 = MessageEnvelopeHandler::new(node1.id.clone());
    let handler2 = MessageEnvelopeHandler::new(node2.id.clone());

    // 设置消息处理器
    transport1.set_message_handler(handler1.start());
    transport2.set_message_handler(handler2.start());

    // 设置注册表
    let registry1 = Arc::new(ActorRegistry::new(node1.id.clone()));
    let registry2 = Arc::new(ActorRegistry::new(node2.id.clone()));

    transport1.set_registry_adapter(registry1);
    transport2.set_registry_adapter(registry2);

    // 连接节点1到节点2
    let connect_result = transport1.connect_to_peer(node2_addr).await;
    assert!(connect_result.is_ok());

    // 等待连接建立
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 验证连接状态
    assert!(transport1.is_connected(&node2.id));

    // 发送消息
    let message = TransportMessage::Heartbeat(node1.clone());
    let send_result = transport1.send_message(&node2.id, message).await;
    assert!(send_result.is_ok());

    // 等待消息处理
    tokio::time::sleep(Duration::from_millis(500)).await;
}
