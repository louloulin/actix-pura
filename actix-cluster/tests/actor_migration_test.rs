// Tests for actor migration and placement strategies

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use actix::prelude::*;
use actix_cluster::prelude::*;
use actix_cluster::{
    error::{ClusterError, ClusterResult},
    migration::{MigratableActor, MigrationManager, MigrationOptions, MigrationReason, MigrationStatus},
    node::{NodeId, NodeInfo, PlacementStrategy},
    placement::{NodeSelector, PlacementStrategyImpl},
    registry::ActorRegistry,
    serialization::SerializationFormat,
};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

// Mock actor with state that can be migrated
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TestMigratableActor {
    id: Uuid,
    counter: i32,
    name: String,
}

impl Actor for TestMigratableActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("TestMigratableActor started with ID: {}", self.id);
    }
}

// Add trait implementation directly in the test
trait MigratableActorTest: Actor {
    fn get_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
    fn restore_state(&mut self, state: Vec<u8>) -> Result<(), Box<dyn std::error::Error>>;
    fn before_migration(&mut self, ctx: &mut Self::Context) {}
    fn after_migration(&mut self, ctx: &mut Self::Context) {}
    fn can_migrate(&self) -> bool { true }
}

impl MigratableActorTest for TestMigratableActor {
    fn get_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let bytes = bincode::serialize(self)?;
        Ok(bytes)
    }
    
    fn restore_state(&mut self, state: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        let restored: TestMigratableActor = bincode::deserialize(&state)?;
        self.counter = restored.counter;
        self.name = restored.name;
        // Don't restore ID to maintain identity
        Ok(())
    }
    
    fn before_migration(&mut self, _ctx: &mut Self::Context) {
        println!("Before migration for actor: {}", self.id);
    }
    
    fn after_migration(&mut self, _ctx: &mut Self::Context) {
        println!("After migration for actor: {}", self.id);
        // Increment counter when migrated
        self.counter += 1;
    }
    
    fn can_migrate(&self) -> bool {
        // Example condition: only migrate actors with counter < 5
        self.counter < 5
    }
}

// Add trait implementation directly in the test
trait DistributedActorTest: Actor {
    fn actor_path(&self) -> String;
    fn placement_strategy(&self) -> PlacementStrategy {
        PlacementStrategy::RoundRobin
    }
    fn serialization_format(&self) -> SerializationFormat {
        SerializationFormat::Bincode
    }
}

impl DistributedActorTest for TestMigratableActor {
    fn actor_path(&self) -> String {
        format!("/user/test-actor-{}", self.id)
    }
    
    fn placement_strategy(&self) -> PlacementStrategy {
        PlacementStrategy::RoundRobin
    }
    
    fn serialization_format(&self) -> SerializationFormat {
        SerializationFormat::Bincode
    }
}

// Message to increment counter
#[derive(Message)]
#[rtype(result = "i32")]
struct IncrementCounter(i32);

impl Handler<IncrementCounter> for TestMigratableActor {
    type Result = i32;
    
    fn handle(&mut self, msg: IncrementCounter, _ctx: &mut Self::Context) -> Self::Result {
        self.counter += msg.0;
        self.counter
    }
}

// Message to get current counter value
#[derive(Message)]
#[rtype(result = "i32")]
struct GetCounter;

impl Handler<GetCounter> for TestMigratableActor {
    type Result = i32;
    
    fn handle(&mut self, _msg: GetCounter, _ctx: &mut Self::Context) -> Self::Result {
        self.counter
    }
}

// Mock node selector for testing
struct MockNodeSelector {
    nodes: HashMap<NodeId, NodeInfo>,
    active_nodes: Vec<NodeId>,
}

impl MockNodeSelector {
    fn new() -> Self {
        let mut nodes = HashMap::new();
        let mut active_nodes = Vec::new();
        
        // Create 3 mock nodes
        for i in 0..3 {
            let id = Uuid::new_v4();
            let mut info = NodeInfo::new(
                id,
                format!("node-{}", i),
                NodeRole::Peer,
                format!("127.0.0.1:{}00", 80 + i).parse().unwrap(),
            );
            
            // Set different loads
            info.set_load((i as u8 * 30) % 100);
            
            nodes.insert(id, info);
            active_nodes.push(id);
        }
        
        Self { nodes, active_nodes }
    }
}

impl NodeSelector for MockNodeSelector {
    fn select_node(&self, actor_path: &str, strategy: &PlacementStrategy) -> ClusterResult<NodeId> {
        let placement = PlacementStrategyImpl::new(Arc::new(Self::new()));
        placement.select_node(actor_path, strategy)
    }
    
    fn get_active_nodes(&self) -> Vec<NodeId> {
        self.active_nodes.clone()
    }
    
