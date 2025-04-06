// Distributed actor implementation
//
// API Optimization Implementation (2024-04-06):
// 1. Added ActorProps for fluent configuration of actors (similar to Proto.Actor Props)
// 2. Improved actor creation with a builder pattern approach
// 3. Added simplified actor() factory function for cleaner instantiation
// 4. Enhanced message delivery with automatic timeout handling
// 5. Added async helpers for easier actor interaction
// 6. Simplified placement strategy configuration with descriptive methods

use std::sync::Arc;
use actix::prelude::*;
use actix::dev::ToEnvelope;
use serde::{Serialize, Deserialize};
use std::fmt;
use std::time::Duration;

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, PlacementStrategy};
use crate::registry::ActorRegistry;
use crate::migration::{MigratableActor, MigrationReason, MigrationOptions};
use crate::serialization::SerializationFormat;

/// Distributed actor trait
pub trait DistributedActor: Actor<Context = Context<Self>> + Clone + 'static {
    /// Get the actor path
    fn actor_path(&self) -> String {
        format!("/user/{}", std::any::type_name::<Self>())
    }
    
    /// Get the placement strategy for this actor
    fn placement_strategy(&self) -> PlacementStrategy {
        PlacementStrategy::Random
    }
    
    /// Get the serialization format for this actor
    fn serialization_format(&self) -> SerializationFormat {
        SerializationFormat::Bincode
    }
    
    /// Migrate this actor to another node
    fn migrate_to(&self, _node_id: NodeId, _reason: MigrationReason) -> ClusterResult<uuid::Uuid> {
        // In a real implementation, this would use the MigrationManager
        Err(ClusterError::NotImplemented("Actor migration".to_string()))
    }
}

/// Actor Props for configuring actor creation
/// Similar to Props in Proto.Actor, providing a fluent API for actor configuration
pub struct ActorProps<A: DistributedActor> {
    /// Actor instance
    actor: A,
    /// Custom actor path
    path: Option<String>,
    /// Placement strategy
    strategy: PlacementStrategy,
    /// Serialization format
    format: SerializationFormat,
    /// Affinity group for local affinity placement
    affinity_group: Option<String>,
}

impl<A: DistributedActor> ActorProps<A> {
    /// Create new actor props with default settings
    pub fn new(actor: A) -> Self {
        Self {
            actor,
            path: None,
            strategy: PlacementStrategy::Random,
            format: SerializationFormat::Bincode,
            affinity_group: None,
        }
    }
    
    /// Set a custom actor path
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
    
    /// Set the placement strategy
    pub fn with_strategy(mut self, strategy: PlacementStrategy) -> Self {
        self.strategy = strategy;
        self
    }
    
    /// Set the serialization format
    pub fn with_format(mut self, format: SerializationFormat) -> Self {
        self.format = format;
        self
    }
    
    /// Use local affinity placement strategy
    pub fn with_local_affinity(mut self, group: Option<String>, fallback: PlacementStrategy) -> Self {
        self.strategy = PlacementStrategy::LocalAffinity {
            fallback: Box::new(fallback),
            group: group.clone(),
        };
        self.affinity_group = group;
        self
    }
    
    /// Use round robin placement strategy
    pub fn with_round_robin(mut self) -> Self {
        self.strategy = PlacementStrategy::RoundRobin;
        self
    }
    
    /// Use least loaded placement strategy
    pub fn with_least_loaded(mut self) -> Self {
        self.strategy = PlacementStrategy::LeastLoaded;
        self
    }
    
    /// Use redundant placement strategy with multiple replicas
    pub fn with_redundancy(mut self, replicas: usize) -> Self {
        self.strategy = PlacementStrategy::Redundant { replicas };
        self
    }
    
    /// Set specific node for placement
    pub fn on_node(mut self, node_id: uuid::Uuid) -> Self {
        self.strategy = PlacementStrategy::Node(node_id);
        self
    }
    
    /// Start the actor with the configured properties
    pub fn start(self) -> Addr<A> {
        // Create the actor with the custom configuration
        let addr = self.actor.clone().start();
        
        // Here we would register with the cluster registry using the configured props
        // In a real implementation, this would get the registry from the system and
        // register the actor with all the custom settings
        
        addr
    }
    
    /// Get the actor instance
    pub fn actor(&self) -> &A {
        &self.actor
    }
    
    /// Get the actor path (custom or default)
    pub fn actor_path(&self) -> String {
        self.path.clone().unwrap_or_else(|| self.actor.actor_path())
    }
    
    /// Get the placement strategy
    pub fn placement_strategy(&self) -> PlacementStrategy {
        self.strategy.clone()
    }
    
    /// Get the serialization format
    pub fn serialization_format(&self) -> SerializationFormat {
        self.format
    }
}

