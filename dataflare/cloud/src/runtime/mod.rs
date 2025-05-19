//! Cloud runtime module
//!
//! This module provides the cloud runtime for DataFlare.

use dataflare_core::error::Result;
use dataflare_runtime::RuntimeMode;
use std::sync::Arc;

use crate::cluster::ClusterManager;
use crate::scheduler::TaskScheduler;
use crate::coordinator::StateCoordinator;
use crate::CloudRuntimeConfig;

/// Cloud runtime for DataFlare
pub struct CloudRuntime {
    /// Cloud runtime configuration
    config: CloudRuntimeConfig,
    /// Cluster manager
    cluster_manager: Arc<ClusterManager>,
    /// Task scheduler
    task_scheduler: Arc<TaskScheduler>,
    /// State coordinator
    state_coordinator: Arc<StateCoordinator>,
}

impl CloudRuntime {
    /// Create a new cloud runtime with the given configuration
    pub async fn new(config: CloudRuntimeConfig) -> Result<Self> {
        let cluster_manager = Arc::new(ClusterManager::new(config.clone())?);
        let task_scheduler = Arc::new(TaskScheduler::new(cluster_manager.clone())?);
        let state_coordinator = Arc::new(StateCoordinator::new(cluster_manager.clone())?);
        
        Ok(Self {
            config,
            cluster_manager,
            task_scheduler,
            state_coordinator,
        })
    }
    
    /// Start the cloud runtime
    pub async fn start(&self) -> Result<()> {
        // Start cluster manager
        self.cluster_manager.start().await?;
        
        // Start task scheduler
        self.task_scheduler.start().await?;
        
        // Start state coordinator
        self.state_coordinator.start().await?;
        
        Ok(())
    }
    
    /// Stop the cloud runtime
    pub async fn stop(&self) -> Result<()> {
        // Stop state coordinator
        self.state_coordinator.stop().await?;
        
        // Stop task scheduler
        self.task_scheduler.stop().await?;
        
        // Stop cluster manager
        self.cluster_manager.stop().await?;
        
        Ok(())
    }
    
    /// Get the runtime mode
    pub fn mode(&self) -> RuntimeMode {
        RuntimeMode::Cloud
    }
    
    /// Get the cluster manager
    pub fn cluster_manager(&self) -> Arc<ClusterManager> {
        self.cluster_manager.clone()
    }
    
    /// Get the task scheduler
    pub fn task_scheduler(&self) -> Arc<TaskScheduler> {
        self.task_scheduler.clone()
    }
    
    /// Get the state coordinator
    pub fn state_coordinator(&self) -> Arc<StateCoordinator> {
        self.state_coordinator.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiscoveryMethod, NodeRole};
    
    #[tokio::test]
    async fn test_cloud_runtime() {
        let config = CloudRuntimeConfig {
            discovery_method: DiscoveryMethod::Static,
            node_role: NodeRole::Coordinator,
            heartbeat_interval: 5,
            node_timeout: 30,
        };
        
        let runtime = CloudRuntime::new(config).await.unwrap();
        
        assert_eq!(runtime.mode(), RuntimeMode::Cloud);
        assert_eq!(runtime.config.discovery_method, DiscoveryMethod::Static);
        assert_eq!(runtime.config.node_role, NodeRole::Coordinator);
        assert_eq!(runtime.config.heartbeat_interval, 5);
        assert_eq!(runtime.config.node_timeout, 30);
        
        runtime.start().await.unwrap();
        runtime.stop().await.unwrap();
    }
}
