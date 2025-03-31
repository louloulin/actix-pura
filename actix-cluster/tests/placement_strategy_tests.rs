use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use actix_cluster::error::{ClusterError, ClusterResult};
use actix_cluster::node::{NodeId, NodeInfo, PlacementStrategy};
use actix_cluster::placement::{NodeSelector, PlacementStrategyImpl};
use actix_cluster::config::NodeRole;

// Mock node selector for testing placement strategies
struct MockNodeSelector {
    nodes: HashMap<NodeId, NodeInfo>,
    active_nodes: Vec<NodeId>,
}

impl MockNodeSelector {
    // Create a new selector with configurable node count and load pattern
    fn new(count: usize, load_pattern: &[u8]) -> Self {
        let mut nodes = HashMap::new();
        let mut active_nodes = Vec::new();
        
        // Create the specified number of mock nodes
        for i in 0..count {
            let id = NodeId::new();
            let mut info = NodeInfo::new(
                id.clone(),
                format!("node-{}", i),
                NodeRole::Peer,
                format!("127.0.0.1:{}00", 80 + i).parse().unwrap(),
            );
            
            // Set the load based on the pattern
            let load = if i < load_pattern.len() { load_pattern[i] } else { 50 };
            info.set_load(load);
            
            nodes.insert(id.clone(), info);
            active_nodes.push(id);
        }
        
        Self { nodes, active_nodes }
    }
    
    // Create a standard set of nodes with different loads
    fn standard() -> Self {
        Self::new(5, &[10, 30, 50, 70, 90]) // 5 nodes with loads from 10% to 90%
    }
    
    // Create an empty selector with no nodes
    fn empty() -> Self {
        Self { nodes: HashMap::new(), active_nodes: Vec::new() }
    }
}

impl NodeSelector for MockNodeSelector {
    fn select_node(&self, actor_path: &str, strategy: &PlacementStrategy) -> ClusterResult<NodeId> {
        // We create a new placement to properly test its implementation,
        // not to test the routing logic of MockNodeSelector itself
        let placement = PlacementStrategyImpl::new(Arc::new(self.clone()));
        placement.select_node(actor_path, strategy)
    }
    
    fn get_active_nodes(&self) -> Vec<NodeId> {
        self.active_nodes.clone()
    }
    
    fn get_node_info(&self, node_id: &NodeId) -> Option<NodeInfo> {
        self.nodes.get(node_id).cloned()
    }
}

impl Clone for MockNodeSelector {
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            active_nodes: self.active_nodes.clone(),
        }
    }
}

#[test]
fn test_random_placement_strategy() {
    // Create a selector with 10 nodes
    let selector = Arc::new(MockNodeSelector::new(10, &[50; 10]));
    let placement = PlacementStrategyImpl::new(selector.clone());
    
    // Get 100 random placements
    let mut selected_nodes = HashMap::new();
    for i in 0..100 {
        let actor_path = format!("test_actor_{}", i);
        let node_id = placement.select_node(&actor_path, &PlacementStrategy::Random).unwrap();
        *selected_nodes.entry(node_id).or_insert(0) += 1;
    }
    
    // Each node should have been selected roughly 10 times (statistically)
    // but we'll be more lenient to avoid test flakiness
    assert_eq!(selected_nodes.len(), 10); // All nodes should be used
    
    // None should be selected too many times (e.g., >30)
    // or too few times (e.g., <3)
    for count in selected_nodes.values() {
        assert!(*count > 0); // At least selected once
        assert!(*count < 40); // Not selected too many times
    }
}

#[test]
fn test_round_robin_placement_strategy() {
    // Create a selector with 5 nodes
    let selector = Arc::new(MockNodeSelector::standard());
    let placement = PlacementStrategyImpl::new(selector.clone());
    
    // Get first 5 round-robin placements
    let mut nodes = Vec::new();
    for i in 0..5 {
        let actor_path = format!("test_actor_{}", i);
        let node_id = placement.select_node(&actor_path, &PlacementStrategy::RoundRobin).unwrap();
        nodes.push(node_id);
    }
    
    // All 5 selections should be different nodes
    let unique_nodes: std::collections::HashSet<_> = nodes.iter().cloned().collect();
    assert_eq!(unique_nodes.len(), 5);
    
    // Next 5 selections should match the first 5 in order
    for i in 0..5 {
        let actor_path = format!("test_actor_{}", i + 5);
        let node_id = placement.select_node(&actor_path, &PlacementStrategy::RoundRobin).unwrap();
        assert_eq!(node_id, nodes[i]);
    }
}

