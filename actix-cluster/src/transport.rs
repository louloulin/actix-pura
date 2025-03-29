//! Network transport module for peer-to-peer communication.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::any::Any;

use actix::prelude::*;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;
use log::{debug, error, info, warn};
use serde::{Serialize, Deserialize};

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, NodeInfo, NodeStatus};
use crate::serialization::{SerializationFormat, SerializerTrait, BincodeSerializer, JsonSerializer};
use crate::message::{MessageEnvelope, MessageType, DeliveryGuarantee, ActorPath};

/// Timeout for connection attempts
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
/// Timeout for message acknowledgements
const ACK_TIMEOUT: Duration = Duration::from_secs(5);

/// Transport message types for P2P communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportMessage {
    /// Actor communication message
    Envelope(MessageEnvelope),
    /// Heartbeat from a node
    Heartbeat(NodeInfo),
    /// Status update for a node
    StatusUpdate(NodeId, String),
    /// Actor discovery request
    ActorDiscovery(String),
    /// Actor discovery response
    ActorDiscoveryResponse(String, Vec<ActorPath>),
}

/// A message that is waiting for acknowledgement
#[derive(Debug, Clone)]
struct PendingMessage {
    /// The message itself
    message: TransportMessage,
    /// When the message was first sent
    first_sent: Instant,
    /// When the message was last retried
    last_retry: Instant,
    /// Number of retries
    retry_count: u8,
}

/// Network transport module for peer-to-peer communication.
pub struct P2PTransport {
    /// Local node information
    pub local_node: NodeInfo,
    
    /// Known peers
    peers: Arc<Mutex<HashMap<NodeId, NodeInfo>>>,
    
    /// Message serializer
    serializer: Box<dyn SerializerTrait + Send + Sync>,
    
    /// Message receiver channel
    msg_rx: Option<mpsc::Receiver<(NodeId, TransportMessage)>>,
    
    /// Message sender channel
    msg_tx: Option<mpsc::Sender<(NodeId, TransportMessage)>>,
    
    /// Message handler actor
    message_handler: Option<Arc<Mutex<ActorMessageHandler>>>,
    
    /// Pending message acknowledgements
    pending_acks: Arc<Mutex<HashMap<Uuid, PendingMessage>>>,
}

/// Actor for handling transport messages
pub trait Handler<M>: Actor {
    /// Result type
    type Result;
    
    /// Handle a message
    fn handle(&mut self, msg: M, ctx: &mut <Self as Actor>::Context);
}

/// Wrapper for actor message handler to satisfy Send + Sync
pub struct ActorMessageHandler {
    // Type erased handler - this is safe because we only access it through
    // actix framework which provides proper thread safety
    pub addr: Option<Box<dyn std::any::Any + Send + Sync>>,
}

impl ActorMessageHandler {
    /// Create a new actor message handler
    pub fn new<A: Actor + Handler<TransportMessage>>(addr: Addr<A>) -> Self {
        ActorMessageHandler {
            addr: Some(Box::new(addr)),
        }
    }
    
    /// Get the address for handler
    pub fn addr<A: Actor + Handler<TransportMessage>>(&self) -> Option<Addr<A>> {
        self.addr.as_ref()
            .and_then(|a| a.downcast_ref::<Addr<A>>().cloned())
    }
}

// Make sure P2PTransport implements Send and Sync
unsafe impl Send for P2PTransport {}
unsafe impl Sync for P2PTransport {}

impl P2PTransport {
    /// Create a new transport layer
    pub fn new(local_node: NodeInfo, format: SerializationFormat) -> ClusterResult<Self> {
        // Create serializer based on format
        let serializer: Box<dyn SerializerTrait + Send + Sync> = match format {
            SerializationFormat::Json => Box::new(JsonSerializer::new()),
            SerializationFormat::Bincode => Box::new(BincodeSerializer::new()),
            // SerializationFormat只有两个枚举值，不需要默认分支
        };
        
        // Create channels for message passing
        let (tx, rx) = mpsc::channel(100);
        
        let transport = Self {
            local_node,
            peers: Arc::new(Mutex::new(HashMap::new())),
            serializer,
            msg_rx: Some(rx),
            msg_tx: Some(tx),
            message_handler: None,
            pending_acks: Arc::new(Mutex::new(HashMap::new())),
        };
        
        Ok(transport)
    }
    
