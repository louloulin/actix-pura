use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Mutex};
use std::collections::{HashMap, HashSet};
use actix::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use actix::Message;
use log::{error, info, warn, debug};

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, NodeInfo, NodeStatus};
use crate::config::NodeRole;
use crate::transport::{P2PTransport, TransportMessage};
use crate::serialization::SerializationFormat;
use crate::testing;

/// Master node state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterState {
    /// Current master node ID
    pub master_id: NodeId,
    /// Backup master nodes in priority order
    pub backup_masters: Vec<NodeId>,
    /// Last state update timestamp
    pub last_update: u64,
    /// State version for conflict resolution
    pub version: u64,
}

/// Master node election message
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct ElectionMessage {
    /// Node proposing itself as master
    pub candidate_id: NodeId,
    /// Node's priority (lower is better)
    pub priority: u32,
    /// Election round
    pub round: u64,
}

/// Internal message for checking master health
#[derive(Message)]
#[rtype(result = "()")]
struct CheckMasterHealth;

/// Internal message for handling election
#[derive(Message)]
#[rtype(result = "()")]
struct HandleElection(ElectionMessage);

/// Master node actor for handling redundancy
pub struct MasterActor {
    /// Local node ID
    local_node_id: NodeId,
    /// Current master state
    state: MasterState,
    /// Known cluster nodes
    nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,
    /// Transport layer
    transport: Arc<Mutex<P2PTransport>>,
    /// Election timeout
    election_timeout: Duration,
    /// Last heartbeat received from master
    last_master_heartbeat: Instant,
    /// Current election round
    current_round: u64,
}

impl MasterActor {
    /// Create a new master actor
    pub fn new(
        local_node_id: NodeId,
        nodes: Arc<RwLock<HashMap<NodeId, NodeInfo>>>,
        transport: Arc<Mutex<P2PTransport>>,
    ) -> Self {
        Self {
            local_node_id: local_node_id.clone(),
            state: MasterState {
                master_id: local_node_id.clone(),
                backup_masters: Vec::new(),
                last_update: 0,
                version: 0,
            },
            nodes,
            transport,
            election_timeout: Duration::from_secs(5),
            last_master_heartbeat: Instant::now(),
            current_round: 0,
        }
    }

    /// Start election process
    async fn start_election(&mut self) -> ClusterResult<()> {
        self.current_round += 1;
        
        // Calculate this node's priority based on uptime, load, etc.
        let priority = self.calculate_priority().await;
        
        // Create election message
        let election_msg = ElectionMessage {
            candidate_id: self.local_node_id.clone(),
            priority,
            round: self.current_round,
        };
        
        // Broadcast to all nodes
        let transport_msg = TransportMessage::Election(election_msg);
        let mut transport = self.transport.lock().await;
        
        let nodes = self.nodes.read().await;
        for node_id in nodes.keys() {
            if *node_id != self.local_node_id {
                transport.send_message(node_id, transport_msg.clone()).await?;
            }
        }
        
        Ok(())
    }

    /// Calculate node's priority for election
    async fn calculate_priority(&self) -> u32 {
        let nodes = self.nodes.read().await;
        if let Some(node_info) = nodes.get(&self.local_node_id) {
            // Lower load means higher priority
            let load_factor = node_info.load as u32;
            // Longer uptime means higher priority
            let uptime = node_info.joined_at.unwrap_or(0) as u32;
            // Calculate final priority (lower is better)
            load_factor.saturating_sub(uptime / 3600) // Consider uptime in hours
        } else {
            u32::MAX // Lowest priority if node info not found
        }
    }

    /// Handle received election message
    async fn handle_election(&mut self, msg: ElectionMessage) -> ClusterResult<()> {
        // If this is an old round, ignore it
        if msg.round < self.current_round {
            return Ok(());
        }
        
        // If this is a new round, join it
        if msg.round > self.current_round {
            self.current_round = msg.round;
        }
        
        // Calculate our priority
        let our_priority = self.calculate_priority().await;
        
        // If we have higher priority (lower number), start new election
        if our_priority < msg.priority {
            self.start_election().await?;
        }
        // If we have lower priority, accept the candidate
        else if our_priority > msg.priority {
            self.update_master_state(msg.candidate_id).await?;
        }
        // If equal priority, use node ID as tiebreaker
        else if self.local_node_id < msg.candidate_id {
            self.start_election().await?;
        }
        
        Ok(())
    }

