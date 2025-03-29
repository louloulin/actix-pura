//! Cluster module for managing the distributed actor system.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use actix::prelude::*;
use tokio::sync::{RwLock, Mutex};
use log::*;

use crate::config::ClusterConfig;
use crate::discovery::ServiceDiscovery;
use crate::node::{Node, NodeId, NodeInfo, NodeStatus};
use crate::error::{ClusterError, ClusterResult};
use crate::message::{ActorPath, MessageEnvelope, MessageType, DeliveryGuarantee};
use crate::registry::{ActorRegistry, RegistryActor, RegisterLocal, RegisterRemote, Lookup};
use crate::transport::{RemoteActorRef, TransportMessage, P2PTransport};

// 导入需要的特质
pub use crate::registry::{ActorRef, LocalActorRef};

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
    
    /// Actor注册表
    registry: Arc<ActorRegistry>,
    
    /// 注册表Actor
    registry_actor: Option<Addr<RegistryActor>>,
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
        
        // 创建注册表
        let registry = Arc::new(ActorRegistry::new(local_node.id.clone()));
        
        ClusterSystem {
            config,
            local_node,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            discovery: Arc::new(Mutex::new(discovery)),
            system_actor: None,
            transport: None,
            registry,
            registry_actor: None,
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
        
        // 启动注册表Actor
        let registry_actor = RegistryActor::new(self.registry.clone()).start();
        self.registry_actor = Some(registry_actor);
        
        // 设置传输到注册表
        if let Some(transport) = &self.transport {
            let mut registry = self.registry.clone();
            registry.set_transport(transport.clone());
        }
        
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
    
    /// 获取Actor注册表
    pub fn registry(&self) -> Arc<ActorRegistry> {
        self.registry.clone()
    }
    
    /// 注册本地Actor
    pub async fn register<A: 'static>(&self, path: &str, addr: Addr<A>) -> ClusterResult<()>
    where
        A: Actor + Handler<Box<dyn std::any::Any + Send>>,
        <A as Handler<Box<dyn std::any::Any + Send>>>::Result: Send,
    {
        let actor_ref = Box::new(LocalActorRef::new(addr, path.to_string()));
        if let Some(registry) = &self.registry_actor {
            registry.send(RegisterLocal {
                path: path.to_string(),
                actor_ref,
            }).await?
        } else {
            Err(ClusterError::RegistryNotInitialized)
        }
    }
    
    /// 查找Actor
    pub async fn lookup(&self, path: &str) -> Option<Box<dyn ActorRef>> {
        if let Some(registry) = &self.registry_actor {
            match registry.send(Lookup { path: path.to_string() }).await {
                Ok(actor_ref) => actor_ref,
                Err(_) => None,
            }
        } else {
            None
        }
    }
    
    /// 查找远程Actor
    pub async fn lookup_remote(&self, node_id: &NodeId, path: &str) -> Option<RemoteActorRef> {
        if let Some(transport) = &self.transport {
            let actor_path = ActorPath::new(node_id.clone(), path.to_string());
            Some(RemoteActorRef::new(
                actor_path,
                transport.clone(),
                DeliveryGuarantee::AtLeastOnce,
            ))
        } else {
            None
        }
    }
    
    /// 发送远程消息
    pub async fn send_remote<M: serde::Serialize + Message + 'static>(
        &self,
        target_node: &NodeId,
        target_actor: &str,
        message: M,
        delivery_guarantee: DeliveryGuarantee,
    ) -> ClusterResult<()> {
        if let Some(transport) = &self.transport {
            let remote_ref = self.lookup_remote(target_node, target_actor).await
                .ok_or_else(|| ClusterError::ActorNotFound(target_actor.to_string()))?;
            
            remote_ref.send(message).await
        } else {
            Err(ClusterError::TransportNotInitialized)
        }
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

impl Handler<MessageEnvelope> for ClusterSystemActor {
    type Result = ();
    
    fn handle(&mut self, envelope: MessageEnvelope, ctx: &mut Self::Context) {
        // Clone necessary data for async processing
        let transport = self.transport.clone();
        let nodes = self.nodes.clone();
        
        // Process the message asynchronously
        let fut = async move {
            debug!("Handling message envelope: {:?}", envelope);
            
            // Check message type and route accordingly
            match envelope.message_type {
                MessageType::ActorMessage => {
                    // In a full implementation, this would look up the local actor
                    // and forward the message
                    debug!("Received actor message for {}", envelope.target_actor);
                },
                MessageType::SystemControl => {
                    // Handle system control messages
                    debug!("Received system control message");
                },
                MessageType::Discovery => {
                    // Handle discovery related messages
                    debug!("Received discovery message");
                },
                MessageType::Ping => {
                    // Handle ping messages - respond with pong
                    if let Some(transport) = &transport {
                        if let Ok(mut t) = transport.lock().await {
                            debug!("Received ping, sending pong to {}", envelope.source_node);
                            let response = MessageEnvelope::new(
                                envelope.target_node.clone(),
                                envelope.source_node.clone(),
                                "system".to_string(),
                                MessageType::Pong,
                                DeliveryGuarantee::AtMostOnce,
                                vec![],
                            );
                            if let Err(e) = t.send_envelope(response).await {
                                error!("Failed to send pong: {}", e);
                            }
                        }
                    }
                },
                MessageType::Pong => {
                    // Update last seen timestamp for the node
                    if let Ok(mut node_map) = nodes.write().await {
                        if let Some(node) = node_map.get_mut(&envelope.source_node) {
                            node.update_last_seen();
                            debug!("Updated last seen for node {}", envelope.source_node);
                        }
                    }
                },
            }
        };
        
        // Spawn the future into the actor context
        ctx.spawn(fut.into_actor(self));
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