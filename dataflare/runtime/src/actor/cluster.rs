//! ClusterActor for DataFlare
//! 
//! This actor is responsible for cluster-wide coordination and resource management.
//! It serves as the top-level actor in the new flattened architecture.

use std::collections::HashMap;
use actix::prelude::*;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use std::sync::{Arc, RwLock};

use dataflare_core::error::{DataFlareError, Result};
use dataflare_core::message::WorkflowPhase;

use crate::actor::{
    ActorRegistry, ActorRef, MessageRouter,
    WorkflowActor, SupervisorActor, 
    Initialize, Finalize, GetStatus, ActorStatus
};

/// Node status in the cluster
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeStatus {
    /// Node is active and responding
    Active,
    /// Node is starting up
    Starting,
    /// Node is shutting down
    ShuttingDown,
    /// Node is disconnected
    Disconnected,
}

/// Resource profile for a node
#[derive(Debug, Clone, Default)]
pub struct ResourceProfile {
    /// Available CPU cores
    pub cpu_cores: usize,
    /// Available memory in MB
    pub memory_mb: usize,
    /// Available disk space in MB
    pub disk_mb: usize,
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage percentage
    pub memory_usage: f64,
    /// Disk usage percentage
    pub disk_usage: f64,
}

/// Workflow status
#[derive(Debug, Clone)]
pub enum WorkflowStatus {
    /// Workflow is created but not initialized
    Created,
    /// Workflow is initialized
    Initialized,
    /// Workflow is running
    Running,
    /// Workflow is paused
    Paused,
    /// Workflow is stopped
    Stopped,
    /// Workflow has failed
    Failed(String),
    /// Workflow is completed
    Completed,
}

/// Cluster configuration
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// Cluster ID
    pub id: String,
    /// Cluster name
    pub name: String,
    /// Node ID
    pub node_id: String,
    /// Is this node the leader
    pub is_leader: bool,
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            id: format!("cluster-{}", Uuid::new_v4()),
            name: "dataflare-cluster".to_string(),
            node_id: format!("node-{}", Uuid::new_v4()),
            is_leader: true, // Default to single-node mode
            heartbeat_interval_ms: 5000,
        }
    }
}

/// Cluster node information
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Node ID
    pub id: String,
    /// Node address
    pub address: String,
    /// Node status
    pub status: NodeStatus,
    /// Available resources
    pub resources: ResourceProfile,
    /// Assigned workflows
    pub workflows: Vec<String>,
    /// Last heartbeat
    pub last_heartbeat: DateTime<Utc>,
}

/// Message to register a node
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct RegisterNode {
    /// Node ID
    pub id: String,
    /// Node address
    pub address: String,
    /// Available resources
    pub resources: ResourceProfile,
}

/// Message to unregister a node
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct UnregisterNode {
    /// Node ID
    pub id: String,
}

/// Message to send a heartbeat
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Heartbeat {
    /// Node ID
    pub id: String,
    /// Node status
    pub status: NodeStatus,
    /// Available resources
    pub resources: ResourceProfile,
}

/// Message to deploy a workflow
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct DeployWorkflow {
    /// Workflow ID
    pub id: String,
    /// Workflow configuration
    pub config: serde_json::Value,
    /// Target node ID (optional)
    pub node_id: Option<String>,
}

/// Message to stop a workflow
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct StopWorkflow {
    /// Workflow ID
    pub id: String,
}

/// Message to get workflow status
#[derive(Message)]
#[rtype(result = "Result<WorkflowStatus>")]
pub struct GetWorkflowStatus {
    /// Workflow ID
    pub id: String,
}

/// Message to get all workflows
#[derive(Message)]
#[rtype(result = "Result<HashMap<String, WorkflowStatus>>")]
pub struct GetAllWorkflows;

/// Message to get all nodes
#[derive(Message)]
#[rtype(result = "Result<HashMap<String, NodeInfo>>")]
pub struct GetAllNodes;

