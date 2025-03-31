//! Cluster module for managing the distributed actor system.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use actix::prelude::*;
use actix::dev::ToEnvelope;
use tokio::sync::{RwLock, Mutex};
use log::{debug, error, info};

use crate::config::ClusterConfig;
use crate::discovery::ServiceDiscovery;
use crate::node::{Node, NodeId, NodeInfo, NodeStatus};
use crate::error::{ClusterError, ClusterResult};
use crate::message::{MessageEnvelope, MessageType, DeliveryGuarantee, AnyMessage};
use crate::registry::{ActorRegistry, RegistryActor, RegisterLocal, Lookup, DiscoverActor};
use crate::transport::{RemoteActorRef, P2PTransport};
use crate::config::NodeRole;

// Import traits
use crate::registry::ActorRef;

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
    pub transport: Option<Arc<Mutex<crate::transport::P2PTransport>>>,
    
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
    pub async fn start(&mut self) -> ClusterResult<Addr<ClusterSystemActor>> {
        info!("Starting cluster system...");
        debug!("Local node: {:?}", self.local_node);
        debug!("Config: {:?}", self.config);
        println!("Starting cluster system with local node: {:?}", self.local_node);
        
        // 创建传输层
        let transport = match self.config.architecture {
            Architecture::Decentralized => {
                // 对于去中心化架构，使用P2P传输
                info!("Creating P2P transport for decentralized architecture");
                println!("Creating P2P transport for decentralized architecture");
                let transport = P2PTransport::new(
                    self.local_node.clone(),
                    self.config.serialization_format,
                )?;
                Some(Arc::new(Mutex::new(transport)))
            },
            Architecture::Centralized => {
                if self.local_node.role == NodeRole::Peer {
                    // 对于中心化架构中的对等节点，也使用P2P传输
                    info!("Creating P2P transport for centralized architecture (peer node)");
                    println!("Creating P2P transport for centralized architecture (peer node)");
                    let transport = P2PTransport::new(
                        self.local_node.clone(),
                        self.config.serialization_format,
                    )?;
                    Some(Arc::new(Mutex::new(transport)))
                } else {
                    // 对于中心化架构中的主节点，目前不需要传输层
                    info!("No transport needed for centralized master node");
                    println!("No transport needed for centralized master node");
                    None
                }
            }
        };
        
        self.transport = transport.clone();
        
        // 创建注册表Actor
        let registry_actor = RegistryActor::new(self.registry.clone()).start();
        self.registry_actor = Some(registry_actor);
        
        println!("Registry actor started");
        
        // Create a new instance of the registry with the correct node ID
        let mut new_registry = ActorRegistry::new(self.local_node.id.clone());
        
        // If we have a transport, set it on the registry first
        if let Some(transport) = &transport {
            println!("Setting transport for registry");
            new_registry.set_transport(transport.clone());
        }
        
        // Replace the old registry with the new one that has transport set
        self.registry = Arc::new(new_registry);
        
        // Now that we have a registry with transport, update the registry_actor
        if let Some(registry_actor) = &self.registry_actor {
            // Update the registry actor with our new registry
            registry_actor.do_send(crate::registry::UpdateRegistry {
                registry: self.registry.clone(),
            });
        }
        
        // If we have a transport, set the registry adapter
        if let Some(transport) = &transport {
            println!("Setting registry adapter for transport");
            let mut transport_lock = transport.lock().await;
            
            // Set the registry adapter with our updated registry that has transport set
            transport_lock.set_registry_adapter(self.registry.clone());
            
            // 启动传输层
            if !transport_lock.is_started() {
                debug!("Starting transport layer");
                println!("Starting transport layer");
                transport_lock.init().await?;
                
                // Let the transport layer know about itself
                println!("Transport layer started successfully");
            }
        }
        
        // 创建系统Actor
        let system_actor = ClusterSystemActor::new(
            self.config.clone(),
            self.local_node.clone(),
            self.nodes.clone(),
            self.discovery.clone(),
            self.transport.clone(),
        );
        
        // 启动系统Actor
        let addr = system_actor.start();
        self.system_actor = Some(addr.clone());
        
        println!("Cluster system started successfully");
        Ok(addr)
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
    
    /// 注册本地Actor - 使用SimpleActorRef来注册
    pub async fn register<A>(&self, path: &str, addr: Addr<A>) -> ClusterResult<()>
    where
        A: Actor + Handler<AnyMessage> + 'static,
        <A as Handler<AnyMessage>>::Result: Send,
        A::Context: ToEnvelope<A, AnyMessage>,
    {
        // 检查是否已初始化
        if self.registry_actor.is_none() {
            return Err(ClusterError::RegistryNotInitialized);
        }
        
        // 创建一个SimpleActorRef
        let actor_ref = Box::new(SimpleActorRef::new(addr, path.to_string())) as Box<dyn ActorRef>;
        
        // 发送注册消息
        self.registry_actor.as_ref().unwrap()
            .send(RegisterLocal {
                path: path.to_string(), 
                actor_ref
            })
            .await?
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
            Some(RemoteActorRef::new(
                node_id.clone(),
                path.to_string(),
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
        if let Some(_transport) = &self.transport {
            let remote_ref = self.lookup_remote(target_node, target_actor).await
                .ok_or_else(|| ClusterError::ActorNotFound(target_actor.to_string()))?;
            
            remote_ref.send(message).await
        } else {
            Err(ClusterError::TransportNotInitialized)
        }
    }

    /// 发现并获取远程Actor
    pub async fn discover_actor(&self, path: &str) -> Option<Box<dyn ActorRef>> {
        if self.registry_actor.is_none() {
            return None;
        }
        
        match self.registry_actor.as_ref().unwrap().send(DiscoverActor { 
            path: path.to_string() 
        }).await {
            Ok(actor_ref) => actor_ref,
            Err(e) => {
                error!("Failed to discover actor: {}", e);
                None
            }
        }
    }

    /// Get a list of all known peers
    pub async fn get_peers(&self) -> Vec<NodeInfo> {
        if let Some(ref cluster_actor) = self.system_actor {
            match cluster_actor.send(GetPeers {}).await {
                Ok(peers) => peers,
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }

    /// Get a list of all local actors
    pub async fn get_local_actors(&self) -> Vec<String> {
        if let Some(ref registry_actor) = self.registry_actor {
            match registry_actor.send(GetLocalActors {}).await {
                Ok(actors) => actors,
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }

    /// Send a message to an actor
    #[allow(unused_variables)]
    pub fn send<M: 'static + actix::Message + Send>(
        &self,
        actor_path: String,
        message: M,
        _delivery_guarantee: DeliveryGuarantee,
    ) -> ClusterResult<()> {
        // Lookup the actor in the registry
        if let Some(actor_ref) = self.registry.lookup(&actor_path) {
            actor_ref.send_any(Box::new(message))?;
            return Ok(());
        } else {
            // Not a local actor - attempt to route via the registry
            // We need to implement a way to send remote messages
            log::warn!("Actor not found locally: {}", actor_path);
            return Err(ClusterError::ActorNotFound(actor_path));
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
                        let message = crate::transport::TransportMessage::StatusUpdate(
                            node_id.clone(),
                            status.to_string()
                        );
                        
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
                
                // Create async function to discover nodes
                ctx.spawn(async move {
                    let mut discovery = discovery_clone.lock().await;
                    discovery.discover_nodes().await
                }
                .into_actor(act)
                .map(move |res, _act, _ctx| {
                    if let Ok(nodes) = res {
                        debug!("Discovered {} nodes", nodes.len());
                        // Process discovered nodes
                    } else {
                        error!("Failed to discover nodes: {:?}", res);
                    }
                }));
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
    type Result = ();  // 与MessageEnvelope::Result类型一致
    
    fn handle(&mut self, envelope: MessageEnvelope, ctx: &mut Self::Context) {
        // 使用一个异步块来处理消息，避免阻塞actor
        let transport = self.transport.clone();
        let nodes = self.nodes.clone();
        
        ctx.spawn(
            async move {
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
                            let mut t = transport.lock().await;
                            debug!("Received ping, sending pong to {}", envelope.sender_node);
                            let response = MessageEnvelope::new(
                                envelope.target_node.clone(),
                                envelope.sender_node.clone(),
                                "system".to_string(),
                                MessageType::Pong,
                                DeliveryGuarantee::AtMostOnce,
                                vec![],
                            );
                            if let Err(e) = t.send_envelope(response).await {
                                error!("Failed to send pong response: {}", e);
                            }
                        }
                    },
                    MessageType::Pong => {
                        // Update last seen timestamp for the node
                        let mut node_map = nodes.write().await;
                        if let Some(node) = node_map.get_mut(&envelope.sender_node) {
                            node.update_last_seen();
                            debug!("Updated last seen for node {}", envelope.sender_node);
                        }
                    },
                }
            }
            .into_actor(self)
            .map(|_, _, _| ()) // 忽略结果
        );
    }
}

/// 简单的ActorRef实现，避免使用泛型参数
pub struct SimpleActorRef {
    /// Actor路径
    path: String,
    /// Actor的发送函数，使用Arc允许克隆
    sender: Arc<dyn Fn(Box<dyn std::any::Any + Send>) -> ClusterResult<()> + Send + Sync>,
}

impl SimpleActorRef {
    /// 创建一个新的SimpleActorRef
    pub fn new<A>(addr: Addr<A>, path: String) -> Self 
    where 
        A: Actor + Handler<AnyMessage>,
        <A as Handler<AnyMessage>>::Result: Send,
        A::Context: ToEnvelope<A, AnyMessage>,
    {
        let sender = Arc::new(move |msg| {
            // 这里我们克隆addr来创建一个新的引用，这样可以在多个地方使用
            let addr_clone = addr.clone();
            addr_clone.do_send(AnyMessage(msg));
            Ok(())
        });
        
        Self { path, sender }
    }
}

impl Clone for SimpleActorRef {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            sender: self.sender.clone(),
        }
    }
}

impl ActorRef for SimpleActorRef {
    fn send_any(&self, msg: Box<dyn std::any::Any + Send>) -> ClusterResult<()> {
        (self.sender)(msg)
    }
    
    fn path(&self) -> &str {
        &self.path
    }
    
    fn clone_box(&self) -> Box<dyn ActorRef> {
        Box::new(self.clone())
    }
}

/// Message to get a list of all known peers
#[derive(Message)]
#[rtype(result = "Vec<NodeInfo>")]
pub struct GetPeers {}

impl Handler<GetPeers> for ClusterSystemActor {
    type Result = MessageResult<GetPeers>;
    
    fn handle(&mut self, _msg: GetPeers, _ctx: &mut Self::Context) -> Self::Result {
        let peers = {
            let nodes = self.nodes.blocking_read();
            nodes.values()
                .filter(|node| node.status() == NodeStatus::Up)
                .map(|node| node.info.clone())
                .collect()
        };
        
        MessageResult(peers)
    }
}

/// Message to get a list of all local actors
#[derive(Message)]
#[rtype(result = "Vec<String>")]
pub struct GetLocalActors {}

impl Handler<GetLocalActors> for RegistryActor {
    type Result = MessageResult<GetLocalActors>;
    
    fn handle(&mut self, _msg: GetLocalActors, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(registry) = self.registry() {
            MessageResult(registry.get_local_actors())
        } else {
            MessageResult(Vec::new())
        }
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