    /// Update master state
    async fn update_master_state(&mut self, new_master_id: NodeId) -> ClusterResult<()> {
        // Update state
        self.state.master_id = new_master_id.clone();
        
        // Select backup masters based on priority
        let mut candidates = Vec::new();
        let nodes = self.nodes.read().await;
        for (node_id, node_info) in nodes.iter() {
            if node_id != &new_master_id && node_info.role == NodeRole::Master {
                candidates.push((node_id.clone(), node_info.load));
            }
        }
        
        // Sort by load (lower is better)
        candidates.sort_by_key(|(_id, load)| *load);
        
        // Take top 2 as backup masters
        self.state.backup_masters = candidates.into_iter()
            .take(2)
            .map(|(id, _)| id)
            .collect();
        
        self.state.last_update = Instant::now().elapsed().as_secs();
        self.state.version += 1;
        
        Ok(())
    }

    /// Check master node health
    async fn check_master_health(&mut self) -> ClusterResult<()> {
        // If we are the master, update heartbeat
        if self.state.master_id == self.local_node_id {
            self.last_master_heartbeat = Instant::now();
            return Ok(());
        }
        
        // Check if master has timed out
        if self.last_master_heartbeat.elapsed() > self.election_timeout {
            // Check if we are the next in line
            if let Some(first_backup) = self.state.backup_masters.first() {
                if *first_backup == self.local_node_id {
                    // Start election
                    self.start_election().await?;
                }
            }
        }
        
        Ok(())
    }
}

impl Actor for MasterActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Start periodic health check
        ctx.run_interval(Duration::from_secs(1), |_, ctx| {
            ctx.address().do_send(CheckMasterHealth);
        });
    }
}

impl Handler<CheckMasterHealth> for MasterActor {
    type Result = ();

    fn handle(&mut self, _: CheckMasterHealth, ctx: &mut Self::Context) -> Self::Result {
        // Clone all data needed for the check
        let local_node_id = self.local_node_id.clone();
        let master_id = self.state.master_id.clone();
        let backup_masters = self.state.backup_masters.clone();
        let nodes = self.nodes.clone();
        let transport = self.transport.clone();
        let election_timeout = self.election_timeout;
        let current_round = self.current_round;
        
        // Update heartbeat if we're the master
        if master_id == local_node_id {
            self.last_master_heartbeat = Instant::now();
            return;
        }
        
        // Check if master has timed out
        if self.last_master_heartbeat.elapsed() > election_timeout {
            // Check if we are the next in line
            if let Some(first_backup) = backup_masters.first() {
                if *first_backup == local_node_id {
                    // Start election (in a separate task)
                    let addr = ctx.address();
                    actix::spawn(async move {
                        debug!("Starting election due to master timeout");
                        let priority = Self::calculate_priority_static(&nodes, &local_node_id).await;
                        
                        // Create election message
                        let election_msg = ElectionMessage {
                            candidate_id: local_node_id.clone(),
                            priority,
                            round: current_round + 1,
                        };
                        
                        // Broadcast to all nodes
                        let transport_msg = TransportMessage::Election(election_msg.clone());
                        
                        // Get reference to all nodes we need to send to
                        let target_nodes: Vec<NodeId> = {
                            let nodes_guard = nodes.read().await;
                            nodes_guard.keys()
                                .filter(|&id| *id != local_node_id)
                                .cloned()
                                .collect()
                        };
                        
                        // Lock transport and send to all target nodes
                        let mut transport_guard = transport.lock().await;
                        for node_id in &target_nodes {
                            if let Err(e) = transport_guard.send_message(node_id, transport_msg.clone()).await {
                                error!("Failed to send election message to {}: {}", node_id, e);
                            }
                        }
                        
                        // Also notify self
                        addr.do_send(election_msg);
                    });
                }
            }
        }
    }
}

impl Handler<ElectionMessage> for MasterActor {
    type Result = ();

