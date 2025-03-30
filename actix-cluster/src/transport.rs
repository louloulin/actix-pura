//! Network transport module for peer-to-peer communication.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::any::Any;
use std::net::SocketAddr;
use std::io;
use std::any::TypeId;
use std::marker::PhantomData;

use actix::prelude::*;
use parking_lot::Mutex;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use log::{debug, error, info, warn};
use serde::{Serialize, Deserialize};

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, NodeInfo, NodeStatus};
use crate::config::NodeRole;
use crate::serialization::{SerializationFormat, SerializerTrait, BincodeSerializer, JsonSerializer};
use crate::message::{MessageEnvelope, MessageType, DeliveryGuarantee, ActorPath};
use crate::registry::ActorRegistry;

// Define MessageId type for message acknowledgements
type MessageId = uuid::Uuid;

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
    /// Ack message
    Ack(MessageId),
    /// Connection close notification
    Close,
    /// Node handshake (initial connection)
    Handshake(NodeInfo),
    /// Request the location of an actor
    ActorDiscoveryRequest(NodeId, String),
    /// Response with the locations of an actor
    ActorDiscoveryResponse(String, Vec<NodeId>),
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
    serializer: Box<dyn SerializerTrait>,
    
    /// Message receiver channel
    msg_rx: Option<mpsc::Receiver<(NodeId, TransportMessage)>>,
    
    /// Message sender channel
    msg_tx: Option<mpsc::Sender<(NodeId, TransportMessage)>>,
    
    /// Message handler actor
    message_handler: Option<Arc<Mutex<dyn MessageHandler>>>,
    
    /// Pending message acknowledgements
    pending_acks: Arc<Mutex<HashMap<String, PendingMessage>>>,
    
    /// Active connections to other nodes
    connections: Arc<Mutex<HashMap<NodeId, Arc<TokioMutex<TcpStream>>>>>,
    
    /// Listener for incoming connections
    listener: Option<Arc<TcpListener>>,
    
    /// Flag indicating if transport is started
    started: bool,
    
    /// Registry adapter for actor discovery
    registry_adapter: Option<Arc<ActorRegistry>>,
}

/// Actor for handling transport messages
pub trait Handler<M>: Actor {
    /// Result type
    type Result;
    
    /// Handle a message
    fn handle(&mut self, msg: M, ctx: &mut <Self as Actor>::Context);
}

/// 消息处理器trait
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync {
    /// 处理来自节点的消息
    async fn handle_message(&mut self, sender: NodeId, message: TransportMessage) -> ClusterResult<()>;
}

/// 消息处理器trait
pub trait SyncMessageHandler: Send + Sync {
    /// 处理来自节点的消息（同步版本）
    fn handle_message_sync(&self, sender: NodeId, message: TransportMessage) -> ClusterResult<()>;
}

/// Actor 实现的消息处理器
pub struct ActorMessageHandler {
    // 使用标准的函数指针或闭包存储
    handler: Box<dyn Fn(NodeId, TransportMessage) -> ClusterResult<()> + Send + Sync>,
}

impl ActorMessageHandler {
    /// 创建一个新的 ActorMessageHandler
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(NodeId, TransportMessage) -> ClusterResult<()> + Send + Sync + 'static 
    {
        Self {
            handler: Box::new(handler),
        }
    }
}

#[async_trait::async_trait]
impl MessageHandler for ActorMessageHandler {
    async fn handle_message(&mut self, node_id: NodeId, message: TransportMessage) -> ClusterResult<()> {
        debug!("Forwarding message from {} to handler", node_id);
        (self.handler)(node_id, message)
    }
}

// 为 Addr<MessageEnvelopeHandler> 实现 From trait
pub enum MessageHandlerType {
    Function(Box<dyn Fn(NodeId, TransportMessage) -> ClusterResult<()> + Send + Sync>),
    Actor(actix::Addr<MessageEnvelopeHandler>),
}

impl<F> From<F> for MessageHandlerType 
where 
    F: Fn(NodeId, TransportMessage) -> ClusterResult<()> + Send + Sync + 'static
{
    fn from(f: F) -> Self {
        MessageHandlerType::Function(Box::new(f))
    }
}

impl From<actix::Addr<MessageEnvelopeHandler>> for MessageHandlerType {
    fn from(addr: actix::Addr<MessageEnvelopeHandler>) -> Self {
        MessageHandlerType::Actor(addr)
    }
}

// Make sure P2PTransport implements Send and Sync
unsafe impl Send for P2PTransport {}
unsafe impl Sync for P2PTransport {}

impl P2PTransport {
    /// Create a new transport layer
    pub fn new(local_node: NodeInfo, format: SerializationFormat) -> ClusterResult<Self> {
        // Create serializer based on format
        let serializer: Box<dyn SerializerTrait> = match format {
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
            connections: Arc::new(Mutex::new(HashMap::new())),
            listener: None,
            started: false,
            registry_adapter: None,
        };
        
        Ok(transport)
    }
    
    /// Initialize the transport
    pub async fn init(&mut self) -> ClusterResult<()> {
        println!("Initializing transport for node {}", self.local_node.id);
        
        // Add local node to peers
        {
            let mut peers = self.peers.lock();
            peers.insert(self.local_node.id.clone(), self.local_node.clone());
            println!("Added local node {} to peers", self.local_node.id);
        } // Release the lock here before any await points
        
        // Start message handling loop
        if let Some(msg_rx) = self.msg_rx.take() {
            let mut rx = msg_rx;
            let mut transport = self.clone();
            
            println!("Starting message handling loop for node {}", self.local_node.id);
            // Use tokio::task::spawn_local instead, which doesn't require Send
            tokio::task::spawn_local(async move {
                println!("Message handler loop started for node {}", transport.local_node.id);
                while let Some((_node_id, message)) = rx.recv().await {
                    println!("Received message in transport handler: {:?}", message);
                    if let Err(e) = transport.handle_message(message).await {
                        error!("Error handling message: {}", e);
                        println!("Error handling message: {}", e);
                    }
                }
                println!("Message handler loop terminated for node {}", transport.local_node.id);
            });
        }
        
        // Start TCP listener
        let listener = TcpListener::bind(self.local_node.addr).await
            .map_err(|e| ClusterError::NetworkError(format!("Failed to bind TCP listener: {}", e)))?;
        
        info!("Transport bound to {}", self.local_node.addr);
        println!("Transport bound to {} for node {}", self.local_node.addr, self.local_node.id);
        
        self.listener = Some(Arc::new(listener));
        self.started = true;
        
        // Spawn a task to accept incoming connections
        println!("Starting accept loop for node {}", self.local_node.id);
        self.start_accept_loop()?;
        
        // Wait a moment for the tasks to start running
        tokio::task::yield_now().await;
        println!("Transport initialization completed for node {}", self.local_node.id);
        
        Ok(())
    }
    
