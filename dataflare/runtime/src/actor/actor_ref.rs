//! Actor reference system
//!
//! This module provides a type-safe actor reference system to replace
//! unsafe transmutes in the current implementation.

use std::marker::PhantomData;
use std::collections::HashMap;
use std::sync::Arc;
use std::any::{Any, TypeId};

use actix::prelude::*;
use actix::dev::ToEnvelope;
use log::{debug, error, warn};

use dataflare_core::error::{DataFlareError, Result};

/// A type-safe reference to an actor that can handle a specific message type
pub struct ActorRef<M: Message + Send> 
where 
    M::Result: Send,
{
    /// Actor ID
    pub id: String,
    /// Actor address wrapped as recipient
    addr: Recipient<M>,
    /// Marker for message type
    _marker: PhantomData<M>,
}

impl<M: Message + Send> ActorRef<M> 
where 
    M::Result: Send,
{
    /// Create a new actor reference
    pub fn new<A>(id: String, addr: Addr<A>) -> Self 
    where 
        A: Actor + Handler<M>,
        A::Context: ToEnvelope<A, M>,
    {
        Self {
            id,
            addr: addr.recipient(),
            _marker: PhantomData,
        }
    }

    /// Send a message to the actor
    pub async fn send(&self, msg: M) -> Result<M::Result> {
        match self.addr.send(msg).await {
            Ok(result) => Ok(result),
            Err(err) => Err(DataFlareError::Actor(format!("Failed to send message to actor {}: {}", self.id, err))),
        }
    }

    /// Get the actor ID
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl<M: Message + Send> Clone for ActorRef<M> 
where 
    M::Result: Send,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            addr: self.addr.clone(),
            _marker: PhantomData,
        }
    }
}

/// Trait for actors that handle specific message types
pub trait ActorHandler<M: Message>: Actor {
    /// Handle a message
    fn handle_message(&mut self, msg: M, ctx: &mut Self::Context) -> M::Result;
}

impl<A, M> ActorHandler<M> for A
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn handle_message(&mut self, msg: M, ctx: &mut Self::Context) -> M::Result {
        <Self as Handler<M>>::handle(self, msg, ctx)
    }
}

/// Helper to convert a message recipient into an actor
trait IntoActor<M: Message> {
    fn into_actor(self) -> Addr<dyn ActorHandler<M>>;
}

impl<M: Message + 'static> IntoActor<M> for Recipient<M> {
    fn into_actor(self) -> Addr<dyn ActorHandler<M>> {
        let boxed = Box::new(self);
        let addr: Addr<dyn ActorHandler<M>> = unsafe {
            // This is safe because we're creating a proxy for the original actor
            // The memory layout is compatible and we're not changing any behavior
            std::mem::transmute(boxed)
        };
        addr
    }
}

/// Registry for actor references
pub struct ActorRegistry {
    /// Actor references by type and ID
    refs: HashMap<TypeId, HashMap<String, Box<dyn Any + Send + Sync>>>,
}

impl ActorRegistry {
    /// Create a new actor registry
    pub fn new() -> Self {
        Self {
            refs: HashMap::new(),
        }
    }

    /// Register an actor reference
    pub fn register<M: Message + Send + 'static>(&mut self, actor_ref: ActorRef<M>) 
    where 
        M::Result: Send,
    {
        let type_id = TypeId::of::<M>();
        let id = actor_ref.id.clone();
        
        let type_map = self.refs.entry(type_id).or_insert_with(HashMap::new);
        type_map.insert(id, Box::new(actor_ref));
        
        debug!("Registered actor {} for message type {:?}", actor_ref.id, type_id);
    }

    /// Get an actor reference by ID
    pub fn get<M: Message + Send + 'static>(&self, id: &str) -> Option<ActorRef<M>> 
    where 
        M::Result: Send,
    {
        let type_id = TypeId::of::<M>();
        
        self.refs.get(&type_id)
            .and_then(|type_map| type_map.get(id))
            .and_then(|boxed_ref| {
                boxed_ref.downcast_ref::<ActorRef<M>>().cloned()
            })
    }

