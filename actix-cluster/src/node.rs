//! Node module for cluster node identification and management.

use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

use crate::config::NodeRole;
use crate::error::ClusterResult;

/// Unique identifier for a node in the cluster
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Copy)]
pub struct NodeId(pub Uuid);

impl PartialOrd for NodeId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl Ord for NodeId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl NodeId {
    /// Create a new random node ID
    pub fn new() -> Self {
        NodeId(Uuid::new_v4())
    }
    
    /// Create a local node ID (uses nil UUID)
    pub fn local() -> Self {
        NodeId(Uuid::nil())
    }
    
    /// Check if this is a local node ID
    pub fn is_local(&self) -> bool {
        self.0 == Uuid::nil()
    }
    
    /// Get the underlying UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
    
    /// Convert to string representation
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Node health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is up and connected
    Up,
    
    /// Node is up but unreachable/suspicious
    Unreachable,
    
    /// Node is down or has left the cluster
    Down,
    
    /// Node is joining the cluster
    Joining,
    
    /// Node is leaving the cluster
    Leaving,
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeStatus::Up => write!(f, "Up"),
            NodeStatus::Unreachable => write!(f, "Unreachable"),
            NodeStatus::Down => write!(f, "Down"),
            NodeStatus::Joining => write!(f, "Joining"),
            NodeStatus::Leaving => write!(f, "Leaving"),
        }
    }
}

/// Actor placement strategies for distributed actors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementStrategy {
    /// Place actor on any node (default)
    Random,
    
    /// Place actor on a specific node
    Node(Uuid),
    
    /// Place actor on nodes in a round-robin fashion
    RoundRobin,
    
    /// Place actor on the node with the least load
    LeastLoaded,
    
    /// Place actor on multiple nodes for redundancy
    Redundant {
        /// Number of replicas to create
        replicas: usize,
    },
}

/// Information about a node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique node identifier
    pub id: NodeId,
    
    /// Human-readable node name
    pub name: String,
    
    /// Node role in the cluster
    pub role: NodeRole,
    
    /// Node host and port for cluster communication
    pub addr: SocketAddr,
    
    /// Current status of the node
    pub status: NodeStatus,
    
    /// Time when the node joined the cluster
    pub joined_at: Option<u64>,
    
    /// Node capabilities and features
    pub capabilities: Vec<String>,
    
    /// Current node load (0-100)
    pub load: u8,
    
    /// Node metadata for custom attributes
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

impl NodeInfo {
    /// Create a new node info
    pub fn new(id: NodeId, name: String, role: NodeRole, addr: SocketAddr) -> Self {
        NodeInfo {
            id,
            name,
            role,
            addr,
            status: NodeStatus::Joining,
            joined_at: None,
            capabilities: Vec::new(),
            load: 0,
            metadata: serde_json::Map::new(),
        }
    }
    
    /// Mark node as up
    pub fn mark_up(&mut self) {
        self.status = NodeStatus::Up;
        if self.joined_at.is_none() {
            // Current time in milliseconds since UNIX epoch
            self.joined_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
            );
        }
    }
    
    /// Mark node as unreachable
    pub fn mark_unreachable(&mut self) {
        self.status = NodeStatus::Unreachable;
    }
    
    /// Mark node as down
    pub fn mark_down(&mut self) {
        self.status = NodeStatus::Down;
    }
    
    /// Mark node as leaving
    pub fn mark_leaving(&mut self) {
        self.status = NodeStatus::Leaving;
    }
    
    /// Check if node is up
    pub fn is_up(&self) -> bool {
        self.status == NodeStatus::Up
    }
    
    /// Add a capability to the node
    pub fn add_capability(&mut self, capability: String) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
    }
    
    /// Check if node has a specific capability
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(&capability.to_string())
    }
    
    /// Set node load
    pub fn set_load(&mut self, load: u8) {
        self.load = load.min(100); // Ensure load is between 0-100
    }
    
    /// Add metadata to the node
    pub fn add_metadata(&mut self, key: String, value: serde_json::Value) {
        self.metadata.insert(key, value);
    }
    
    /// Get metadata from the node
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }
}

