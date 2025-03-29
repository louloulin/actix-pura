//! Service discovery module for finding and connecting to cluster nodes.

use std::net::SocketAddr;
use std::time::Duration;
use std::collections::{HashMap, HashSet};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, NodeInfo, NodeStatus};
use crate::config::{DiscoveryMethod, NodeRole};

/// Default interval for discovery refresh in seconds
pub const DEFAULT_DISCOVERY_INTERVAL: u64 = 30;

/// A trait for node discovery in the cluster
#[async_trait]
pub trait ServiceDiscovery: Send + Sync {
    /// Initialize the discovery service
    async fn init(&mut self) -> ClusterResult<()>;
    
    /// Register a node in the discovery service
    async fn register_node(&mut self, node: &NodeInfo) -> ClusterResult<()>;
    
    /// Deregister a node from the discovery service
    async fn deregister_node(&mut self, node_id: &NodeId) -> ClusterResult<()>;
    
    /// Discover available nodes
    async fn discover_nodes(&mut self) -> ClusterResult<Vec<NodeInfo>>;
    
    /// Update node status in the discovery service
    async fn update_node_status(&mut self, node_id: &NodeId, status: NodeStatus) -> ClusterResult<()>;
    
    /// Get discovery interval
    fn discovery_interval(&self) -> Duration;
    
    /// Set discovery interval
    fn set_discovery_interval(&mut self, interval: Duration);
    
    /// Clean up resources when discovery service is shutting down
    async fn shutdown(&mut self) -> ClusterResult<()>;
}

/// Static discovery implementation using a predefined list of seed nodes
pub struct StaticDiscovery {
    /// Local node information
    local_node: Option<NodeInfo>,
    
    /// List of seed node addresses
    seed_nodes: Vec<String>,
    
    /// Discovered nodes
    nodes: HashMap<NodeId, NodeInfo>,
    
    /// Discovery refresh interval
    discovery_interval: Duration,
}

impl StaticDiscovery {
    /// Create a new static discovery service
    pub fn new(seed_nodes: Vec<String>) -> Self {
        StaticDiscovery {
            local_node: None,
            seed_nodes,
            nodes: HashMap::new(),
            discovery_interval: Duration::from_secs(DEFAULT_DISCOVERY_INTERVAL),
        }
    }
    
    /// Resolve seed node addresses to node infos
    async fn resolve_seed_nodes(&self) -> ClusterResult<Vec<NodeInfo>> {
        let mut result = Vec::new();
        
        for seed in &self.seed_nodes {
            match seed.parse::<SocketAddr>() {
                Ok(addr) => {
                    // Create a placeholder node info for the seed node
                    // In a real implementation, you would connect to the node
                    // and retrieve its actual information
                    let node_id = NodeId::new();
                    let name = format!("seed-{}", node_id);
                    let role = NodeRole::Peer; // Assume peer role for seed nodes
                    
                    let node_info = NodeInfo::new(node_id, name, role, addr);
                    result.push(node_info);
                },
                Err(e) => {
                    return Err(ClusterError::AddressParseError(e));
                }
            }
        }
        
        Ok(result)
    }
}

#[async_trait]
impl ServiceDiscovery for StaticDiscovery {
    async fn init(&mut self) -> ClusterResult<()> {
        // Resolve seed nodes
        let seed_nodes = self.resolve_seed_nodes().await?;
        
        // Add seed nodes to discovered nodes
        for node in seed_nodes {
            self.nodes.insert(node.id.clone(), node);
        }
        
        Ok(())
    }
    
    async fn register_node(&mut self, node: &NodeInfo) -> ClusterResult<()> {
        // Store local node information
        if self.local_node.is_none() {
            self.local_node = Some(node.clone());
        }
        
        // Add node to discovered nodes
        self.nodes.insert(node.id.clone(), node.clone());
        
        Ok(())
    }
    
    async fn deregister_node(&mut self, node_id: &NodeId) -> ClusterResult<()> {
        // Remove node from discovered nodes
        self.nodes.remove(node_id);
        
        Ok(())
    }
    
    async fn discover_nodes(&mut self) -> ClusterResult<Vec<NodeInfo>> {
        // In static discovery, we just return the known nodes
        let nodes: Vec<NodeInfo> = self.nodes.values().cloned().collect();
        
        Ok(nodes)
    }
    
    async fn update_node_status(&mut self, node_id: &NodeId, status: NodeStatus) -> ClusterResult<()> {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.status = status;
            Ok(())
        } else {
            Err(ClusterError::NodeNotFoundError(format!("Node not found: {}", node_id)))
        }
    }
    
    fn discovery_interval(&self) -> Duration {
        self.discovery_interval
    }
    
    fn set_discovery_interval(&mut self, interval: Duration) {
        self.discovery_interval = interval;
    }
    
    async fn shutdown(&mut self) -> ClusterResult<()> {
        // No special cleanup needed for static discovery
        Ok(())
    }
}

/// LibP2P-based discovery using Kademlia DHT and mDNS
pub struct LibP2PDiscovery {
    /// Local node information
    local_node: Option<NodeInfo>,
    
