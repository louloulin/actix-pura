//! Configuration module for Actix cluster.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use serde::{Serialize, Deserialize};
use crate::serialization::SerializationFormat;
use crate::compression::CompressionConfig;
use crate::cluster::Architecture;
use crate::error::{ClusterError, ClusterResult};
use crate::transport_trait::TransportType;

/// Default heartbeat interval in seconds
pub const DEFAULT_HEARTBEAT_INTERVAL: u64 = 5;

/// Default node timeout in seconds
pub const DEFAULT_NODE_TIMEOUT: u64 = 15;

/// Default port for cluster communication
pub const DEFAULT_PORT: u16 = 8558;

/// Node role in the cluster
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeRole {
    /// Master node in a centralized architecture
    Master,

    /// Worker node in a centralized architecture
    Worker,

    /// Peer node in a decentralized architecture
    Peer,
}

/// Methods for service discovery
#[derive(Debug, Clone, PartialEq)]
pub enum DiscoveryMethod {
    /// Static list of seed nodes
    Static {
        /// List of seed nodes to connect to
        seed_nodes: Vec<String>,
    },

    /// DNS-based discovery
    Dns(String),

    /// Kubernetes API based discovery
    Kubernetes {
        /// Namespace to search for pods
        namespace: String,
        /// Label selector
        selector: String,
    },

    /// Multicast discovery
    Multicast,

    /// Gossip-based discovery
    Gossip,

    /// LibP2P-based discovery with Kademlia DHT and mDNS
    LibP2P {
        /// Bootstrap nodes
        bootstrap_nodes: Vec<String>,
        /// Enable mDNS for local network discovery
        enable_mdns: bool,
    },
}

/// Cache configuration options for the actor registry
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Whether the actor lookup cache is enabled
    enabled: bool,
    /// Time-to-live for cache entries in seconds
    ttl: u64,
    /// Maximum number of cache entries (0 means unlimited)
    max_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl: 60,  // 60 seconds by default
            max_size: 1000,
        }
    }
}

impl CacheConfig {
    /// Create a new cache configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether the cache is enabled
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the cache TTL in seconds
    pub fn ttl(mut self, ttl: u64) -> Self {
        self.ttl = ttl;
        self
    }

    /// Set the maximum cache size
    pub fn max_size(mut self, max_size: usize) -> Self {
        self.max_size = max_size;
        self
    }

    /// Get whether the cache is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the cache TTL in seconds
    pub fn get_ttl(&self) -> u64 {
        self.ttl
    }

    /// Get the maximum cache size
    pub fn get_max_size(&self) -> usize {
        self.max_size
    }
}

/// Cluster configuration builder
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// Cluster architecture (centralized or decentralized)
    pub architecture: Architecture,

    /// Local node role
    pub node_role: NodeRole,

    /// Local node bind address
    pub bind_addr: SocketAddr,

    /// Public address for other nodes to connect to (if behind NAT)
    pub public_addr: Option<SocketAddr>,

    /// Seed nodes for cluster joining
    pub seed_nodes: Vec<String>,

    /// Master nodes for centralized architecture
    pub master_nodes: Vec<String>,

    /// Service discovery method
    pub discovery: DiscoveryMethod,

    /// Node heartbeat interval
    pub heartbeat_interval: Duration,

    /// Node timeout for failure detection
    pub node_timeout: Duration,

    /// Serialization format for network messages
    pub serialization_format: SerializationFormat,

    /// Transport type for network communication
    pub transport_type: TransportType,

    /// TLS enabled
    pub tls_enabled: bool,

    /// TLS certificate path
    pub tls_cert_path: Option<String>,

    /// TLS private key path
    pub tls_key_path: Option<String>,

    /// Cluster name
    pub cluster_name: String,

    /// Cache configuration
    cache_config: CacheConfig,

    /// Compression configuration
    compression_config: Option<CompressionConfig>,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        ClusterConfig {
            architecture: Architecture::Decentralized,
            node_role: NodeRole::Peer,
            bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), DEFAULT_PORT),
            public_addr: None,
            seed_nodes: Vec::new(),
            master_nodes: Vec::new(),
            discovery: DiscoveryMethod::Static { seed_nodes: Vec::new() },
            heartbeat_interval: Duration::from_secs(DEFAULT_HEARTBEAT_INTERVAL),
            node_timeout: Duration::from_secs(DEFAULT_NODE_TIMEOUT),
            serialization_format: SerializationFormat::Bincode,
            transport_type: TransportType::TCP,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            cluster_name: "actix-cluster".to_string(),
            cache_config: CacheConfig::default(),
            compression_config: None,
        }
    }
}