/// Extension methods for distributed actors
pub trait DistributedActorExt: DistributedActor {
    /// Start the actor in a distributed context
    fn start_distributed(self) -> Addr<Self>
    where
        Self: Sized,
    {
        ActorProps::new(self).start()
    }
    
    /// Create props for this actor with fluent configuration API
    fn props(self) -> ActorProps<Self>
    where
        Self: Sized,
    {
        ActorProps::new(self)
    }
    
    /// Start the actor and wait for a message (async version)
    async fn start_and_wait_async<M, T>(self, msg: M, timeout: Duration) -> ClusterResult<T>
    where
        Self: Handler<M> + Sized,
        M: Message<Result = T> + Send + 'static,
        T: 'static + Send,
        Self::Context: ToEnvelope<Self, M>,
    {
        let addr = self.start_distributed();
        match actix::clock::timeout(timeout, addr.send(msg)).await {
            Ok(result) => result.map_err(|e| ClusterError::ActorError(format!("Actor mailbox error: {}", e))),
            Err(_) => Err(ClusterError::Timeout(format!("Timeout waiting for actor response after {:?}", timeout)))
        }
    }
    
    /// Start the actor and wait for a message (sync version)
    fn start_and_wait<M, T>(self, msg: M, timeout: Duration) -> Request<Self, M>
    where
        Self: Handler<M> + Sized,
        M: Message<Result = T> + Send + 'static,
        T: 'static + Send,
        Self::Context: ToEnvelope<Self, M>,
    {
        let addr = self.start_distributed();
        addr.send(msg)
    }
    
    /// Ask the actor for a response (async helper)
    fn ask<M, T>(&self, addr: &Addr<Self>, msg: M) -> Request<Self, M>
    where
        Self: Handler<M>,
        M: Message<Result = T> + Send + 'static,
        T: 'static + Send,
    {
        addr.send(msg)
    }
    
    /// Ask the actor for a response with timeout (async version)
    async fn ask_async<M, T>(&self, addr: &Addr<Self>, msg: M, timeout: Duration) -> ClusterResult<T>
    where
        Self: Handler<M>,
        M: Message<Result = T> + Send + 'static,
        T: 'static + Send,
    {
        match actix::clock::timeout(timeout, addr.send(msg)).await {
            Ok(result) => result.map_err(|e| ClusterError::ActorError(format!("Actor mailbox error: {}", e))),
            Err(_) => Err(ClusterError::Timeout(format!("Timeout asking actor after {:?}", timeout)))
        }
    }
}

// Implement the extension trait for all DistributedActor implementors
impl<T> DistributedActorExt for T 
where
    T: DistributedActor
{}

/// Simplified actor creation function (inspired by Proto.Actor)
pub fn actor<A: DistributedActor>(actor_instance: A) -> ActorProps<A> {
    ActorProps::new(actor_instance)
}

/// Message wrapper for remote actor invocation
#[derive(Message)]
#[rtype(result = "ClusterResult<Vec<u8>>")]
pub struct RemoteInvocation {
    /// Actor path
    pub actor_path: String,
    /// Method name
    pub method: String,
    /// Serialized arguments
    pub args: Vec<u8>,
    /// Serialization format
    pub format: SerializationFormat,
}

/// Actor implementation for system-managed distributed actors
pub struct ClusterSystemActor {
    /// Actor registry
    registry: Option<Arc<ActorRegistry>>,
}

impl ClusterSystemActor {
    /// Create a new cluster system actor
    pub fn new(registry: Arc<ActorRegistry>) -> Self {
        Self {
            registry: Some(registry),
        }
    }
    
    /// Get the actor registry
    pub fn registry(&self) -> Option<Arc<ActorRegistry>> {
        self.registry.clone()
    }
}

impl Actor for ClusterSystemActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("ClusterSystemActor started");
    }
}

/// Message to register a distributed actor
#[derive(Message)]
#[rtype(result = "ClusterResult<()>")]
pub struct RegisterDistributedActor {
    /// Actor path
    pub path: String,
    /// Actor address
    pub addr: Box<dyn crate::registry::ActorRef>,
    /// Placement strategy
    pub strategy: PlacementStrategy,
}

impl Handler<RegisterDistributedActor> for ClusterSystemActor {
    type Result = ClusterResult<()>;
    
    fn handle(&mut self, msg: RegisterDistributedActor, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(registry) = &self.registry {
            registry.register_local(msg.path, msg.addr)
        } else {
            Err(ClusterError::NotInitialized("Actor registry".to_string()))
        }
    }
}

/// Message to handle remote actor invocation
#[derive(Message)]
#[rtype(result = "ClusterResult<Vec<u8>>")]
pub struct HandleRemoteInvocation {
    /// Remote invocation details
    pub invocation: RemoteInvocation,
}

impl Handler<HandleRemoteInvocation> for ClusterSystemActor {
    type Result = ResponseFuture<ClusterResult<Vec<u8>>>;
    
