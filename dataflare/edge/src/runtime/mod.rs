//! Edge runtime module
//!
//! This module provides the edge runtime for DataFlare.

use dataflare_core::error::Result;
use dataflare_runtime::RuntimeMode;
use std::sync::Arc;

use crate::resource::ResourceMonitor;
use crate::cache::OfflineCache;
use crate::sync::SyncManager;
use crate::EdgeRuntimeConfig;

/// Edge runtime for DataFlare
pub struct EdgeRuntime {
    /// Edge runtime configuration
    config: EdgeRuntimeConfig,
    /// Resource monitor
    resource_monitor: Arc<ResourceMonitor>,
    /// Offline cache
    offline_cache: Arc<OfflineCache>,
    /// Sync manager
    sync_manager: Arc<SyncManager>,
}

impl EdgeRuntime {
    /// Create a new edge runtime with the given configuration
    pub fn new(config: EdgeRuntimeConfig) -> Result<Self> {
        let resource_monitor = Arc::new(ResourceMonitor::new(config.max_memory_mb, config.max_cpu_usage));
        let offline_cache = Arc::new(OfflineCache::new());
        let sync_manager = Arc::new(SyncManager::new(config.sync_interval));
        
        Ok(Self {
            config,
            resource_monitor,
            offline_cache,
            sync_manager,
        })
    }
    
    /// Start the edge runtime
    pub async fn start(&self) -> Result<()> {
        // Start resource monitoring
        self.resource_monitor.start();
        
        // Start sync manager if not in offline mode
        if !self.config.offline_mode {
            self.sync_manager.start().await?;
        }
        
        Ok(())
    }
    
    /// Stop the edge runtime
    pub async fn stop(&self) -> Result<()> {
        // Stop resource monitoring
        self.resource_monitor.stop();
        
        // Stop sync manager if not in offline mode
        if !self.config.offline_mode {
            self.sync_manager.stop().await?;
        }
        
        Ok(())
    }
    
    /// Get the runtime mode
    pub fn mode(&self) -> RuntimeMode {
        RuntimeMode::Edge
    }
    
    /// Get the resource monitor
    pub fn resource_monitor(&self) -> Arc<ResourceMonitor> {
        self.resource_monitor.clone()
    }
    
    /// Get the offline cache
    pub fn offline_cache(&self) -> Arc<OfflineCache> {
        self.offline_cache.clone()
    }
    
    /// Get the sync manager
    pub fn sync_manager(&self) -> Arc<SyncManager> {
        self.sync_manager.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_edge_runtime() {
        let config = EdgeRuntimeConfig {
            max_memory_mb: 256,
            max_cpu_usage: 0.5,
            offline_mode: true,
            sync_interval: 60,
        };
        
        let runtime = EdgeRuntime::new(config).unwrap();
        
        assert_eq!(runtime.mode(), RuntimeMode::Edge);
        assert_eq!(runtime.config.max_memory_mb, 256);
        assert_eq!(runtime.config.max_cpu_usage, 0.5);
        assert_eq!(runtime.config.offline_mode, true);
        assert_eq!(runtime.config.sync_interval, 60);
        
        runtime.start().await.unwrap();
        runtime.stop().await.unwrap();
    }
}
