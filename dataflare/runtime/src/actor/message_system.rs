//! # Actor Message System
//!
//! This module provides a unified message passing system for DataFlare actors.
//! It implements a more structured approach to actor communication with
//! improved error handling and message routing.

use actix::{Actor, Addr, Context, Handler, Message, MessageResult};
use dataflare_core::{
    DataFlareError, DataRecord, DataRecordBatch, Result,
    Configurable, Monitorable, Lifecycle,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Unique identifier for an actor in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(pub String);

impl ActorId {
    /// Create a new actor ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    
    /// Get the string representation of the actor ID
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ActorId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ActorId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Actor role in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorRole {
    /// Source actor that reads data
    Source,
    /// Processor actor that transforms data
    Processor,
    /// Destination actor that writes data
    Destination,
    /// Supervisor actor that manages other actors
    Supervisor,
    /// Router actor that routes messages
    Router,
    /// Workflow actor that orchestrates a workflow
    Workflow,
    /// System actor for internal operations
    System,
}

/// Message envelope that wraps all messages in the system
#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<MessageResponse>")]
pub struct MessageEnvelope {
    /// Unique message ID
    pub message_id: String,
    /// Sender actor ID
    pub sender: ActorId,
    /// Recipient actor ID
    pub recipient: ActorId,
    /// Message payload
    pub payload: MessagePayload,
    /// Message metadata
    pub metadata: HashMap<String, String>,
    /// Message timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl MessageEnvelope {
    /// Create a new message envelope
    pub fn new(
        sender: impl Into<ActorId>,
        recipient: impl Into<ActorId>,
        payload: MessagePayload,
    ) -> Self {
        Self {
            message_id: uuid::Uuid::new_v4().to_string(),
            sender: sender.into(),
            recipient: recipient.into(),
            payload,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }
    
    /// Add metadata to the message
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Message payload types
#[derive(Debug, Clone)]
pub enum MessagePayload {
    /// Single data record
    Record(DataRecord),
    /// Batch of data records
    Batch(DataRecordBatch),
    /// Command to an actor
    Command(ActorCommand),
    /// Query to an actor
    Query(ActorQuery),
    /// Response from an actor
    Response(ActorResponse),
    /// Event notification
    Event(ActorEvent),
    /// Error message
    Error(String),
}

/// Commands that can be sent to actors
#[derive(Debug, Clone)]
pub enum ActorCommand {
    /// Initialize the actor
    Initialize,
    /// Start the actor
    Start,
    /// Stop the actor
    Stop,
    /// Pause the actor
    Pause,
    /// Resume the actor
    Resume,
    /// Configure the actor
    Configure(serde_json::Value),
    /// Process a record
    ProcessRecord(DataRecord),
    /// Process a batch
    ProcessBatch(DataRecordBatch),
    /// Read data
    Read,
    /// Write data
    Write(DataRecordBatch),
    /// Flush any buffered data
    Flush,
    /// Close the actor
    Close,
    /// Custom command with arbitrary data
    Custom(String, serde_json::Value),
}

/// Queries that can be sent to actors
#[derive(Debug, Clone)]
pub enum ActorQuery {
    /// Get actor status
    Status,
    /// Get actor metrics
    Metrics,
    /// Get actor configuration
    Configuration,
    /// Get actor state
    State,
    /// Custom query with arbitrary data
    Custom(String, serde_json::Value),
}

/// Responses from actors
#[derive(Debug, Clone)]
pub enum ActorResponse {
    /// Acknowledgement
    Ack,
    /// Status response
    Status(ActorStatus),
    /// Metrics response
    Metrics(serde_json::Value),
    /// Configuration response
    Configuration(serde_json::Value),
    /// State response
    State(serde_json::Value),
    /// Data response
    Data(DataRecordBatch),
    /// Custom response with arbitrary data
    Custom(String, serde_json::Value),
}

/// Events that can be emitted by actors
#[derive(Debug, Clone)]
pub enum ActorEvent {
    /// Actor initialized
    Initialized,
    /// Actor started
    Started,
    /// Actor stopped
    Stopped,
    /// Actor paused
    Paused,
    /// Actor resumed
    Resumed,
    /// Actor configured
    Configured,
    /// Actor error
    Error(String),
    /// Progress update
    Progress(f64),
    /// Custom event with arbitrary data
    Custom(String, serde_json::Value),
}

/// Actor status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorStatus {
    /// Actor is initializing
    Initializing,
    /// Actor is ready
    Ready,
    /// Actor is running
    Running,
    /// Actor is paused
    Paused,
    /// Actor is stopping
    Stopping,
    /// Actor is stopped
    Stopped,
    /// Actor has failed
    Failed(String),
}

/// Response to a message
#[derive(Debug, Clone)]
pub struct MessageResponse {
    /// Original message ID
    pub request_id: String,
    /// Response payload
    pub payload: MessagePayload,
    /// Response metadata
    pub metadata: HashMap<String, String>,
    /// Response timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl MessageResponse {
    /// Create a new message response
    pub fn new(request_id: String, payload: MessagePayload) -> Self {
        Self {
            request_id,
            payload,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }
    
    /// Add metadata to the response
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Trait for actors that can handle DataFlare messages
pub trait MessageHandler: Actor {
    /// Handle a message envelope
    fn handle_message(&mut self, msg: MessageEnvelope, ctx: &mut Self::Context) -> Result<MessageResponse>;
}

/// Actor that implements the MessageHandler trait
pub struct DataFlareActor<T: MessageHandler> {
    /// Actor ID
    pub id: ActorId,
    /// Actor role
    pub role: ActorRole,
    /// Actor status
    pub status: ActorStatus,
    /// Actor implementation
    pub handler: T,
    /// Actor metrics
    pub metrics: HashMap<String, serde_json::Value>,
    /// Actor configuration
    pub config: serde_json::Value,
}

impl<T: MessageHandler> DataFlareActor<T> {
    /// Create a new DataFlare actor
    pub fn new(id: impl Into<ActorId>, role: ActorRole, handler: T) -> Self {
        Self {
            id: id.into(),
            role,
            status: ActorStatus::Initializing,
            handler,
            metrics: HashMap::new(),
            config: serde_json::Value::Null,
        }
    }
    
    /// Update actor status
    pub fn update_status(&mut self, status: ActorStatus) {
        self.status = status;
    }
    
    /// Update actor metrics
    pub fn update_metric(&mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) {
        self.metrics.insert(key.into(), value.into());
    }
}

impl<T: MessageHandler> Actor for DataFlareActor<T> {
    type Context = Context<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("Actor {} started with role {:?}", self.id.as_str(), self.role);
        self.update_status(ActorStatus::Ready);
    }
    
    fn stopped(&mut self, ctx: &mut Self::Context) {
        log::info!("Actor {} stopped", self.id.as_str());
        self.update_status(ActorStatus::Stopped);
    }
}

impl<T: MessageHandler> Handler<MessageEnvelope> for DataFlareActor<T> {
    type Result = MessageResult<MessageEnvelope>;
    
    fn handle(&mut self, msg: MessageEnvelope, ctx: &mut Self::Context) -> Self::Result {
        log::debug!("Actor {} received message: {:?}", self.id.as_str(), msg);
        
        // Check if the message is intended for this actor
        if msg.recipient != self.id {
            let error = DataFlareError::Actor(format!(
                "Message routing error: message intended for {} but received by {}",
                msg.recipient.as_str(), self.id.as_str()
            ));
            return MessageResult(Err(error));
        }
        
        // Handle the message
        let result = self.handler.handle_message(msg.clone(), ctx);
        
        match result {
            Ok(response) => {
                log::debug!("Actor {} processed message successfully", self.id.as_str());
                MessageResult(Ok(response))
            }
            Err(error) => {
                log::error!("Actor {} failed to process message: {}", self.id.as_str(), error);
                MessageResult(Err(error))
            }
        }
    }
}

/// Actor registry for managing actor references
pub struct ActorRegistry {
    /// Map of actor IDs to actor addresses
    actors: HashMap<ActorId, Arc<dyn Any + Send + Sync>>,
}

impl ActorRegistry {
    /// Create a new actor registry
    pub fn new() -> Self {
        Self {
            actors: HashMap::new(),
        }
    }
    
    /// Register an actor
    pub fn register<A: Actor>(&mut self, id: ActorId, addr: Addr<A>) {
        self.actors.insert(id, Arc::new(addr));
    }
    
    /// Get an actor address
    pub fn get<A: Actor>(&self, id: &ActorId) -> Option<Addr<A>> {
        self.actors.get(id).and_then(|addr| {
            addr.downcast_ref::<Addr<A>>().cloned()
        })
    }
    
    /// Remove an actor
    pub fn remove(&mut self, id: &ActorId) -> bool {
        self.actors.remove(id).is_some()
    }
    
    /// Check if an actor is registered
    pub fn contains(&self, id: &ActorId) -> bool {
        self.actors.contains_key(id)
    }
    
    /// Get all actor IDs
    pub fn get_all_ids(&self) -> Vec<ActorId> {
        self.actors.keys().cloned().collect()
    }
}

use std::any::Any;

/// Message router for routing messages between actors
pub struct MessageRouter {
    /// Actor registry
    registry: ActorRegistry,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new() -> Self {
        Self {
            registry: ActorRegistry::new(),
        }
    }
    
    /// Register an actor
    pub fn register<A: Actor>(&mut self, id: ActorId, addr: Addr<A>) {
        self.registry.register(id, addr);
    }
    
    /// Send a message to an actor
    pub async fn send<A: Actor + Handler<MessageEnvelope>>(&self, msg: MessageEnvelope) -> Result<MessageResponse> {
        let recipient_id = msg.recipient.clone();
        
        if let Some(addr) = self.registry.get::<A>(&recipient_id) {
            addr.send(msg).await.map_err(|e| {
                DataFlareError::Actor(format!("Failed to send message to actor {}: {}", recipient_id.as_str(), e))
            })?
        } else {
            Err(DataFlareError::Actor(format!("Actor not found: {}", recipient_id.as_str())))
        }
    }
    
    /// Broadcast a message to all actors of a specific role
    pub async fn broadcast<A: Actor + Handler<MessageEnvelope>>(&self, sender: ActorId, role: ActorRole, payload: MessagePayload) -> Result<Vec<Result<MessageResponse>>> {
        let mut results = Vec::new();
        
        for id in self.registry.get_all_ids() {
            // Create a message for each recipient
            let msg = MessageEnvelope::new(sender.clone(), id.clone(), payload.clone());
            
            // Send the message
            let result = self.send::<A>(msg).await;
            results.push(result);
        }
        
        Ok(results)
    }
}
