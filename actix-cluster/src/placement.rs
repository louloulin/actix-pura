// Placement strategies for actor distribution

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::Rng;
use uuid::Uuid;
use std::net::SocketAddr;

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, NodeInfo, PlacementStrategy};
use crate::config::NodeRole;

// NodeSelector trait for selecting nodes based on different strategies
pub trait NodeSelector: Send + Sync {
    // Select a node based on the provided strategy
    fn select_node(&self, actor_path: &str, strategy: &PlacementStrategy) -> ClusterResult<NodeId>;
    
    // Get all active nodes
    fn get_active_nodes(&self) -> Vec<NodeId>;
    
    // Get information about a specific node
    fn get_node_info(&self, node_id: &NodeId) -> Option<NodeInfo>;
}

/// Implementation of placement strategies
pub struct PlacementStrategyImpl {
    // Node selector for finding available nodes
    selector: Arc<dyn NodeSelector>,
    // Track round-robin placement
    round_robin_index: Mutex<usize>,
    // Consistent hashing state
    consistent_hash_nodes: Mutex<Vec<(NodeId, u64)>>,
    // Store redundant nodes for each actor path
    redundant_nodes: Mutex<HashMap<String, Vec<NodeId>>>,
}

impl PlacementStrategyImpl {
    /// Create a new placement strategy
    pub fn new(selector: Arc<dyn NodeSelector>) -> Self {
        Self {
            selector,
            round_robin_index: Mutex::new(0),
            consistent_hash_nodes: Mutex::new(Vec::new()),
            redundant_nodes: Mutex::new(HashMap::new()),
        }
    }
    
    /// Select a node using the requested strategy
    pub fn select_node(&self, actor_path: &str, strategy: &PlacementStrategy) -> ClusterResult<NodeId> {
        match strategy {
            PlacementStrategy::Random => self.select_random_node(),
            PlacementStrategy::RoundRobin => self.select_round_robin_node(),
            PlacementStrategy::LeastLoaded => self.select_least_loaded_node(),
            PlacementStrategy::Node(node_id) => self.select_specific_node(&NodeId(*node_id)),
            PlacementStrategy::Redundant { replicas } => {
                // Check if we have any nodes available
                let available_nodes = self.selector.get_active_nodes();
                if available_nodes.is_empty() {
                    return Err(ClusterError::NoNodesAvailable);
                }
                
                // Check if we have enough nodes for the requested replicas
                if *replicas > available_nodes.len() {
                    return Err(ClusterError::InvalidOperation(
                        format!("Requested {} replicas but only {} nodes available", 
                            replicas, available_nodes.len())
                    ));
                }
                
                // Get or compute redundant nodes
                let nodes = {
                    let mut redundant_nodes = self.redundant_nodes.lock().unwrap();
                    if let Some(nodes) = redundant_nodes.get(actor_path) {
                        nodes.clone()
                    } else {
                        let nodes = self.select_redundant_nodes(actor_path, *replicas)?;
                        redundant_nodes.insert(actor_path.to_string(), nodes.clone());
                        nodes
                    }
                };
                
                // Return the first node and store others for redundancy
                Ok(nodes[0].clone())
            },
            PlacementStrategy::LocalAffinity { fallback, group } => {
                self.select_with_local_affinity(actor_path, group, fallback)
            }
        }
    }
    
    /// Select a specific node
    fn select_specific_node(&self, node_id: &NodeId) -> ClusterResult<NodeId> {
        // Check if the node exists
        if self.selector.get_node_info(node_id).is_some() {
            Ok(node_id.clone())
        } else {
            Err(ClusterError::NodeNotFound(node_id.clone()))
        }
    }
    
    /// Select a random node
    fn select_random_node(&self) -> ClusterResult<NodeId> {
        let available_nodes = self.selector.get_active_nodes();
        
        if available_nodes.is_empty() {
            return Err(ClusterError::NoNodesAvailable);
        }
        
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..available_nodes.len());
        
