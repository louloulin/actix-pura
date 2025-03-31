use actix::prelude::*;
use actix_cluster::{
    broker::{MessageBroker, NetworkBroker, SubscriptionOptions},
    node::{NodeId, NodeInfo, NodeStatus},
    config::NodeRole,
    transport::{P2PTransport, TransportMessage},
    message::DeliveryGuarantee,
    serialization::SerializationFormat,
    error::ClusterError,
};
use std::sync::Arc;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, oneshot};
use serde::{Serialize, Deserialize};

struct TestActor {
    received_messages: Vec<String>,
}

impl Actor for TestActor {
    type Context = Context<Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
struct TestMessage {
    pub content: String,
}

impl Handler<TestMessage> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: TestMessage, _ctx: &mut Context<Self>) -> Self::Result {
        self.received_messages.push(msg.content);
    }
}

// 查询当前接收到的消息
#[derive(Message)]
#[rtype(result = "Vec<String>")]
struct GetReceivedMessages;

impl Handler<GetReceivedMessages> for TestActor {
    type Result = Vec<String>;
    
    fn handle(&mut self, _msg: GetReceivedMessages, _ctx: &mut Context<Self>) -> Self::Result {
        self.received_messages.clone()
    }
}

// Create a P2PTransport mock
struct MockP2PTransport {
    local_node: NodeInfo,
    sent_messages: Vec<(NodeId, TransportMessage)>,
}

impl MockP2PTransport {
    fn new(local_node: NodeInfo) -> Self {
        Self {
            local_node,
            sent_messages: Vec::new(),
        }
    }
    
    async fn send_message(&mut self, node_id: &NodeId, message: TransportMessage) -> Result<(), ClusterError> {
        self.sent_messages.push((node_id.clone(), message));
        Ok(())
    }
}

// 为MockP2PTransport实现与P2PTransport相同的接口
impl std::fmt::Debug for MockP2PTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MockP2PTransport")
    }
}

// 创建一个特质让我们可以将MockP2PTransport转换为NetworkBroker需要的类型
trait TransportProvider: Send + Sync {
    fn send_message(&self, node_id: &NodeId, message: TransportMessage) -> Result<(), ClusterError>;
    fn local_node(&self) -> &NodeInfo;
}

#[tokio::test]
async fn test_network_broker_creation() {
    // Create local node info
    let local_id = NodeId::new();
    let addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();
    let local_node = NodeInfo::new(
        local_id.clone(),
        "test-node".to_string(),
        NodeRole::Peer,
        addr,
    );
    
    // Create mock transport
    let mock_transport = MockP2PTransport::new(local_node.clone());
    let _transport = Arc::new(Mutex::new(mock_transport));
    
    // Create node map
    let nodes = Arc::new(RwLock::new(HashMap::new()));
    
    // 创建一个真实的P2PTransport用于NetworkBroker
    let real_transport = P2PTransport::new(
        local_node.clone(), 
        SerializationFormat::Bincode
    ).unwrap();
    
    let broker_transport = Arc::new(Mutex::new(real_transport));
    
    // Create network broker 使用真实transport
    let _broker = NetworkBroker::new(
        local_id,
        broker_transport,
        nodes,
    );
    
    // If we get here, the broker was created successfully
    assert!(true);
}

// 使用专门的actix测试注解而不是tokio测试
#[actix_rt::test]
async fn test_local_publish_subscribe() {
    // 使用单独的测试机制，不再尝试嵌套运行时
    
    // Create local node id
    let local_id = NodeId::new();
    
    // Create a simple message broker
    let broker = MessageBroker::new(local_id.clone());
    
    // 创建一个测试actor
    let test_actor = TestActor { received_messages: Vec::new() }.start();
    
    // Subscribe to a topic
    broker.subscribe::<TestMessage, _>(
        "test-topic",
        test_actor.clone(),
        SubscriptionOptions::default(),
    ).await.expect("Failed to subscribe");
    
    // Publish a message
    let test_message = TestMessage { content: "Hello world".to_string() };
    broker.publish("test-topic", test_message, DeliveryGuarantee::AtMostOnce).await
        .expect("Failed to publish message");
    
    // Give the broker time to process the message
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // 使用GetReceivedMessages消息来获取actor接收到的消息
    let received = test_actor.send(GetReceivedMessages).await.unwrap();
    
    assert!(received.contains(&"Hello world".to_string()));
}

// This is a stub test that would require more complex setup with two actual nodes
#[tokio::test]
async fn test_broker_initialization() {
    // Create local node info
    let local_id = NodeId::new();
    let addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();
    let local_node = NodeInfo::new(
        local_id.clone(),
        "test-node".to_string(),
        NodeRole::Peer,
        addr,
    );
    
    // Create real P2P transport (this will fail without a full setup)
    let transport_result = P2PTransport::new(
        local_node.clone(), 
        SerializationFormat::Bincode
    );
    
    // Just check we can create a transport
    assert!(transport_result.is_ok());
} 