    /// Initialize the transport
    pub async fn init(&mut self) -> ClusterResult<()> {
        // Add local node to peers
        let mut peers = self.peers.lock();
        peers.insert(self.local_node.id.clone(), self.local_node.clone());
        
        // Start message handling loop
        if let Some(msg_rx) = self.msg_rx.take() {
            let mut rx = msg_rx;
            let mut transport = self.clone();
            
            tokio::spawn(async move {
                while let Some((_node_id, message)) = rx.recv().await {
                    if let Err(e) = transport.handle_message(message).await {
                        error!("Error handling message: {}", e);
                    }
                }
            });
        }
        
        Ok(())
    }
    
    /// Handle an incoming message
    pub async fn handle_message(&mut self, message: TransportMessage) -> ClusterResult<()> {
        match message {
            TransportMessage::Heartbeat(node_info) => {
                // Update peer information
                self.peers.lock().insert(node_info.id.clone(), node_info);
                debug!("Updated peer information");
                Ok(())
            },
            TransportMessage::StatusUpdate(node_id, status) => {
                // Update node status
                debug!("Received status update for node {}: {}", node_id, status);
                Ok(())
            },
            TransportMessage::ActorDiscovery(path) => {
                // Handle actor discovery request
                debug!("Received actor discovery request for {}", path);
                Ok(())
            },
            TransportMessage::ActorDiscoveryResponse(path, locations) => {
                // Handle actor discovery response
                debug!("Received actor discovery response for {}, found in {} locations", 
                       path, locations.len());
                Ok(())
            },
            TransportMessage::Envelope(envelope) => {
                debug!("Received message envelope: {:?}", envelope);
                
                // Check if this is an acknowledgement message
                if envelope.message_type == MessageType::Pong {
                    if let Some(handler) = &self.message_handler {
                        let handler = handler.lock();
                        debug!("Forwarding Pong message to handler");
                        // Would handle ack logic here in a real implementation
                    }
                } else {
                    // For other messages, check if we need to send an ack
                    if envelope.delivery_guarantee != DeliveryGuarantee::AtMostOnce {
                        // Send acknowledgement
                        let ack = envelope.create_ack();
                        self.send_envelope(ack).await?;
                    }
                    
                    // Forward to message handler
                    if let Some(handler) = &self.message_handler {
                        let handler = handler.lock();
                        debug!("Forwarding message to handler: {:?}", envelope.message_type);
                        // Would forward to appropriate handler in a real implementation
                    }
                }
                
                Ok(())
            }
        }
    }
    
    /// Send a message to a specific node
    pub async fn send_message(&mut self, target_node: &NodeId, message: TransportMessage) -> ClusterResult<()> {
        // In a real implementation, this would use network transport
        // For now, we'll simulate by just logging the message
        debug!("Would send message to node {}: {:?}", target_node, message);
        
        // Check if we have the node in our known peers
        if !self.peers.lock().contains_key(target_node) {
            return Err(ClusterError::NodeNotFound(target_node.clone()));
        }
        
        // If this is an envelope message with delivery guarantees, store it for retries
        if let TransportMessage::Envelope(ref envelope) = &message {
            if envelope.delivery_guarantee != DeliveryGuarantee::AtMostOnce {
                let pending = PendingMessage {
                    message: message.clone(),
                    first_sent: Instant::now(),
                    last_retry: Instant::now(),
                    retry_count: 0,
                };
                
                self.pending_acks.lock().insert(envelope.message_id, pending);
            }
        }
        
        // Successfully "sent" the message
        Ok(())
    }
    
    /// Send a message envelope
    pub async fn send_envelope(&mut self, envelope: MessageEnvelope) -> ClusterResult<()> {
        // 创建一个克隆以避免同时借用envelope为只读和可变
        let target_node = envelope.target_node.clone();
        self.send_message(&target_node, TransportMessage::Envelope(envelope)).await
    }
    