/// Top-level actor for cluster coordination
pub struct ClusterActor {
    /// Cluster configuration
    config: ClusterConfig,
    /// Node information
    nodes: HashMap<String, NodeInfo>,
    /// Workflow status
    workflows: HashMap<String, WorkflowStatus>,
    /// Workflow to node mapping
    workflow_nodes: HashMap<String, String>,
    /// Actor registry
    registry: Arc<RwLock<ActorRegistry>>,
    /// Message router
    router: MessageRouter,
    /// Actor status
    status: ActorStatus,
}

impl ClusterActor {
    /// Create a new cluster actor
    pub fn new(config: ClusterConfig) -> Self {
        let registry = Arc::new(RwLock::new(ActorRegistry::new()));
        let router = MessageRouter::new(Arc::clone(&registry));
        
        Self {
            config,
            nodes: HashMap::new(),
            workflows: HashMap::new(),
            workflow_nodes: HashMap::new(),
            registry,
            router,
            status: ActorStatus::Initialized,
        }
    }
    
    /// Register this node
    fn register_self(&mut self) {
        let node_info = NodeInfo {
            id: self.config.node_id.clone(),
            address: "local".to_string(), // Local node
            status: NodeStatus::Active,
            resources: ResourceProfile::default(),
            workflows: Vec::new(),
            last_heartbeat: Utc::now(),
        };
        
        self.nodes.insert(self.config.node_id.clone(), node_info);
        info!("Registered local node: {}", self.config.node_id);
    }
    
    /// Create a workflow actor
    fn create_workflow_actor(&mut self, id: &str, config: &serde_json::Value, ctx: &mut <Self as Actor>::Context) -> Result<()> {
        // Create workflow actor
        let workflow_actor = WorkflowActor::new(id.to_string());
        let addr = workflow_actor.start();
        
        // Register the workflow actor
        let actor_ref = ActorRef::<Initialize>::new(id.to_string(), addr.clone());
        self.registry.write().unwrap().register(actor_ref);
        
        // Send initialize message
        let msg = Initialize {
            workflow_id: id.to_string(),
            config: config.clone(),
        };
        
        let actor_id = id.to_string();
        let fut = addr.send(msg);
        
        ctx.spawn(fut.into_actor(self).map(move |res, actor, _ctx| {
            match res {
                Ok(Ok(_)) => {
                    info!("Workflow {} initialized", actor_id);
                    actor.workflows.insert(actor_id.clone(), WorkflowStatus::Initialized);
                },
                Ok(Err(e)) => {
                    error!("Failed to initialize workflow {}: {}", actor_id, e);
                    actor.workflows.insert(actor_id.clone(), WorkflowStatus::Failed(format!("Initialization failed: {}", e)));
                },
                Err(e) => {
                    error!("Failed to send initialize message to workflow {}: {}", actor_id, e);
                    actor.workflows.insert(actor_id.clone(), WorkflowStatus::Failed(format!("Communication error: {}", e)));
                }
            }
        }));
        
        // Associate workflow with this node
        self.workflow_nodes.insert(id.to_string(), self.config.node_id.clone());
        
        // Add workflow to node's list
        if let Some(node) = self.nodes.get_mut(&self.config.node_id) {
            node.workflows.push(id.to_string());
        }
        
        Ok(())
    }
}

impl Actor for ClusterActor {
    type Context = Context<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        info!("ClusterActor started: {}", self.config.id);
        
        // Register this node
        self.register_self();
        
        // Set status to Running
        self.status = ActorStatus::Running;
        
        // Set up heartbeat interval
        ctx.run_interval(std::time::Duration::from_millis(self.config.heartbeat_interval_ms), |actor, ctx| {
            // Check node heartbeats
            let now = Utc::now();
            let timeout = chrono::Duration::milliseconds((actor.config.heartbeat_interval_ms * 3) as i64);
            
            let timed_out_nodes: Vec<String> = actor.nodes.iter()
                .filter(|(id, node)| {
                    *id != &actor.config.node_id && // Skip self
                    (now - node.last_heartbeat) > timeout
                })
                .map(|(id, _)| id.clone())
                .collect();
            
            for node_id in timed_out_nodes {
                warn!("Node {} timed out", node_id);
                if let Some(node) = actor.nodes.get_mut(&node_id) {
                    node.status = NodeStatus::Disconnected;
                }
                
                // Handle workflows on disconnected node
                if actor.config.is_leader {
                    actor.handle_disconnected_node(&node_id, ctx);
                }
            }
        });
    }
    
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("ClusterActor stopped: {}", self.config.id);
    }
}

