//! Actor registry module for tracking and discovering actors in the cluster.

use std::collections::HashMap;
use std::sync::Arc;
use actix::prelude::*;
use tokio::sync::RwLock;

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, PlacementStrategy};

/// Actor path representation
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ActorPath {
    /// Actor name
    pub name: String,
    
    /// Actor type
    pub actor_type: String,
    
    /// Node ID where the actor is located
    pub node_id: Option<NodeId>,
}

impl ActorPath {
    /// Create a new actor path
    pub fn new(name: String, actor_type: String, node_id: Option<NodeId>) -> Self {
        ActorPath {
            name,
            actor_type,
            node_id,
        }
    }
    
    /// Convert to string representation
    pub fn to_string(&self) -> String {
        if let Some(node_id) = &self.node_id {
            format!("{}@{}/{}", self.actor_type, node_id, self.name)
        } else {
            format!("{}/{}", self.actor_type, self.name)
        }
    }
}

/// Actor registration entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActorEntry {
    /// Actor path
    pub path: ActorPath,
    
    /// Placement strategy
    pub placement: PlacementStrategy,
    
    /// Is the actor a singleton in the cluster
    pub singleton: bool,
    
    /// Last heartbeat timestamp
    pub last_heartbeat: u64,
}

/// Actor registry for managing distributed actors
pub struct ActorRegistry {
    /// Registered actors by path
    actors: Arc<RwLock<HashMap<String, ActorEntry>>>,
    
    /// Local node ID
    local_node_id: NodeId,
}

impl ActorRegistry {
    /// Create a new actor registry
    pub fn new(local_node_id: NodeId) -> Self {
        ActorRegistry {
            actors: Arc::new(RwLock::new(HashMap::new())),
            local_node_id,
        }
    }
    
    /// Register an actor
    pub async fn register_actor<A: Actor>(&self, name: &str, addr: Addr<A>, singleton: bool, placement: PlacementStrategy) -> ClusterResult<ActorPath> {
        let actor_type = std::any::type_name::<A>().to_string();
        let path = ActorPath::new(name.to_string(), actor_type, Some(self.local_node_id.clone()));
        
        let entry = ActorEntry {
            path: path.clone(),
            placement,
            singleton,
            last_heartbeat: current_timestamp(),
        };
        
        let mut actors = self.actors.write().await;
        actors.insert(path.to_string(), entry);
        
        Ok(path)
    }
    
    /// Deregister an actor
    pub async fn deregister_actor(&self, path: &ActorPath) -> ClusterResult<()> {
        let mut actors = self.actors.write().await;
        actors.remove(&path.to_string());
        
        Ok(())
    }
    
    /// Lookup an actor
    pub async fn lookup_actor(&self, path: &ActorPath) -> ClusterResult<Option<ActorEntry>> {
        let actors = self.actors.read().await;
        Ok(actors.get(&path.to_string()).cloned())
    }
    
    /// Get all registered actors
    pub async fn get_all_actors(&self) -> ClusterResult<Vec<ActorEntry>> {
        let actors = self.actors.read().await;
        Ok(actors.values().cloned().collect())
    }
    
    /// Update heartbeat for an actor
    pub async fn update_heartbeat(&self, path: &ActorPath) -> ClusterResult<()> {
        let mut actors = self.actors.write().await;
        if let Some(entry) = actors.get_mut(&path.to_string()) {
            entry.last_heartbeat = current_timestamp();
        }
        
        Ok(())
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeId;
    
    // Mock actor for testing
    struct MockActor;
    
    impl Actor for MockActor {
        type Context = Context<Self>;
    }
    
    #[tokio::test]
    async fn test_actor_path() {
        let node_id = NodeId::new();
        let path = ActorPath::new(
            "test".to_string(),
            "MockActor".to_string(),
            Some(node_id.clone()),
        );
        
        assert_eq!(path.name, "test");
        assert_eq!(path.actor_type, "MockActor");
        assert_eq!(path.node_id, Some(node_id));
        
        let path_str = path.to_string();
        assert!(path_str.contains("MockActor"));
        assert!(path_str.contains("test"));
    }
} 