        Ok(available_nodes[index].clone())
    }
    
    /// Select a node using round-robin
    fn select_round_robin_node(&self) -> ClusterResult<NodeId> {
        let available_nodes = self.selector.get_active_nodes();
        
        if available_nodes.is_empty() {
            return Err(ClusterError::NoNodesAvailable);
        }
        
        let mut index = self.round_robin_index.lock().unwrap();
        let node_id = available_nodes[*index % available_nodes.len()].clone();
        
        // Update index for next selection
        *index = (*index + 1) % available_nodes.len();
        
        Ok(node_id)
    }
    
    /// Select the least loaded node
    fn select_least_loaded_node(&self) -> ClusterResult<NodeId> {
        let available_nodes = self.selector.get_active_nodes();
        
        if available_nodes.is_empty() {
            return Err(ClusterError::NoNodesAvailable);
        }
        
        let mut least_loaded_node_id = available_nodes[0].clone();
        let mut least_load = u8::MAX;
        
        for node_id in available_nodes {
            if let Some(node_info) = self.selector.get_node_info(&node_id) {
                if node_info.load < least_load {
                    least_load = node_info.load;
                    least_loaded_node_id = node_id;
                }
            }
        }
        
        Ok(least_loaded_node_id)
    }
    
    /// Select a node using consistent hashing
    fn select_consistent_hash_node(&self, actor_path: &str) -> ClusterResult<NodeId> {
        let available_nodes = self.selector.get_active_nodes();
        
        if available_nodes.is_empty() {
            return Err(ClusterError::NoNodesAvailable);
        }
        
        // Generate hash for the actor path
        let hash = self.hash_string(actor_path);
        
        // Find the node with the closest hash
        let mut closest_node_id = available_nodes[0].clone();
        let mut closest_distance = u64::MAX;
        
        for node_id in available_nodes {
            let node_hash = self.hash_string(&node_id.to_string());
            let distance = if node_hash > hash {
                node_hash - hash
            } else {
                hash - node_hash
            };
            
            if distance < closest_distance {
                closest_distance = distance;
                closest_node_id = node_id;
            }
        }
        
        Ok(closest_node_id)
    }
    
    /// Select a node based on locality
    fn select_locality_node(&self, actor_path: &str) -> ClusterResult<NodeId> {
        // Simple locality based on prefix matching
        // In a real system, this would use more sophisticated metrics
        let available_nodes = self.selector.get_active_nodes();
        
        if available_nodes.is_empty() {
            return Err(ClusterError::NoNodesAvailable);
        }
        
        // Try to find a node with a matching prefix
        for node_id in &available_nodes {
            if let Some(node_info) = self.selector.get_node_info(node_id) {
                if actor_path.starts_with(&node_info.name) {
                    return Ok(node_id.clone());
                }
            }
        }
        
        // Fall back to consistent hashing if no locality match
        self.select_consistent_hash_node(actor_path)
    }
    
    /// Hash a string to a u64
    fn hash_string(&self, s: &str) -> u64 {
        let mut hash: u64 = 0;
        
        for b in s.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u64);
        }
        
        hash
    }

    /// Select multiple nodes for redundancy using consistent hashing
    fn select_redundant_nodes(&self, actor_path: &str, replicas: usize) -> ClusterResult<Vec<NodeId>> {
        let available_nodes = self.selector.get_active_nodes();
        
        if available_nodes.is_empty() {
            return Err(ClusterError::NoNodesAvailable);
        }
        
        if replicas > available_nodes.len() {
            return Err(ClusterError::InvalidOperation(
                format!("Requested {} replicas but only {} nodes available", 
                    replicas, available_nodes.len())
            ));
        }
        
        let mut selected_nodes = Vec::with_capacity(replicas);
        let mut used_indices = std::collections::HashSet::new();
        
        // Generate hash for the actor path
        let base_hash = self.hash_string(actor_path);
        
        // Select nodes using different hash variations
        for i in 0..replicas {
            let hash = base_hash.wrapping_add(i as u64);
            
            // Find the node with the closest hash that hasn't been used
            let mut closest_distance = u64::MAX;
            let mut selected_index = 0;
            let mut found_unused_node = false;
            
            for (index, node_id) in available_nodes.iter().enumerate() {
                if used_indices.contains(&index) {
                    continue;
                }
                
                found_unused_node = true;
                let node_hash = self.hash_string(&node_id.to_string());
                let distance = if node_hash > hash {
                    node_hash - hash
                } else {
                    hash - node_hash
                };
                
                if distance < closest_distance {
                    closest_distance = distance;
                    selected_index = index;
                }
            }
            
            if !found_unused_node {
                return Err(ClusterError::InvalidOperation(
                    format!("Not enough unique nodes available for {} replicas", replicas)
                ));
            }
            
            used_indices.insert(selected_index);
            selected_nodes.push(available_nodes[selected_index].clone());
        }
        
        Ok(selected_nodes)
    }

    /// Select a node with local affinity
    fn select_with_local_affinity(
        &self, 
        actor_path: &str, 
        group: &Option<String>, 
        fallback: &PlacementStrategy
    ) -> ClusterResult<NodeId> {
        let available_nodes = self.selector.get_active_nodes();
        
        if available_nodes.is_empty() {
            return Err(ClusterError::NoNodesAvailable);
        }
        
        // 首先尝试找到本地节点
        let local_node_id = NodeId::local();
        if available_nodes.contains(&local_node_id) {
            return Ok(local_node_id);
        }
        
        // 如果指定了亲和性分组，尝试找到同一组内的actor所在的节点
        if let Some(group_id) = group {
            // 根据组ID扫描注册表，查找已注册的同组actor
            // 这里需要依赖注册表的实现，暂时使用一个简单的基于路径前缀的方法
            
            // 假设路径格式为 /user/group_id/actor_name
            let group_prefix = format!("/user/{}/", group_id);
            
            for node_id in &available_nodes {
                if let Some(node_info) = self.selector.get_node_info(node_id) {
                    // 此处简化实现，实际系统需要查询注册表
                    if node_info.capabilities.iter().any(|cap| cap.starts_with(&group_prefix)) {
                        return Ok(node_id.clone());
                    }
                }
            }
        }
        
        // 如果无法找到符合亲和性要求的节点，使用回退策略
        self.select_node(actor_path, fallback)
    }
}

