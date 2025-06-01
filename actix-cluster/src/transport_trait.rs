//! Transport trait module for pluggable transport mechanisms.
//!
//! This module defines the common interface for different transport implementations
//! such as TCP, UDP, WebSocket, etc.

use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Duration;
use std::collections::HashMap;

use actix::prelude::*;
use async_trait::async_trait;

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, NodeInfo};
use crate::message::MessageEnvelope;
use crate::transport::TransportMessage;
use crate::message::MessageEnvelopeHandler;
use crate::registry::ActorRegistry;
use crate::serialization::SerializationFormat;
use crate::compression::CompressionConfig;

/// Transport type enum
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportType {
    /// TCP transport
    TCP,
    /// UDP transport
    UDP,
    /// WebSocket transport
    WebSocket,
    /// QUIC transport
    QUIC,
    /// Custom transport
    Custom(u8),
}

/// Transport configuration
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Transport type
    pub transport_type: TransportType,
    /// Bind address
    pub bind_addr: SocketAddr,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Serialization format
    pub serialization_format: SerializationFormat,
    /// Compression configuration
    pub compression_config: Option<CompressionConfig>,
    /// Custom configuration parameters
    pub custom_params: HashMap<String, String>,
}

impl TransportConfig {
    /// Create a new transport configuration
    pub fn new(transport_type: TransportType, bind_addr: SocketAddr) -> Self {
        Self {
            transport_type,
            bind_addr,
            connection_timeout: Duration::from_secs(10),
            serialization_format: SerializationFormat::Bincode,
            compression_config: None,
            custom_params: HashMap::new(),
        }
    }

    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set serialization format
    pub fn with_serialization_format(mut self, format: SerializationFormat) -> Self {
        self.serialization_format = format;
        self
    }

    /// Set compression configuration
    pub fn with_compression_config(mut self, config: CompressionConfig) -> Self {
        self.compression_config = Some(config);
        self
    }

    /// Add a custom parameter
    pub fn with_custom_param(mut self, key: &str, value: &str) -> Self {
        self.custom_params.insert(key.to_string(), value.to_string());
        self
    }
}

/// Message handler trait
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle a message from a node
    async fn handle_message(&self, sender: NodeId, message: TransportMessage) -> ClusterResult<()>;
}

/// Transport trait defining the common interface for all transport implementations
#[async_trait]
pub trait TransportTrait: Send + Sync + 'static {
    /// Get the transport type
    fn transport_type(&self) -> TransportType;

    /// Get the local node information
    fn local_node(&self) -> &NodeInfo;

    /// Initialize the transport
    async fn init(&mut self) -> ClusterResult<()>;

    /// Send a message to a specific node
    async fn send_message(&mut self, node_id: &NodeId, message: TransportMessage) -> ClusterResult<()>;

    /// Send a message envelope
    async fn send_envelope(&mut self, envelope: MessageEnvelope) -> ClusterResult<()>;

    /// Get all peers
    fn get_peers(&self) -> Vec<NodeInfo>;

    /// Set message handler for envelope processing
    fn set_message_handler(&mut self, handler: Addr<MessageEnvelopeHandler>);

    /// Set registry adapter for actor discovery
    fn set_registry_adapter(&mut self, registry: Arc<ActorRegistry>);

    /// Check if transport is started
    fn is_started(&self) -> bool;

    /// Connect to a peer node
    async fn connect_to_peer(&mut self, peer_addr: SocketAddr) -> ClusterResult<()>;

    /// Check if a node is connected
    fn is_connected(&self, node_id: &NodeId) -> bool;

    /// Get the compression configuration
    fn get_compression_config(&self) -> Option<&CompressionConfig>;

    /// Clone the transport
    fn clone_box(&self) -> Box<dyn TransportTrait>;
}

/// Factory function to create a transport instance based on configuration
pub fn create_transport(config: TransportConfig, local_node: NodeInfo) -> ClusterResult<Box<dyn TransportTrait>> {
    match config.transport_type {
        TransportType::TCP => {
            // Create TCP transport
            let transport = crate::transport::P2PTransport::new(
                local_node,
                config.serialization_format,
            )?;

            Ok(Box::new(TCPTransport::new(transport, config)))
        },
        TransportType::UDP => {
            // UDP transport not implemented yet
            Err(ClusterError::UnsupportedTransport("UDP transport not implemented yet".to_string()))
        },
        TransportType::WebSocket => {
            // WebSocket transport not implemented yet
            Err(ClusterError::UnsupportedTransport("WebSocket transport not implemented yet".to_string()))
        },
        TransportType::QUIC => {
            // QUIC transport not implemented yet
            Err(ClusterError::UnsupportedTransport("QUIC transport not implemented yet".to_string()))
        },
        TransportType::Custom(_) => {
            // Custom transport not implemented yet
            Err(ClusterError::UnsupportedTransport("Custom transport not implemented yet".to_string()))
        },
    }
}

/// TCP transport implementation that wraps P2PTransport
pub struct TCPTransport {
    /// Inner P2PTransport
    inner: crate::transport::P2PTransport,
    /// Transport configuration
    config: TransportConfig,
}

impl TCPTransport {
    /// Create a new TCP transport
    pub fn new(inner: crate::transport::P2PTransport, config: TransportConfig) -> Self {
        Self {
            inner,
            config,
        }
    }
}

#[async_trait]
impl TransportTrait for TCPTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::TCP
    }

    fn local_node(&self) -> &NodeInfo {
        &self.inner.local_node
    }

    async fn init(&mut self) -> ClusterResult<()> {
        self.inner.init().await
    }

    async fn send_message(&mut self, node_id: &NodeId, message: TransportMessage) -> ClusterResult<()> {
        self.inner.send_message(node_id, message).await
    }

    async fn send_envelope(&mut self, envelope: MessageEnvelope) -> ClusterResult<()> {
        self.inner.send_envelope(envelope).await
    }

    fn get_peers(&self) -> Vec<NodeInfo> {
        self.inner.get_peers()
    }

    fn set_message_handler(&mut self, handler: Addr<MessageEnvelopeHandler>) {
        self.inner.set_message_handler(handler)
    }

    fn set_registry_adapter(&mut self, registry: Arc<ActorRegistry>) {
        self.inner.set_registry_adapter(registry)
    }

    fn is_started(&self) -> bool {
        self.inner.is_started()
    }

    async fn connect_to_peer(&mut self, peer_addr: SocketAddr) -> ClusterResult<()> {
        self.inner.connect_to_peer(peer_addr).await
    }

    fn is_connected(&self, node_id: &NodeId) -> bool {
        self.inner.is_connected(node_id)
    }

    fn get_compression_config(&self) -> Option<&CompressionConfig> {
        self.inner.get_compression_config()
    }

    fn clone_box(&self) -> Box<dyn TransportTrait> {
        Box::new(Self {
            inner: self.inner.clone(),
            config: self.config.clone(),
        })
    }
}

// Allow TransportTrait objects to be cloned
impl Clone for Box<dyn TransportTrait> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