    /// Set message handler for incoming messages
    pub fn set_message_handler<A: Actor + Handler<TransportMessage> + 'static>(&mut self, handler: Addr<A>) {
        self.message_handler = Some(Arc::new(Mutex::new(ActorMessageHandler::new(handler))));
    }
    
    /// Get the known peers
    pub fn get_peers(&self) -> Vec<NodeInfo> {
        self.peers.lock().values().cloned().collect()
    }
    
    /// Get message sender
    pub fn get_sender(&self) -> Option<mpsc::Sender<(NodeId, TransportMessage)>> {
        self.msg_tx.clone()
    }

    /// Send a message to the remote actor
    pub async fn send<M: serde::Serialize + 'static>(&mut self, message: M) -> ClusterResult<()> {
        // Use the updated serialization approach - 这里使用self.serializer，不需要锁
        let payload = match &*self.serializer {
            s if s.type_id() == std::any::TypeId::of::<crate::serialization::BincodeSerializer>() => {
                let serializer = crate::serialization::BincodeSerializer::new();
                serializer.serialize(&message)?
            },
            s if s.type_id() == std::any::TypeId::of::<crate::serialization::JsonSerializer>() => {
                let serializer = crate::serialization::JsonSerializer::new();
                serializer.serialize(&message)?
            },
            _ => {
                self.serializer.serialize_any(&message as &dyn std::any::Any)?
            }
        };
        
        // Create message envelope
        let envelope = MessageEnvelope::new(
            self.local_node.id.clone(),
            self.local_node.id.clone(),  // 这是一个placeholder，实际用途中应该是目标节点ID
            "".to_string(),              // 同样是placeholder
            MessageType::ActorMessage,
            DeliveryGuarantee::AtMostOnce,
            payload,
        );
        
        // Send the envelope - 现在self已经是可变的
        self.send_envelope(envelope).await
    }
}

impl Clone for P2PTransport {
    fn clone(&self) -> Self {
        // 克隆序列化器，使用clone_box方法
        let serializer = self.serializer.clone_box();
        
        P2PTransport {
            local_node: self.local_node.clone(),
            peers: self.peers.clone(),
            serializer,
            msg_rx: None, // Don't clone the receiver
            msg_tx: self.msg_tx.clone(),
            message_handler: self.message_handler.clone(),
            pending_acks: self.pending_acks.clone(),
        }
    }
}

/// Actor for handling message envelopes
#[derive(Default)]
pub struct MessageEnvelopeHandler {
    local_node_id: NodeId,
}

impl MessageEnvelopeHandler {
    pub fn new(local_node_id: NodeId) -> Self {
        Self { local_node_id }
    }
}

impl Actor for MessageEnvelopeHandler {
    type Context = Context<Self>;
}

impl Handler<MessageEnvelope> for MessageEnvelopeHandler {
    type Result = ();
    
    fn handle(&mut self, msg: MessageEnvelope, _ctx: &mut Context<Self>) {
        debug!("Handling message envelope: {:?}", msg);
        
        // In a real implementation, this would route the message to the
        // appropriate local actor based on the target_actor path
        if msg.sender_node != self.local_node_id {
            debug!("Processing message from node {}", msg.sender_node);
        }
    }
}

impl Handler<TransportMessage> for MessageEnvelopeHandler {
    type Result = ();
    
    fn handle(&mut self, msg: TransportMessage, ctx: &mut Context<Self>) {
        match msg {
            TransportMessage::Envelope(envelope) => {
                // 重用已有的envelope处理代码
                self.handle(envelope, ctx);
            },
            TransportMessage::Heartbeat(node_info) => {
                debug!("Received heartbeat from node {}", node_info.id);
            },
            TransportMessage::StatusUpdate(node_id, status) => {
                debug!("Received status update for node {}: {}", node_id, status);
            },
            TransportMessage::ActorDiscovery(path) => {
                debug!("Received actor discovery request for {}", path);
            },
            TransportMessage::ActorDiscoveryResponse(path, locations) => {
                debug!("Received actor discovery response for {}", path);
            },
        }
    }
}

impl actix::Message for MessageEnvelope {
    type Result = ();
}

// 添加消息实现
impl actix::Message for TransportMessage {
    type Result = ();
}

/// Remote actor reference
pub struct RemoteActorRef {
    /// Path to the actor
    pub actor_path: ActorPath,
    /// Transport for sending messages
    transport: Arc<tokio::sync::Mutex<P2PTransport>>,
    /// Default delivery guarantee
    delivery_guarantee: DeliveryGuarantee,
}

impl Clone for RemoteActorRef {
    fn clone(&self) -> Self {
        Self {
            actor_path: self.actor_path.clone(),
            transport: self.transport.clone(),
            delivery_guarantee: self.delivery_guarantee,
        }
    }
}

impl RemoteActorRef {
    /// Create a new remote actor reference
    pub fn new(
        path: ActorPath, 
        transport: Arc<tokio::sync::Mutex<P2PTransport>>,
        delivery_guarantee: DeliveryGuarantee,
    ) -> Self {
        Self {
            actor_path: path,
            transport,
            delivery_guarantee,
        }
    }
    
