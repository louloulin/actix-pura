//! Actor Registry Module for the Actix Cluster
//!
//! This module provides a registry for actors in the cluster, allowing actors to be
//! registered both locally and remotely, and looked up by name.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use actix::dev::ToEnvelope;
use log::{debug, error, info};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::error::{ClusterError, ClusterResult};
use crate::message::{ActorPath, DeliveryGuarantee, MessageEnvelope, AnyMessage};
use crate::node::{NodeId, NodeInfo};
use crate::transport::{P2PTransport, RemoteActorRef, TransportMessage};

/// Actor Placement Strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementStrategy {
    /// Always place actor on the local node
    Local,
    /// Place actor on a random node
    Random,
    /// Place actor on the node with the least actors
    LeastBusy,
    /// Place actor using a round-robin strategy
    RoundRobin,
    /// Place actor using consistent hashing
    ConsistentHashing,
    /// Place actor on a specific node
    SpecificNode,
}

/// A reference to an actor that can be sent messages
pub trait ActorRef: Send + Sync {
    /// Send a dynamically typed message to the actor
    fn send_any(&self, msg: Box<dyn std::any::Any + Send>) -> ClusterResult<()>;
    
    /// Get the actor path
    fn path(&self) -> &str;
    
    /// Clone this actor reference
    fn clone_box(&self) -> Box<dyn ActorRef>;
}

/// 实现ActorRef的Clone特质
impl Clone for Box<dyn ActorRef> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// A wrapper for local actor addresses
pub struct LocalActorRef<A: Actor> {
    /// The actor's address
    addr: Addr<A>,
    /// Actor path
    path: String,
}

impl<A: Actor> LocalActorRef<A> {
    /// Create a new local actor reference
    pub fn new(addr: Addr<A>, path: String) -> Self {
        Self { addr, path }
    }
}

impl<A: Actor> Clone for LocalActorRef<A> {
    fn clone(&self) -> Self {
        Self {
            addr: self.addr.clone(),
            path: self.path.clone(),
        }
    }
}

impl<A: Actor> ActorRef for LocalActorRef<A>
where
    A: Handler<AnyMessage>,
    <A as Handler<AnyMessage>>::Result: Send,
    A::Context: ToEnvelope<A, AnyMessage>,
{
    fn send_any(&self, msg: Box<dyn std::any::Any + Send>) -> ClusterResult<()> {
        self.addr.do_send(AnyMessage(msg));
        Ok(())
    }
    
    fn path(&self) -> &str {
        &self.path
    }
    
    fn clone_box(&self) -> Box<dyn ActorRef> {
        Box::new(self.clone())
    }
}

/// The registry for actors in the cluster
pub struct ActorRegistry {
    /// The local node's ID
    local_node_id: NodeId,
    /// Map of local actor paths to addresses
    local_actors: RwLock<HashMap<String, Box<dyn ActorRef>>>,
    /// Map of remote actor paths to node IDs
    remote_actors: RwLock<HashMap<ActorPath, NodeId>>,
    /// Transport for sending messages to remote actors
    transport: Option<Arc<TransportAdapter>>,
    /// Map of pending actor discovery requests, keyed by path
    pending_discoveries: RwLock<HashMap<String, Vec<oneshot::Sender<Option<Box<dyn ActorRef>>>>>>,
}

impl ActorRegistry {
    /// Create a new actor registry
    pub fn new(local_node_id: NodeId) -> Self {
        Self {
            local_node_id,
            local_actors: RwLock::new(HashMap::new()),
            remote_actors: RwLock::new(HashMap::new()),
            transport: None,
            pending_discoveries: RwLock::new(HashMap::new()),
        }
    }
    
    /// Set the transport for sending messages to remote actors
    pub fn set_transport(&mut self, transport: Arc<tokio::sync::Mutex<P2PTransport>>) {
        self.transport = Some(Arc::new(TransportAdapter::new(transport)));
    }
    
