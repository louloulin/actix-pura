//! Cluster module for managing the distributed actor system.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use actix::prelude::*;
use tokio::sync::{RwLock, Mutex};

use crate::config::ClusterConfig;
use crate::discovery::ServiceDiscovery;
use crate::node::{Node, NodeId, NodeInfo, NodeStatus};
use crate::error::{ClusterError, ClusterResult};

/// Cluster architecture type
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Architecture {
    /// Centralized architecture with master-worker nodes
    Centralized,
    
    /// Decentralized architecture with peer nodes
    Decentralized,
}

/// ClusterSystem is the main entry point for the distributed actor system
pub struct ClusterSystem {
    /// Cluster configuration
    config: ClusterConfig,
    
    /// Local node
    local_node: NodeInfo,
    
    /// Known nodes in the cluster
    nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
    
    /// Service discovery
    discovery: Arc<Mutex<Box<dyn ServiceDiscovery>>>,
    
    /// System actor address
    system_actor: Option<Addr<ClusterSystemActor>>,
    
    /// P2P transport for decentralized architecture
    transport: Option<Arc<Mutex<crate::transport::P2PTransport>>>,
}

impl ClusterSystem {
    /// Create a new cluster system with the given name and configuration
    pub fn new(name: &str, config: ClusterConfig) -> Self {
        // Generate node info based on configuration
        let node_id = NodeId::new();
        let node_name = format!("{}-{}", name, node_id);
        
        let local_node = NodeInfo::new(
            node_id,
            node_name,
            config.node_role.clone(),
            config.bind_addr,
        );
        
        // Create discovery service based on configuration
        let discovery = crate::discovery::create_discovery_service(config.discovery.clone());
        
        ClusterSystem {
            config,
            local_node,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            discovery: Arc::new(Mutex::new(discovery)),
            system_actor: None,
            transport: None,
        }
    }
    
    /// Start the cluster system
    pub async fn start(mut self) -> ClusterResult<Addr<ClusterSystemActor>> {
        // Initialize transport for decentralized architecture
        if self.config.architecture == Architecture::Decentralized {
            // Create and initialize P2P transport
            let mut transport = crate::transport::P2PTransport::new(
                self.local_node.clone(),
                self.config.serialization_format,
            )?;
            
            transport.init().await?;
            
            self.transport = Some(Arc::new(Mutex::new(transport)));
        }
        
        // Initialize the discovery service
        {
            let mut discovery = self.discovery.lock().await;
            discovery.register_node(&self.local_node).await?;
            discovery.init().await?;
        }
        
        // Create and start the system actor
        let system_actor = ClusterSystemActor::new(
            self.config.clone(),
            self.local_node.clone(),
            self.nodes.clone(),
            self.discovery.clone(),
            self.transport.clone(),
        ).start();
        
        self.system_actor = Some(system_actor.clone());
        
        Ok(system_actor)
    }
    
    /// Get the local node information
    pub fn local_node(&self) -> &NodeInfo {
        &self.local_node
    }
    
    /// Get the cluster configuration
    pub fn config(&self) -> &ClusterConfig {
        &self.config
    }
}

/// ClusterSystemActor manages the cluster state and handles communication
pub struct ClusterSystemActor {
    /// Cluster configuration
    config: ClusterConfig,
    
    /// Local node information
    local_node: NodeInfo,
    
    /// Known nodes in the cluster
    nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
    
    /// Service discovery
    discovery: Arc<Mutex<Box<dyn ServiceDiscovery>>>,
    
    /// P2P transport for decentralized architecture
    transport: Option<Arc<Mutex<crate::transport::P2PTransport>>>,
}

impl ClusterSystemActor {
    /// Create a new cluster system actor
    pub fn new(
        config: ClusterConfig,
        local_node: NodeInfo,
        nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
        discovery: Arc<Mutex<Box<dyn ServiceDiscovery>>>,
        transport: Option<Arc<Mutex<crate::transport::P2PTransport>>>,
    ) -> Self {
        ClusterSystemActor {
            config,
            local_node,
            nodes,
            discovery,
            transport,
        }
    }
    
