//! Error types for the actix-cluster crate.

use std::io;
use std::net::AddrParseError;
use thiserror::Error;

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
}

/// Type alias for Result with ClusterError
pub type ClusterResult<T> = Result<T, ClusterError>; 