/// Mock implementation of NodeSelector for testing
#[cfg(test)]
pub struct MockNodeSelector {
    nodes: Vec<(NodeId, NodeInfo)>,
}

#[cfg(test)]
impl MockNodeSelector {
    /// Create a new mock selector
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }
    
    /// Add a node to the selector
    pub fn add_node(&mut self, node_id: NodeId, node_info: NodeInfo) {
        self.nodes.push((node_id, node_info));
    }
    
    /// Create a new mock selector with default nodes
    pub fn with_default_nodes() -> Self {
        let mut selector = Self::new();
        
        // Create 5 mock nodes
        for i in 0..5 {
            let id = NodeId(Uuid::new_v4());
            let info = NodeInfo::new(
                id.clone(),
                format!("node-{}", i),
                NodeRole::Peer,
                format!("127.0.0.1:{}00", 80 + i).parse().unwrap(),
            );
            
            selector.add_node(id, info);
        }
        
        selector
    }
}

#[cfg(test)]
impl NodeSelector for MockNodeSelector {
    fn select_node(&self, actor_path: &str, strategy: &PlacementStrategy) -> ClusterResult<NodeId> {
        let placement = PlacementStrategyImpl::new(Arc::new(Self::new()));
        placement.select_node(actor_path, strategy)
    }
    
    fn get_active_nodes(&self) -> Vec<NodeId> {
        self.nodes.iter().map(|(id, _)| id.clone()).collect()
    }
    