    fn handle(&mut self, msg: HandleRemoteInvocation, _ctx: &mut Self::Context) -> Self::Result {
        let registry = self.registry.clone();
        
        Box::pin(async move {
            if let Some(registry) = registry {
                if let Some(actor_ref) = registry.lookup(&msg.invocation.actor_path) {
                    // In a real implementation, this would deserialize the args,
                    // invoke the method on the actor, and serialize the result
                    Err(ClusterError::NotImplemented("Remote invocation".to_string()))
                } else {
                    Err(ClusterError::ActorNotFound(msg.invocation.actor_path))
                }
            } else {
                Err(ClusterError::NotInitialized("Actor registry".to_string()))
            }
        })
    }
}

/// Unit tests for distributed actors
#[cfg(test)]
mod tests {
    use super::*;
    use actix::prelude::*;
    use crate::migration::MigratableActor;
    
    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestDistributedActor {
        value: i32,
    }
    
    impl Actor for TestDistributedActor {
        type Context = Context<Self>;
        
        fn started(&mut self, ctx: &mut Self::Context) {
            println!("TestDistributedActor started with path: {:?}", ctx.address());
        }
    }
    
    impl MigratableActor for TestDistributedActor {
        fn get_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            Ok(bincode::serialize(self).unwrap())
        }
        
        fn restore_state(&mut self, state: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
            let actor: TestDistributedActor = bincode::deserialize(&state)?;
            self.value = actor.value;
            Ok(())
        }
        
        fn before_migration(&mut self, _ctx: &mut Self::Context) {}
        
        fn after_migration(&mut self, _ctx: &mut Self::Context) {}
        
        fn can_migrate(&self) -> bool {
            true
        }
    }
    
    impl DistributedActor for TestDistributedActor {
        fn actor_path(&self) -> String {
            format!("/user/test-actor-{}", self.value)
        }
        
        fn placement_strategy(&self) -> PlacementStrategy {
            PlacementStrategy::RoundRobin
        }
        
        fn serialization_format(&self) -> SerializationFormat {
            SerializationFormat::Bincode
        }
    }
    
    #[derive(Message)]
    #[rtype(result = "i32")]
    struct GetValue;
    
    impl Handler<GetValue> for TestDistributedActor {
        type Result = i32;
        
        fn handle(&mut self, _msg: GetValue, _ctx: &mut Self::Context) -> Self::Result {
            self.value
        }
    }
    
    #[derive(Message)]
    #[rtype(result = "i32")]
    struct IncrementBy(i32);
    
    impl Handler<IncrementBy> for TestDistributedActor {
        type Result = i32;
        
        fn handle(&mut self, msg: IncrementBy, _ctx: &mut Self::Context) -> Self::Result {
            self.value += msg.0;
            self.value
        }
    }
    
    #[actix_rt::test]
    async fn test_distributed_actor() {
        // Create and start a distributed actor
        let actor = TestDistributedActor { value: 42 };
        let addr = actor.start_distributed();
        
        // Send a message and check the result
        let res = addr.send(GetValue).await.unwrap();
        assert_eq!(res, 42);
    }
    
    #[actix_rt::test]
    async fn test_actor_props() {
        // Create an actor with props
        let actor = TestDistributedActor { value: 42 };
        
        // Test fluent API for actor configuration
        let addr = actor.props()
            .with_path("/user/custom-actor-path")
            .with_local_affinity(Some("test-group".to_string()), PlacementStrategy::RoundRobin)
            .with_format(SerializationFormat::Json)
            .start();
            
        // Send a message and check the result
        let res = addr.send(GetValue).await.unwrap();
        assert_eq!(res, 42);
        
        // Test the increment operation
        let new_value = addr.send(IncrementBy(8)).await.unwrap();
        assert_eq!(new_value, 50);
    }
    
    #[actix_rt::test]
    async fn test_actor_factory() {
        // Test the simplified actor creation function
        let addr = actor(TestDistributedActor { value: 100 })
            .with_round_robin()
            .start();
            
        // Send a message and check the result
        let res = addr.send(GetValue).await.unwrap();
        assert_eq!(res, 100);
    }
    
    #[actix_rt::test]
    async fn test_ask_pattern() {
        // Create an actor - keep a clone for later use
        let actor = TestDistributedActor { value: 42 };
        let actor_ref = actor.clone(); // Clone before moving
        
        // Start the actor - this consumes the original actor
        let addr = actor.start_distributed();
        
        // Test the ask pattern
        let res = addr.send(GetValue).await.unwrap();
        assert_eq!(res, 42);
        
        // Test the ask_async helper with timeout
        let ask_result = actor_ref.ask_async(&addr, IncrementBy(10), Duration::from_secs(1)).await.unwrap();
        assert_eq!(ask_result, 52);
    }
} 