    /// Register a local actor with the registry
    pub fn register_local(&self, path: String, actor_ref: Box<dyn ActorRef>) -> ClusterResult<()> {
        let mut actors = self.local_actors.write();
        if actors.contains_key(&path) {
            return Err(ClusterError::ActorAlreadyRegistered(path));
        }
        
        actors.insert(path, actor_ref);
        Ok(())
    }
    
    /// Register a remote actor with the registry
    pub fn register_remote(&self, path: ActorPath, node_id: NodeId) -> ClusterResult<()> {
        let mut actors = self.remote_actors.write();
        actors.insert(path, node_id);
        Ok(())
    }
    
    /// Lookup an actor by path
    pub fn lookup(&self, path: &str) -> Option<Box<dyn ActorRef>> {
        // First check local actors
        let local_actors = self.local_actors.read();
        if let Some(actor_ref) = local_actors.get(path) {
            return Some(actor_ref.clone());
        }
        
        // Then check remote actors
        let remote_actors = self.remote_actors.read();
        let actor_path = ActorPath::new(self.local_node_id.clone(), path.to_string());
        
        if let Some(node_id) = remote_actors.get(&actor_path) {
            if let Some(transport) = &self.transport {
                let remote_path = ActorPath::new(node_id.clone(), path.to_string());
                let remote_ref = RemoteActorRef::new(
                    remote_path,
                    transport.transport.clone(),
                    DeliveryGuarantee::AtLeastOnce,
                );
                
                // Wrap in a trait object
                return Some(Box::new(remote_ref) as Box<dyn ActorRef>);
            }
        }
        
        None
    }
    
    /// Deregister a local actor
    pub fn deregister_local(&self, path: &str) -> ClusterResult<()> {
        let mut actors = self.local_actors.write();
        if actors.remove(path).is_none() {
            return Err(ClusterError::ActorNotFound(path.to_string()));
        }
        
        Ok(())
    }
    
    /// Deregister a remote actor
    pub fn deregister_remote(&self, path: &ActorPath) -> ClusterResult<()> {
        let mut actors = self.remote_actors.write();
        if actors.remove(path).is_none() {
            return Err(ClusterError::ActorNotFound(path.path.clone()));
        }
        
        Ok(())
    }
    
    /// Get all registered local actors
    pub fn get_local_actors(&self) -> Vec<String> {
        let actors = self.local_actors.read();
        actors.keys().cloned().collect()
    }
    
    /// Get all registered remote actors
    pub fn get_remote_actors(&self) -> Vec<ActorPath> {
        let actors = self.remote_actors.read();
        actors.keys().cloned().collect()
    }

    /// Discover an actor in the cluster
    pub async fn discover_actor(&self, path: &str) -> Option<Box<dyn ActorRef>> {
        // First check local registry
        if let Some(actor_ref) = self.lookup(path) {
            return Some(actor_ref);
        }

        // If we have no transport, we can't discover actors remotely
        if self.transport.is_none() {
            return None;
        }

        let transport = self.transport.as_ref().unwrap();
        let (sender, receiver) = oneshot::channel();

        // Register the pending discovery request
        {
            let mut pending = self.pending_discoveries.write();
            pending.entry(path.to_string())
                .or_insert_with(Vec::new)
                .push(sender);
        }

        // Send actor discovery request to all known nodes
        let discovery_message = TransportMessage::ActorDiscoveryRequest(
            self.local_node_id.clone(), 
            path.to_string()
        );

        // Get a list of all known nodes from the transport
        let mut transport_lock = transport.transport.lock().await;
        let nodes = transport_lock.get_peer_list();

        // Send discovery request to all nodes
        for node_id in nodes {
            // Skip local node
            if node_id == self.local_node_id {
                continue;
            }

            // Send discovery request to the remote node
            if let Err(e) = transport_lock.send_message(&node_id, discovery_message.clone()).await {
                error!("Failed to send actor discovery request to node {}: {}", node_id, e);
            }
        }
        
        // Wait for a response with timeout
        match tokio::time::timeout(Duration::from_secs(5), receiver).await {
            Ok(Ok(actor_ref)) => actor_ref,
            Ok(Err(_)) => {
                error!("Actor discovery channel was closed");
                None
            },
            Err(_) => {
                // Timeout - clean up pending request
                let mut pending = self.pending_discoveries.write();
                if let Some(senders) = pending.get_mut(path) {
                    senders.retain(|s| !s.is_closed());
                    if senders.is_empty() {
                        pending.remove(path);
                    }
                }
                error!("Actor discovery timed out for path: {}", path);
                None
            }
        }
    }