impl ClusterConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set cluster architecture
    pub fn architecture(mut self, architecture: Architecture) -> Self {
        self.architecture = architecture;

        // Update node role based on architecture if it's inconsistent
        match (&self.architecture, &self.node_role) {
            (Architecture::Centralized, NodeRole::Peer) => {
                self.node_role = NodeRole::Worker;
            },
            (Architecture::Decentralized, NodeRole::Master) | (Architecture::Decentralized, NodeRole::Worker) => {
                self.node_role = NodeRole::Peer;
            },
            _ => {}
        }

        self
    }

    /// Set node role
    pub fn node_role(mut self, role: NodeRole) -> Self {
        self.node_role = role;
        self
    }

    /// Set bind address
    pub fn bind_addr(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr;
        self
    }

    /// Set public address
    pub fn public_addr(mut self, addr: SocketAddr) -> Self {
        self.public_addr = Some(addr);
        self
    }

    /// Set seed nodes for decentralized architecture
    pub fn seed_nodes(mut self, nodes: Vec<String>) -> Self {
        self.seed_nodes = nodes;
        self
    }

    /// Set master nodes for centralized architecture
    pub fn master_nodes(mut self, nodes: Vec<String>) -> Self {
        self.master_nodes = nodes;
        self
    }

    /// Set discovery method
    pub fn discovery(mut self, method: DiscoveryMethod) -> Self {
        self.discovery = method;
        self
    }

    /// Set heartbeat interval
    pub fn heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval = interval;
        self
    }

    /// Set node timeout
    pub fn node_timeout(mut self, timeout: Duration) -> Self {
        self.node_timeout = timeout;
        self
    }

    /// Set serialization format
    pub fn serialization_format(mut self, format: SerializationFormat) -> Self {
        self.serialization_format = format;
        self
    }

    /// Set transport type
    pub fn transport_type(mut self, transport_type: TransportType) -> Self {
        self.transport_type = transport_type;
        self
    }

    /// Enable TLS with certificate and key paths
    pub fn with_tls(mut self, cert_path: String, key_path: String) -> Self {
        self.tls_enabled = true;
        self.tls_cert_path = Some(cert_path);
        self.tls_key_path = Some(key_path);
        self
    }

    /// Set cluster name
    pub fn cluster_name(mut self, name: String) -> Self {
        self.cluster_name = name;
        self
    }

    /// Set the cache configuration
    pub fn cache_config(mut self, cache_config: CacheConfig) -> Self {
        self.cache_config = cache_config;
        self
    }

    /// Get the cache configuration
    pub fn get_cache_config(&self) -> &CacheConfig {
        &self.cache_config
    }

    /// Configure compression settings
    pub fn with_compression_config(mut self, config: CompressionConfig) -> Self {
        self.compression_config = Some(config);
        self
    }

    /// Get compression configuration
    pub fn get_compression_config(&self) -> Option<&CompressionConfig> {
        self.compression_config.as_ref()
    }

    /// Check if compression is enabled
    pub fn is_compression_enabled(&self) -> bool {
        self.compression_config.as_ref().map_or(false, |c| c.enabled)
    }

    /// Helper method for setting node name - same as cluster name
    pub fn with_node_name(mut self, name: String) -> Self {
        self.cluster_name = name;
        self
    }

    /// Validate and build the configuration
    pub fn build(self) -> ClusterResult<Self> {
        // Validate configuration based on architecture
        match (&self.architecture, &self.node_role) {
            (Architecture::Centralized, NodeRole::Master) => {
                // Master node needs to bind to a port for workers to connect
                if self.bind_addr.port() == 0 {
                    return Err(ClusterError::ConfigurationError(
                        "Master node must bind to a specific port".to_string(),
                    ));
                }
            },
            (Architecture::Centralized, NodeRole::Worker) => {
                // Worker node needs at least one master node address
                if self.master_nodes.is_empty() {
                    return Err(ClusterError::ConfigurationError(
                        "Worker node requires at least one master node address".to_string(),
                    ));
                }
            },
            (Architecture::Decentralized, NodeRole::Peer) => {
                // For decentralized architecture with static discovery,
                // we need seed nodes unless this is the first node
                if matches!(self.discovery, DiscoveryMethod::Static { seed_nodes: ref nodes } if nodes.is_empty()) {
                    log::warn!("No seed nodes provided for static discovery in a decentralized architecture");
                }
            },
            (arch, role) => {
                return Err(ClusterError::ConfigurationError(
                    format!("Invalid combination of architecture {:?} and role {:?}", arch, role),
                ));
            }
        }

        // Validate TLS configuration
        if self.tls_enabled {
            if self.tls_cert_path.is_none() || self.tls_key_path.is_none() {
                return Err(ClusterError::ConfigurationError(
                    "TLS is enabled but certificate or key path is missing".to_string(),
                ));
            }
        }

        Ok(self)
    }

    /// Build a NodeInfo from this configuration
    pub fn build_node_info(&self) -> crate::node::NodeInfo {
        crate::node::NodeInfo {
            id: crate::node::NodeId::new(),
            name: format!("node-{}", self.bind_addr),
            addr: self.bind_addr,
            role: self.node_role.clone(),
            status: crate::node::NodeStatus::Joining,
            joined_at: None,
            capabilities: Vec::new(),
            load: 0,
            metadata: serde_json::Map::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClusterConfig::default();
        assert_eq!(config.architecture, Architecture::Decentralized);
        assert_eq!(config.node_role, NodeRole::Peer);
        assert_eq!(config.bind_addr.port(), DEFAULT_PORT);
        assert_eq!(config.heartbeat_interval, Duration::from_secs(DEFAULT_HEARTBEAT_INTERVAL));
    }

    #[test]
    fn test_centralized_master_config() {
        let config = ClusterConfig::new()
            .architecture(Architecture::Centralized)
            .node_role(NodeRole::Master)
            .build()
            .unwrap();

        assert_eq!(config.architecture, Architecture::Centralized);
        assert_eq!(config.node_role, NodeRole::Master);
    }

    #[test]
    fn test_centralized_worker_config() {
        let config = ClusterConfig::new()
            .architecture(Architecture::Centralized)
            .node_role(NodeRole::Worker)
            .master_nodes(vec!["localhost:8558".to_string()])
            .build();

        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.architecture, Architecture::Centralized);
        assert_eq!(config.node_role, NodeRole::Worker);
        assert_eq!(config.master_nodes.len(), 1);
    }

    #[test]
    fn test_worker_without_master_fails() {
        let config = ClusterConfig::new()
            .architecture(Architecture::Centralized)
            .node_role(NodeRole::Worker)
            .build();

        assert!(config.is_err());
        match config.unwrap_err() {
            ClusterError::ConfigurationError(msg) => {
                assert!(msg.contains("requires at least one master node"));
            },
            _ => panic!("Expected ConfigurationError"),
        }
    }

    #[test]
    fn test_role_adjustment_with_architecture() {
        // Peer role in centralized architecture should become Worker
        let config = ClusterConfig::new()
            .node_role(NodeRole::Peer)
            .architecture(Architecture::Centralized)
            .master_nodes(vec!["localhost:8558".to_string()])
            .build()
            .unwrap();

        assert_eq!(config.node_role, NodeRole::Worker);

        // Master/Worker role in decentralized architecture should become Peer
        let config = ClusterConfig::new()
            .node_role(NodeRole::Master)
            .architecture(Architecture::Decentralized)
            .build()
            .unwrap();

        assert_eq!(config.node_role, NodeRole::Peer);
    }

    #[test]
    fn test_tls_configuration() {
        // TLS enabled with cert and key should be valid
        let config = ClusterConfig::new()
            .with_tls("cert.pem".to_string(), "key.pem".to_string())
            .build()
            .unwrap();

        assert!(config.tls_enabled);
        assert_eq!(config.tls_cert_path, Some("cert.pem".to_string()));
        assert_eq!(config.tls_key_path, Some("key.pem".to_string()));
    }
}