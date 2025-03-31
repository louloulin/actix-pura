//! Network adapter for consensus protocol
//!
//! This module adapts the P2PTransport to the network interface needed for consensus.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::consensus::{ConsensusCommand, ConsensusResponse, ConsensusState};
use crate::node::NodeId;
use crate::transport::{MessageHandler, P2PTransport, TransportMessage};
use crate::error::{ClusterError, ClusterResult};

/// Message types exchanged over the network for consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusNetworkMessage {
    /// Command to be executed
    Command(ConsensusCommand),
    
    /// Response to a command
    Response(ConsensusResponse),
    
    /// Request for the current state
    StateRequest,
    
    /// Response with the current state
    StateResponse(ConsensusState),
}

/// Network implementation for consensus using P2PTransport
pub struct ConsensusNetwork {
    /// Transport for communication
    transport: Arc<Mutex<P2PTransport>>,
    
    /// Node ID of the local node
    node_id: NodeId,
    
    /// Mapping to known nodes
    node_mapping: Arc<Mutex<HashMap<String, NodeId>>>,
}

impl ConsensusNetwork {
    /// Create a new instance of ConsensusNetwork
    pub fn new(transport: Arc<Mutex<P2PTransport>>, node_id: NodeId) -> Self {
        Self {
            transport,
            node_id,
            node_mapping: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Register a mapping from node name to NodeId
    pub async fn register_node(&self, name: String, node_id: NodeId) -> Result<()> {
        let mut mapping = self.node_mapping.lock().await;
        mapping.insert(name, node_id);
        Ok(())
    }
    
    /// Lookup NodeId by name
    async fn lookup_node(&self, name: &str) -> Result<NodeId> {
        let mapping = self.node_mapping.lock().await;
        if let Some(node_id) = mapping.get(name) {
            Ok(node_id.clone())
        } else {
            Err(anyhow::anyhow!("Unknown node name: {}", name))
        }
    }
    
    /// Send a command to a target node
    pub async fn send_command(&self, target: &NodeId, command: ConsensusCommand) -> Result<ConsensusResponse> {
        debug!("Sending command to node: {}", target);
        
        // Clone the transport reference to avoid any thread safety issues
        let transport_clone = self.transport.clone();
        let message = ConsensusNetworkMessage::Command(command);
        let transport_message = TransportMessage::Consensus(bincode::serialize(&message)?);
        
        {
            let mut transport = transport_clone.lock().await;
            transport.send_message(target, transport_message).await?;
        }
        
        // Simulated response for testing purposes
        Ok(ConsensusResponse {
            success: true,
            message: Some("Command sent successfully".to_string()),
            data: None,
        })
    }
    
    /// Request the current state from a target node
    pub async fn request_state(&self, target: &NodeId) -> Result<ConsensusState> {
        debug!("Requesting state from node: {}", target);
        
        // Clone the transport reference to avoid any thread safety issues
        let transport_clone = self.transport.clone();
        let message = ConsensusNetworkMessage::StateRequest;
        let transport_message = TransportMessage::Consensus(bincode::serialize(&message)?);
        
        {
            let mut transport = transport_clone.lock().await;
            transport.send_message(target, transport_message).await?;
        }
        
        // Return a default state for testing purposes
        Ok(ConsensusState::default())
    }
}

/// Consensus message handler to process incoming consensus messages
pub struct ConsensusMessageHandler {
    /// Node ID of the local node
    node_id: NodeId,
    
    /// Transport for sending responses
    transport: Arc<Mutex<P2PTransport>>,
    
    /// Callback for handling commands
    command_handler: Box<dyn Fn(ConsensusCommand) -> Result<ConsensusResponse> + Send + Sync + 'static>,
    
    /// Callback for getting state
    state_handler: Box<dyn Fn() -> Result<ConsensusState> + Send + Sync + 'static>,
}

impl ConsensusMessageHandler {
    /// Create a new instance of ConsensusMessageHandler
    pub fn new<C, S>(
        node_id: NodeId,
        transport: Arc<Mutex<P2PTransport>>,
        command_handler: C,
        state_handler: S,
    ) -> Self 
    where
        C: Fn(ConsensusCommand) -> Result<ConsensusResponse> + Send + Sync + 'static,
        S: Fn() -> Result<ConsensusState> + Send + Sync + 'static,
    {
        Self {
            node_id,
            transport,
            command_handler: Box::new(command_handler),
            state_handler: Box::new(state_handler),
        }
    }
    
    /// Helper method to send a response message
    async fn send_response(&self, target: &NodeId, message: ConsensusNetworkMessage) -> ClusterResult<()> {
        let data = bincode::serialize(&message)
            .map_err(|e| ClusterError::SerializationError(e.to_string()))?;
        
        // Clone the transport to avoid thread safety issues
        let transport_clone = self.transport.clone();
        let mut transport = transport_clone.lock().await;
        transport.send_message(target, TransportMessage::Consensus(data)).await
    }
}

#[async_trait]
impl MessageHandler for ConsensusMessageHandler {
    async fn handle_message(&self, sender: NodeId, message: TransportMessage) -> ClusterResult<()> {
        // Extract data from message first to avoid moving it across await boundaries
        if let TransportMessage::Consensus(data) = message {
            self.handle_consensus_message(sender, data).await
        } else {
            // Ignore non-consensus messages
            debug!("Ignoring non-consensus message");
            Ok(())
        }
    }
}

// Add helper methods implementation
impl ConsensusMessageHandler {
    // Process consensus message bytes
    async fn handle_consensus_message(&self, sender: NodeId, data: Vec<u8>) -> ClusterResult<()> {
        // Try to deserialize as ConsensusNetworkMessage
        match bincode::deserialize::<ConsensusNetworkMessage>(&data) {
            Ok(network_message) => {
                self.process_network_message(sender, network_message).await
            },
            Err(e) => {
                error!("Failed to deserialize consensus message: {}", e);
                Err(ClusterError::DeserializationError(format!("Failed to deserialize consensus message: {}", e)))
            }
        }
    }
    
    // Process the deserialized network message
    async fn process_network_message(&self, sender: NodeId, message: ConsensusNetworkMessage) -> ClusterResult<()> {
        match message {
            ConsensusNetworkMessage::Command(command) => {
                self.handle_command(sender, command).await
            },
            ConsensusNetworkMessage::StateRequest => {
                self.handle_state_request(sender).await
            },
            // Responses are typically handled by the sender
            ConsensusNetworkMessage::Response(_) |
            ConsensusNetworkMessage::StateResponse(_) => {
                debug!("Received response message, but it should be handled by the caller");
                Ok(())
            }
        }
    }
    
    // Handle command message
    async fn handle_command(&self, sender: NodeId, command: ConsensusCommand) -> ClusterResult<()> {
        // Clone command to avoid ownership issues
        let command_clone = command.clone();
        
        // Process command synchronously before any await points
        let result = (self.command_handler)(command_clone);
        
        match result {
            Ok(response) => {
                // Prepare response message
                let response_message = ConsensusNetworkMessage::Response(response);
                
                // Send response back
                self.send_response(&sender, response_message).await
            },
            Err(e) => {
                error!("Error handling command: {}", e);
                Err(ClusterError::ConsensusError(format!("Failed to process command: {}", e)))
            }
        }
    }
    
    // Handle state request message
    async fn handle_state_request(&self, sender: NodeId) -> ClusterResult<()> {
        // Process state request synchronously before any await points
        let result = (self.state_handler)();
        
        match result {
            Ok(state) => {
                // Prepare response message
                let response_message = ConsensusNetworkMessage::StateResponse(state);
                
                // Send state back
                self.send_response(&sender, response_message).await
            },
            Err(e) => {
                error!("Error getting state: {}", e);
                Err(ClusterError::ConsensusError(format!("Failed to get state: {}", e)))
            }
        }
    }
} 