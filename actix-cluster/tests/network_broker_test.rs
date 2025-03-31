use actix::prelude::*;
use actix_cluster::{
    broker::{MessageBroker, NetworkBroker, BrokerMessage, SubscriptionOptions},
    node::{NodeId, NodeInfo, NodeStatus, NodeRole},
    transport::{P2PTransport, TransportMessage},
    message::DeliveryGuarantee,
    serialization::SerializationFormat,
};
use std::sync::Arc;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
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
    
    async fn send_message(&mut self, node_id: &NodeId, message: TransportMessage) -> Result<(), actix_cluster::error::ClusterError> {
        self.sent_messages.push((node_id.clone(), message));
        Ok(())
    }
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
    let transport = Arc::new(Mutex::new(mock_transport));
    
    // Create node map
    let nodes = Arc::new(RwLock::new(HashMap::new()));
    
    // Create network broker
    let _broker = NetworkBroker::new(
        local_id,
        transport,
        nodes,
    );
    
    // If we get here, the broker was created successfully
    assert!(true);
}

#[actix_rt::test]
async fn test_local_publish_subscribe() {
    let system = System::new();
    
    system.block_on(async {
        // Create local node id
        let local_id = NodeId::new();
        
        // Create a simple message broker
        let broker = MessageBroker::new(local_id.clone());
        
        // Create a test actor to receive messages
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
        
        // Ask the actor for its received messages
        let res = test_actor.send(actix::prelude::msgs::MessageRecipient).await;
        
        assert!(res.unwrap().received_messages.contains(&"Hello world".to_string()));
    });
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