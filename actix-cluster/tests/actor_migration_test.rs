// Tests for actor migration and placement strategies

use actix::prelude::*;
use actix_cluster::{
    node::{NodeId, NodeInfo, PlacementStrategy},
    error::{ClusterError, ClusterResult},
    message::{DeliveryGuarantee, AnyMessage},
    migration::{MigrationManager, MigrationOptions, MigrationReason, MigrationStatus},
    registry::{ActorRegistry, ActorRef},
    config::NodeRole,
    placement::NodeSelector,
    placement::PlacementStrategyImpl,
    serialization::SerializationFormat,
    transport::{P2PTransport, TransportMessage}
};
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use rand::{random, seq::SliceRandom};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use tokio::sync::Mutex as TokioMutex;
use std::net::SocketAddr;

// TestMigratableActor 测试可迁移的 Actor
#[derive(Clone, Serialize, Deserialize)]
struct TestMigratableActor {
    id: Uuid,
    counter: i32,
    name: String,
}

// AnyMessage处理
impl Handler<AnyMessage> for TestMigratableActor {
    type Result = ();
    
    fn handle(&mut self, _msg: AnyMessage, _ctx: &mut Self::Context) -> Self::Result {
        println!("TestMigratableActor received AnyMessage");
    }
}

impl Actor for TestMigratableActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("TestMigratableActor started, id={}", self.id);
    }
}

// 测试可迁移Actor所需的trait
trait MigratableActorTest: Actor {
    fn get_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
    fn restore_state(&mut self, state: Vec<u8>) -> Result<(), Box<dyn std::error::Error>>;
    fn before_migration(&mut self, _ctx: &mut Self::Context) {}
    fn after_migration(&mut self, _ctx: &mut Self::Context) {}
    fn can_migrate(&self) -> bool { true }
}

impl MigratableActorTest for TestMigratableActor {
    fn get_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let serialized = serde_json::to_vec(self)?;
        Ok(serialized)
    }
    
    fn restore_state(&mut self, state: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        let deserialized: TestMigratableActor = serde_json::from_slice(&state)?;
        self.counter = deserialized.counter;
        self.name = deserialized.name;
        Ok(())
    }
}

// DistributedActor接口定义
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
}

// 测试消息定义
struct IncrementCounter(i32);

impl Message for IncrementCounter {
    type Result = i32;
}

impl Handler<IncrementCounter> for TestMigratableActor {
    type Result = i32;
    
    fn handle(&mut self, msg: IncrementCounter, _ctx: &mut Self::Context) -> Self::Result {
        self.counter += msg.0;
        self.counter
    }
}

// 获取计数器消息
struct GetCounter;

impl Message for GetCounter {
    type Result = i32;
}

impl Handler<GetCounter> for TestMigratableActor {
    type Result = i32;
    
    fn handle(&mut self, _msg: GetCounter, _ctx: &mut Self::Context) -> Self::Result {
        self.counter
    }
}

// Mock实现NodeSelector接口的选择器
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
            let uuid = Uuid::new_v4();
            let id = NodeId(uuid);
            let mut info = NodeInfo::new(
                id.clone(),
                format!("node-{}", i),
                NodeRole::Peer,
                format!("127.0.0.1:{}00", 80 + i).parse().unwrap(),
            );
            
            // Set different loads
            info.set_load((i as u8 * 30) % 100);
            
            // Clone id to avoid ownership issues
            nodes.insert(id.clone(), info);
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
trait ActorTestExt: Actor<Context = Context<Self>> + Sized {
    fn start_distributed(self) -> Addr<Self> {
        Self::create(|_| self)
    }
}

impl<T: Actor<Context = Context<T>> + Sized> ActorTestExt for T {}

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
        let addr = actor.clone().start_distributed();
        
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
        
        // 比较counter值为1，不是3
        assert_eq!(new_actor.counter, 1);
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
        let uuid = Uuid::new_v4();
        let local_node_id = NodeId(uuid);
        
        // 创建本地节点信息用于创建真实的P2PTransport
        let addr: SocketAddr = "127.0.0.1:8000".parse().unwrap();
        let local_node = NodeInfo::new(
            local_node_id.clone(),
            "test-node".to_string(),
            NodeRole::Peer,
            addr,
        );
        
        // 创建真实的P2PTransport
        let transport_result = P2PTransport::new(local_node, SerializationFormat::Bincode);
        assert!(transport_result.is_ok(), "Failed to create transport");
        let transport = Arc::new(TokioMutex::new(transport_result.unwrap()));
        
        // 修改为先创建registry，设置transport，然后再包装成Arc
        let mut registry = ActorRegistry::new(local_node_id.clone());
        registry.set_transport(transport.clone());
        let registry = Arc::new(registry);
        
        // Create migration manager
        let mut migration_manager = MigrationManager::new(local_node_id, registry.clone());
        
        // 为MigrationManager设置transport
        migration_manager.set_transport(transport);
        
        // Create target node
        let target_uuid = Uuid::new_v4();
        let target_node = NodeId(target_uuid);
        
        // Register a local actor for testing
        let actor = TestMigratableActor {
            id: Uuid::new_v4(),
            counter: 1,
            name: "test-actor".to_string(),
        };
        
        let actor_path = actor.actor_path();
        let addr = actor.clone().start_distributed();
        
        // 创建结构体来包装Recipient
        struct SimpleActorRef {
            path: String,
        }
        
        impl ActorRef for SimpleActorRef {
            fn send_any(&self, _msg: Box<dyn std::any::Any + Send>) -> ClusterResult<()> {
                // 在测试中我们不需要实际发送消息
                Ok(())
            }
            
            fn path(&self) -> &str {
                &self.path
            }
            
            fn clone_box(&self) -> Box<dyn ActorRef> {
                Box::new(SimpleActorRef {
                    path: self.path.clone(),
                })
            }
        }
        
        // 创建简单的 ActorRef 实现
        let simple_ref = SimpleActorRef {
            path: actor_path.clone(),
        };
        
        // Register the actor with the registry
        registry.register_local(actor_path.clone(), Box::new(simple_ref)).unwrap();
        
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