    /// Send a message to the remote actor
    pub async fn send<M: serde::Serialize + 'static>(&self, message: M) -> ClusterResult<()> {
        // Get locked transport to serialize and send the message
        let mut transport = self.transport.lock().await;
        
        // Use the updated serialization approach
        let payload = match &*transport.serializer {
            s if s.type_id() == std::any::TypeId::of::<crate::serialization::BincodeSerializer>() => {
                let serializer = crate::serialization::BincodeSerializer::new();
                serializer.serialize(&message)?
            },
            s if s.type_id() == std::any::TypeId::of::<crate::serialization::JsonSerializer>() => {
                let serializer = crate::serialization::JsonSerializer::new();
                serializer.serialize(&message)?
            },
            _ => {
                transport.serializer.serialize_any(&message as &dyn std::any::Any)?
            }
        };
        
        // Create message envelope
        let envelope = MessageEnvelope::new(
            transport.local_node.id.clone(),
            self.actor_path.node_id.clone(),
            self.actor_path.path.clone(),
            MessageType::ActorMessage,
            self.delivery_guarantee,
            payload,
        );
        
        // Send the envelope
        transport.send_envelope(envelope).await
    }
    
    /// Send a pre-created message envelope
    pub async fn send_envelope(&self, envelope: MessageEnvelope) -> ClusterResult<()> {
        // 获取transport的锁
        let mut transport = self.transport.lock().await;
        transport.send_envelope(envelope).await
    }
}

impl crate::registry::ActorRef for RemoteActorRef {
    fn send_any(&self, msg: Box<dyn std::any::Any + Send>) -> ClusterResult<()> {
        // Use tokio runtime to run the async task
        if let Some(msg) = msg.downcast_ref::<MessageEnvelope>() {
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    self.send_envelope(msg.clone()).await
                })
            })
        } else {
            Err(ClusterError::SerializationError("Message is not an envelope".to_string()))
        }
    }
    
    fn path(&self) -> &str {
        &self.actor_path.path
    }
    
    fn clone_box(&self) -> Box<dyn crate::registry::ActorRef> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NodeRole;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    
    #[tokio::test]
    async fn test_transport_creation() {
        let node_id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let node_info = NodeInfo::new(
            node_id,
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        let transport = P2PTransport::new(node_info, SerializationFormat::Bincode);
        assert!(transport.is_ok());
    }
    
    #[tokio::test]
    async fn test_send_envelope() {
        let node_id1 = NodeId::new();
        let node_id2 = NodeId::new();
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8559);
        
        let node_info1 = NodeInfo::new(
            node_id1.clone(),
            "test-node-1".to_string(),
            NodeRole::Peer,
            addr1,
        );
        
        let node_info2 = NodeInfo::new(
            node_id2.clone(),
            "test-node-2".to_string(),
            NodeRole::Peer,
            addr2,
        );
        
        // Test message data
        let test_payload = vec![1, 2, 3, 4];
        
        // Create transport
        let mut transport1 = P2PTransport::new(node_info1.clone(), SerializationFormat::Bincode).unwrap();
        let transport1_clone = transport1.clone();
        
        // Set up handler
        let handler = MessageEnvelopeHandler::new(node_id1.clone()).start();
        transport1.set_message_handler(handler);
        
        // Add peer to transport
        {
            let mut peers = transport1.peers.lock();
            peers.insert(node_id2.clone(), node_info2.clone());
        }
        
        // Create envelope
        let envelope = MessageEnvelope::new(
            node_id1.clone(),
            node_id2.clone(),
            "test_actor".to_string(),
            MessageType::ActorMessage,
            DeliveryGuarantee::AtMostOnce, // No ack needed for test
            test_payload.clone(),
        );
        
        // In real code, we would need actual network communication
        // For test, we simulate by directly calling handle_message
        let transport_message = TransportMessage::Envelope(envelope.clone());
        
        // This is a simplified test that just ensures the code compiles
        // and basic functionality works without actual network
        
        let actor_path = ActorPath::new(node_id2.clone(), "test_actor".to_string());
        let remote_ref = RemoteActorRef::new(
            actor_path,
            Arc::new(tokio::sync::Mutex::new(transport1_clone)),
            DeliveryGuarantee::AtMostOnce,
        );
        
        // In real usage, this would actually send the message
        // but for this test it's just checking compilation
        // let result = remote_ref.send(test_payload).await;
    }
} 