    /// P2P transport instance
    transport: Option<Arc<Mutex<crate::transport::P2PTransport>>>,
    
    /// Known nodes
    nodes: HashMap<NodeId, NodeInfo>,
    
    /// Discovery refresh interval
    discovery_interval: Duration,
    
    /// Bootstrap nodes
    bootstrap_nodes: Vec<String>,
    
    /// Enable mDNS for local discovery
    enable_mdns: bool,
}

// Mark LibP2PDiscovery as safe to send and share across threads
unsafe impl Send for LibP2PDiscovery {}
unsafe impl Sync for LibP2PDiscovery {}

impl LibP2PDiscovery {
    /// Create a new LibP2P discovery service
    pub fn new(bootstrap_nodes: Vec<String>, enable_mdns: bool) -> Self {
        LibP2PDiscovery {
            local_node: None,
            transport: None,
            nodes: HashMap::new(),
            discovery_interval: Duration::from_secs(DEFAULT_DISCOVERY_INTERVAL),
            bootstrap_nodes,
            enable_mdns,
        }
    }
}

#[async_trait]
impl ServiceDiscovery for LibP2PDiscovery {
    async fn init(&mut self) -> ClusterResult<()> {
        if self.local_node.is_none() {
            return Err(ClusterError::ConfigurationError(
                "Local node must be registered before initializing LibP2P discovery".to_string()
            ));
        }
        
        // Initialize P2P transport
        let local_node = self.local_node.as_ref().unwrap().clone();
        let mut transport = crate::transport::P2PTransport::new(
            local_node.clone(),
            crate::serialization::SerializationFormat::Bincode, // Use bincode for efficiency
        )?;
        
        // Initialize the transport
        transport.init().await?;
        
        // Store the transport
        self.transport = Some(Arc::new(Mutex::new(transport)));
        
        // Add local node to known nodes
        self.nodes.insert(local_node.id.clone(), local_node);
        
        Ok(())
    }
    
    async fn register_node(&mut self, node: &NodeInfo) -> ClusterResult<()> {
        // Store local node information
        if self.local_node.is_none() {
            self.local_node = Some(node.clone());
        }
        
        // Add node to discovered nodes
        self.nodes.insert(node.id.clone(), node.clone());
        
        Ok(())
    }
    
    async fn deregister_node(&mut self, node_id: &NodeId) -> ClusterResult<()> {
        // Remove node from discovered nodes
        self.nodes.remove(node_id);
        
        Ok(())
    }
    
    async fn discover_nodes(&mut self) -> ClusterResult<Vec<NodeInfo>> {
        if let Some(transport) = &self.transport {
            // Get peers from transport
            let peers = transport.lock().unwrap().get_peers();
            
            // Merge with already known nodes
            for peer in peers {
                self.nodes.insert(peer.id.clone(), peer);
            }
        }
        
        // Return all known nodes
        let nodes: Vec<NodeInfo> = self.nodes.values().cloned().collect();
        
        Ok(nodes)
    }
    
    async fn update_node_status(&mut self, node_id: &NodeId, status: NodeStatus) -> ClusterResult<()> {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.status = status;
            Ok(())
        } else {
            Err(ClusterError::NodeNotFoundError(format!("Node not found: {}", node_id)))
        }
    }
    
    fn discovery_interval(&self) -> Duration {
        self.discovery_interval
    }
    
    fn set_discovery_interval(&mut self, interval: Duration) {
        self.discovery_interval = interval;
    }
    
    async fn shutdown(&mut self) -> ClusterResult<()> {
        // No special cleanup needed for now
        Ok(())
    }
}

/// Factory for creating discovery services based on configuration
pub fn create_discovery_service(method: DiscoveryMethod) -> Box<dyn ServiceDiscovery> {
    match method {
        DiscoveryMethod::Static { seed_nodes } => {
            Box::new(StaticDiscovery::new(seed_nodes))
        },
        DiscoveryMethod::Dns(domain) => {
            // In real implementation, create a DNS based discovery
            log::warn!("DNS discovery not fully implemented, using static discovery");
            Box::new(StaticDiscovery::new(Vec::new()))
        },
        DiscoveryMethod::Kubernetes { namespace, selector } => {
            // In real implementation, create a Kubernetes based discovery
            log::warn!("Kubernetes discovery not fully implemented, using static discovery");
            Box::new(StaticDiscovery::new(Vec::new()))
        },
        DiscoveryMethod::Multicast => {
            // In real implementation, create a multicast based discovery
            log::warn!("Multicast discovery not fully implemented, using static discovery");
            Box::new(StaticDiscovery::new(Vec::new()))
        },
        DiscoveryMethod::Gossip => {
            // In real implementation, create a gossip based discovery
            log::warn!("Gossip discovery not fully implemented, using static discovery");
            Box::new(StaticDiscovery::new(Vec::new()))
        },
        DiscoveryMethod::LibP2P { bootstrap_nodes, enable_mdns } => {
            // Create a LibP2P-based discovery service
            Box::new(LibP2PDiscovery::new(bootstrap_nodes, enable_mdns))
        },
    }
}

