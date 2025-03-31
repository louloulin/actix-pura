// Distributed actor implementation

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

/// Extension methods for distributed actors
pub trait DistributedActorExt: DistributedActor {
    /// Start the actor in a distributed context
    fn start_distributed(self) -> Addr<Self>
    where
        Self: Sized,
    {
        let addr = Self::create(|_| self);
        
        // Here we would register with the cluster registry
        // In a real implementation, this would get the registry from the system
        
        addr
    }
    
    /// Start the actor and wait for a message
    fn start_and_wait<M, T>(self, msg: M, _timeout: Duration) -> Request<Self, M>
    where
        Self: Handler<M> + Sized,
        M: Message<Result = T> + Send + 'static,
        T: 'static + Send,
        Self::Context: ToEnvelope<Self, M>,
    {
        let addr = self.start_distributed();
        addr.send(msg)
    }
}

// Implement the extension trait for all DistributedActor implementors
impl<T> DistributedActorExt for T 
where
    T: DistributedActor
{}

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
    
    #[actix_rt::test]
    async fn test_distributed_actor() {
        // Create and start a distributed actor
        let actor = TestDistributedActor { value: 42 };
        let addr = actor.start_distributed();
        
        // Send a message and check the result
        let res = addr.send(GetValue).await.unwrap();
        assert_eq!(res, 42);
    }
} 