/// Node tracking state for heartbeat and failure detection
pub struct NodeTrackingState {
    /// Last time a heartbeat was received from this node
    pub last_heartbeat: Instant,
    
    /// Number of consecutive heartbeat failures
    pub heartbeat_failures: u32,
    
    /// Maximum allowed heartbeat failures before marking node as unreachable
    pub max_heartbeat_failures: u32,
}

impl NodeTrackingState {
    /// Create a new node tracking state
    pub fn new(max_heartbeat_failures: u32) -> Self {
        NodeTrackingState {
            last_heartbeat: Instant::now(),
            heartbeat_failures: 0,
            max_heartbeat_failures,
        }
    }
    
    /// Record a heartbeat
    pub fn record_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
        self.heartbeat_failures = 0;
    }
    
    /// Check if node is considered unreachable
    pub fn is_unreachable(&self, timeout: Duration) -> bool {
        self.last_heartbeat.elapsed() > timeout || self.heartbeat_failures >= self.max_heartbeat_failures
    }
    
    /// Record a heartbeat failure
    pub fn record_failure(&mut self) -> bool {
        self.heartbeat_failures += 1;
        self.heartbeat_failures >= self.max_heartbeat_failures
    }
}

/// Active node representation with management logic
pub struct Node {
    /// Node information
    pub info: NodeInfo,
    
    /// Node tracking state for failure detection
    tracking: NodeTrackingState,
}

impl Node {
    /// Create a new node
    pub fn new(info: NodeInfo, max_heartbeat_failures: u32) -> Self {
        Node {
            info,
            tracking: NodeTrackingState::new(max_heartbeat_failures),
        }
    }
    
    /// Get node ID
    pub fn id(&self) -> &NodeId {
        &self.info.id
    }
    
    /// Get node address
    pub fn addr(&self) -> SocketAddr {
        self.info.addr
    }
    
    /// Get node role
    pub fn role(&self) -> &NodeRole {
        &self.info.role
    }
    
    /// Get node status
    pub fn status(&self) -> NodeStatus {
        self.info.status
    }
    
    /// Record a heartbeat from this node
    pub fn record_heartbeat(&mut self) {
        self.tracking.record_heartbeat();
        self.info.mark_up(); // Ensure node is marked as up
    }
    
    /// Update the last seen timestamp (convenience method for record_heartbeat)
    pub fn update_last_seen(&mut self) {
        self.record_heartbeat();
    }
    
    /// Check if node is unreachable based on heartbeat timeout
    pub fn check_timeout(&mut self, timeout: Duration) -> bool {
        if self.tracking.is_unreachable(timeout) && self.info.status == NodeStatus::Up {
            self.info.mark_unreachable();
            true
        } else {
            false
        }
    }
    