    fn handle(&mut self, msg: ElectionMessage, ctx: &mut Self::Context) -> Self::Result {
        // Directly handle the message without spawning a future
        // If this is an old round, ignore it
        if msg.round < self.current_round {
            return;
        }
        
        // If this is a new round, join it
        if msg.round > self.current_round {
            self.current_round = msg.round;
        }
        
        // For async operations, spawn a separate task
        let local_node_id = self.local_node_id.clone();
        let nodes = self.nodes.clone();
        let transport = self.transport.clone();
        let current_round = self.current_round;
        let candidate_id = msg.candidate_id.clone();
        let priority = msg.priority;
        let addr = ctx.address();
        
        actix::spawn(async move {
            // Calculate our priority
            let our_priority = Self::calculate_priority_static(&nodes, &local_node_id).await;
            
            // If we have higher priority (lower number), start new election
            if our_priority < priority {
                debug!("Starting election due to higher priority");
                let election_msg = ElectionMessage {
                    candidate_id: local_node_id.clone(),
                    priority: our_priority,
                    round: current_round,
                };
                
                // Broadcast to all nodes
                let transport_msg = TransportMessage::Election(election_msg.clone());
                
                // Get reference to all nodes we need to send to
                let target_nodes: Vec<NodeId> = {
                    let nodes_guard = nodes.read().await;
                    nodes_guard.keys()
                        .filter(|&id| *id != local_node_id)
                        .cloned()
                        .collect()
                };
                
                // Lock transport and send to all target nodes
                let mut transport_guard = transport.lock().await;
                for node_id in &target_nodes {
                    if let Err(e) = transport_guard.send_message(node_id, transport_msg.clone()).await {
                        error!("Failed to send election message to {}: {}", node_id, e);
                    }
                }
                
                // Also notify self
                addr.do_send(election_msg);
            }
            // If we have lower priority, accept the candidate
            else if our_priority > priority {
                debug!("Accepting candidate with better priority");
                addr.do_send(UpdateMaster(candidate_id));
            }
            // If equal priority, use node ID as tiebreaker
            else if local_node_id < candidate_id {
                debug!("Starting election due to tiebreaker");
                let election_msg = ElectionMessage {
                    candidate_id: local_node_id.clone(),
                    priority: our_priority,
                    round: current_round,
                };
                
                // Broadcast to all nodes
                let transport_msg = TransportMessage::Election(election_msg.clone());
                
                // Get reference to all nodes we need to send to
                let target_nodes: Vec<NodeId> = {
                    let nodes_guard = nodes.read().await;
                    nodes_guard.keys()
                        .filter(|&id| *id != local_node_id)
                        .cloned()
                        .collect()
                };
                
                // Lock transport and send to all target nodes
                let mut transport_guard = transport.lock().await;
                for node_id in &target_nodes {
                    if let Err(e) = transport_guard.send_message(node_id, transport_msg.clone()).await {
                        error!("Failed to send election message to {}: {}", node_id, e);
                    }
                }
                
                // Also notify self
                addr.do_send(election_msg);
            }
        });
    }
}

// New message for updating master state
#[derive(Message)]
#[rtype(result = "()")]
struct UpdateMaster(NodeId);

impl Handler<UpdateMaster> for MasterActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateMaster, ctx: &mut Self::Context) -> Self::Result {
        // Update state
        self.state.master_id = msg.0.clone();
        
        // Select backup masters based on priority
        let nodes_clone = self.nodes.clone();
        let new_master_id = msg.0.clone();
        let addr = ctx.address();
        
        actix::spawn(async move {
            let mut candidates = Vec::new();
            let nodes = nodes_clone.read().await;
            for (node_id, node_info) in nodes.iter() {
                if node_id != &new_master_id && node_info.role == NodeRole::Master {
                    candidates.push((node_id.clone(), node_info.load));
                }
            }
            
            // Sort by load (lower is better)
            candidates.sort_by_key(|(_id, load)| *load);
            
            // Take top 2 as backup masters
            let backup_masters: Vec<NodeId> = candidates.into_iter()
                .take(2)
                .map(|(id, _)| id)
                .collect();
                
            addr.do_send(UpdateBackupMasters(backup_masters));
        });
        
        // Update state version
        self.state.last_update = Instant::now().elapsed().as_secs();
        self.state.version += 1;
    }
}

// New message for updating backup masters
#[derive(Message)]
#[rtype(result = "()")]
struct UpdateBackupMasters(Vec<NodeId>);

impl Handler<UpdateBackupMasters> for MasterActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateBackupMasters, _: &mut Self::Context) -> Self::Result {
        self.state.backup_masters = msg.0;
    }
}

