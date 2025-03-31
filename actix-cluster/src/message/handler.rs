use actix::{Actor, Context, Handler};
use crate::message::envelope::MessageEnvelope;
use log::debug;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::node::NodeId;

/// Message envelope handler function type
type EnvelopeHandlerFn = Box<dyn Fn(&MessageEnvelope) -> () + Send + Sync>;

/// Handler for MessageEnvelope - processes messages from remote actors
pub struct MessageEnvelopeHandler {
    /// Local node ID
    node_id: NodeId,
    /// Handlers for different types of messages
    handlers: Arc<Mutex<HashMap<String, EnvelopeHandlerFn>>>,
}

impl MessageEnvelopeHandler {
    /// Creates a new message envelope handler with the specified node ID
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            handlers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Register a handler for a specific actor path
    pub fn register_handler<F>(&self, path: &str, handler: F)
    where
        F: Fn(&MessageEnvelope) -> () + Send + Sync + 'static,
    {
        let mut handlers = self.handlers.lock().unwrap();
        handlers.insert(path.to_string(), Box::new(handler));
    }
}

impl Actor for MessageEnvelopeHandler {
    type Context = Context<Self>;
}

impl Handler<MessageEnvelope> for MessageEnvelopeHandler {
    type Result = ();
    
    fn handle(&mut self, msg: MessageEnvelope, _ctx: &mut Self::Context) -> Self::Result {
        // Default implementation
        debug!("Received message envelope: {:?}", msg);
        
        // If we have a registered handler for this actor, call it
        let target_actor = &msg.target_actor;
        let handlers = self.handlers.lock().unwrap();
        
        if let Some(handler) = handlers.get(target_actor) {
            // Call the handler
            handler(&msg);
        } else {
            debug!("No handler registered for target actor: {}", target_actor);
        }
    }
} 