    /// Register the local node with the discovery service
    async fn register_local_node(&self) -> ClusterResult<()> {
        let mut discovery = self.discovery.lock().await;
        discovery.register_node(&self.local_node).await
    }
    
    /// Discover nodes using the discovery service
    async fn discover_nodes(&self) -> ClusterResult<Vec<NodeInfo>> {
        log::debug!("Discovering nodes...");
        let mut discovery = self.discovery.lock().await;
        let result = discovery.discover_nodes().await?;
        
        log::debug!("Discovered {} nodes", result.len());
        Ok(result)
    }
    
    /// Update node statuses based on heartbeats
    async fn update_node_statuses(&self) -> ClusterResult<()> {
        let mut nodes_to_update = Vec::new();
        
        // Collect nodes to update
        {
            let nodes = self.nodes.read().await;
            for (id, node) in nodes.iter() {
                if node.is_timed_out(self.config.node_timeout) {
                    nodes_to_update.push(id.clone());
                }
            }
        }
        
        // Update node statuses in discovery service
        let mut discovery = self.discovery.lock().await;
        for node_id in nodes_to_update {
            discovery.update_node_status(&node_id, NodeStatus::Unreachable).await?;
        }
        
        Ok(())
    }

    /// Check for node timeouts and update status accordingly
    fn check_node_timeouts(&mut self) {
        // This needs to be done asynchronously, but we're in a synchronous function
        // For now, we'll just log a warning
        log::warn!("check_node_timeouts is not fully implemented for asynchronous access");
    }

    /// Send a heartbeat to all known nodes
    fn send_heartbeat(&mut self, ctx: &mut <Self as Actor>::Context) {
        log::debug!("Sending heartbeat from node {}", self.local_node.id);
        
        // In decentralized mode, send heartbeats directly via transport
        if self.config.architecture == Architecture::Decentralized {
            if let Some(transport) = &self.transport {
                let transport_clone = transport.clone();
                let local_node_info = self.local_node.clone();
                let nodes_clone = self.nodes.clone();
                
                // Send heartbeat in background
                let fut = async move {
                    // Get nodes to send heartbeat to
                    let nodes_vec = {
                        let nodes = nodes_clone.read().await;
                        nodes.keys().cloned().collect::<Vec<NodeId>>()
                    };
                    
                    for node_id in nodes_vec {
                        // Skip local node
                        if node_id == local_node_info.id {
                            continue;
                        }
                        
                        // Send heartbeat message
                        let message = crate::transport::TransportMessage::Heartbeat(local_node_info.clone());
                        let mut transport = transport_clone.lock().await;
                        if let Err(e) = transport.send_message(&node_id, message).await {
                            log::warn!("Failed to send heartbeat to node {}: {}", node_id, e);
                        }
                    }
                    Ok::<(), ClusterError>(())
                };
                
                // Spawn future and convert Result to ()
                let fut = fut.into_actor(self)
                    .map(|res, _act, _ctx| {
                        if let Err(e) = res {
                            log::warn!("Failed to send heartbeats: {}", e);
                        }
                        // Return unit to match ActorFuture<Output = ()>
                    });
                
                ctx.spawn(fut);
            }
        }
        
        // In both modes, update discovery service
        let discovery = self.discovery.clone();
        let local_node = self.local_node.clone();
        
        let fut = async move {
            let mut discovery = discovery.lock().await;
            discovery.update_node_status(&local_node.id, NodeStatus::Up).await
        }.into_actor(self)
        .map(|res, _act, _ctx| {
            if let Err(e) = res {
                log::warn!("Failed to update local node status: {}", e);
            }
            // Return unit to match ActorFuture<Output = ()>
        });
        
        ctx.spawn(fut);
    }
    