impl Handler<RegisterNode> for ClusterActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: RegisterNode, _ctx: &mut Self::Context) -> Self::Result {
        if self.nodes.contains_key(&msg.id) {
            return Err(DataFlareError::Cluster(format!("Node already registered: {}", msg.id)));
        }
        
        let node_info = NodeInfo {
            id: msg.id.clone(),
            address: msg.address,
            status: NodeStatus::Active,
            resources: msg.resources,
            workflows: Vec::new(),
            last_heartbeat: Utc::now(),
        };
        
        self.nodes.insert(msg.id.clone(), node_info);
        info!("Registered node: {}", msg.id);
        
        Ok(())
    }
}

impl Handler<UnregisterNode> for ClusterActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: UnregisterNode, ctx: &mut Self::Context) -> Self::Result {
        if !self.nodes.contains_key(&msg.id) {
            return Err(DataFlareError::Cluster(format!("Node not registered: {}", msg.id)));
        }
        
        // Remove node
        self.nodes.remove(&msg.id);
        info!("Unregistered node: {}", msg.id);
        
        // Handle workflows on unregistered node
        if self.config.is_leader {
            self.handle_disconnected_node(&msg.id, ctx);
        }
        
        Ok(())
    }
}

impl Handler<Heartbeat> for ClusterActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: Heartbeat, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(node) = self.nodes.get_mut(&msg.id) {
            node.status = msg.status;
            node.resources = msg.resources;
            node.last_heartbeat = Utc::now();
            debug!("Received heartbeat from node: {}", msg.id);
        } else {
            return Err(DataFlareError::Cluster(format!("Node not registered: {}", msg.id)));
        }
        
        Ok(())
    }
}

impl Handler<DeployWorkflow> for ClusterActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: DeployWorkflow, ctx: &mut Self::Context) -> Self::Result {
        // Check if workflow already exists
        if self.workflows.contains_key(&msg.id) {
            return Err(DataFlareError::Workflow(format!("Workflow already exists: {}", msg.id)));
        }
        
        // Determine target node
        let target_node = match msg.node_id {
            Some(node_id) => {
                if !self.nodes.contains_key(&node_id) {
                    return Err(DataFlareError::Cluster(format!("Target node not found: {}", node_id)));
                }
                node_id
            },
            None => {
                // Use local node if no target specified
                self.config.node_id.clone()
            }
        };
        
        // If target is this node, create workflow locally
        if target_node == self.config.node_id {
            return self.create_workflow_actor(&msg.id, &msg.config, ctx);
        } else {
            // TODO: Forward request to target node
            // For now, just return an error
            return Err(DataFlareError::NotImplemented("Deploying to remote nodes not yet implemented".to_string()));
        }
    }
}

impl Handler<StopWorkflow> for ClusterActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: StopWorkflow, ctx: &mut Self::Context) -> Self::Result {
        // Check if workflow exists
        if !self.workflows.contains_key(&msg.id) {
            return Err(DataFlareError::Workflow(format!("Workflow not found: {}", msg.id)));
        }
        
        // Get workflow node
        let node_id = match self.workflow_nodes.get(&msg.id) {
            Some(node_id) => node_id.clone(),
            None => return Err(DataFlareError::Workflow(format!("Workflow node mapping not found: {}", msg.id))),
        };
        