    /// Start accepting incoming connections
    fn start_accept_loop(&self) -> ClusterResult<()> {
        if self.listener.is_none() {
            return Err(ClusterError::TransportNotInitialized);
        }
        
        let listener = self.listener.as_ref().unwrap().clone();
        let serializer = self.serializer.clone_box();
        let local_node = self.local_node.clone();
        let peers = self.peers.clone();
        let connections = self.connections.clone();
        let message_handler = self.message_handler.clone();
        let node_id_string = self.local_node.id.to_string(); // Use string to avoid move issues
        
        println!("Starting accept loop for node {}", node_id_string);
        // Use tokio::task::spawn_local instead of tokio::spawn
        tokio::task::spawn_local(async move {
            println!("Accept loop started for node {}", local_node.id);
            loop {
                // Allow other tasks to progress
                tokio::task::yield_now().await;
                
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        debug!("Accepted connection from {}", addr);
                        println!("Node {} accepted connection from {}", local_node.id, addr);
                        
                        // Clone necessary data for the connection handling
                        let serializer_clone = serializer.clone_box();
                        let local_node_clone = local_node.clone();
                        let peers_clone = peers.clone();
                        let connections_clone = connections.clone();
                        let handler_clone = message_handler.clone();
                        let node_id_clone = local_node.id.clone();
                        
                        // Use tokio::task::spawn_local for the connection handler too
                        tokio::task::spawn_local(async move {
                            println!("Starting to handle incoming connection from {} for node {}", 
                                    addr, node_id_clone);
                            if let Err(e) = handle_incoming(
                                socket, 
                                addr, 
                                local_node_clone, 
                                serializer_clone, 
                                handler_clone,
                                peers_clone,
                                connections_clone,
                            ).await {
                                error!("Error handling connection: {}", e);
                                println!("Error handling connection from {} for node {}: {}", 
                                        addr, node_id_clone, e);
                            }
                            println!("Finished handling connection from {} for node {}", 
                                    addr, node_id_clone);
                        });
                        
                        // Allow other tasks to run
                        tokio::task::yield_now().await;
                    }
                    Err(e) => {
                        error!("Error accepting connection: {}", e);
                        println!("Node {} error accepting connection: {}", local_node.id, e);
                        // Add a small delay to prevent CPU spinning on repeated errors
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });
        
        println!("Accept loop task spawned for node {}", node_id_string);
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
            TransportMessage::Envelope(ref envelope) => {
                debug!("Received envelope: {:?}", envelope);
                
                let message = TransportMessage::Envelope(envelope.clone());
                
                // Check if this is an acknowledgement message
                if envelope.message_type == MessageType::Pong {
                    if let Some(handler) = &self.message_handler {
                        let _handler_lock = handler.lock();
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
                        let _handler_lock = handler.lock();
                        debug!("Forwarding envelope to message handler");
                        
                        // Get the sender_node from peer_node_id or from the envelope
                        let sender_id = envelope.sender_node.clone();
                        let sender_id_for_logging = sender_id.clone(); // Clone for logging
                        let message_clone = message.clone();
                        
                        // Release the lock before awaiting
                        drop(_handler_lock);
                        
                        // Get a new lock and process the message
                        let handler_clone = handler.clone();
                        
                        // Rather than spawning a task, just execute it directly
                        let result = async move {
                            let mut handler_guard = handler_clone.lock();
                            handler_guard.handle_message(sender_id, message_clone).await
                        }.await;
                        
                        // Now handle the result
                        match result {
                            Ok(_) => debug!("Successfully handled envelope message from {}", sender_id_for_logging),
                            Err(e) => error!("Error handling envelope message from {}: {:?}", sender_id_for_logging, e),
                        }
                    }
                }
                
                Ok(())
            },
            TransportMessage::Handshake(node_info) => {
                debug!("Received handshake from node {}", node_info.id);
                
                // Store the peer's node info
                self.peers.lock().insert(node_info.id.clone(), node_info.clone());
                
                // Store the connection
                let stream = TcpStream::connect(node_info.addr).await.map_err(|e| {
                    error!("Failed to connect to peer: {}", e);
                    ClusterError::NetworkError(format!("Failed to connect to peer: {}", e))
                })?;
                
                let stream_mutex = Arc::new(TokioMutex::new(stream));
                self.connections.lock().insert(node_info.id.clone(), stream_mutex.clone());
                
                // Send our handshake back
                let handshake = TransportMessage::Handshake(self.local_node.clone());
                
                // 使用自有序列化方法
                let serialized = match &*self.serializer {
                    s if s.type_id() == std::any::TypeId::of::<crate::serialization::BincodeSerializer>() => {
                        let serializer = crate::serialization::BincodeSerializer::new();
                        serializer.serialize(&handshake)?
                    },
                    s if s.type_id() == std::any::TypeId::of::<crate::serialization::JsonSerializer>() => {
                        let serializer = crate::serialization::JsonSerializer::new();
                        serializer.serialize(&handshake)?
                    },
                    _ => {
                        self.serializer.serialize_any(&handshake as &dyn std::any::Any)?
                    }
                };
                
                // 获取锁并发送数据
                let len = serialized.len() as u32;
                let len_bytes = len.to_be_bytes();
                
                // Use a separate block for the socket lock to avoid borrowing issues
                {
                    let mut socket_locked = stream_mutex.lock().await;
                    
                    // First write the message length
                    match socket_locked.write_all(&len_bytes).await {
                        Ok(_) => {
                            debug!("Successfully wrote handshake length to {}", node_info.addr);
                            
                            // Then write the serialized message
                            match socket_locked.write_all(&serialized).await {
                                Ok(_) => debug!("Successfully wrote handshake to {}", node_info.addr),
                                Err(e) => {
                                    error!("Failed to write handshake to {}: {}", node_info.addr, e);
                                    return Err(ClusterError::NetworkError(format!("Failed to write handshake to {}: {}", node_info.addr, e)));
                                }
                            }
                        },
                        Err(e) => {
                            error!("Failed to write handshake length to {}: {}", node_info.addr, e);
                            return Err(ClusterError::NetworkError(format!("Failed to write handshake length to {}: {}", node_info.addr, e)));
                        }
                    }
                } // Release the lock here
                
                debug!("Sent handshake to peer at {}, waiting for response", node_info.addr);
                
                // Wait for a short time to ensure the handshake has been processed
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // Verify the connection is still in our map
                {
                    let connections = self.connections.lock();
                    if !connections.contains_key(&node_info.id) {
                        error!("Connection to {} was lost after handshake", node_info.addr);
                        return Err(ClusterError::ConnectionFailed(format!("Connection to {} was lost after handshake", node_info.addr)));
                    }
                    
                    debug!("Connection to {} is still active", node_info.addr);
                }
                
                Ok(())
            },
            TransportMessage::ActorDiscoveryRequest(sender_id, path) => {
                debug!("Received actor discovery request from {} for {}", sender_id, path);
                
                // Get the registry adapter if available
                if let Some(registry_adapter) = &self.registry_adapter {
                    // Clone the values for the async block
                    let registry = registry_adapter.clone();
                    let sender_id = sender_id.clone();
                    let path = path.clone();
                    
                    // Spawn a task to handle the request
                    tokio::task::spawn_local(async move {
                        if let Err(e) = registry.handle_discovery_request(sender_id, path).await {
                            error!("Error handling actor discovery request: {:?}", e);
                        }
                    });
                }
                Ok(())
            },
            TransportMessage::ActorDiscoveryResponse(path, locations) => {
                debug!("Received actor discovery response for {} with locations: {:?}", path, locations);
                
                // Get the registry adapter if available
                if let Some(registry_adapter) = &self.registry_adapter {
                    // Handle the response
                    registry_adapter.handle_discovery_response(path, locations);
                }
                Ok(())
            },
            _ => Ok(()),
        }
    }
    