    /// Notify about node status change
    fn notify_node_status_changed(&mut self, node_id: &NodeId, status: NodeStatus) {
        log::info!("Node {} status changed to {:?}", node_id, status);
        
        // In decentralized mode, notify other nodes via transport
        if self.config.architecture == Architecture::Decentralized {
            if let Some(transport) = &self.transport {
                let node_id = node_id.clone();
                let status = status.clone();
                let transport_clone = transport.clone();
                let nodes_clone = self.nodes.clone();
                
                // Send status update in background
                actix::spawn(async move {
                    // Get nodes to notify
                    let nodes_vec = {
                        let nodes = nodes_clone.read().await;
                        nodes.keys().cloned().collect::<Vec<NodeId>>()
                    };
                    
                    for target_id in nodes_vec {
                        // Skip the node that changed status
                        if target_id == node_id {
                            continue;
                        }
                        
                        // Send status update message
                        let message = crate::transport::TransportMessage::StatusUpdate {
                            node_id: node_id.clone(),
                            status: status.clone(),
                        };
                        
                        let mut transport = transport_clone.lock().await;
                        if let Err(e) = transport.send_message(&target_id, message).await {
                            log::warn!("Failed to send status update to node {}: {}", target_id, e);
                        }
                    }
                });
            }
        }
        
        // TODO: Notify local subscribers when actor registry is implemented
    }
}

impl Actor for ClusterSystemActor {
    type Context = Context<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        // Register the local node
        let discovery = self.discovery.clone();
        let local_node = self.local_node.clone();
        
        // Register with the discovery service
        let fut = async move {
            let mut discovery = discovery.lock().await;
            discovery.register_node(&local_node).await
        }.into_actor(self)
        .map(|res, act, ctx| {
            if let Err(e) = res {
                log::error!("Failed to register local node: {}", e);
                ctx.stop();
                return;
            }
            
            log::info!("Local node registered: {}", act.local_node.id);
            
            // Start regular heartbeat
            ctx.run_interval(act.config.heartbeat_interval, |act, ctx| {
                // Execute heartbeat
                act.send_heartbeat(ctx);
            });
            
            // Start discovery interval - using default 30 seconds
            let discovery_interval = Duration::from_secs(30);
            
            ctx.run_interval(discovery_interval, move |act, ctx| {
                // 使用克隆的变量进行节点发现，避免借用act
                let discovery_clone = act.discovery.clone();
                let nodes_clone = act.nodes.clone();
                let config_clone = act.config.clone();
                
                // 创建一个独立的Future
                let discovery_fut = async move {
                    let mut discovery = discovery_clone.lock().await;
                    discovery.discover_nodes().await
                }.into_actor(act)
                .map(move |res, act, _ctx| {
                    if let Err(e) = &res {
                        log::error!("Node discovery failed: {}", e);
                    } else if let Ok(discovered_nodes) = res {
                        // 处理发现的节点
                        log::debug!("Discovered {} nodes", discovered_nodes.len());
                        
                        // 简单记录节点发现，实际项目中可能要更新节点信息
                        for node_info in discovered_nodes {
                            log::debug!("Found node: {} at {}", 
                                node_info.id, node_info.addr);
                        }
                    }
                });
                
                // 生成discovery_fut
                ctx.spawn(discovery_fut);
                
                // 只记录操作不检查超时
                log::debug!("Checking node timeouts");
                // 实际的timeout检查应该在此处实现
            });
        });
        
        ctx.spawn(fut);
    }
    
    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        log::info!("Cluster system actor is stopping");
        Running::Stop
    }
    
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        log::info!("Cluster system actor stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{NodeRole, DiscoveryMethod};
    use crate::discovery::MockDiscovery;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_cluster_system_creation() {
        let config = ClusterConfig::new()
            .architecture(Architecture::Decentralized)
            .node_role(NodeRole::Peer)
            .cluster_name("test-cluster".to_string())
            .build()
            .unwrap();
        
        let system = ClusterSystem::new("test", config);
        
        assert_eq!(system.config().architecture, Architecture::Decentralized);
        assert_eq!(system.config().node_role, NodeRole::Peer);
        assert_eq!(system.local_node().role, NodeRole::Peer);
    }
} 