#[test]
fn test_least_loaded_placement_strategy() {
    // Create a selector with 5 nodes and specific loads
    let loads = [10, 30, 50, 70, 90]; // 10%, 30%, 50%, 70%, 90% load
    let selector = Arc::new(MockNodeSelector::new(5, &loads));
    let placement = PlacementStrategyImpl::new(selector.clone());
    
    // The least loaded node should consistently be selected
    for i in 0..10 {
        let actor_path = format!("test_actor_{}", i);
        let node_id = placement.select_node(&actor_path, &PlacementStrategy::LeastLoaded).unwrap();
        
        // This should be the node with 10% load
        let selected_load = selector.nodes.get(&node_id).unwrap().load;
        assert_eq!(selected_load, 10);
    }
    
    // Create a selector where all nodes have the same load
    let selector = Arc::new(MockNodeSelector::new(5, &[50, 50, 50, 50, 50]));
    let placement = PlacementStrategyImpl::new(selector.clone());
    
    // With equal loads, the first node should be selected
    let node_id = placement.select_node("test_actor", &PlacementStrategy::LeastLoaded).unwrap();
    assert_eq!(node_id, selector.active_nodes[0]);
}

#[test]
fn test_specific_node_placement_strategy() {
    // Create a selector with 5 nodes
    let selector = Arc::new(MockNodeSelector::standard());
    let placement = PlacementStrategyImpl::new(selector.clone());
    
    // Get the ID of the third node
    let target_node_id = selector.active_nodes[2].clone();
    let strategy = PlacementStrategy::Node(target_node_id.as_uuid().clone());
    
    // Should select the specific node
    let selected_node = placement.select_node("test_actor", &strategy).unwrap();
    assert_eq!(selected_node, target_node_id);
    
    // Try with a non-existent node ID
    let missing_node_id = NodeId::new();
    let strategy = PlacementStrategy::Node(missing_node_id.as_uuid().clone());
    
    // Should return an error
    match placement.select_node("test_actor", &strategy) {
        Err(ClusterError::NodeNotFound(_)) => {}, // Expected
        other => panic!("Expected NodeNotFound error, got {:?}", other),
    }
}

#[test]
fn test_redundant_placement_strategy() {
    // Create a selector with 5 nodes
    let selector = Arc::new(MockNodeSelector::standard());
    let placement = PlacementStrategyImpl::new(selector.clone());
    
    // The same actor path should consistently map to the same node
    // for the same redundancy level
    let actor_path = "test_actor_consistent";
    let strategy = PlacementStrategy::Redundant { replicas: 1 };
    
    // For the redundant test, we'll check that the same actor path consistently
    // maps to the same node for each lookup
    let first_node = placement.select_node(actor_path, &strategy).unwrap();
    
    // Multiple lookups should return the same node
    for _ in 0..10 {
        let node = placement.select_node(actor_path, &strategy).unwrap();
        assert_eq!(node, first_node);
    }
    
    // Different actor paths should map to different nodes (probably)
    let mut selected_nodes = std::collections::HashSet::new();
    for i in 0..100 {
        let path = format!("different_actor_{}", i);
        let node = placement.select_node(&path, &strategy).unwrap();
        selected_nodes.insert(node);
    }
    
    // Should have used most of the 5 nodes (statistically likely)
    assert!(selected_nodes.len() >= 3);
}

#[test]
fn test_empty_node_list() {
    // Create a selector with no nodes
    let selector = Arc::new(MockNodeSelector::empty());
    let placement = PlacementStrategyImpl::new(selector);
    
    // All strategies should return NoNodesAvailable
    let strategies = vec![
        PlacementStrategy::Random,
        PlacementStrategy::RoundRobin,
        PlacementStrategy::LeastLoaded,
        PlacementStrategy::Redundant { replicas: 1 },
    ];
    
    for strategy in strategies {
        match placement.select_node("test_actor", &strategy) {
            Err(ClusterError::NoNodesAvailable) => {}, // Expected
            other => panic!("Expected NoNodesAvailable error, got {:?}", other),
        }
    }
} 