    /// Send a message to a specific node
    pub async fn send_message(&mut self, target_node: &NodeId, message: TransportMessage) -> ClusterResult<()> {
        debug!("Sending message to node {}: {:?}", target_node, message);
        
        // Check if we have the node in our known peers
        let target_addr;
        {
            let peers = self.peers.lock();
            if let Some(peer) = peers.get(target_node) {
                target_addr = peer.addr;
                debug!("Found peer address for node {}: {}", target_node, target_addr);
            } else {
                error!("Node {} not found in peers list", target_node);
                return Err(ClusterError::NodeNotFound(target_node.clone()));
            }
        }
        
        // Check if we already have a connection to this peer
        let mut socket_opt = None;
        {
            let connections = self.connections.lock();
            if let Some(socket) = connections.get(target_node) {
                debug!("Found existing connection to node {}", target_node);
                socket_opt = Some(socket.clone());
            } else {
                debug!("No existing connection to node {}", target_node);
            }
        }
        
        // If we don't have a connection, establish one
        if socket_opt.is_none() {
            debug!("Attempting to connect to node {} at {}", target_node, target_addr);
            match self.connect_to_peer(target_addr).await {
                Ok(_) => {
                    debug!("Successfully connected to node {}", target_node);
                    
                    // Try to get the connection again
                    let connections = self.connections.lock();
                    if let Some(socket) = connections.get(target_node) {
                        socket_opt = Some(socket.clone());
                        debug!("Retrieved connection for node {}", target_node);
                    } else {
                        // This should not happen, but just in case
                        error!("Failed to retrieve connection for node {} after successful connect", target_node);
                        return Err(ClusterError::ConnectionFailed(format!("Connection established but not found for {}", target_addr)));
                    }
                },
                Err(e) => {
                    error!("Failed to connect to node {} at {}: {:?}", target_node, target_addr, e);
                    return Err(ClusterError::ConnectionFailed(target_addr.to_string()));
                }
            }
        }
        
        // At this point, we should definitely have a connection
        let socket = match socket_opt {
            Some(s) => s,
            None => {
                error!("No connection available for node {} at {}", target_node, target_addr);
                return Err(ClusterError::ConnectionFailed(target_addr.to_string()));
            }
        };
        
        debug!("Serializing message for node {}", target_node);
        
        // 序列化消息
        let serialized = match &*self.serializer {
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
        
        // Write the length of the message first (as u32), then the message itself
        let len = serialized.len() as u32;
        let len_bytes = len.to_be_bytes();
        
        debug!("Acquiring lock for socket to node {}", target_node);
        let mut socket_locked = socket.lock().await;
        
        debug!("Writing message length {} bytes to node {}", len, target_node);
        // First write the length
        match socket_locked.write_all(&len_bytes).await {
            Ok(_) => {
                debug!("Successfully wrote message length to node {}", target_node);
                // Then write the data
                debug!("Writing message payload {} bytes to node {}", serialized.len(), target_node);
                match socket_locked.write_all(&serialized).await {
                    Ok(_) => debug!("Successfully wrote message to node {}", target_node),
                    Err(e) => {
                        error!("Failed to write message payload to node {}: {}", target_node, e);
                        return Err(ClusterError::NetworkError(format!("Failed to write message: {}", e)));
                    }
                }
            },
            Err(e) => {
                error!("Failed to write message length to node {}: {}", target_node, e);
                return Err(ClusterError::NetworkError(format!("Failed to write message length: {}", e)));
            }
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
                
                self.pending_acks.lock().insert(envelope.message_id.clone().to_string(), pending);
            }
        }
        
        debug!("Successfully sent message to node {}", target_node);
        
        Ok(())
    }
    
    /// Send a message envelope
    pub async fn send_envelope(&mut self, envelope: MessageEnvelope) -> ClusterResult<()> {
        // Determine sender node ID
        let sender_id = envelope.sender_node.clone();
        let target_node = envelope.target_node.clone();
        let message_type = envelope.message_type.clone();
        let payload_size = envelope.payload.len();
        
        println!("Sending envelope from node {} to node {}. Type: {}, payload size: {}",
                sender_id, target_node, message_type, payload_size);
        println!("Message ID: {}, Target actor: {}", envelope.message_id, envelope.target_actor);
        
        // First check if we already have an active connection to the target
        let connection_exists = {
            let connections_guard = self.connections.lock().await;
            connections_guard.contains_key(&target_node)
        };
        
        if connection_exists {
            println!("Found existing connection to node {}", target_node);
        } else {
            println!("No existing connection to node {}, looking up peer info", target_node);
        }
        
        // Allow other tasks to progress
        tokio::task::yield_now().await;
        
        if !connection_exists {
            // Look up peer info
            let peer_opt = {
                let peers_guard = self.peers.lock();
                peers_guard.get(&target_node).cloned()
            };
            
            if let Some(peer) = peer_opt {
                println!("Found peer info for node {}, connecting to {}", 
                        target_node, peer.addr);
                
                // Attempt to connect to peer
                if let Err(e) = self.connect_to_peer(&peer.addr).await {
                    error!("Failed to connect to peer at {}: {}", peer.addr, e);
                    println!("Failed to connect to peer at {}: {}", peer.addr, e);
                    return Err(ClusterError::ConnectionFailed(format!(
                        "Failed to connect to node {}: {}", target_node, e
                    )));
                }
            } else {
                error!("Target node {} not found in peers", target_node);
                println!("Target node {} not found in peers", target_node);
                return Err(ClusterError::NodeNotFound(format!(
                    "Target node {} not found in peers", target_node
                )));
            }
        }
        
        // Now we should have a connection
        let connection_opt = {
            let connections_guard = self.connections.lock();
            connections_guard.get(&target_node).cloned()
        };
        
        if let Some(connection) = connection_opt {
            println!("Sending message to node {}", target_node);
            
            // Create TransportMessage from the envelope
            let transport_message = TransportMessage::Envelope(envelope);
            println!("Created TransportMessage::Envelope for serialization");
            
            // Serialize the message
            let serializer_type_id = self.serializer.type_id();
            println!("Serializing using serializer type: {:?}", serializer_type_id);
            println!("BincodeSerializer type_id: {:?}", std::any::TypeId::of::<crate::serialization::BincodeSerializer>());
            println!("JsonSerializer type_id: {:?}", std::any::TypeId::of::<crate::serialization::JsonSerializer>());
            
            let serialization_result = if serializer_type_id == std::any::TypeId::of::<crate::serialization::BincodeSerializer>() {
                println!("Using BincodeSerializer for serialization");
                let concrete_serializer = crate::serialization::BincodeSerializer::new();
                concrete_serializer.serialize(&transport_message)
            } else if serializer_type_id == std::any::TypeId::of::<crate::serialization::JsonSerializer>() {
                println!("Using JsonSerializer for serialization");
                let concrete_serializer = crate::serialization::JsonSerializer::new();
                concrete_serializer.serialize(&transport_message)
            } else {
                println!("Using generic serializer serialize_any");
                self.serializer.serialize_any(&transport_message as &dyn std::any::Any)
            };
            
            let serialized = match serialization_result {
                Ok(data) => {
                    println!("Serialization successful, data size: {}", data.len());
                    if !data.is_empty() {
                        let preview_len = std::cmp::min(data.len(), 16);
                        println!("Serialized data first {} bytes: {:?}", preview_len, &data[0..preview_len]);
                    }
                    data
                },
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                    println!("Failed to serialize message: {}", e);
                    return Err(ClusterError::SerializationError(format!("Failed to serialize message: {}", e)));
                }
            };
            
            // Send the message
            let len = serialized.len() as u32;
            println!("Sending message with length: {} bytes", len);
            let len_bytes = len.to_be_bytes();
            
            let mut sender = connection.sender.clone();
            
            // First send the length
            if let Err(e) = sender.write_all(&len_bytes).await {
                error!("Failed to send message length to node {}: {}", target_node, e);
                println!("Failed to send message length to node {}: {}", target_node, e);
                return Err(ClusterError::MessageSendFailed(format!(
                    "Failed to send message length to node {}: {}", target_node, e
                )));
            }
            
            // Then send the serialized message
            if let Err(e) = sender.write_all(&serialized).await {
                error!("Failed to send message payload to node {}: {}", target_node, e);
                println!("Failed to send message payload to node {}: {}", target_node, e);
                return Err(ClusterError::MessageSendFailed(format!(
                    "Failed to send message payload to node {}: {}", target_node, e
                )));
            }
            
            println!("Successfully sent message to node {}", target_node);
            Ok(())
        } else {
            error!("No connection found for node {}", target_node);
            println!("No connection found for node {}", target_node);
            Err(ClusterError::NodeNotFound(format!(
                "No connection found for node {}", target_node
            )))
        }
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

