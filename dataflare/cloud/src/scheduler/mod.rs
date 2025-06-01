//! Task scheduler module
//!
//! This module provides functionality for scheduling and executing tasks in a distributed environment.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use dataflare_core::error::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::cluster::ClusterManager;
use crate::NodeRole;

/// Task ID type
pub type TaskId = String;

/// Task status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,
    /// Task is running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// Task priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Low priority
    Low = 0,
    /// Normal priority
    Normal = 1,
    /// High priority
    High = 2,
    /// Critical priority
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Task ID
    pub id: TaskId,
    /// Task type
    pub task_type: String,
    /// Task parameters
    pub parameters: serde_json::Value,
    /// Task priority
    pub priority: TaskPriority,
    /// Task status
    pub status: TaskStatus,
    /// Node ID that the task is assigned to
    pub assigned_node: Option<String>,
    /// Task result
    pub result: Option<serde_json::Value>,
    /// Task error message
    pub error: Option<String>,
}

impl Task {
    /// Create a new task
    pub fn new(task_type: &str, parameters: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            task_type: task_type.to_string(),
            parameters,
            priority: TaskPriority::default(),
            status: TaskStatus::Pending,
            assigned_node: None,
            result: None,
            error: None,
        }
    }
    
    /// Set the task priority
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

/// Task scheduler for scheduling and executing tasks
pub struct TaskScheduler {
    /// Cluster manager
    cluster_manager: Arc<ClusterManager>,
    /// Task queue
    task_queue: Arc<RwLock<VecDeque<Task>>>,
    /// Task map
    tasks: Arc<RwLock<HashMap<TaskId, Task>>>,
    /// Scheduler task handle
    scheduler_task: Mutex<Option<JoinHandle<()>>>,
}

impl TaskScheduler {
    /// Create a new task scheduler
    pub fn new(cluster_manager: Arc<ClusterManager>) -> Result<Self> {
        Ok(Self {
            cluster_manager,
            task_queue: Arc::new(RwLock::new(VecDeque::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            scheduler_task: Mutex::new(None),
        })
    }
    
    /// Start the task scheduler
    pub async fn start(&self) -> Result<()> {
        // Start scheduler task
        let task_queue = self.task_queue.clone();
        let tasks = self.tasks.clone();
        let cluster_manager = self.cluster_manager.clone();
        
        let task = tokio::spawn(async move {
            loop {
                // Get available worker nodes
                let worker_nodes = cluster_manager.get_nodes_by_role(NodeRole::Worker);
                
                if !worker_nodes.is_empty() {
                    // Get next task from queue
                    let mut task_opt = None;
                    
                    {
                        let mut queue = task_queue.write().unwrap();
                        if !queue.is_empty() {
                            task_opt = queue.pop_front();
                        }
                    }
                    
                    if let Some(mut task) = task_opt {
                        // Assign task to a worker node
                        let worker_node = &worker_nodes[0]; // Simple round-robin for now
                        
                        task.status = TaskStatus::Running;
                        task.assigned_node = Some(worker_node.id.clone());
                        
                        // Update task in map
                        {
                            let mut tasks_write = tasks.write().unwrap();
                            tasks_write.insert(task.id.clone(), task.clone());
                        }
                        
                        // Send task to worker node (placeholder)
                        log::info!("Assigned task {} to node {}", task.id, worker_node.id);
                    }
                }
                
                // Sleep for a while
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });
        
        // Store task handle
        let mut scheduler_task = self.scheduler_task.lock().await;
        *scheduler_task = Some(task);
        
        Ok(())
    }
    
    /// Stop the task scheduler
    pub async fn stop(&self) -> Result<()> {
        // Stop scheduler task
        let mut scheduler_task = self.scheduler_task.lock().await;
        if let Some(task) = scheduler_task.take() {
            task.abort();
        }
        
        Ok(())
    }
    
    /// Submit a task for execution
    pub fn submit_task(&self, task: Task) -> Result<TaskId> {
        // Add task to map
        {
            let mut tasks = self.tasks.write().unwrap();
            tasks.insert(task.id.clone(), task.clone());
        }
        
        // Add task to queue
        {
            let mut queue = self.task_queue.write().unwrap();
            queue.push_back(task.clone());
        }
        
        Ok(task.id)
    }
    
    /// Get a task by ID
    pub fn get_task(&self, task_id: &str) -> Option<Task> {
        let tasks = self.tasks.read().unwrap();
        tasks.get(task_id).cloned()
    }
    
    /// Cancel a task
    pub fn cancel_task(&self, task_id: &str) -> Result<()> {
        let mut tasks = self.tasks.write().unwrap();
        
        if let Some(mut task) = tasks.get_mut(task_id) {
            if task.status == TaskStatus::Pending || task.status == TaskStatus::Running {
                task.status = TaskStatus::Cancelled;
                
                // Remove from queue if still pending
                if task.status == TaskStatus::Pending {
                    let mut queue = self.task_queue.write().unwrap();
                    queue.retain(|t| t.id != task_id);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get all tasks
    pub fn get_tasks(&self) -> Vec<Task> {
        let tasks = self.tasks.read().unwrap();
        tasks.values().cloned().collect()
    }
    
    /// Get tasks by status
    pub fn get_tasks_by_status(&self, status: TaskStatus) -> Vec<Task> {
        let tasks = self.tasks.read().unwrap();
        tasks
            .values()
            .filter(|task| task.status == status)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CloudRuntimeConfig;
    use crate::DiscoveryMethod;
    
    #[tokio::test]
    async fn test_task_scheduler() {
        let config = CloudRuntimeConfig {
            discovery_method: DiscoveryMethod::Static,
            node_role: NodeRole::Coordinator,
            heartbeat_interval: 1,
            node_timeout: 5,
        };
        
        let cluster_manager = Arc::new(ClusterManager::new(config).unwrap());
        let scheduler = TaskScheduler::new(cluster_manager.clone()).unwrap();
        
        scheduler.start().await.unwrap();
        
        // Create a task
        let task = Task::new("test_task", serde_json::json!({
            "param1": "value1",
            "param2": 42
        })).with_priority(TaskPriority::High);
        
        // Submit the task
        let task_id = scheduler.submit_task(task.clone()).unwrap();
        
        // Get the task
        let retrieved_task = scheduler.get_task(&task_id).unwrap();
        assert_eq!(retrieved_task.id, task_id);
        assert_eq!(retrieved_task.task_type, "test_task");
        assert_eq!(retrieved_task.priority, TaskPriority::High);
        
        // Cancel the task
        scheduler.cancel_task(&task_id).unwrap();
        
        // Check that the task was cancelled
        let cancelled_task = scheduler.get_task(&task_id).unwrap();
        assert_eq!(cancelled_task.status, TaskStatus::Cancelled);
        
        scheduler.stop().await.unwrap();
    }
}