    /// Handle an actor discovery request from another node
    pub async fn handle_discovery_request(&self, sender_id: NodeId, path: String) -> ClusterResult<()> {
        debug!("Handling actor discovery request from {} for {}", sender_id, path);

        // Check if we have the actor locally
        let actor_found = {
            let local_actors = self.local_actors.read();
            local_actors.contains_key(&path)
        };

        // If we have the transport and the actor, respond with the location
        if actor_found {
            if let Some(transport) = &self.transport {
                // Send actor discovery response
                let response = TransportMessage::ActorDiscoveryResponse(
                    path,
                    vec![self.local_node_id.clone()]
                );

                let mut transport_lock = transport.transport.lock().await;
                transport_lock.send_message(&sender_id, response).await?;
            }
        }

        Ok(())
    }

    /// Handle an actor discovery response from another node
    pub fn handle_discovery_response(&self, path: String, locations: Vec<NodeId>) {
        debug!("Received actor discovery response for {} with locations: {:?}", path, locations);

        // If we have no pending discovery requests for this path, ignore
        let mut pending = self.pending_discoveries.write();
        let senders = match pending.remove(&path) {
            Some(s) => s,
            None => return,
        };

        // If we have a transport, create a remote actor reference
        if let Some(transport) = &self.transport {
            // Use the first location
            if let Some(node_id) = locations.first() {
                // Create a RemoteActorRef
                let actor_path = ActorPath::new(node_id.clone(), path.clone());
                let remote_ref = RemoteActorRef::new(
                    actor_path.clone(),
                    transport.transport.clone(),
                    DeliveryGuarantee::AtLeastOnce,
                );

                // Register the remote actor
                let mut remote_actors = self.remote_actors.write();
                remote_actors.insert(actor_path, node_id.clone());

                // Notify all pending discovery requests
                for sender in senders {
                    let _ = sender.send(Some(Box::new(remote_ref.clone()) as Box<dyn ActorRef>));
                }
                return;
            }
        }

        // If we couldn't create a remote reference, notify failure
        for sender in senders {
            let _ = sender.send(None);
        }
    }
}

/// Actor for managing the registry
#[derive(Default)]
pub struct RegistryActor {
    registry: Option<Arc<ActorRegistry>>,
}

impl RegistryActor {
    /// Create a new registry actor
    pub fn new(registry: Arc<ActorRegistry>) -> Self {
        Self {
            registry: Some(registry),
        }
    }
}

impl Actor for RegistryActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("Registry actor started");
    }
}

/// Message to register a local actor
#[derive(Message)]
#[rtype(result = "ClusterResult<()>")]
pub struct RegisterLocal {
    /// Path to register the actor under
    pub path: String,
    /// Actor address with type erased
    pub actor_ref: Box<dyn ActorRef>,
}

impl Handler<RegisterLocal> for RegistryActor {
    type Result = ClusterResult<()>;
    
    fn handle(&mut self, msg: RegisterLocal, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(registry) = &self.registry {
            registry.register_local(msg.path, msg.actor_ref)
        } else {
            Err(ClusterError::RegistryNotInitialized)
        }
    }
}

/// Message to register a remote actor
#[derive(Message)]
#[rtype(result = "ClusterResult<()>")]
pub struct RegisterRemote {
    /// Actor path
    pub path: ActorPath,
    /// Node hosting the actor
    pub node_id: NodeId,
}