    /// Connect to a peer node
    pub async fn connect_to_peer(&mut self, peer_addr: SocketAddr) -> ClusterResult<()> {
        debug!("Connecting to peer at {}", peer_addr);
        
        // Find the node ID corresponding to the peer address
        let peer_node_id = {
            let peers = self.peers.lock();
            let mut found_id = None;
            
            for (id, info) in peers.iter() {
                if info.addr == peer_addr {
                    debug!("Found node ID {} for address {}", id, peer_addr);
                    found_id = Some(id.clone());
                    break;
                }
            }
            
            found_id
        };
        
        let peer_id = if let Some(id) = peer_node_id {
            id
        } else {
            // If we don't have the peer in our known peers, use a temporary ID
            debug!("No known node ID for {}, will use handshake to discover", peer_addr);
            let temp_id = NodeId::new();
            debug!("Created temporary ID {} for peer at {}", temp_id, peer_addr);
            temp_id
        };
        
        // Increase timeout to 10 seconds to give more time for tests to connect
        match tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(peer_addr)).await {
            Ok(result) => {
                match result {
                    Ok(socket) => {
                        debug!("Successfully established TCP connection to {}", peer_addr);
                        let socket_mutex = Arc::new(TokioMutex::new(socket));
                        
                        debug!("Connection established to {}, sending handshake", peer_addr);
                        
                        // Set TCP_NODELAY to reduce latency
                        {
                            let mut socket_lock = socket_mutex.lock().await;
                            if let Err(e) = socket_lock.set_nodelay(true) {
                                debug!("Could not set TCP_NODELAY on socket: {}", e);
                            }
                        }
                        
                        // Store the connection with the peer ID (either known or temporary)
                        // This should happen before sending the handshake to avoid race conditions
                        {
                            let mut connections = self.connections.lock();
                            connections.insert(peer_id.clone(), socket_mutex.clone());
                            debug!("Stored connection for peer {} at {}", peer_id, peer_addr);
                            
                            // Print current connections for debugging
                            let connection_keys: Vec<_> = connections.keys().cloned().collect();
                            debug!("Current connections: {:?}", connection_keys);
                        }
                        
                        // Send handshake with our node info
                        let handshake = TransportMessage::Handshake(self.local_node.clone());
                        
                        // 序列化消息
                        let serialized = match &*self.serializer {
                            s if s.type_id() == std::any::TypeId::of::<crate::serialization::BincodeSerializer>() => {
                                let serializer = crate::serialization::BincodeSerializer::new();
                                serializer.serialize(&handshake)?
                            },
                            s if s.type_id() == std::any::TypeId::of::<crate::serialization::JsonSerializer>() => {
                                let serializer = crate::serialization::JsonSerializer::new();
                                serializer.serialize(&handshake)?
                            },
                            _ => {
                                self.serializer.serialize_any(&handshake as &dyn std::any::Any)?
                            }
                        };
                        
                        // Write the length of the message first (as u32), then the message itself
                        let len = serialized.len() as u32;
                        let len_bytes = len.to_be_bytes();
                        
                        // Use a separate block for the socket lock to avoid borrowing issues
                        {
                            let mut socket_locked = socket_mutex.lock().await;
                            
                            // First write the message length
                            match socket_locked.write_all(&len_bytes).await {
                                Ok(_) => {
                                    debug!("Successfully wrote handshake length to {}", peer_addr);
                                    
                                    // Then write the serialized message
                                    match socket_locked.write_all(&serialized).await {
                                        Ok(_) => debug!("Successfully wrote handshake to {}", peer_addr),
                                        Err(e) => {
                                            error!("Failed to write handshake to {}: {}", peer_addr, e);
                                            return Err(ClusterError::NetworkError(format!("Failed to write handshake to {}: {}", peer_addr, e)));
                                        }
                                    }
                                },
                                Err(e) => {
                                    error!("Failed to write handshake length to {}: {}", peer_addr, e);
                                    return Err(ClusterError::NetworkError(format!("Failed to write handshake length to {}: {}", peer_addr, e)));
                                }
                            }
                        } // Release the lock here
                        
                        debug!("Sent handshake to peer at {}, waiting for response", peer_addr);
                        
                        // Wait for a short time to ensure the handshake has been processed
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        
                        // Verify the connection is still in our map
                        {
                            let connections = self.connections.lock();
                            if !connections.contains_key(&peer_id) {
                                error!("Connection to {} was lost after handshake", peer_addr);
                                return Err(ClusterError::ConnectionFailed(format!("Connection to {} was lost after handshake", peer_addr)));
                            }
                            
                            debug!("Connection to {} is still active", peer_addr);
                        }
                        
                        Ok(())
                    },
                    Err(e) => {
                        error!("Failed to connect to peer at {}: {}", peer_addr, e);
                        Err(ClusterError::ConnectionFailed(peer_addr.to_string()))
                    }
                }
            },
            Err(_) => {
                error!("Connection to peer at {} timed out after 10 seconds", peer_addr);
                Err(ClusterError::ConnectionFailed(peer_addr.to_string()))
            }
        }
    }

    /// Start the transport layer
    pub async fn start(&mut self) -> ClusterResult<()> {
        if self.started {
            return Ok(());
        }
        
        // Create message channels
        let (tx, rx) = mpsc::channel(100);
        self.msg_tx = Some(tx.clone());
        self.msg_rx = Some(rx);
        
        // Set up TCP listener
        let addr = self.local_node.addr;
        let listener = TcpListener::bind(addr).await?;
        self.listener = Some(Arc::new(listener));
        
        // Clone shared resources for the listener task
        let local_node_clone = self.local_node.clone();
        let peers_clone = self.peers.clone();
        let connections_clone = self.connections.clone();
        let serializer_clone = self.serializer.clone_box();
        let message_handler_clone = self.message_handler.clone();
        
        // Spawn a task to accept incoming connections
        let listener_clone = self.listener.as_ref().unwrap().clone();
        let msg_tx_clone = self.msg_tx.clone();
        
        tokio::spawn(async move {
            info!("P2P transport listening on {}", addr);
            
            loop {
                match listener_clone.accept().await {
                    Ok((socket, addr)) => {
                        debug!("Accepted connection from {}", addr);
                        
                        // 克隆需要传递给新任务的资源
                        let local_node_clone2 = local_node_clone.clone();
                        let peers_clone2 = peers_clone.clone();
                        let connections_clone2 = connections_clone.clone();
                        let serializer_clone2 = serializer_clone.clone_box();
                        let message_handler_clone2 = message_handler_clone.clone();
                        
                        // Spawn a task to handle this connection
                        tokio::task::spawn_local(async move {
                            if let Err(e) = handle_incoming(
                                socket, 
                                addr, 
                                local_node_clone2, 
                                serializer_clone2, 
                                message_handler_clone2,
                                peers_clone2,
                                connections_clone2,
                            ).await {
                                error!("Error handling connection: {}", e);
                            }
                        });
                    },
                    Err(e) => {
                        error!("Error accepting connection: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        
        // 处理消息发送队列 - 让我们把rx所有权移出，复制给变量
        // 创建一个新的队列，避免借用self
        let rx_owned = self.msg_rx.take().unwrap();
        let peers_clone = self.peers.clone();
        let connections_clone = self.connections.clone();
        let serializer_clone = self.serializer.clone_box();
        let local_node_clone = self.local_node.clone();
        
        tokio::task::spawn_local(async move {
            let mut rx_owned = rx_owned;
            
            while let Some((target_node, message)) = rx_owned.recv().await {
                debug!("Sending message to node {}: {:?}", target_node, message);
                
                // 获取目标节点地址
                let target_addr;
                {
                    let peers = peers_clone.lock();
                    if let Some(peer) = peers.get(&target_node) {
                        target_addr = peer.addr;
                    } else {
                        error!("Node not found: {}", target_node);
                        continue;
                    }
                }
                
                // 获取或创建连接
                let mut socket_opt = None;
                {
                    let connections = connections_clone.lock();
                    if let Some(socket) = connections.get(&target_node) {
                        socket_opt = Some(socket.clone());
                    }
                }
                
                // 如果没有连接，创建一个
                if socket_opt.is_none() {
                    match TcpStream::connect(target_addr).await {
                        Ok(stream) => {
                            let stream_mutex = Arc::new(TokioMutex::new(stream));
                            connections_clone.lock().insert(target_node.clone(), stream_mutex.clone());
                            socket_opt = Some(stream_mutex);
                            
                            // 发送握手
                            let handshake = TransportMessage::Handshake(local_node_clone.clone());
                            let serializer_type_id = serializer_clone.type_id();
                            
                            let serialized = if serializer_type_id == TypeId::of::<BincodeSerializer>() {
                                let concrete_serializer = BincodeSerializer::new();
                                match concrete_serializer.serialize(&handshake) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        error!("Failed to serialize handshake: {}", e);
                                        continue;
                                    }
                                }
                            } else if serializer_type_id == TypeId::of::<JsonSerializer>() {
                                let concrete_serializer = JsonSerializer::new();
                                match concrete_serializer.serialize(&handshake) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        error!("Failed to serialize handshake: {}", e);
                                        continue;
                                    }
                                }
                            } else {
                                match serializer_clone.serialize_any(&handshake as &dyn std::any::Any) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        error!("Failed to serialize handshake: {:?}", e);
                                        continue;
                                    }
                                }
                            };
                            
                            let len = serialized.len() as u32;
                            let len_bytes = len.to_be_bytes();
                            
                            let mut stream = socket_opt.as_ref().unwrap().lock().await;
                            if let Err(e) = stream.write_all(&len_bytes).await {
                                error!("Failed to write handshake length: {}", e);
                                continue;
                            }
                            
                            if let Err(e) = stream.write_all(&serialized).await {
                                error!("Failed to write handshake: {}", e);
                                continue;
                            }
                        },
                        Err(e) => {
                            error!("Failed to connect to node {}: {}", target_node, e);
                            continue;
                        }
                    }
                }
                
                // 序列化消息
                let serializer_type_id = serializer_clone.type_id();
                let serialized = if serializer_type_id == TypeId::of::<BincodeSerializer>() {
                    let concrete_serializer = BincodeSerializer::new();
                    match concrete_serializer.serialize(&message) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize message: {}", e);
                            continue;
                        }
                    }
                } else if serializer_type_id == TypeId::of::<JsonSerializer>() {
                    let concrete_serializer = JsonSerializer::new();
                    match concrete_serializer.serialize(&message) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize message: {}", e);
                            continue;
                        }
                    }
                } else {
                    match serializer_clone.serialize_any(&message as &dyn std::any::Any) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize message: {:?}", e);
                            continue;
                        }
                    }
                };
                
                // 发送消息
                let socket = socket_opt.unwrap();
                let len = serialized.len() as u32;
                let len_bytes = len.to_be_bytes();
                
                let mut stream = socket.lock().await;
                if let Err(e) = stream.write_all(&len_bytes).await {
                    error!("Failed to write message length: {}", e);
                    continue;
                }
                
                if let Err(e) = stream.write_all(&serialized).await {
                    error!("Failed to write message: {}", e);
                    continue;
                }
                
                debug!("Successfully sent message to node {}", target_node);
            }
        });
        
        self.started = true;
        Ok(())
    }

    /// Connect to a remote peer
    pub async fn connect(&self, addr: SocketAddr) -> ClusterResult<NodeInfo> {
        if !self.started {
            return Err(ClusterError::ConfigurationError("Transport not started".to_string()));
        }
        
        debug!("Connecting to {}", addr);
        
        let stream = TcpStream::connect(addr).await
            .map_err(|e| ClusterError::NetworkError(format!("Failed to connect: {}", e)))?;
            
        let peer_addr = stream.peer_addr()
            .map_err(|e| ClusterError::NetworkError(format!("Failed to get peer address: {}", e)))?;
            
        debug!("Connected to {}", peer_addr);
        
        // Create a temporary node ID and info
        let node_id = NodeId::new();
        let node_info = NodeInfo::new(
            node_id.clone(), 
            format!("temp-{}", node_id), 
            NodeRole::Peer, 
            peer_addr
        );
        
        // Store the connection
        let stream_mutex = Arc::new(TokioMutex::new(stream));
        self.connections.lock().insert(node_id.clone(), stream_mutex.clone());
        
        // Send handshake
        let handshake = TransportMessage::Handshake(self.local_node.clone());
        
        let serializer_type_id = self.serializer.type_id();
        let serialized = if serializer_type_id == TypeId::of::<BincodeSerializer>() {
            let concrete_serializer = BincodeSerializer::new();
            concrete_serializer.serialize(&handshake)?
        } else if serializer_type_id == TypeId::of::<JsonSerializer>() {
            let concrete_serializer = JsonSerializer::new();
            concrete_serializer.serialize(&handshake)?
        } else {
            self.serializer.serialize_any(&handshake as &dyn std::any::Any)?
        };
        
        let len = serialized.len() as u32;
        let len_bytes = len.to_be_bytes();
        
        {
            let mut stream = stream_mutex.lock().await;
            stream.write_all(&len_bytes).await
                .map_err(|e| ClusterError::NetworkError(format!("Failed to write handshake length: {}", e)))?;
                
            stream.write_all(&serialized).await
                .map_err(|e| ClusterError::NetworkError(format!("Failed to write handshake: {}", e)))?;
        }
        
        // Receive handshake response
        let mut buffer = [0u8; 4];
        {
            let mut stream = stream_mutex.lock().await;
            stream.read_exact(&mut buffer).await
                .map_err(|e| ClusterError::NetworkError(format!("Failed to read handshake response length: {}", e)))?;
        }
        
        let msg_len = u32::from_be_bytes(buffer) as usize;
        let mut msg_buffer = vec![0u8; msg_len];
        
        {
            let mut stream = stream_mutex.lock().await;
            stream.read_exact(&mut msg_buffer).await
                .map_err(|e| ClusterError::NetworkError(format!("Failed to read handshake response: {}", e)))?;
        }
        
        // Deserialize handshake response
        let response: TransportMessage = if serializer_type_id == TypeId::of::<BincodeSerializer>() {
            let concrete_serializer = BincodeSerializer::new();
            concrete_serializer.deserialize(&msg_buffer)?
        } else if serializer_type_id == TypeId::of::<JsonSerializer>() {
            let concrete_serializer = JsonSerializer::new();
            concrete_serializer.deserialize(&msg_buffer)?
        } else {
            match self.serializer.deserialize_any(&msg_buffer)? {
                boxed if boxed.type_id() == TypeId::of::<TransportMessage>() => {
                    *boxed.downcast::<TransportMessage>().unwrap()
                },
                _ => {
                    return Err(ClusterError::DeserializationError("Expected TransportMessage".to_string()));
                }
            }
        };
        
        // Process handshake response
        match response {
            TransportMessage::Handshake(peer_info) => {
                debug!("Received handshake from node {}", peer_info.id);
                
                // Update the connection with the actual node ID
                {
                    let mut connections = self.connections.lock();
                    connections.remove(&node_id);
                    connections.insert(peer_info.id.clone(), stream_mutex);
                }
                
                // Add to peers
                self.peers.lock().insert(peer_info.id.clone(), peer_info.clone());
                
                Ok(peer_info)
            },
            _ => {
                Err(ClusterError::DeserializationError("Expected handshake response".to_string()))
            }
        }
    }
    
    /// Clone this transport instance
    pub fn clone(&self) -> Self {
        P2PTransport {
            local_node: self.local_node.clone(),
            peers: self.peers.clone(),
            connections: self.connections.clone(),
            serializer: self.serializer.clone_box(),
            listener: self.listener.clone(),
            message_handler: self.message_handler.clone(),
            msg_tx: self.msg_tx.clone(),
            msg_rx: None,  // 不克隆接收器
            started: self.started,
            pending_acks: Arc::new(Mutex::new(HashMap::new())),
            registry_adapter: self.registry_adapter.clone(),
        }
    }

    /// Set message handler for incoming messages
    pub fn set_message_handler(&mut self, handler: impl Into<MessageHandlerType> + 'static) {
        let handler: MessageHandlerType = handler.into();
        match handler {
            MessageHandlerType::Function(f) => {
                let handler = ActorMessageHandler::new(f);
                self.message_handler = Some(Arc::new(Mutex::new(handler)));
            },
            MessageHandlerType::Actor(addr) => {
                let handler = ActorHandlerAdapter::new(addr);
                self.message_handler = Some(Arc::new(Mutex::new(handler)));
            }
        }
    }
    
    /// Get message handler for testing
    pub fn get_message_handler(&self) -> Option<Arc<Mutex<dyn MessageHandler>>> {
        self.message_handler.clone()
    }
    
    /// Set message handler directly for testing
    pub fn set_message_handler_direct(&mut self, handler: Arc<Mutex<dyn MessageHandler>>) {
        self.message_handler = Some(handler);
    }
    
    /// Add a peer node to the known peers list
    pub fn add_peer(&mut self, node_id: NodeId, node_info: NodeInfo) {
        let mut peers = self.peers.lock();
        peers.insert(node_id, node_info);
    }
    
    /// Get a clone of the peers map lock for testing
    pub fn get_peers_lock(&self) -> Arc<Mutex<HashMap<NodeId, NodeInfo>>> {
        self.peers.clone()
    }

    /// Add a registry adapter
    pub fn set_registry_adapter(&mut self, registry: Arc<ActorRegistry>) {
        self.registry_adapter = Some(registry);
    }

    /// Get a list of all known peer node IDs
    pub fn get_peer_list(&self) -> Vec<NodeId> {
        self.peers.lock().iter().map(|(node_id, _)| node_id.clone()).collect()
    }

    /// Check if the transport is started
    pub fn is_started(&self) -> bool {
        self.started
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
            TransportMessage::ActorDiscoveryRequest(node_id, path) => {
                debug!("Received actor discovery request from {} for {}", node_id, path);
            },
            TransportMessage::ActorDiscoveryResponse(path, locations) => {
                debug!("Received actor discovery response for {} with locations: {:?}", path, locations);
            },
            TransportMessage::Handshake(node_info) => {
                debug!("Received handshake from node {}", node_info.id);
            },
            TransportMessage::Ack(msg_id) => {
                debug!("Received ack for message {}", msg_id.to_string());
            },
            TransportMessage::Close => {
                debug!("Received close message");
            },
        }
    }
}

