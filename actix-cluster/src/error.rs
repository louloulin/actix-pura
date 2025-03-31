//! Error types for the actix-cluster crate.

use std::fmt;
use std::io;
use std::net::AddrParseError;
use thiserror::Error;
use actix::MailboxError;

use crate::node::NodeId;

/// Main error type for cluster operations
#[derive(Error, Debug)]
pub enum ClusterError {
    /// Failed to connect to a remote node
    #[error("Failed to connect to node: {0}")]
    ConnectionError(String),

    /// Failed to parse node address
    #[error("Failed to parse node address: {0}")]
    AddressParseError(#[from] AddrParseError),

    /// IO error occurred
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    /// Node discovery error
    #[error("Node discovery error: {0}")]
    DiscoveryError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// Message routing error
    #[error("Message routing error: {0}")]
    RoutingError(String),

    /// Node already exists in the cluster
    #[error("Node already exists: {0}")]
    NodeExistsError(String),

    /// Node not found in the cluster
    #[error("Node not found: {0}")]
    NodeNotFoundError(String),

    /// Actor not found
    #[error("Actor not found: {0}")]
    ActorNotFoundError(String),

    /// Actor registration error
    #[error("Actor registration error: {0}")]
    ActorRegistrationError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Network timeout
    #[error("Network timeout: {0}")]
    TimeoutError(String),

    /// Authentication error
    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    /// Authorization error
    #[error("Authorization error: {0}")]
    AuthorizationError(String),

    /// Node not found error
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    /// Transport not initialized
    #[error("Transport not initialized")]
    TransportNotInitialized,

    /// Node already exists
    #[error("Node already exists: {0}")]
    NodeAlreadyExists(NodeId),

    /// Node registration failed
    #[error("Node registration failed: {0}")]
    NodeRegistrationFailed(String),

    /// Actor already registered
    #[error("Actor already registered: {0}")]
    ActorAlreadyRegistered(String),

    /// Registry not initialized
    #[error("Registry not initialized")]
    RegistryNotInitialized,

    /// Message send failed
    #[error("Message send failed: {0}")]
    MessageSendFailed(String),

    /// Node not connected
    #[error("Node not connected: {0}")]
    NodeNotConnected(NodeId),

    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Message deliver failed
    #[error("Message delivery failed: {0}")]
    MessageDeliveryFailed(String),

    /// Actor registration failed
    #[error("Actor registration failed: {0}")]
    ActorRegistrationFailed(String),

    /// Messaging system error
    #[error("Messaging system error: {0}")]
    MessagingError(String),

    /// Remote actor error
    #[error("Remote actor error: {0}")]
    RemoteActorError(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),

    /// Actor not found (简化版本)
    #[error("Actor not found: {0}")]
    ActorNotFound(String),

    /// Mailbox error
    #[error("Mailbox error: {0}")]
    MailboxError(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Unknown peer
    #[error("Unknown peer: {0}")]
    UnknownPeer(NodeId),

    /// No message handler configured
    #[error("No message handler configured")]
    NoMessageHandler,
}

/// Type alias for Result with ClusterError
pub type ClusterResult<T> = Result<T, ClusterError>;

// 为ClusterError实现From<MailboxError>特质
impl From<MailboxError> for ClusterError {
    fn from(err: MailboxError) -> Self {
        ClusterError::MailboxError(err.to_string())
    }
} 