    fn get_node_info(&self, node_id: &NodeId) -> Option<NodeInfo> {
        self.nodes.iter()
            .find(|(id, _)| id == node_id)
            .map(|(_, info)| info.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{NodeStatus, NodeId, NodeInfo};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use crate::config::NodeRole;

    #[test]
    fn test_local_affinity_strategy() {
        let mut selector = MockNodeSelector::new();
        
        // 创建本地节点
        let local_node_id = NodeId::local();
        let local_node_info = NodeInfo {
            id: local_node_id.clone(),
            name: "local-node".to_string(),
            role: NodeRole::Peer,
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8000),
            status: NodeStatus::Up,
            joined_at: Some(1000),
            capabilities: vec!["/user/group1/actor1".to_string()],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        
        // 创建远程节点
        let remote_node_id = NodeId::new();
        let remote_node_info = NodeInfo {
            id: remote_node_id.clone(),
            name: "remote-node".to_string(),
            role: NodeRole::Peer,
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8000),
            status: NodeStatus::Up,
            joined_at: Some(1000),
            capabilities: vec!["/user/group2/actor1".to_string()],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        
        // 创建带有特定组的远程节点
        let group1_node_id = NodeId::new();
        let group1_node_info = NodeInfo {
            id: group1_node_id.clone(),
            name: "group1-node".to_string(),
            role: NodeRole::Peer,
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 8000),
            status: NodeStatus::Up,
            joined_at: Some(1000),
            capabilities: vec!["/user/group1/actor2".to_string()],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        
        // 添加节点到选择器
        selector.add_node(local_node_id.clone(), local_node_info);
        selector.add_node(remote_node_id.clone(), remote_node_info);
        selector.add_node(group1_node_id.clone(), group1_node_info);
        
        let strategy_impl = PlacementStrategyImpl::new(Arc::new(selector));
        
        // 测试场景1：使用本地亲和性策略，应该选择本地节点
        let local_affinity_strategy = PlacementStrategy::LocalAffinity {
            fallback: Box::new(PlacementStrategy::Random),
            group: None,
        };
        
        let selected_node = strategy_impl.select_node("/user/test-actor", &local_affinity_strategy).unwrap();
        assert_eq!(selected_node, local_node_id);
        
        // 测试场景2：使用带分组的本地亲和性策略，应该选择相同组的节点
        let group_affinity_strategy = PlacementStrategy::LocalAffinity {
            fallback: Box::new(PlacementStrategy::Random),
            group: Some("group1".to_string()),
        };
        
        // 创建一个不包含本地节点的选择器
        let mut group_selector = MockNodeSelector::new();
        
        // 重新创建节点信息以避免引用已移动的值
        let remote_node_info2 = NodeInfo {
            id: remote_node_id.clone(),
            name: "remote-node".to_string(),
            role: NodeRole::Peer,
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8000),
            status: NodeStatus::Up,
            joined_at: Some(1000),
            capabilities: vec!["/user/group2/actor1".to_string()],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        
        let group1_node_info2 = NodeInfo {
            id: group1_node_id.clone(),
            name: "group1-node".to_string(),
            role: NodeRole::Peer,
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 8000),
            status: NodeStatus::Up,
            joined_at: Some(1000),
            capabilities: vec!["/user/group1/actor2".to_string()],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        
        group_selector.add_node(remote_node_id.clone(), remote_node_info2);
        group_selector.add_node(group1_node_id.clone(), group1_node_info2);
        
        let group_strategy_impl = PlacementStrategyImpl::new(Arc::new(group_selector));
        let selected_node = group_strategy_impl.select_node("/user/test-actor", &group_affinity_strategy).unwrap();
        assert_eq!(selected_node, group1_node_id);
        
        // 测试场景3：使用带分组的本地亲和性策略，但没有匹配的组，应该使用回退策略
        let no_match_strategy = PlacementStrategy::LocalAffinity {
            fallback: Box::new(PlacementStrategy::Node(*remote_node_id.as_uuid())),
            group: Some("group3".to_string()),
        };
        
        let selected_node = group_strategy_impl.select_node("/user/test-actor", &no_match_strategy).unwrap();
        assert_eq!(selected_node, remote_node_id);
    }
} 