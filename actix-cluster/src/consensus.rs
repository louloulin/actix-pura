//! Consensus module for Actix Cluster
//! 
//! This module implements a simplified consensus layer for the Actix cluster.
//! It provides basic coordination of critical operations across the cluster.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use log::debug;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use actix::prelude::*;

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, NodeInfo, NodeStatus};
use crate::transport::P2PTransport;

/// Consensus command types that can be replicated through the consensus system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusCommand {
    /// Add a node to the cluster
    AddNode(NodeInfo),
    
    /// Remove a node from the cluster
    RemoveNode(NodeId),
    
    /// Update a node's status
    UpdateNodeStatus(NodeId, NodeStatus),
    
    /// Register an actor on a specific node
    RegisterActor {
        actor_path: String,
        node_id: NodeId,
    },
    
    /// Custom command that can be extended by users
    Custom(Vec<u8>),
}

/// Result of applying a consensus command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResponse {
    /// Success flag
    pub success: bool,
    
    /// Optional message
    pub message: Option<String>,
    
    /// Optional data
    pub data: Option<Vec<u8>>,
}

/// State managed by the consensus protocol
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsensusState {
    /// All nodes in the cluster
    pub nodes: HashMap<NodeId, NodeInfo>,
    
    /// Actor registrations
    pub actor_registrations: HashMap<String, NodeId>,
    
    /// Last applied index
    pub last_applied_index: u64,
}

/// Entry in the consensus log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusLogEntry {
    /// Entry index
    pub index: u64,
    
    /// Entry term
    pub term: u64,
    
    /// Command payload
    pub payload: Option<Vec<u8>>,
}

/// Simplified consensus actor that manages the consensus state
pub struct ConsensusActor {
    /// NodeId of the current node
    node_id: NodeId,
    
    /// Current state
    state: Arc<RwLock<ConsensusState>>,
    
    /// Log entries
    logs: Arc<RwLock<HashMap<u64, ConsensusLogEntry>>>,
    
    /// Current term
    current_term: Arc<RwLock<u64>>,
    
    /// Transport for communication
    transport: Arc<Mutex<P2PTransport>>,
    
    /// Next log index
    next_index: Arc<RwLock<u64>>,
}

impl ConsensusActor {
    /// Create a new instance of ConsensusActor
    pub fn new(node_id: NodeId, transport: Arc<Mutex<P2PTransport>>) -> Self {
        Self {
            node_id,
            state: Arc::new(RwLock::new(ConsensusState::default())),
            logs: Arc::new(RwLock::new(HashMap::new())),
            current_term: Arc::new(RwLock::new(0)),
            transport,
            next_index: Arc::new(RwLock::new(1)),
        }
    }
    
    /// Initialize the consensus actor
    pub async fn init(&mut self) -> Result<()> {
        // In a real implementation, this would bootstrap the consensus system
        Ok(())
    }
    
    /// Get the current consensus state
    pub async fn get_state(&self) -> Result<ConsensusState> {
        let state = self.state.read().await.clone();
        Ok(state)
    }
    
    /// Apply a command to the consensus state (for testing)
    pub async fn apply_command(&self, command: ConsensusCommand) -> Result<ConsensusResponse> {
        // Serialize the command
        let data = bincode::serialize(&command)?;
        
        // Create a new entry
        let mut next_index = self.next_index.write().await;
        let entry = ConsensusLogEntry {
            index: *next_index,
            term: *self.current_term.read().await,
            payload: Some(data),
        };
        
        // Apply the entry to the state
        self.apply_entry(&entry).await?;
        
        // Store the entry in the log
        let mut logs = self.logs.write().await;
        logs.insert(entry.index, entry);
        
        // Increment the next index
        *next_index += 1;
        
        Ok(ConsensusResponse {
            success: true,
            message: Some("Command applied successfully".to_string()),
            data: None,
        })
    }
    
    /// Submit a command to the consensus system
    pub async fn submit_command(&self, command: ConsensusCommand) -> Result<ConsensusResponse> {
        // In a real implementation, this would replicate the command to other nodes
        self.apply_command(command).await
    }
    
    /// Apply an entry to the state
    async fn apply_entry(&self, entry: &ConsensusLogEntry) -> Result<()> {
        // Extract and deserialize the command
        let cmd = if let Some(data) = &entry.payload {
            bincode::deserialize::<ConsensusCommand>(data)
                .map_err(|e| anyhow!("Failed to deserialize entry payload: {}", e))?
        } else {
            return Ok(());  // Empty entry, nothing to apply
        };
        
        let mut state = self.state.write().await;
        
        // Apply the command to the state
        match cmd {
            ConsensusCommand::AddNode(node_info) => {
                debug!("Adding node {} to consensus state", node_info.id);
                state.nodes.insert(node_info.id.clone(), node_info);
            },
            ConsensusCommand::RemoveNode(node_id) => {
                debug!("Removing node {} from consensus state", node_id);
                state.nodes.remove(&node_id);
            },
            ConsensusCommand::UpdateNodeStatus(node_id, status) => {
                debug!("Updating status of node {} to {:?}", node_id, status);
                if let Some(node) = state.nodes.get_mut(&node_id) {
                    node.status = status;
                }
            },
            ConsensusCommand::RegisterActor { actor_path, node_id } => {
                debug!("Registering actor {} on node {}", actor_path, node_id);
                state.actor_registrations.insert(actor_path, node_id);
            },
            ConsensusCommand::Custom(_) => {
                debug!("Applying custom command");
                // Custom commands are handled by the application
            },
        }
        
        // Update the last applied index
        state.last_applied_index = entry.index;
        
        Ok(())
    }
}

/// Message to get the consensus state
#[derive(Message)]
#[rtype(result = "ClusterResult<ConsensusState>")]
pub struct GetConsensusState;

/// Implement for ConsensusCommand to be used as an Actor message
impl Message for ConsensusCommand {
    type Result = Result<ConsensusResponse, anyhow::Error>;
}

impl Actor for ConsensusActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        debug!("ConsensusActor started");
    }
}

impl Handler<ConsensusCommand> for ConsensusActor {
    type Result = ResponseFuture<Result<ConsensusResponse, anyhow::Error>>;
    
    fn handle(&mut self, msg: ConsensusCommand, _ctx: &mut Self::Context) -> Self::Result {
        let this = self.clone();
        
        Box::pin(async move {
            this.apply_command(msg).await
        })
    }
}

impl Handler<GetConsensusState> for ConsensusActor {
    type Result = ClusterResult<ConsensusState>;
    
    fn handle(&mut self, _msg: GetConsensusState, _ctx: &mut Self::Context) -> Self::Result {
        match futures::executor::block_on(self.get_state()) {
            Ok(state) => Ok(state),
            Err(e) => Err(ClusterError::ConsensusError(e.to_string())),
        }
    }
}

impl Clone for ConsensusActor {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            state: self.state.clone(),
            logs: self.logs.clone(),
            current_term: self.current_term.clone(),
            transport: self.transport.clone(),
            next_index: self.next_index.clone(),
        }
    }
} 