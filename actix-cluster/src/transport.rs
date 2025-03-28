//! Network transport module for peer-to-peer communication.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, NodeInfo, NodeStatus};
use crate::serialization::{Serializer, SerializationFormat, create_serializer};
use crate::cluster::ClusterSystemActor;

/// Timeout for connection attempts
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

/// Message types that can be sent through the transport
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TransportMessage {
    /// Node heartbeat
    Heartbeat(NodeInfo),
    
    /// Node status update
    StatusUpdate {
        /// Node ID
        node_id: NodeId,
        /// Node status
        status: NodeStatus,
    },
    
    /// Message to a specific actor
    ActorMessage {
        /// Target actor path
        target: String,
        /// Source actor path (for replies)
        source: String,
        /// Message payload
        payload: Vec<u8>,
        /// Message ID for tracking
        message_id: Uuid,
    },
    
    /// Actor discovery request
    ActorDiscovery {
        /// Actor path to discover
        actor_path: String,
        /// Requester ID for reply
        requester_id: NodeId,
    },
    
    /// Actor discovery response
    ActorDiscoveryResponse {
        /// Actor path that was requested
        actor_path: String,
        /// Node hosting the actor
        host_node: Option<NodeId>,
        /// Requester ID
        requester_id: NodeId,
    },
}

/// Simplified transport service
pub struct P2PTransport {
    /// Local node information
    local_node: NodeInfo,
    
    /// Known peers
    peers: Arc<Mutex<HashMap<NodeId, NodeInfo>>>,
    
    /// Serializer for messages
    serializer: Serializer,
    
    /// Message receiver channel
    msg_rx: Option<mpsc::Receiver<(NodeId, TransportMessage)>>,
    
    /// Message sender channel
    msg_tx: Option<mpsc::Sender<(NodeId, TransportMessage)>>,
    
    /// Received message handler
    message_handler: Option<Arc<Mutex<ActorMessageHandler>>>,
}

/// Actor for handling transport messages
pub trait Handler<M>: Actor {
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
    /// Create a new P2P transport
    pub fn new(local_node: NodeInfo, format: SerializationFormat) -> ClusterResult<Self> {
        // Create message channels
        let (msg_tx, msg_rx) = mpsc::channel(100);
        
        Ok(P2PTransport {
            local_node,
            peers: Arc::new(Mutex::new(HashMap::new())),
            serializer: create_serializer(format),
            msg_rx: Some(msg_rx),
            msg_tx: Some(msg_tx),
            message_handler: None,
        })
    }
    
    /// Initialize the transport
    pub async fn init(&mut self) -> ClusterResult<()> {
        // Add local node to peers
        let mut peers = self.peers.lock();
        peers.insert(self.local_node.id.clone(), self.local_node.clone());
        
        // Start message handling loop
        if let Some(msg_rx) = self.msg_rx.take() {
            let mut rx = msg_rx;
            let transport = self.clone();
            
            tokio::spawn(async move {
                while let Some((node_id, message)) = rx.recv().await {
                    if let Err(e) = transport.handle_message(&node_id, message).await {
                        log::error!("Error handling message: {}", e);
                    }
                }
            });
        }
        
        Ok(())
    }
    
    /// Handle an incoming message
    async fn handle_message(&self, node_id: &NodeId, message: TransportMessage) -> ClusterResult<()> {
        log::debug!("Handling message from node {}: {:?}", node_id, message);
        
        match &message {
            TransportMessage::Heartbeat(node_info) => {
                // Update node info in peers
                let mut peers = self.peers.lock();
                peers.insert(node_info.id.clone(), node_info.clone());
            },
            TransportMessage::StatusUpdate { node_id, status } => {
                // Update node status in peers
                let mut peers = self.peers.lock();
                if let Some(node) = peers.get_mut(node_id) {
                    node.status = *status;
                }
            },
            _ => {
                // Just log the message for now until we have proper handlers
                log::debug!("Received message: {:?}", message);
            }
        }
        
        Ok(())
    }
    
    /// Send a message to a node
    pub async fn send_message(&self, node_id: &NodeId, message: TransportMessage) -> ClusterResult<()> {
        if let Some(tx) = &self.msg_tx {
            tx.send((node_id.clone(), message)).await
                .map_err(|e| ClusterError::IoError(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe, 
                    format!("Failed to send message: {}", e)
                )))?;
            Ok(())
        } else {
            Err(ClusterError::ConfigurationError("Transport not initialized".to_string()))
        }
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
}

impl Clone for P2PTransport {
    fn clone(&self) -> Self {
        P2PTransport {
            local_node: self.local_node.clone(),
            peers: self.peers.clone(),
            serializer: match &self.serializer {
                Serializer::Bincode(_) => create_serializer(SerializationFormat::Bincode),
                Serializer::Json(_) => create_serializer(SerializationFormat::Json),
            },
            msg_rx: None, // Don't clone the receiver
            msg_tx: self.msg_tx.clone(),
            message_handler: self.message_handler.clone(),
        }
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
} 