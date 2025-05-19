//! Cluster manager module
//!
//! This module provides functionality for managing a cluster of nodes.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use dataflare_core::error::Result;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time;
use uuid::Uuid;

use crate::CloudRuntimeConfig;
use crate::{DiscoveryMethod, NodeRole};

/// Node ID type
pub type NodeId = String;

/// Node information
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Node ID
    pub id: NodeId,
    /// Node role
    pub role: NodeRole,
    /// Node address
    pub address: String,
    /// Last heartbeat time
    pub last_heartbeat: Instant,
    /// Node metadata
    pub metadata: HashMap<String, String>,
}

/// Cluster manager for managing a cluster of nodes
pub struct ClusterManager {
    /// Cluster configuration
    config: CloudRuntimeConfig,
    /// Local node ID
    local_node_id: NodeId,
    /// Local node information
    local_node: NodeInfo,
    /// Known nodes in the cluster
    nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,
    /// Heartbeat task handle
    heartbeat_task: Mutex<Option<JoinHandle<()>>>,
}

impl ClusterManager {
    /// Create a new cluster manager
    pub fn new(config: CloudRuntimeConfig) -> Result<Self> {
        let local_node_id = Uuid::new_v4().to_string();
        
        let local_node = NodeInfo {
            id: local_node_id.clone(),
            role: config.node_role.clone(),
            address: "localhost:0".to_string(), // Placeholder
            last_heartbeat: Instant::now(),
            metadata: HashMap::new(),
        };
        
        Ok(Self {
            config,
            local_node_id,
            local_node,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_task: Mutex::new(None),
        })
    }
    
    /// Start the cluster manager
    pub async fn start(&self) -> Result<()> {
        // Register local node
        self.register_node(self.local_node.clone())?;
        
        // Start heartbeat task
        let nodes = self.nodes.clone();
        let heartbeat_interval = self.config.heartbeat_interval;
        let node_timeout = self.config.node_timeout;
        let local_node_id = self.local_node_id.clone();
        
        let task = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(heartbeat_interval));
            
            loop {
                interval.tick().await;
                
                // Update local node heartbeat
                {
                    let mut nodes_write = nodes.write().unwrap();
                    if let Some(node) = nodes_write.get_mut(&local_node_id) {
                        node.last_heartbeat = Instant::now();
                    }
                }
                
                // Check for timed out nodes
                let now = Instant::now();
                let timeout = Duration::from_secs(node_timeout);
                
                let timed_out_nodes: Vec<NodeId> = {
                    let nodes_read = nodes.read().unwrap();
                    nodes_read
                        .iter()
                        .filter(|(id, node)| {
                            **id != local_node_id && now.duration_since(node.last_heartbeat) > timeout
                        })
                        .map(|(id, _)| id.clone())
                        .collect()
                };
                
                // Remove timed out nodes
                if !timed_out_nodes.is_empty() {
                    let mut nodes_write = nodes.write().unwrap();
                    for node_id in timed_out_nodes {
                        nodes_write.remove(&node_id);
                        log::info!("Node {} timed out and was removed from the cluster", node_id);
                    }
                }
            }
        });
        
        // Store task handle
        let mut heartbeat_task = self.heartbeat_task.lock().await;
        *heartbeat_task = Some(task);
        
        Ok(())
    }
    
    /// Stop the cluster manager
    pub async fn stop(&self) -> Result<()> {
        // Stop heartbeat task
        let mut heartbeat_task = self.heartbeat_task.lock().await;
        if let Some(task) = heartbeat_task.take() {
            task.abort();
        }
        
        // Unregister local node
        self.unregister_node(&self.local_node_id)?;
        
        Ok(())
    }
    
    /// Register a node in the cluster
    pub fn register_node(&self, node: NodeInfo) -> Result<()> {
        let mut nodes = self.nodes.write().unwrap();
        nodes.insert(node.id.clone(), node);
        Ok(())
    }
    
    /// Unregister a node from the cluster
    pub fn unregister_node(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().unwrap();
        nodes.remove(node_id);
        Ok(())
    }
    
    /// Get a node by ID
    pub fn get_node(&self, node_id: &str) -> Option<NodeInfo> {
        let nodes = self.nodes.read().unwrap();
        nodes.get(node_id).cloned()
    }
    
    /// Get all nodes in the cluster
    pub fn get_nodes(&self) -> Vec<NodeInfo> {
        let nodes = self.nodes.read().unwrap();
        nodes.values().cloned().collect()
    }
    
    /// Get nodes by role
    pub fn get_nodes_by_role(&self, role: NodeRole) -> Vec<NodeInfo> {
        let nodes = self.nodes.read().unwrap();
        nodes
            .values()
            .filter(|node| node.role == role)
            .cloned()
            .collect()
    }
    
    /// Get the local node ID
    pub fn local_node_id(&self) -> &str {
        &self.local_node_id
    }
    
    /// Get the local node information
    pub fn local_node(&self) -> &NodeInfo {
        &self.local_node
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cluster_manager() {
        let config = CloudRuntimeConfig {
            discovery_method: DiscoveryMethod::Static,
            node_role: NodeRole::Coordinator,
            heartbeat_interval: 1,
            node_timeout: 5,
        };
        
        let manager = ClusterManager::new(config).unwrap();
        
        assert_eq!(manager.local_node().role, NodeRole::Coordinator);
        
        manager.start().await.unwrap();
        
        // Register a worker node
        let worker_id = Uuid::new_v4().to_string();
        let worker_node = NodeInfo {
            id: worker_id.clone(),
            role: NodeRole::Worker,
            address: "localhost:8000".to_string(),
            last_heartbeat: Instant::now(),
            metadata: HashMap::new(),
        };
        
        manager.register_node(worker_node.clone()).unwrap();
        
        // Check that the node was registered
        let retrieved_node = manager.get_node(&worker_id).unwrap();
        assert_eq!(retrieved_node.id, worker_id);
        assert_eq!(retrieved_node.role, NodeRole::Worker);
        
        // Get all nodes
        let nodes = manager.get_nodes();
        assert_eq!(nodes.len(), 2); // Local node + worker node
        
        // Get nodes by role
        let coordinators = manager.get_nodes_by_role(NodeRole::Coordinator);
        assert_eq!(coordinators.len(), 1);
        
        let workers = manager.get_nodes_by_role(NodeRole::Worker);
        assert_eq!(workers.len(), 1);
        
        manager.stop().await.unwrap();
    }
}