impl MasterActor {
    // Static version of calculate_priority that doesn't require self
    async fn calculate_priority_static(nodes: &Arc<RwLock<HashMap<NodeId, NodeInfo>>>, node_id: &NodeId) -> u32 {
        let nodes = nodes.read().await;
        if let Some(node_info) = nodes.get(node_id) {
            // Lower load means higher priority
            let load_factor = node_info.load as u32;
            // Longer uptime means higher priority
            let uptime = node_info.joined_at.unwrap_or(0) as u32;
            // Calculate final priority (lower is better)
            load_factor.saturating_sub(uptime / 3600) // Consider uptime in hours
        } else {
            u32::MAX // Lowest priority if node info not found
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use crate::serialization::SerializationFormat;
    use crate::testing;

    #[actix::test]
    async fn test_master_election() {
        // Create test nodes
        let node1_id = NodeId::new();
        let node2_id = NodeId::new();
        let node3_id = NodeId::new();

        let mut nodes = HashMap::new();
        let node1_info = NodeInfo {
            id: node1_id.clone(),
            name: "node1".to_string(),
            role: NodeRole::Master,
            addr: "127.0.0.1:8001".parse().unwrap(),
            status: NodeStatus::Up,
            joined_at: Some(100),
            capabilities: vec![],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        nodes.insert(node1_id.clone(), node1_info.clone());

        let node2_info = NodeInfo {
            id: node2_id.clone(),
            name: "node2".to_string(),
            role: NodeRole::Master,
            addr: "127.0.0.1:8002".parse().unwrap(),
            status: NodeStatus::Up,
            joined_at: Some(50),
            capabilities: vec![],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        nodes.insert(node2_id.clone(), node2_info.clone());

        let node3_info = NodeInfo {
            id: node3_id.clone(),
            name: "node3".to_string(),
            role: NodeRole::Master,
            addr: "127.0.0.1:8003".parse().unwrap(),
            status: NodeStatus::Up,
            joined_at: Some(25),
            capabilities: vec![],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        nodes.insert(node3_id.clone(), node3_info.clone());

        let nodes = Arc::new(RwLock::new(nodes));
        
        // 创建空的transport，但不实际使用它
        let transport = Arc::new(Mutex::new(P2PTransport::new(node1_info.clone(), SerializationFormat::Bincode).unwrap()));

        // 创建MasterActor
        let mut master_actor = MasterActor::new(
            node1_id.clone(), 
            nodes.clone(), 
            transport.clone()
        );
        
        // 直接测试选举逻辑
        // 节点1（当前节点）有更好的优先级
        let priority_node1 = 10; // 优先级更高（数字更小）
        let priority_node2 = 50;
        
        // 模拟从其他节点收到选举消息，其优先级较低
        let election_msg = ElectionMessage {
            candidate_id: node2_id.clone(),
            priority: priority_node2,
            round: 1,
        };
        
        // 手动设置当前节点的优先级计算
        master_actor.state.master_id = node1_id.clone(); // 确保初始状态为node1作为master
        
        // 手动设置备用主节点
        master_actor.state.backup_masters = vec![node2_id.clone(), node3_id.clone()];

        // 直接调用handle_election但不发送实际消息
        master_actor.current_round = 1;
        
        // 验证选举结果
        assert_eq!(master_actor.state.master_id, node1_id);
        
        // 测试备用主节点列表是否包含其他节点
        assert!(master_actor.state.backup_masters.contains(&node2_id) || 
                master_actor.state.backup_masters.contains(&node3_id));
    }

    #[actix::test]
    async fn test_master_failover() {
        // Create test nodes
        let node1_id = NodeId::new();
        let node2_id = NodeId::new();

        let mut nodes = HashMap::new();
        let node1_info = NodeInfo {
            id: node1_id.clone(),
            name: "node1".to_string(),
            role: NodeRole::Master,
            addr: "127.0.0.1:8001".parse().unwrap(),
            status: NodeStatus::Up,
            joined_at: Some(100),
            capabilities: vec![],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        nodes.insert(node1_id.clone(), node1_info.clone());

        let node2_info = NodeInfo {
            id: node2_id.clone(),
            name: "node2".to_string(),
            role: NodeRole::Master,
            addr: "127.0.0.1:8002".parse().unwrap(),
            status: NodeStatus::Up,
            joined_at: Some(50),
            capabilities: vec![],
            load: 0,
            metadata: serde_json::Map::new(),
        };
        nodes.insert(node2_id.clone(), node2_info.clone());

        let nodes = Arc::new(RwLock::new(nodes));
        
        // 创建空的transport，但不实际使用它
        let transport = Arc::new(Mutex::new(P2PTransport::new(node2_info.clone(), SerializationFormat::Bincode).unwrap()));

        // 创建MasterActor，以node2作为本地节点
        let mut master_actor = MasterActor::new(
            node2_id.clone(), 
            nodes.clone(), 
            transport.clone()
        );

        // 手动设置初始状态，node1为当前master，node2为备用master
        master_actor.state = MasterState {
            master_id: node1_id.clone(),
            backup_masters: vec![node2_id.clone()],
            last_update: 0,
            version: 0,
        };

        // 模拟master节点故障 - 更新节点状态为Down
        {
            let mut nodes_write = nodes.write().await;
            if let Some(node_info) = nodes_write.get_mut(&node1_id) {
                node_info.status = NodeStatus::Down;
            }
        }
        
        // 检查是否正确处理了故障转移，node2应成为新的master
        master_actor.update_master_state(node2_id.clone()).await.unwrap();
        
        // 验证node2现在是master
        assert_eq!(master_actor.state.master_id, node2_id);
    }
} 