impl Handler<RegisterRemote> for RegistryActor {
    type Result = ClusterResult<()>;
    
    fn handle(&mut self, msg: RegisterRemote, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(registry) = &self.registry {
            registry.register_remote(msg.path, msg.node_id)
        } else {
            Err(ClusterError::RegistryNotInitialized)
        }
    }
}

/// Message to lookup an actor
#[derive(Message)]
#[rtype(result = "Option<Box<dyn ActorRef>>")]
pub struct Lookup {
    /// Path to lookup
    pub path: String,
}

impl Handler<Lookup> for RegistryActor {
    type Result = MessageResult<Lookup>;
    
    fn handle(&mut self, msg: Lookup, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(registry) = &self.registry {
            MessageResult(registry.lookup(&msg.path))
        } else {
            MessageResult(None)
        }
    }
}

/// Message to discover an actor
#[derive(Message)]
#[rtype(result = "Option<Box<dyn ActorRef>>")]
pub struct DiscoverActor {
    /// Path to discover
    pub path: String,
}

impl Handler<DiscoverActor> for RegistryActor {
    type Result = ResponseFuture<Option<Box<dyn ActorRef>>>;
    
    fn handle(&mut self, msg: DiscoverActor, _ctx: &mut Self::Context) -> Self::Result {
        let registry = match &self.registry {
            Some(r) => r.clone(),
            None => return Box::pin(async { None }),
        };
        
        Box::pin(async move {
            registry.discover_actor(&msg.path).await
        })
    }
}

/// 适配器，让Arc<Mutex<P2PTransport>>可以被当作Arc<P2PTransport>使用
pub struct TransportAdapter {
    pub transport: Arc<tokio::sync::Mutex<P2PTransport>>,
}

impl TransportAdapter {
    /// 创建新的适配器
    pub fn new(transport: Arc<tokio::sync::Mutex<P2PTransport>>) -> Self {
        Self { transport }
    }
}

impl Clone for TransportAdapter {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    // Mock actor for testing
    struct MockActor;
    
    impl Actor for MockActor {
        type Context = Context<Self>;
    }
    
    impl Handler<AnyMessage> for MockActor {
        type Result = ();
        
        fn handle(&mut self, _msg: AnyMessage, _ctx: &mut Self::Context) {
            // Just a mock implementation
        }
    }
    
    #[test]
    fn test_actor_registry() {
        let node_id = NodeId::new();
        let registry = ActorRegistry::new(node_id.clone());
        
        // Test local actor registration
        let system = System::new();
        system.block_on(async {
            let addr = MockActor.start();
            
            let result = registry.register_local("test_actor".to_string(), Box::new(LocalActorRef::new(addr, "test_actor".to_string())) as Box<dyn ActorRef>);
            assert!(result.is_ok());
            
            let actors = registry.get_local_actors();
            assert_eq!(actors.len(), 1);
            assert_eq!(actors[0], "test_actor");
            
            // Test lookup
            let actor_ref = registry.lookup("test_actor");
            assert!(actor_ref.is_some());
            
            // Test deregistration
            let result = registry.deregister_local("test_actor");
            assert!(result.is_ok());
            
            let actors = registry.get_local_actors();
            assert_eq!(actors.len(), 0);
        });
    }
    
    #[test]
    fn test_remote_actor_registration() {
        let local_node_id = NodeId::new();
        let remote_node_id = NodeId::new();
        let registry = ActorRegistry::new(local_node_id.clone());
        
        let path = ActorPath::new(remote_node_id.clone(), "remote_actor".to_string());
        
        let result = registry.register_remote(path.clone(), remote_node_id);
        assert!(result.is_ok());
        
        let actors = registry.get_remote_actors();
        assert_eq!(actors.len(), 1);
        assert_eq!(actors[0], path);
        
        let result = registry.deregister_remote(&path);
        assert!(result.is_ok());
        
        let actors = registry.get_remote_actors();
        assert_eq!(actors.len(), 0);
    }
} 