    fn get_node_info(&self, node_id: &NodeId) -> Option<NodeInfo> {
        self.nodes.get(node_id).cloned()
    }
}

// Extension trait to simplify testing
trait ActorTestExt: Actor + Sized {
    fn start_distributed(self) -> Addr<Self> {
        Self::create(|_| self)
    }
}

impl<T: Actor + Sized> ActorTestExt for T {}

#[test]
fn test_placement_strategy() {
    let system = System::new();
    
    system.block_on(async {
        let selector = Arc::new(MockNodeSelector::new());
        let placement = PlacementStrategyImpl::new(selector.clone());
        
        // Test random strategy
        let node1 = placement.select_node("test_actor", &PlacementStrategy::Random).unwrap();
        assert!(selector.nodes.contains_key(&node1));
        
        // Test round-robin strategy
        let node2 = placement.select_node("test_actor", &PlacementStrategy::RoundRobin).unwrap();
        let node3 = placement.select_node("test_actor", &PlacementStrategy::RoundRobin).unwrap();
        let node4 = placement.select_node("test_actor", &PlacementStrategy::RoundRobin).unwrap();
        
        // We should have cycled through all nodes
        assert_ne!(node2, node3);
        assert_ne!(node3, node4);
        
        // Test least loaded strategy
        let node5 = placement.select_node("test_actor", &PlacementStrategy::LeastLoaded).unwrap();
        let info = selector.nodes.get(&node5).unwrap();
        
        // Verify it's the least loaded node
        let is_least_loaded = selector.nodes.values().all(|n| n.load >= info.load);
        assert!(is_least_loaded);
        
        System::current().stop();
    });
}

#[test]
fn test_migratable_actor() {
    let system = System::new();
    
    system.block_on(async {
        // Create a migratable actor
        let actor = TestMigratableActor {
            id: Uuid::new_v4(),
            counter: 1,
            name: "test-actor".to_string(),
        };
        
        // Start the actor
        let addr = actor.start_distributed();
        
        // Increment counter
        let res = addr.send(IncrementCounter(2)).await.unwrap();
        assert_eq!(res, 3);
        
        // Serialize actor state
        let actor_state = addr.send(GetCounter).await.unwrap();
        assert_eq!(actor_state, 3);
        
        // Test state serialization and restoration
        let actor_id = actor.id;
        let actor_clone = actor.clone();
        
        // Serialize the state
        let state_bytes = actor_clone.get_state().unwrap();
        
        // Create a new actor instance
        let mut new_actor = TestMigratableActor {
            id: actor_id, // Keep same ID
            counter: 0,   // Different counter
            name: "".to_string(), // Empty name
        };
        
        // Restore state
        new_actor.restore_state(state_bytes).unwrap();
        
        // Verify restored state (counter and name should be restored)
        assert_eq!(new_actor.counter, 3);
        assert_eq!(new_actor.name, "test-actor");
        assert_eq!(new_actor.id, actor_id); // ID should be preserved
        
        System::current().stop();
    });
}

#[test]
fn test_migration_manager() {
    let system = System::new();
    
    system.block_on(async {
        // Create registry and nodes
        let local_node_id = Uuid::new_v4();
        let registry = Arc::new(ActorRegistry::new(local_node_id));
        
        // Create migration manager
        let mut migration_manager = MigrationManager::new(local_node_id, registry.clone());
        
        // Create target node
        let target_node = Uuid::new_v4();
        
        // Register a local actor for testing
        let actor = TestMigratableActor {
            id: Uuid::new_v4(),
            counter: 1,
            name: "test-actor".to_string(),
        };
        
        let actor_path = actor.actor_path();
        let addr = actor.start_distributed();
        
        // Register the actor with the registry
        registry.register_local(&actor_path, addr.recipient()).unwrap();
        
        // Verify we can look it up
        assert!(registry.lookup(&actor_path).is_some());
        
        // Test migration request
        let migration_id = migration_manager.migrate_actor(
            &actor_path,
            target_node.clone(),
            MigrationReason::LoadBalancing,
            MigrationOptions::default(),
        ).await.unwrap();
        
        // Check that migration is in progress
        assert_eq!(
            migration_manager.get_migration_status(migration_id), 
            Some(MigrationStatus::InProgress)
        );
        
        // Complete the migration 
        migration_manager.complete_migration(
            migration_id, 
            format!("/user/test-actor-migrated-{}", actor.id)
        ).await.unwrap();
        
        // Check that migration is completed
        assert_eq!(
            migration_manager.get_migration_status(migration_id), 
            Some(MigrationStatus::Completed)
        );
        
        System::current().stop();
    });
} 