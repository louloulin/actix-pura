//! State coordinator module
//!
//! This module provides functionality for coordinating state across a cluster.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use dataflare_core::error::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::cluster::ClusterManager;

/// State key type
pub type StateKey = String;

/// State value type
pub type StateValue = serde_json::Value;

/// State entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEntry {
    /// State key
    pub key: StateKey,
    /// State value
    pub value: StateValue,
    /// Version number
    pub version: u64,
    /// Last modified timestamp
    pub last_modified: u64,
    /// Node ID that last modified the state
    pub modified_by: String,
}

/// State coordinator for coordinating state across a cluster
pub struct StateCoordinator {
    /// Cluster manager
    cluster_manager: Arc<ClusterManager>,
    /// State store
    state_store: Arc<RwLock<HashMap<StateKey, StateEntry>>>,
    /// Coordinator task handle
    coordinator_task: Mutex<Option<JoinHandle<()>>>,
}

impl StateCoordinator {
    /// Create a new state coordinator
    pub fn new(cluster_manager: Arc<ClusterManager>) -> Result<Self> {
        Ok(Self {
            cluster_manager,
            state_store: Arc::new(RwLock::new(HashMap::new())),
            coordinator_task: Mutex::new(None),
        })
    }
    
    /// Start the state coordinator
    pub async fn start(&self) -> Result<()> {
        // Start coordinator task
        let state_store = self.state_store.clone();
        let cluster_manager = self.cluster_manager.clone();
        
        let task = tokio::spawn(async move {
            loop {
                // Synchronize state with other nodes (placeholder)
                
                // Sleep for a while
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });
        
        // Store task handle
        let mut coordinator_task = self.coordinator_task.lock().await;
        *coordinator_task = Some(task);
        
        Ok(())
    }
    
    /// Stop the state coordinator
    pub async fn stop(&self) -> Result<()> {
        // Stop coordinator task
        let mut coordinator_task = self.coordinator_task.lock().await;
        if let Some(task) = coordinator_task.take() {
            task.abort();
        }
        
        Ok(())
    }
    
    /// Set a state value
    pub fn set_state(&self, key: &str, value: StateValue) -> Result<()> {
        let mut state_store = self.state_store.write().unwrap();
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let version = if let Some(entry) = state_store.get(key) {
            entry.version + 1
        } else {
            1
        };
        
        let entry = StateEntry {
            key: key.to_string(),
            value,
            version,
            last_modified: now,
            modified_by: self.cluster_manager.local_node_id().to_string(),
        };
        
        state_store.insert(key.to_string(), entry);
        
        Ok(())
    }
    
    /// Get a state value
    pub fn get_state(&self, key: &str) -> Option<StateValue> {
        let state_store = self.state_store.read().unwrap();
        state_store.get(key).map(|entry| entry.value.clone())
    }
    
    /// Get a state entry
    pub fn get_state_entry(&self, key: &str) -> Option<StateEntry> {
        let state_store = self.state_store.read().unwrap();
        state_store.get(key).cloned()
    }
    
    /// Delete a state value
    pub fn delete_state(&self, key: &str) -> Result<()> {
        let mut state_store = self.state_store.write().unwrap();
        state_store.remove(key);
        Ok(())
    }
    
    /// Get all state entries
    pub fn get_all_state(&self) -> HashMap<StateKey, StateEntry> {
        let state_store = self.state_store.read().unwrap();
        state_store.clone()
    }
    
    /// Compare and set a state value
    pub fn compare_and_set(&self, key: &str, expected_version: u64, value: StateValue) -> Result<bool> {
        let mut state_store = self.state_store.write().unwrap();
        
        if let Some(entry) = state_store.get(key) {
            if entry.version != expected_version {
                return Ok(false);
            }
        } else if expected_version != 0 {
            return Ok(false);
        }
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let version = expected_version + 1;
        
        let entry = StateEntry {
            key: key.to_string(),
            value,
            version,
            last_modified: now,
            modified_by: self.cluster_manager.local_node_id().to_string(),
        };
        
        state_store.insert(key.to_string(), entry);
        
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CloudRuntimeConfig;
    use crate::{DiscoveryMethod, NodeRole};
    
    #[tokio::test]
    async fn test_state_coordinator() {
        let config = CloudRuntimeConfig {
            discovery_method: DiscoveryMethod::Static,
            node_role: NodeRole::Coordinator,
            heartbeat_interval: 1,
            node_timeout: 5,
        };
        
        let cluster_manager = Arc::new(ClusterManager::new(config).unwrap());
        let coordinator = StateCoordinator::new(cluster_manager.clone()).unwrap();
        
        coordinator.start().await.unwrap();
        
        // Set a state value
        coordinator.set_state("test_key", serde_json::json!("test_value")).unwrap();
        
        // Get the state value
        let value = coordinator.get_state("test_key").unwrap();
        assert_eq!(value, serde_json::json!("test_value"));
        
        // Get the state entry
        let entry = coordinator.get_state_entry("test_key").unwrap();
        assert_eq!(entry.key, "test_key");
        assert_eq!(entry.value, serde_json::json!("test_value"));
        assert_eq!(entry.version, 1);
        
        // Compare and set with wrong version
        let result = coordinator.compare_and_set("test_key", 0, serde_json::json!("new_value")).unwrap();
        assert_eq!(result, false);
        
        // Compare and set with correct version
        let result = coordinator.compare_and_set("test_key", 1, serde_json::json!("new_value")).unwrap();
        assert_eq!(result, true);
        
        // Get the updated value
        let value = coordinator.get_state("test_key").unwrap();
        assert_eq!(value, serde_json::json!("new_value"));
        
        // Delete the state
        coordinator.delete_state("test_key").unwrap();
        
        // Check that the state was deleted
        assert!(coordinator.get_state("test_key").is_none());
        
        coordinator.stop().await.unwrap();
    }
}