        // If workflow is on this node, stop it locally
        if node_id == self.config.node_id {
            // Send finalize message to workflow actor
            let finalize_ref = self.registry.read().unwrap().get::<Finalize>(&msg.id);
            
            if let Some(actor_ref) = finalize_ref {
                let actor_id = msg.id.clone();
                let fut = actor_ref.send(Finalize { workflow_id: msg.id.clone() });
                
                ctx.spawn(fut.into_actor(self).map(move |res, actor, _ctx| {
                    match res {
                        Ok(Ok(_)) => {
                            info!("Workflow {} finalized", actor_id);
                            actor.workflows.insert(actor_id.clone(), WorkflowStatus::Stopped);
                        },
                        Ok(Err(e)) => {
                            error!("Failed to finalize workflow {}: {}", actor_id, e);
                            actor.workflows.insert(actor_id.clone(), WorkflowStatus::Failed(format!("Finalization failed: {}", e)));
                        },
                        Err(e) => {
                            error!("Failed to send finalize message to workflow {}: {}", actor_id, e);
                            actor.workflows.insert(actor_id.clone(), WorkflowStatus::Failed(format!("Communication error: {}", e)));
                        }
                    }
                }));
                
                Ok(())
            } else {
                Err(DataFlareError::Actor(format!("Workflow actor not found in registry: {}", msg.id)))
            }
        } else {
            // TODO: Forward request to target node
            // For now, just return an error
            Err(DataFlareError::NotImplemented("Stopping workflows on remote nodes not yet implemented".to_string()))
        }
    }
}

impl Handler<GetWorkflowStatus> for ClusterActor {
    type Result = Result<WorkflowStatus>;
    
    fn handle(&mut self, msg: GetWorkflowStatus, _ctx: &mut Self::Context) -> Self::Result {
        match self.workflows.get(&msg.id) {
            Some(status) => Ok(status.clone()),
            None => Err(DataFlareError::Workflow(format!("Workflow not found: {}", msg.id))),
        }
    }
}

impl Handler<GetAllWorkflows> for ClusterActor {
    type Result = Result<HashMap<String, WorkflowStatus>>;
    
    fn handle(&mut self, _msg: GetAllWorkflows, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.workflows.clone())
    }
}

impl Handler<GetAllNodes> for ClusterActor {
    type Result = Result<HashMap<String, NodeInfo>>;
    
    fn handle(&mut self, _msg: GetAllNodes, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.nodes.clone())
    }
}

impl Handler<GetStatus> for ClusterActor {
    type Result = Result<ActorStatus>;
    
    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.status.clone())
    }
}

impl ClusterActor {
    // Handle workflows on a disconnected node
    fn handle_disconnected_node(&mut self, node_id: &str, _ctx: &mut <Self as Actor>::Context) {
        // Find workflows on this node
        let workflows: Vec<String> = self.workflow_nodes.iter()
            .filter(|(_, n)| *n == node_id)
            .map(|(w, _)| w.clone())
            .collect();
        
        for workflow_id in workflows {
            warn!("Workflow {} was on disconnected node {}", workflow_id, node_id);
            // TODO: Implement workflow recovery/reassignment
            // For now, just mark as failed
            self.workflows.insert(workflow_id.clone(), WorkflowStatus::Failed(format!("Node disconnected: {}", node_id)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;

    #[actix::test]
    async fn test_cluster_actor_creation() {
        let config = ClusterConfig::default();
        let actor = ClusterActor::new(config.clone());
        let addr = actor.start();
        
        // Check status
        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert!(matches!(status, ActorStatus::Running));
        
        // Check node registration
        let nodes = addr.send(GetAllNodes).await.unwrap().unwrap();
        assert_eq!(nodes.len(), 1);
        assert!(nodes.contains_key(&config.node_id));
        
        // Create a workflow
        let workflow_id = "test-workflow".to_string();
        let config = serde_json::json!({
            "name": "Test Workflow",
            "description": "Test workflow for unit tests",
        });
        
        let result = addr.send(DeployWorkflow {
            id: workflow_id.clone(),
            config,
            node_id: None,
        }).await.unwrap();
        
        assert!(result.is_ok());
        
        // Check workflow status
        let workflows = addr.send(GetAllWorkflows).await.unwrap().unwrap();
        assert!(workflows.contains_key(&workflow_id));
        
        // Get specific workflow status
        let status = addr.send(GetWorkflowStatus { id: workflow_id.clone() }).await.unwrap().unwrap();
        assert!(matches!(status, WorkflowStatus::Initialized));
        
        // Stop workflow
        let result = addr.send(StopWorkflow { id: workflow_id.clone() }).await.unwrap();
        assert!(result.is_ok());
        
        // Check workflow status after stopping
        let status = addr.send(GetWorkflowStatus { id: workflow_id }).await.unwrap().unwrap();
        assert!(matches!(status, WorkflowStatus::Stopped));
    }
} 