/// Mock discovery service for testing
#[cfg(test)]
pub struct MockDiscovery {
    nodes: HashMap<NodeId, NodeInfo>,
    discovery_interval: Duration,
}

#[cfg(test)]
impl MockDiscovery {
    pub fn new() -> Self {
        MockDiscovery {
            nodes: HashMap::new(),
            discovery_interval: Duration::from_secs(DEFAULT_DISCOVERY_INTERVAL),
        }
    }
    
    pub fn with_nodes(nodes: Vec<NodeInfo>) -> Self {
        let mut mock = Self::new();
        for node in nodes {
            mock.nodes.insert(node.id.clone(), node);
        }
        mock
    }
}

#[cfg(test)]
#[async_trait]
impl ServiceDiscovery for MockDiscovery {
    async fn init(&mut self) -> ClusterResult<()> {
        Ok(())
    }
    
    async fn register_node(&mut self, node: &NodeInfo) -> ClusterResult<()> {
        self.nodes.insert(node.id.clone(), node.clone());
        Ok(())
    }
    
    async fn deregister_node(&mut self, node_id: &NodeId) -> ClusterResult<()> {
        self.nodes.remove(node_id);
        Ok(())
    }
    
    async fn discover_nodes(&mut self) -> ClusterResult<Vec<NodeInfo>> {
        Ok(self.nodes.values().cloned().collect())
    }
    
    async fn update_node_status(&mut self, node_id: &NodeId, status: NodeStatus) -> ClusterResult<()> {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.status = status;
            Ok(())
        } else {
            Err(ClusterError::NodeNotFoundError(format!("Node not found: {}", node_id)))
        }
    }
    
    fn discovery_interval(&self) -> Duration {
        self.discovery_interval
    }
    
    fn set_discovery_interval(&mut self, interval: Duration) {
        self.discovery_interval = interval;
    }
    
    async fn shutdown(&mut self) -> ClusterResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    
    #[tokio::test]
    async fn test_static_discovery_init() {
        let seed_nodes = vec![
            "127.0.0.1:8558".to_string(),
            "127.0.0.1:8559".to_string(),
        ];
        
        let mut discovery = StaticDiscovery::new(seed_nodes);
        let result = discovery.init().await;
        
        assert!(result.is_ok());
        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 2);
    }
    
    #[tokio::test]
    async fn test_static_discovery_register_node() {
        let mut discovery = StaticDiscovery::new(Vec::new());
        
        let node_id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let node_info = NodeInfo::new(
            node_id.clone(),
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        let result = discovery.register_node(&node_info).await;
        assert!(result.is_ok());
        
        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, node_id);
    }
    
    #[tokio::test]
    async fn test_static_discovery_deregister_node() {
        let mut discovery = StaticDiscovery::new(Vec::new());
        
        let node_id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let node_info = NodeInfo::new(
            node_id.clone(),
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        discovery.register_node(&node_info).await.unwrap();
        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        
        let result = discovery.deregister_node(&node_id).await;
        assert!(result.is_ok());
        
        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 0);
    }
    
    #[tokio::test]
    async fn test_static_discovery_update_status() {
        let mut discovery = StaticDiscovery::new(Vec::new());
        
        let node_id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let node_info = NodeInfo::new(
            node_id.clone(),
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        discovery.register_node(&node_info).await.unwrap();
        
        // Update to UP
        discovery.update_node_status(&node_id, NodeStatus::Up).await.unwrap();
        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes[0].status, NodeStatus::Up);
        
        // Update to Unreachable
        discovery.update_node_status(&node_id, NodeStatus::Unreachable).await.unwrap();
        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes[0].status, NodeStatus::Unreachable);
    }
    
    #[tokio::test]
    async fn test_mock_discovery() {
        let node_id1 = NodeId::new();
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let node_info1 = NodeInfo::new(
            node_id1.clone(),
            "test-node-1".to_string(),
            NodeRole::Peer,
            addr1,
        );
        
        let node_id2 = NodeId::new();
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8559);
        let node_info2 = NodeInfo::new(
            node_id2.clone(),
            "test-node-2".to_string(),
            NodeRole::Peer,
            addr2,
        );
        
        let mut discovery = MockDiscovery::with_nodes(vec![node_info1.clone()]);
        
        // Check initial nodes
        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, node_id1);
        
        // Register a new node
        discovery.register_node(&node_info2).await.unwrap();
        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 2);
        
        // Deregister a node
        discovery.deregister_node(&node_id1).await.unwrap();
        let nodes = discovery.discover_nodes().await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, node_id2);
    }
    
    #[tokio::test]
    async fn test_libp2p_discovery_creation() {
        let bootstrap_nodes = vec![
            "/ip4/127.0.0.1/tcp/8558".to_string(),
            "/ip4/127.0.0.1/tcp/8559".to_string(),
        ];
        
        let discovery = LibP2PDiscovery::new(bootstrap_nodes, true);
        
        assert_eq!(discovery.bootstrap_nodes.len(), 2);
        assert!(discovery.enable_mdns);
        assert_eq!(discovery.discovery_interval(), Duration::from_secs(DEFAULT_DISCOVERY_INTERVAL));
    }
} 