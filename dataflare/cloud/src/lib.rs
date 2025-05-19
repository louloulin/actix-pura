//! # DataFlare Cloud
//!
//! Cloud mode support for the DataFlare data integration framework.
//! This crate provides distributed processing capabilities.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod runtime;
pub mod cluster;
pub mod scheduler;
pub mod coordinator;

// Re-exports for convenience
pub use runtime::CloudRuntime;
pub use cluster::ClusterManager;
pub use scheduler::TaskScheduler;
pub use coordinator::StateCoordinator;

/// Version of the DataFlare Cloud module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Cloud runtime configuration
#[derive(Debug, Clone)]
pub struct CloudRuntimeConfig {
    /// Discovery method for cluster nodes
    pub discovery_method: DiscoveryMethod,
    /// Role of this node in the cluster
    pub node_role: NodeRole,
    /// Heartbeat interval in seconds
    pub heartbeat_interval: u64,
    /// Node timeout in seconds
    pub node_timeout: u64,
}

/// Discovery method for cluster nodes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryMethod {
    /// Static list of nodes
    Static,
    /// DNS-based discovery
    Dns,
    /// Kubernetes-based discovery
    Kubernetes,
    /// Gossip-based discovery
    Gossip,
}

/// Role of a node in the cluster
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeRole {
    /// Coordinator node
    Coordinator,
    /// Worker node
    Worker,
}

impl Default for CloudRuntimeConfig {
    fn default() -> Self {
        Self {
            discovery_method: DiscoveryMethod::Static,
            node_role: NodeRole::Worker,
            heartbeat_interval: 5,
            node_timeout: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
