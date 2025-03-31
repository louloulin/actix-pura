//! Error types for the actix-cluster crate.

use std::fmt;
use std::io;
use std::net::AddrParseError;
use thiserror::Error;
use actix::MailboxError;
use uuid::Uuid;

use crate::node::{NodeId, NodeStatus};

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
    NodeAlreadyExists(NodeId),

    /// Node not found in the cluster
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    /// Actor not found
    #[error("Actor not found: {0}")]
    ActorNotFound(String),

    /// Actor already exists
    #[error("Actor already exists: {0}")]
    ActorAlreadyExists(String),

    /// Actor already registered
    #[error("Actor already registered: {0}")]
    ActorAlreadyRegistered(String),

    /// Actor registration failed
    #[error("Actor registration failed: {0}")]
    ActorRegistrationFailed(String),
    
    /// Actor not registered
    #[error("Actor not registered: {0}")]
    ActorNotRegistered(String),

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

    /// Transport not initialized
    #[error("Transport not initialized")]
    TransportNotInitialized,

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

    /// Message delivery failed
    #[error("Message delivery failed: {0}")]
    MessageDeliveryFailed(String),

    /// Remote actor error
    #[error("Remote actor error: {0}")]
    RemoteActorError(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

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

    /// Transport not available or not initialized
    #[error("Transport not available or not initialized")]
    TransportNotAvailable,
    
    /// Protocol error during message exchange
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    /// Transport layer error
    #[error("Transport error: {0}")]
    TransportError(String),
    
    /// No nodes available for operation
    #[error("No nodes available")]
    NoNodesAvailable,
    
    /// Feature not implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),
    
    /// Component not initialized
    #[error("Not initialized: {0}")]
    NotInitialized(String),

    /// Invalid node ID
    #[error("Invalid node ID: {0}")]
    InvalidNodeId(String),

    /// Placement strategy error
    #[error("Placement strategy error: {0}")]
    PlacementStrategyError(String),

    /// Invalid node status
    #[error("Invalid node status: {0:?}")]
    InvalidNodeStatus(NodeStatus),

    /// UUID parse error
    #[error("UUID parse error: {0}")]
    UuidParseError(#[from] uuid::Error),

    /// Message not found
    #[error("Message not found: {0}")]
    MessageNotFound(Uuid),

    /// Actix error
    #[error("Actix error: {0}")]
    ActixError(String),

    /// Message timeout
    #[error("Message timeout")]
    MessageTimeout,

    /// Unsupported message type
    #[error("Unsupported message type: {0}")]
    UnsupportedMessageType(String),

    /// Node migration error
    #[error("Node migration error: {0}")]
    NodeMigrationError(String),

    /// Too many replicas requested
    #[error("Requested replicas ({0}) exceeds available nodes ({1})")]
    TooManyReplicasRequested(usize, usize),

    /// Consensus error
    #[error("Consensus error: {0}")]
    ConsensusError(String),

    /// Raft error
    #[error("Raft error: {0}")]
    RaftError(String),
    
    /// Channel closed
    #[error("Channel closed")]
    ChannelClosed,
    
    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Type alias for Result with ClusterError
pub type ClusterResult<T> = Result<T, ClusterError>;

// 为ClusterError实现From特质
impl From<MailboxError> for ClusterError {
    fn from(err: MailboxError) -> Self {
        ClusterError::MailboxError(err.to_string())
    }
}

impl From<anyhow::Error> for ClusterError {
    fn from(err: anyhow::Error) -> Self {
        ClusterError::SerializationError(err.to_string())
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for ClusterError {
    fn from(_: tokio::sync::mpsc::error::SendError<T>) -> Self {
        ClusterError::ChannelClosed
    }
} 