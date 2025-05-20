//! # Message Bus
//!
//! Central message bus for Actor communication in DataFlare.
//! This module implements a flattened communication architecture to reduce Actor nesting.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use actix::prelude::*;
use actix::dev::ToEnvelope;
use actix::WeakAddr;
use log::{debug, error, warn};
use uuid::Uuid;

use dataflare_core::error::{DataFlareError, Result};

/// Unique identifier for actors in the system
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ActorId(pub String);

impl From<String> for ActorId {
    fn from(s: String) -> Self {
        ActorId(s)
    }
}

impl From<&str> for ActorId {
    fn from(s: &str) -> Self {
        ActorId(s.to_string())
    }
}

impl From<&ActorId> for ActorId {
    fn from(id: &ActorId) -> Self {
        id.clone()
    }
}

/// Trace information for message tracking
#[derive(Debug, Clone)]
pub struct TraceInfo {
    /// Unique trace ID
    pub trace_id: Uuid,
    /// Parent trace ID, if any
    pub parent_id: Option<Uuid>,
    /// Timestamp when the trace was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Additional trace metadata
    pub metadata: HashMap<String, String>,
}

impl Default for TraceInfo {
    fn default() -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            parent_id: None,
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }
}

/// Generic message wrapper for the DataFlare system
#[derive(Debug, Clone)]
pub struct DataFlareMessage {
    /// Unique message identifier
    pub message_id: Uuid,
    /// Sender actor ID
    pub sender: ActorId,
    /// Target actor ID
    pub target: ActorId,
    /// Actual message payload - Note: Now uses Arc for thread safety
    pub payload: Arc<dyn Any + Send + Sync>,
    /// Trace information for monitoring and debugging
    pub trace_info: TraceInfo,
}

impl DataFlareMessage {
    /// Create a new DataFlare message
    pub fn new<T: 'static + Send + Sync>(sender: impl Into<ActorId>, target: impl Into<ActorId>, payload: T) -> Self {
        Self {
            message_id: Uuid::new_v4(),
            sender: sender.into(),
            target: target.into(),
            payload: Arc::new(payload),
            trace_info: TraceInfo::default(),
        }
    }
    
    /// Try to downcast the payload to a specific type
    pub fn downcast<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }
}

// Implement Message trait for DataFlareMessage
impl Message for DataFlareMessage {
    type Result = ();
}

/// Message bus for central message routing
pub struct MessageBus {
    /// Registry of actors in the system - using direct Recipient rather than WeakAddr<dyn Trait>
    registry: RwLock<HashMap<ActorId, Recipient<DataFlareMessage>>>,
    /// Message type registry for routing
    type_registry: RwLock<HashMap<TypeId, Vec<ActorId>>>,
}

impl MessageBus {
    /// Create a new message bus
    pub fn new() -> Self {
        Self {
            registry: RwLock::new(HashMap::new()),
            type_registry: RwLock::new(HashMap::new()),
        }
    }
    
    /// Register an actor with the message bus
    pub fn register<A>(&self, id: impl Into<ActorId>, addr: Addr<A>) -> Result<()> 
    where 
        A: Actor + Handler<DataFlareMessage>,
        A::Context: ToEnvelope<A, DataFlareMessage>,
    {
            let id = id.into();
    let recipient = addr.recipient::<DataFlareMessage>();
    
    let mut registry = self.registry.write().map_err(|_| {
        DataFlareError::Actor("Failed to acquire write lock on registry".to_string())
    })?;
    
    let id_clone = id.clone();
    registry.insert(id, recipient);
    debug!("Registered actor: {:?}", id_clone);
        Ok(())
    }
    
    /// Register an actor for a specific message type
    pub fn register_handler<A, M>(&self, id: impl Into<ActorId>, _: std::marker::PhantomData<M>) -> Result<()>
    where 
        A: Actor + Handler<M>,
        M: 'static + Message,
    {
        let id = id.into();
        let type_id = TypeId::of::<M>();
        
        let mut type_registry = self.type_registry.write().map_err(|_| {
            DataFlareError::Actor("Failed to acquire write lock on type registry".to_string())
        })?;
        
        let handlers = type_registry.entry(type_id).or_insert_with(Vec::new);
        handlers.push(id);
        
        Ok(())
    }
    