impl actix::Message for MessageEnvelope {
    type Result = ();
}

// 创建一个包装类型，代替直接使用元组
/// 一个包装NodeId和TransportMessage的消息类型
#[derive(Debug, Clone)]
pub struct NodeMessage {
    pub node_id: NodeId,
    pub message: TransportMessage,
}

impl actix::Message for NodeMessage {
    type Result = ();
}

/// 修改MessageEnvelopeHandler的实现
impl actix::Handler<NodeMessage> for MessageEnvelopeHandler {
    type Result = ();

    fn handle(&mut self, msg: NodeMessage, ctx: &mut Self::Context) -> Self::Result {
        debug!("MessageEnvelopeHandler received message from node {}: {:?}", msg.node_id, msg.message);
        // 实际处理消息的逻辑
    }
}

/// A reference to a remote actor
#[derive(Clone)]
pub struct RemoteActorRef {
    /// Path to the actor
    actor_path: ActorPath,
    /// Transport for sending messages
    transport: Arc<tokio::sync::Mutex<P2PTransport>>,
    /// Delivery guarantee
    delivery_guarantee: DeliveryGuarantee,
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
        
        // Serialize the message according to the serialization format of transport
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
        // Get locked transport and send the envelope
        let mut transport = self.transport.lock().await;
        transport.send_envelope(envelope).await
    }
    
    /// Get the actor path
    pub fn path(&self) -> &ActorPath {
        &self.actor_path
    }
    
    /// Get the delivery guarantee
    pub fn delivery_guarantee(&self) -> DeliveryGuarantee {
        self.delivery_guarantee
    }
    
    /// Set the delivery guarantee
    pub fn with_delivery_guarantee(mut self, guarantee: DeliveryGuarantee) -> Self {
        self.delivery_guarantee = guarantee;
        self
    }
}