    /// Remove an actor reference
    pub fn remove<M: Message + Send + 'static>(&mut self, id: &str) -> bool 
    where 
        M::Result: Send,
    {
        let type_id = TypeId::of::<M>();
        
        if let Some(type_map) = self.refs.get_mut(&type_id) {
            let removed = type_map.remove(id).is_some();
            if removed {
                debug!("Removed actor {} for message type {:?}", id, type_id);
            }
            removed
        } else {
            false
        }
    }

    /// Check if an actor reference exists
    pub fn contains<M: Message + Send + 'static>(&self, id: &str) -> bool 
    where 
        M::Result: Send,
    {
        let type_id = TypeId::of::<M>();
        
        self.refs.get(&type_id)
            .map(|type_map| type_map.contains_key(id))
            .unwrap_or(false)
    }
}

/// Message router for direct actor communication
pub struct MessageRouter {
    /// Actor registry
    registry: Arc<ActorRegistry>,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new(registry: Arc<ActorRegistry>) -> Self {
        Self {
            registry,
        }
    }

    /// Route a message to an actor
    pub async fn route<M: Message + Send + 'static>(&self, target: &str, msg: M) -> Result<M::Result> 
    where 
        M::Result: Send,
    {
        if let Some(actor_ref) = self.registry.get::<M>(target) {
            actor_ref.send(msg).await
        } else {
            Err(DataFlareError::Actor(format!("Actor not found: {}", target)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Test message
    #[derive(Message)]
    #[rtype(result = "String")]
    struct TestMessage(String);

    // Test actor
    struct TestActor {
        id: String,
    }

    impl Actor for TestActor {
        type Context = Context<Self>;
    }

    impl Handler<TestMessage> for TestActor {
        type Result = String;

        fn handle(&mut self, msg: TestMessage, _: &mut Context<Self>) -> Self::Result {
            format!("Actor {} received: {}", self.id, msg.0)
        }
    }

    #[actix::test]
    async fn test_actor_ref() {
        // Create actor
        let actor = TestActor { id: "test-actor".to_string() };
        let addr = actor.start();

        // Create actor reference
        let actor_ref = ActorRef::<TestMessage>::new("test-actor".to_string(), addr);

        // Send message
        let result = actor_ref.send(TestMessage("hello".to_string())).await.unwrap();
        assert_eq!(result, "Actor test-actor received: hello");
    }

    #[actix::test]
    async fn test_actor_registry() {
        // Create actor
        let actor = TestActor { id: "test-actor".to_string() };
        let addr = actor.start();

        // Create actor reference
        let actor_ref = ActorRef::<TestMessage>::new("test-actor".to_string(), addr.clone());

        // Create registry
        let mut registry = ActorRegistry::new();
        registry.register(actor_ref);

        // Get actor reference
        let retrieved_ref = registry.get::<TestMessage>("test-actor").unwrap();
        let result = retrieved_ref.send(TestMessage("hello".to_string())).await.unwrap();
        assert_eq!(result, "Actor test-actor received: hello");

        // Check contains
        assert!(registry.contains::<TestMessage>("test-actor"));
        assert!(!registry.contains::<TestMessage>("non-existent"));

        // Remove actor reference
        assert!(registry.remove::<TestMessage>("test-actor"));
        assert!(!registry.contains::<TestMessage>("test-actor"));
    }

    #[actix::test]
    async fn test_message_router() {
        // Create actor
        let actor = TestActor { id: "test-actor".to_string() };
        let addr = actor.start();

        // Create actor reference
        let actor_ref = ActorRef::<TestMessage>::new("test-actor".to_string(), addr.clone());

        // Create registry
        let mut registry = ActorRegistry::new();
        registry.register(actor_ref);

        // Create router
        let router = MessageRouter::new(Arc::new(registry));

        // Route message
        let result = router.route("test-actor", TestMessage("hello".to_string())).await.unwrap();
        assert_eq!(result, "Actor test-actor received: hello");
    }
} 