    /// Send a message to a specific actor
    pub fn send<M: 'static + Send + Sync>(&self, target: impl Into<ActorId>, message: M) -> Result<()> {
        let target = target.into();
        let registry = self.registry.read().map_err(|_| {
            DataFlareError::Actor("Failed to acquire read lock on registry".to_string())
        })?;
        
        if let Some(recipient) = registry.get(&target) {
                    let wrapped_msg = DataFlareMessage::new("message_bus", target.clone(), message);
        
        // do_send returns (), not a Result
        recipient.do_send(wrapped_msg);
        Ok(())
        } else {
            Err(DataFlareError::Actor(format!("Actor not found: {:?}", target)))
        }
    }
    
    /// Broadcast a message to all handlers of a specific message type
    pub fn broadcast<M: 'static + Clone + Send + Sync>(&self, message: M) -> Result<()> {
        let type_id = TypeId::of::<M>();
        
        let type_registry = self.type_registry.read().map_err(|_| {
            DataFlareError::Actor("Failed to acquire read lock on type registry".to_string())
        })?;
        
        let registry = self.registry.read().map_err(|_| {
            DataFlareError::Actor("Failed to acquire read lock on registry".to_string())
        })?;
        
        if let Some(handlers) = type_registry.get(&type_id) {
            for id in handlers {
                if let Some(recipient) = registry.get(id) {
                    let wrapped_msg = DataFlareMessage::new("message_bus", id.clone(), message.clone());
                    let _ = recipient.do_send(wrapped_msg);  // Ignore send errors in broadcast
                }
            }
            Ok(())
        } else {
            warn!("No handlers registered for message type: {:?}", std::any::type_name::<M>());
            Ok(())
        }
    }
    
    /// Unregister an actor from the message bus
    pub fn unregister(&self, id: &ActorId) -> Result<()> {
        let mut registry = self.registry.write().map_err(|_| {
            DataFlareError::Actor("Failed to acquire write lock on registry".to_string())
        })?;
        
        registry.remove(id);
        debug!("Unregistered actor: {:?}", id);
        Ok(())
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for actors that can handle DataFlare messages
pub trait MessageHandler: Actor {
    /// Process a DataFlare message
    fn handle_message(&mut self, msg: DataFlareMessage, ctx: &mut Self::Context);
}

// We define a specific actor struct that can handle DataFlareMessage
// This is a workaround for the foreign trait implementation issue
#[derive(Default)]
pub struct DataFlareHandlerActor<A: MessageHandler> {
    inner: A,
}

impl<A: MessageHandler> Actor for DataFlareHandlerActor<A> {
    type Context = A::Context;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        self.inner.started(ctx);
    }
    
    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        self.inner.stopping(ctx)
    }
    
    fn stopped(&mut self, ctx: &mut Self::Context) {
        self.inner.stopped(ctx);
    }
}

impl<A: MessageHandler> Handler<DataFlareMessage> for DataFlareHandlerActor<A> {
    type Result = ();
    
    fn handle(&mut self, msg: DataFlareMessage, ctx: &mut Self::Context) -> Self::Result {
        self.inner.handle_message(msg, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestActor {
        id: String,
        received_messages: Vec<String>,
    }
    
    impl Actor for TestActor {
        type Context = Context<Self>;
    }
    
    impl MessageHandler for TestActor {
        fn handle_message(&mut self, msg: DataFlareMessage, _ctx: &mut Self::Context) {
            if let Some(payload) = msg.downcast::<String>() {
                self.received_messages.push(payload.clone());
            }
        }
    }
    
    impl Handler<DataFlareMessage> for TestActor {
        type Result = ();
        
        fn handle(&mut self, msg: DataFlareMessage, ctx: &mut Self::Context) -> Self::Result {
            self.handle_message(msg, ctx);
        }
    }
    
    struct TestMessage(String);
    
    impl Message for TestMessage {
        type Result = ();
    }
    
    impl Handler<TestMessage> for TestActor {
        type Result = ();
        
        fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
            self.received_messages.push(msg.0);
        }
    }
    
    #[actix::test]
    async fn test_message_bus_send() {
        let bus = MessageBus::new();
        
        let actor = TestActor {
            id: "test1".to_string(),
            received_messages: Vec::new(),
        };
        
        let addr = actor.start();
        bus.register("test1", addr.clone()).unwrap();
        
        // Send a message through the bus
        bus.send("test1", "Hello, Actor!".to_string()).unwrap();
        
        // Give the actor time to process the message
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
} 