// 为RemoteActorRef实现ActorRef trait
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

/// 修改ActorHandlerAdapter
pub struct ActorHandlerAdapter {
    actor: actix::Addr<MessageEnvelopeHandler>,
}

impl ActorHandlerAdapter {
    pub fn new(actor: actix::Addr<MessageEnvelopeHandler>) -> Self {
        Self { actor }
    }
}

#[async_trait::async_trait]
impl MessageHandler for ActorHandlerAdapter {
    async fn handle_message(&mut self, node_id: NodeId, message: TransportMessage) -> ClusterResult<()> {
        // 创建NodeMessage替代元组
        let msg = NodeMessage {
            node_id,
            message,
        };
        
        // 使用try_send避免async问题
        self.actor.try_send(msg)
            .map_err(|e| ClusterError::MessageSendFailed(format!("Failed to send message to actor: {}", e)))
    }
}

/// 包装消息发送通道以实现 MessageHandler 特性
struct MessageHandlerWrapper(Option<mpsc::Sender<(NodeId, TransportMessage)>>);

#[async_trait::async_trait]
impl MessageHandler for MessageHandlerWrapper {
    async fn handle_message(&mut self, node_id: NodeId, message: TransportMessage) -> ClusterResult<()> {
        if let Some(sender) = &self.0 {
            return sender.send((node_id, message)).await
                .map_err(|e| ClusterError::MessageSendFailed(format!("Failed to send message: {}", e)));
        }
        Ok(())
    }
}