    /// Check if node is timed out without modifying state
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        self.tracking.is_unreachable(timeout) && self.info.status == NodeStatus::Up
    }
    
    /// Record a heartbeat failure
    pub fn record_failure(&mut self) -> bool {
        let is_unreachable = self.tracking.record_failure();
        if is_unreachable && self.info.status == NodeStatus::Up {
            self.info.mark_unreachable();
            true
        } else {
            false
        }
    }
    
    /// Mark node as leaving the cluster
    pub fn mark_leaving(&mut self) {
        self.info.mark_leaving();
    }
    
    /// Mark node as down
    pub fn mark_down(&mut self) {
        self.info.mark_down();
    }
    
    /// Update node information
    pub fn update_info(&mut self, info: NodeInfo) -> ClusterResult<()> {
        // Keep the same ID
        if self.info.id != info.id {
            return Err(crate::error::ClusterError::InvalidOperation(
                format!("Cannot update node info with different ID: {} != {}", 
                    self.info.id, info.id)
            ));
        }
        
        self.info = info;
        Ok(())
    }
    
    /// Time since last heartbeat
    pub fn time_since_last_heartbeat(&self) -> Duration {
        self.tracking.last_heartbeat.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::thread;
    
    #[test]
    fn test_node_id_creation() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        
        assert_ne!(id1, id2, "Node IDs should be unique");
        assert_eq!(id1, id1.clone(), "Node ID should be clonable and equal");
    }
    
    #[test]
    fn test_node_info_status_transitions() {
        let node_id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let mut info = NodeInfo::new(
            node_id,
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        assert_eq!(info.status, NodeStatus::Joining);
        
        info.mark_up();
        assert_eq!(info.status, NodeStatus::Up);
        assert!(info.joined_at.is_some());
        
        info.mark_unreachable();
        assert_eq!(info.status, NodeStatus::Unreachable);
        
        info.mark_up();
        assert_eq!(info.status, NodeStatus::Up);
        
        info.mark_leaving();
        assert_eq!(info.status, NodeStatus::Leaving);
        
        info.mark_down();
        assert_eq!(info.status, NodeStatus::Down);
    }
    
    #[test]
    fn test_node_capabilities() {
        let node_id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let mut info = NodeInfo::new(
            node_id,
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        assert!(!info.has_capability("compute"));
        
        info.add_capability("compute".to_string());
        assert!(info.has_capability("compute"));
        
        // Adding same capability twice should not duplicate
        info.add_capability("compute".to_string());
        assert_eq!(info.capabilities.len(), 1);
    }
    
    #[test]
    fn test_node_metadata() {
        let node_id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let mut info = NodeInfo::new(
            node_id,
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        assert!(info.get_metadata("region").is_none());
        
        info.add_metadata("region".to_string(), serde_json::json!("us-west"));
        assert_eq!(info.get_metadata("region").unwrap(), &serde_json::json!("us-west"));
    }
    
    #[test]
    fn test_node_tracking_state() {
        let mut tracking = NodeTrackingState::new(3);
        
        // New tracking state should not be unreachable
        assert!(!tracking.is_unreachable(Duration::from_secs(1)));
        
        // Record a heartbeat and check state
        tracking.record_heartbeat();
        assert_eq!(tracking.heartbeat_failures, 0);
        
        // Record failures until unreachable
        assert!(!tracking.record_failure());  // 1 failure
        assert!(!tracking.record_failure());  // 2 failures
        assert!(tracking.record_failure());   // 3 failures - unreachable
        
        // Record a heartbeat to reset failures
        tracking.record_heartbeat();
        assert_eq!(tracking.heartbeat_failures, 0);
        
        // Test timeout-based unreachability
        thread::sleep(Duration::from_millis(50));
        assert!(tracking.is_unreachable(Duration::from_millis(20)));
        assert!(!tracking.is_unreachable(Duration::from_millis(100)));
    }
    
    #[test]
    fn test_node_heartbeat_management() {
        let node_id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let info = NodeInfo::new(
            node_id,
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        let mut node = Node::new(info, 3);
        
        // Initial state
        assert_eq!(node.status(), NodeStatus::Joining);
        
        // Mark up and check
        node.info.mark_up();
        assert_eq!(node.status(), NodeStatus::Up);
        
        // Record heartbeat
        node.record_heartbeat();
        assert_eq!(node.status(), NodeStatus::Up);
        
        // Record failures until unreachable
        assert!(!node.record_failure());  // 1 failure - still up
        assert_eq!(node.status(), NodeStatus::Up);
        
        assert!(!node.record_failure());  // 2 failures - still up
        assert_eq!(node.status(), NodeStatus::Up);
        
        assert!(node.record_failure());   // 3 failures - unreachable
        assert_eq!(node.status(), NodeStatus::Unreachable);
        
        // Record heartbeat to become up again
        node.record_heartbeat();
        assert_eq!(node.status(), NodeStatus::Up);
    }
    
    #[test]
    fn test_node_update_info() {
        let node_id = NodeId::new();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8558);
        let info = NodeInfo::new(
            node_id.clone(),
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        let mut node = Node::new(info, 3);
        
        // Update with same ID should work
        let mut new_info = NodeInfo::new(
            node_id,
            "updated-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        new_info.add_capability("compute".to_string());
        
        assert!(node.update_info(new_info).is_ok());
        assert_eq!(node.info.name, "updated-node");
        assert!(node.info.has_capability("compute"));
        
        // Update with different ID should fail
        let different_id = NodeId::new();
        let different_info = NodeInfo::new(
            different_id,
            "different-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        assert!(node.update_info(different_info).is_err());
    }
} 