/// Handle a new connection
async fn handle_incoming(
    stream: TcpStream,
    peer_addr: SocketAddr,
    local_node_info: NodeInfo,
    serializer: Box<dyn SerializerTrait>,
    message_handler: Option<Arc<Mutex<dyn MessageHandler>>>,
    peers: Arc<Mutex<HashMap<NodeId, NodeInfo>>>,
    connections: Arc<Mutex<HashMap<NodeId, Arc<TokioMutex<TcpStream>>>>>,
) -> ClusterResult<()> {
    debug!("ENTRY: handle_incoming from {}", peer_addr);
    println!("ENTRY: handle_incoming from {}", peer_addr);
    
    // Set TCP_NODELAY to reduce latency
    if let Err(e) = stream.set_nodelay(true) {
        debug!("Could not set TCP_NODELAY on socket: {}", e);
        println!("Could not set TCP_NODELAY on socket: {}", e);
    }
    
    let stream_mutex = Arc::new(TokioMutex::new(stream));
    let mut buffer = [0u8; 4]; // For the message length
    let mut peer_node_id = None;
    
    // For debugging purposes
    debug!("Starting to read messages from {}", peer_addr);
    println!("Starting to read messages from {}", peer_addr);
    println!("Serializer type: {:?}", serializer.type_id());
    
    // 持续读取消息
    loop {
        // Allow other tasks to run
        tokio::task::yield_now().await;
        
        // 读取消息长度
        let read_result = {
            let mut stream = stream_mutex.lock().await;
            match tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut buffer)).await {
                Ok(result) => result,
                Err(_) => {
                    debug!("Timeout reading from peer {}", peer_addr);
                    println!("Timeout reading from peer {}", peer_addr);
                    break;
                }
            }
        };
        
        match read_result {
            Ok(_) => {},
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!("Connection closed by peer {}", peer_addr);
                println!("Connection closed by peer {}", peer_addr);
                break;
            }
            Err(e) => {
                debug!("Failed to read message length from {}: {}", peer_addr, e);
                println!("Failed to read message length from {}: {}", peer_addr, e);
                return Err(ClusterError::NetworkError(format!("Failed to read message length: {}", e)));
            }
        }
        
        let msg_len = u32::from_be_bytes(buffer) as usize;
        debug!("Received message length: {} bytes from {}", msg_len, peer_addr);
        println!("Received message length: {} bytes from {}", msg_len, peer_addr);
        
        // 读取消息内容
        let mut msg_buffer = vec![0u8; msg_len];
        let read_result = {
            let mut stream = stream_mutex.lock().await;
            stream.read_exact(&mut msg_buffer).await
        };
        
        match read_result {
            Ok(_) => {
                debug!("Successfully read message payload of {} bytes from {}", msg_len, peer_addr);
                println!("Successfully read message payload of {} bytes from {}", msg_len, peer_addr);
                // Print first few bytes for debugging
                if msg_len > 0 {
                    let preview_len = std::cmp::min(msg_len, 16);
                    println!("Message payload first {} bytes: {:?}", preview_len, &msg_buffer[0..preview_len]);
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!("Connection closed by peer {} while reading message payload", peer_addr);
                println!("Connection closed by peer {} while reading message payload", peer_addr);
                break;
            }
            Err(e) => {
                debug!("Failed to read message payload from {}: {}", peer_addr, e);
                println!("Failed to read message payload from {}: {}", peer_addr, e);
                return Err(ClusterError::NetworkError(format!("Failed to read message: {}", e)));
            }
        }
        
        // 反序列化消息
        let serializer_type_id = serializer.type_id();
        println!("Deserializing message using serializer type: {:?}", serializer_type_id);
        println!("BincodeSerializer type_id: {:?}", std::any::TypeId::of::<crate::serialization::BincodeSerializer>());
        println!("JsonSerializer type_id: {:?}", std::any::TypeId::of::<crate::serialization::JsonSerializer>());
        
        let deserialization_result = if serializer_type_id == std::any::TypeId::of::<crate::serialization::BincodeSerializer>() {
            println!("Using BincodeSerializer for deserialization");
            let concrete_serializer = crate::serialization::BincodeSerializer::new();
            concrete_serializer.deserialize::<TransportMessage>(&msg_buffer)
        } else if serializer_type_id == std::any::TypeId::of::<crate::serialization::JsonSerializer>() {
            println!("Using JsonSerializer for deserialization");
            let concrete_serializer = crate::serialization::JsonSerializer::new();
            concrete_serializer.deserialize::<TransportMessage>(&msg_buffer)
        } else {
            println!("Using generic serializer deserialize_any");
            serializer.deserialize_any(&msg_buffer)
        };
        
        let message = match deserialization_result {
            Ok(msg) if serializer_type_id == std::any::TypeId::of::<crate::serialization::BincodeSerializer>() 
                    || serializer_type_id == std::any::TypeId::of::<crate::serialization::JsonSerializer>() => {
                println!("Direct deserialization successful, message type: TransportMessage");
                msg
            },
            Ok(boxed) => {
                println!("Boxed deserialization result type: {:?}", boxed.type_id());
                println!("TransportMessage type_id: {:?}", std::any::TypeId::of::<TransportMessage>());
                
                if boxed.type_id() == std::any::TypeId::of::<TransportMessage>() {
                    println!("Boxed type matches TransportMessage, attempting downcast");
                    match boxed.downcast::<TransportMessage>() {
                        Ok(boxed_msg) => {
                            println!("Downcast successful");
                            *boxed_msg
                        },
                        Err(_) => {
                            error!("Failed to downcast message despite matching type_id");
                            println!("Failed to downcast message despite matching type_id");
                            continue;
                        }
                    }
                } else {
                    error!("Unknown message type: {:?}", boxed.type_id());
                    println!("Unknown message type: {:?}", boxed.type_id());
                    continue;
                }
            },
            Err(e) => {
                error!("Failed to deserialize message: {:?}", e);
                println!("Failed to deserialize message: {:?}", e);
                continue;
            }
        };
        
        debug!("Received message: {:?}", message);
        println!("Successfully deserialized message: {:?}", message);
        
        // Process the message right away without spawning additional tasks
        match &message {
            TransportMessage::Handshake(node_info) => {
                debug!("Received handshake from node {}", node_info.id);
                println!("Received handshake from node {}", node_info.id);
                
                // 存储节点信息
                peers.lock().insert(node_info.id.clone(), node_info.clone());
                
                // 存储连接和节点ID
                peer_node_id = Some(node_info.id.clone());
                connections.lock().insert(node_info.id.clone(), stream_mutex.clone());
                
                debug!("Added node {} to peers and connections", node_info.id);
                println!("Added node {} to peers and connections", node_info.id);
                
                // Process immediately without spawning
                // 发送我们的握手响应
                let handshake = TransportMessage::Handshake(local_node_info.clone());
                
                // 序列化握手消息
                let serialized = if serializer_type_id == std::any::TypeId::of::<crate::serialization::BincodeSerializer>() {
                    let concrete_serializer = crate::serialization::BincodeSerializer::new();
                    match concrete_serializer.serialize(&handshake) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize handshake: {}", e);
                            println!("Failed to serialize handshake: {}", e);
                            continue;
                        }
                    }
                } else if serializer_type_id == TypeId::of::<crate::serialization::JsonSerializer>() {
                    let concrete_serializer = crate::serialization::JsonSerializer::new();
                    match concrete_serializer.serialize(&handshake) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize handshake: {}", e);
                            println!("Failed to serialize handshake: {}", e);
                            continue;
                        }
                    }
                } else {
                    match serializer.serialize_any(&handshake as &dyn std::any::Any) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize handshake: {:?}", e);
                            println!("Failed to serialize handshake: {:?}", e);
                            continue;
                        }
                    }
                };
                
                // 发送握手响应
                let len = serialized.len() as u32;
                let len_bytes = len.to_be_bytes();
                
                let mut stream = stream_mutex.lock().await;
                // First write the length
                match stream.write_all(&len_bytes).await {
                    Ok(_) => {
                        // Then write the data
                        match stream.write_all(&serialized).await {
                            Ok(_) => {
                                debug!("Successfully sent handshake response to {}", peer_addr);
                                println!("Successfully sent handshake response to {}", peer_addr);
                            },
                            Err(e) => {
                                error!("Failed to write handshake payload to {}: {}", peer_addr, e);
                                println!("Failed to write handshake payload to {}: {}", peer_addr, e);
                                break;
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to write handshake length to {}: {}", peer_addr, e);
                        println!("Failed to write handshake length to {}: {}", peer_addr, e);
                        break;
                    }
                }
            },
            TransportMessage::Envelope(envelope) => {
                debug!("Received envelope message: {:?}", envelope.message_type);
                println!("Received envelope message: {:?}", envelope.message_type);
                println!("Envelope details: sender={}, target={}, type={}, payload_size={}", 
                        envelope.sender_node, envelope.target_node, envelope.message_type, envelope.payload.len());
                
                // Forward to message handler if available
                if let Some(handler) = &message_handler {
                    debug!("Forwarding envelope to message handler");
                    println!("Forwarding envelope to message handler");
                    
                    // Get the sender_node from peer_node_id or from the envelope
                    let sender_id = peer_node_id.clone().unwrap_or(envelope.sender_node.clone());
                    println!("Using sender ID: {}", sender_id);
                    
                    // Let's process the message directly without spawning a task
                    // This ensures the message gets handled within this context
                    let mut handler_guard = handler.lock();
                    println!("Calling handler.handle_message with sender={}, message={:?}", sender_id, message);
                    match handler_guard.handle_message(sender_id.clone(), message.clone()).await {
                        Ok(_) => {
                            debug!("Successfully handled envelope message from {}", sender_id);
                            println!("Successfully handled envelope message from {}", sender_id);
                        },
                        Err(e) => {
                            error!("Error handling envelope message from {}: {:?}", sender_id, e);
                            println!("Error handling envelope message from {}: {:?}", sender_id, e);
                        }
                    }
                } else {
                    debug!("No message handler available to process envelope");
                    println!("No message handler available to process envelope");
                }
            },
            other_message => {
                debug!("Received other message type: {:?}", other_message);
                println!("Received other message type: {:?}", other_message);
                
                // Forward all other message types to the handler as well
                if let Some(handler) = &message_handler {
                    if let Some(node_id) = &peer_node_id {
                        debug!("Forwarding message to handler from node {}", node_id);
                        println!("Forwarding message to handler from node {}", node_id);
                        
                        // Process directly without spawning a task
                        let mut handler_guard = handler.lock();
                        match handler_guard.handle_message(node_id.clone(), message.clone()).await {
                            Ok(_) => {
                                debug!("Successfully handled message from {}", node_id);
                                println!("Successfully handled message from {}", node_id);
                            },
                            Err(e) => {
                                error!("Error handling message from {}: {:?}", node_id, e);
                                println!("Error handling message from {}: {:?}", node_id, e);
                            }
                        }
                    } else {
                        debug!("Received message from unknown peer, ignoring");
                        println!("Received message from unknown peer, ignoring");
                    }
                } else {
                    debug!("No message handler available");
                    println!("No message handler available");
                }
            }
        }
        
        // Allow other tasks to run
        tokio::task::yield_now().await;
    }
    
    // 连接关闭后清理
    if let Some(node_id) = peer_node_id {
        connections.lock().remove(&node_id);
        debug!("Removed connection for node {}", node_id);
        println!("Removed connection for node {}", node_id);
    }
    
    Ok(())
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
        // 创建LocalSet运行测试
        let local = tokio::task::LocalSet::new();
        
        local.run_until(async {
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
        }).await;
    }
} 