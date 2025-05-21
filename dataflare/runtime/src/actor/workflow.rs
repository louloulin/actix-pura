//! Workflow Actor for DataFlare
//!
//! Implements an actor responsible for coordinating the execution of workflows
//! in the new flattened architecture.

use std::collections::HashMap;
use actix::prelude::*;
use log::{debug, error, info, warn};
use chrono::Utc;
use uuid::Uuid;
use std::time::{Duration, Instant};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecordBatch, WorkflowPhase, WorkflowProgress},
    config::{WorkflowConfig, ComponentConfig},
};

use crate::actor::{
    ActorRef, ActorRegistry, MessageRouter,
    TaskActor, TaskKind, ProcessBatch, GetTaskState, TaskState, TaskStats, GetTaskStats,
    Initialize, Finalize, Pause, Resume, GetStatus, ActorStatus, SendBatch,
    SubscribeProgress, UnsubscribeProgress,
};

/// Workflow stage execution status
#[derive(Debug, Clone)]
pub enum StageStatus {
    /// Stage is initialized but not running
    Initialized,
    /// Stage is running
    Running,
    /// Stage is completed
    Completed,
    /// Stage has failed
    Failed(String),
}

/// Workflow execution stage
#[derive(Debug, Clone)]
pub struct ExecutionStage {
    /// Stage ID
    pub id: String,
    /// Stage name
    pub name: String,
    /// Tasks in this stage
    pub tasks: Vec<String>,
    /// Stage status
    pub status: StageStatus,
    /// Current progress (0.0-1.0)
    pub progress: f64,
}

/// Flow statistics
#[derive(Debug, Clone)]
pub struct WorkflowStats {
    /// Start time
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    /// End time
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Records processed
    pub records_processed: usize,
    /// Bytes processed
    pub bytes_processed: usize,
    /// Records per second
    pub records_per_second: f64,
    /// Current execution stage
    pub current_stage: Option<String>,
    /// Execution stages
    pub stages: HashMap<String, ExecutionStage>,
    /// Error count
    pub error_count: usize,
    /// Last error
    pub last_error: Option<String>,
}

/// Message to start a workflow
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct StartWorkflow;

/// Message to stop a workflow
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct StopWorkflow;

/// Message to get workflow statistics
#[derive(Message)]
#[rtype(result = "Result<WorkflowStats>")]
pub struct GetWorkflowStats;

/// Message to check workflow status
#[derive(Message)]
#[rtype(result = "Result<ActorStatus>")]
pub struct CheckWorkflow;

/// Actor that manages workflow execution
pub struct WorkflowActor {
    /// ID of the workflow
    id: String,
    
    /// Workflow name
    name: String,
    
    /// Status
    status: ActorStatus,
    
    /// Workflow configuration
    config: Option<WorkflowConfig>,
    
    /// Task actors by ID
    tasks: HashMap<String, Addr<TaskActor>>,
    
    /// Task kinds
    task_kinds: HashMap<String, TaskKind>,
    
    /// Actor registry for accessing other actors
    registry: Option<Arc<ActorRegistry>>,
    
    /// Message router for direct messaging
    router: Option<MessageRouter>,
    
    /// Progress subscribers
    subscribers: HashMap<Uuid, Recipient<WorkflowProgress>>,
    
    /// Workflow execution statistics
    stats: WorkflowStats,
    
    /// Execution start time
    start_time: Option<Instant>,
    
    /// DAG relationships (task_id -> downstream task_ids)
    dag: HashMap<String, Vec<String>>,
}

/// Message to add a downstream task
#[derive(Message)]
#[rtype(result = "()")]
pub struct AddDownstream {
    /// Task ID to add as downstream
    pub task_id: String,
}

// Message handler for adding downstream tasks to a TaskActor
impl Handler<AddDownstream> for TaskActor {
    type Result = ();
    
    fn handle(&mut self, msg: AddDownstream, _ctx: &mut Self::Context) -> Self::Result {
        // In a real implementation, this would store the task ID and set up connections
        debug!("Adding downstream task {} to task {}", msg.task_id, self.get_id());
    }
}

impl WorkflowActor {
    /// Create a new workflow actor
    pub fn new(id: String) -> Self {
        Self {
            id: id.clone(),
            name: id,
            status: ActorStatus::Initialized,
            config: None,
            tasks: HashMap::new(),
            task_kinds: HashMap::new(),
            registry: None,
            router: None,
            subscribers: HashMap::new(),
            stats: WorkflowStats {
                start_time: None,
                end_time: None,
                records_processed: 0,
                bytes_processed: 0,
                records_per_second: 0.0,
                current_stage: None,
                stages: HashMap::new(),
                error_count: 0,
                last_error: None,
            },
            start_time: None,
            dag: HashMap::new(),
        }
    }
    
    /// Set the actor registry
    pub fn set_registry(&mut self, registry: Arc<ActorRegistry>) {
        self.registry = Some(registry.clone());
        self.router = Some(MessageRouter::new(registry));
    }
    
    /// Initialize the workflow from configuration
    fn init_from_config(&mut self, config: WorkflowConfig, ctx: &mut <Self as Actor>::Context) -> Result<()> {
        info!("Initializing workflow {} from configuration", self.id);
        
        // Store the configuration
        self.name = config.name.clone();
        
        // Create source tasks
        for (id, source_config) in &config.sources {
            let task_id = format!("{}.source.{}", self.id, id);
            let task = TaskActor::new(&format!("Source: {}", id), TaskKind::Source);
            let addr = task.start();
            
            self.tasks.insert(task_id.clone(), addr.clone());
            self.task_kinds.insert(task_id.clone(), TaskKind::Source);
            self.dag.insert(task_id.clone(), vec![]);
            
            // Initialize source task
            let task_config = source_config.config.clone();
            let fut = addr.send(Initialize {
                workflow_id: self.id.clone(),
                config: task_config,
            });
            
            let task_id_clone = task_id.clone();
            ctx.wait(fut.into_actor(self).map(move |res, actor, _ctx| {
                match res {
                    Ok(Ok(_)) => {
                        info!("Successfully initialized source task {}", task_id_clone);
                    },
                    Ok(Err(e)) => {
                        error!("Failed to initialize source task {}: {}", task_id_clone, e);
                        actor.stats.error_count += 1;
                        actor.stats.last_error = Some(format!("Source initialization error: {}", e));
                    },
                    Err(e) => {
                        error!("Failed to send initialization message to source task {}: {}", task_id_clone, e);
                        actor.stats.error_count += 1;
                        actor.stats.last_error = Some(format!("Source communication error: {}", e));
                    }
                }
            }));
        }
        
        // Create processor tasks
        for (id, proc_config) in &config.transformations {
            let task_id = format!("{}.processor.{}", self.id, id);
            let task = TaskActor::new(&format!("Processor: {}", id), TaskKind::Processor);
            let addr = task.start();
            
            self.tasks.insert(task_id.clone(), addr.clone());
            self.task_kinds.insert(task_id.clone(), TaskKind::Processor);
            self.dag.insert(task_id.clone(), vec![]);
            
            // Initialize processor task
            let task_config = proc_config.config.clone();
            let fut = addr.send(Initialize {
                workflow_id: self.id.clone(),
                config: task_config,
            });
            
            let task_id_clone = task_id.clone();
            ctx.wait(fut.into_actor(self).map(move |res, actor, _ctx| {
                match res {
                    Ok(Ok(_)) => {
                        info!("Successfully initialized processor task {}", task_id_clone);
                    },
                    Ok(Err(e)) => {
                        error!("Failed to initialize processor task {}: {}", task_id_clone, e);
                        actor.stats.error_count += 1;
                        actor.stats.last_error = Some(format!("Processor initialization error: {}", e));
                    },
                    Err(e) => {
                        error!("Failed to send initialization message to processor task {}: {}", task_id_clone, e);
                        actor.stats.error_count += 1;
                        actor.stats.last_error = Some(format!("Processor communication error: {}", e));
                    }
                }
            }));
        }
        
        // Create destination tasks
        for (id, dest_config) in &config.destinations {
            let task_id = format!("{}.destination.{}", self.id, id);
            let task = TaskActor::new(&format!("Destination: {}", id), TaskKind::Destination);
            let addr = task.start();
            
            self.tasks.insert(task_id.clone(), addr.clone());
            self.task_kinds.insert(task_id.clone(), TaskKind::Destination);
            self.dag.insert(task_id.clone(), vec![]);
            
            // Initialize destination task
            let task_config = dest_config.config.clone();
            let fut = addr.send(Initialize {
                workflow_id: self.id.clone(),
                config: task_config,
            });
            
            let task_id_clone = task_id.clone();
            ctx.wait(fut.into_actor(self).map(move |res, actor, _ctx| {
                match res {
                    Ok(Ok(_)) => {
                        info!("Successfully initialized destination task {}", task_id_clone);
                    },
                    Ok(Err(e)) => {
                        error!("Failed to initialize destination task {}: {}", task_id_clone, e);
                        actor.stats.error_count += 1;
                        actor.stats.last_error = Some(format!("Destination initialization error: {}", e));
                    },
                    Err(e) => {
                        error!("Failed to send initialization message to destination task {}: {}", task_id_clone, e);
                        actor.stats.error_count += 1;
                        actor.stats.last_error = Some(format!("Destination communication error: {}", e));
                    }
                }
            }));
        }
        
        // Build DAG relationships from inputs
        for (id, proc_config) in &config.transformations {
            let proc_id = format!("{}.processor.{}", self.id, id);
            
            for input in &proc_config.inputs {
                // Determine if input is a source or another processor
                let source_id = format!("{}.source.{}", self.id, input);
                let upstream_proc_id = format!("{}.processor.{}", self.id, input);
                
                if self.tasks.contains_key(&source_id) {
                    // Add this processor as downstream of the source
                    if let Some(downstream) = self.dag.get_mut(&source_id) {
                        downstream.push(proc_id.clone());
                    }
                    
                    // Connect the actors for direct message passing
                    if let Some(source_addr) = self.tasks.get(&source_id) {
                        if let Some(proc_addr) = self.tasks.get(&proc_id) {
                            // Add processor as downstream of source
                            let mut source_actor = source_addr.clone();
                            source_actor.do_send(AddDownstream { task_id: proc_id.clone() });
                        }
                    }
                } else if self.tasks.contains_key(&upstream_proc_id) {
                    // Add this processor as downstream of another processor
                    if let Some(downstream) = self.dag.get_mut(&upstream_proc_id) {
                        downstream.push(proc_id.clone());
                    }
                    
                    // Connect the actors for direct message passing
                    if let Some(upstream_addr) = self.tasks.get(&upstream_proc_id) {
                        if let Some(proc_addr) = self.tasks.get(&proc_id) {
                            // Add processor as downstream of upstream processor
                            let mut upstream_actor = upstream_addr.clone();
                            upstream_actor.do_send(AddDownstream { task_id: proc_id.clone() });
                        }
                    }
                } else {
                    warn!("Input '{}' for processor '{}' not found", input, id);
                }
            }
        }
        
        // Connect processors to destinations
        for (id, dest_config) in &config.destinations {
            let dest_id = format!("{}.destination.{}", self.id, id);
            
            for input in &dest_config.inputs {
                // Determine if input is a source or processor
                let source_id = format!("{}.source.{}", self.id, input);
                let proc_id = format!("{}.processor.{}", self.id, input);
                
                if self.tasks.contains_key(&proc_id) {
                    // Add this destination as downstream of the processor
                    if let Some(downstream) = self.dag.get_mut(&proc_id) {
                        downstream.push(dest_id.clone());
                    }
                    
                    // Connect the actors for direct message passing
                    if let Some(proc_addr) = self.tasks.get(&proc_id) {
                        if let Some(dest_addr) = self.tasks.get(&dest_id) {
                            // Add destination as downstream of processor
                            let mut proc_actor = proc_addr.clone();
                            proc_actor.do_send(AddDownstream { task_id: dest_id.clone() });
                        }
                    }
                } else if self.tasks.contains_key(&source_id) {
                    // Add this destination as downstream of the source (direct path)
                    if let Some(downstream) = self.dag.get_mut(&source_id) {
                        downstream.push(dest_id.clone());
                    }
                    
                    // Connect the actors for direct message passing
                    if let Some(source_addr) = self.tasks.get(&source_id) {
                        if let Some(dest_addr) = self.tasks.get(&dest_id) {
                            // Add destination as downstream of source
                            let mut source_actor = source_addr.clone();
                            source_actor.do_send(AddDownstream { task_id: dest_id.clone() });
                        }
                    }
                } else {
                    warn!("Input '{}' for destination '{}' not found", input, id);
                }
            }
        }
        
        self.config = Some(config);
        self.status = ActorStatus::Initialized;
        
        Ok(())
    }
    
    /// Broadcast a progress update to all subscribers
    fn broadcast_progress(&self, phase: WorkflowPhase, progress: f64, message: &str) {
        let progress_msg = WorkflowProgress {
            workflow_id: self.id.clone(),
            task_id: String::from("workflow"),
            task_name: self.name.clone(),
            phase,
            progress,
            message: message.to_string(),
            timestamp: chrono::Utc::now(),
            records_processed: self.stats.records_processed,
        };
        
        for (_, recipient) in &self.subscribers {
            let _ = recipient.do_send(progress_msg.clone());
        }
    }
    
    /// Update workflow statistics based on task statistics
    async fn update_stats(&mut self) -> Result<()> {
        let mut total_records = 0;
        let mut total_bytes = 0;
        let mut tasks_with_errors = 0;
        
        for (id, addr) in &self.tasks {
            if let Ok(stats) = addr.send(GetTaskStats).await {
                if let Ok(task_stats) = stats {
                    total_records += task_stats.records_processed;
                    total_bytes += task_stats.bytes_processed;
                    
                    if task_stats.errors > 0 {
                        tasks_with_errors += 1;
                        if self.stats.last_error.is_none() {
                            self.stats.last_error = task_stats.last_error.clone();
                        }
                    }
                }
            }
        }
        
        self.stats.records_processed = total_records;
        self.stats.bytes_processed = total_bytes;
        self.stats.error_count = tasks_with_errors;
        
        // Calculate records per second
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                self.stats.records_per_second = total_records as f64 / elapsed;
            }
        }
        
        Ok(())
    }
}

impl Actor for WorkflowActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("WorkflowActor {} started", self.id);
    }
    
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("WorkflowActor {} stopped", self.id);
        
        // Update end time in statistics
        self.stats.end_time = Some(chrono::Utc::now());
    }
}

impl Handler<Initialize> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: Initialize, ctx: &mut Self::Context) -> Self::Result {
        info!("Initializing workflow {}", msg.workflow_id);
        
        // Parse configuration
        let config: WorkflowConfig = match serde_json::from_value(msg.config.clone()) {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("Failed to parse workflow configuration: {}", e);
                return Err(DataFlareError::Config(format!("Invalid workflow configuration: {}", e)));
            }
        };
        
        self.init_from_config(config, ctx)
    }
}

impl Handler<Finalize> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, _msg: Finalize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Finalizing workflow {}", self.id);
        
        // Finalize all task actors
        for (id, addr) in &self.tasks {
            let _ = addr.do_send(Finalize {
                workflow_id: self.id.clone(),
            });
        }
        
        self.status = ActorStatus::Stopped;
        Ok(())
    }
}

impl Handler<StartWorkflow> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, _msg: StartWorkflow, ctx: &mut Self::Context) -> Self::Result {
        info!("Starting workflow {}", self.id);
        
        // Use pattern matching for status comparison
        match self.status {
            ActorStatus::Running => return Ok(()),
            ActorStatus::Initialized => {
                // Set workflow to running
                self.status = ActorStatus::Running;
                self.stats.start_time = Some(chrono::Utc::now());
                self.start_time = Some(Instant::now());
                
                // Start source tasks
                for (id, addr) in &self.tasks {
                    if let Some(&TaskKind::Source) = self.task_kinds.get(id) {
                        info!("Starting source task {}", id);
                        let _ = addr.do_send(Resume {
                            workflow_id: self.id.clone(),
                        });
                    }
                }
                
                // Schedule status update
                ctx.run_interval(Duration::from_secs(5), |actor, ctx| {
                    // Update stats
                    let fut = actor.update_stats();
                    ctx.spawn(fut.into_actor(actor).map(|_, _, _| ()));
                    
                    // Broadcast progress
                    actor.broadcast_progress(WorkflowPhase::Running, 0.5, "Workflow running");
                });
                
                self.broadcast_progress(WorkflowPhase::Starting, 0.0, "Workflow started");
                
                Ok(())
            },
            _ => Err(DataFlareError::Workflow(format!("Workflow {} is not in initialized state", self.id))),
        }
    }
}

impl Handler<StopWorkflow> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, _msg: StopWorkflow, _ctx: &mut Self::Context) -> Self::Result {
        info!("Stopping workflow {}", self.id);
        
        // Use pattern matching for status comparison
        match self.status {
            ActorStatus::Running => {
                // Pause all tasks
                for (id, addr) in &self.tasks {
                    let _ = addr.do_send(Pause {
                        workflow_id: self.id.clone(),
                    });
                }
                
                self.status = ActorStatus::Stopped;
                self.broadcast_progress(WorkflowPhase::Completed, 1.0, "Workflow stopped");
                
                Ok(())
            },
            _ => Ok(()),
        }
    }
}

impl Handler<GetStatus> for WorkflowActor {
    type Result = Result<ActorStatus>;
    
    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.status.clone())
    }
}

impl Handler<GetWorkflowStats> for WorkflowActor {
    type Result = Result<WorkflowStats>;
    
    fn handle(&mut self, _msg: GetWorkflowStats, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.stats.clone())
    }
}

impl Handler<SubscribeProgress> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: SubscribeProgress, _ctx: &mut Self::Context) -> Self::Result {
        let id = Uuid::new_v4();
        self.subscribers.insert(id, msg.recipient);
        
        // Forward subscription to all task actors
        for (_id, addr) in &self.tasks {
            let _ = addr.do_send(msg.clone());
        }
        
        Ok(())
    }
}

impl Handler<UnsubscribeProgress> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: UnsubscribeProgress, _ctx: &mut Self::Context) -> Self::Result {
        // Remove by value since we don't have the UUID
        self.subscribers.retain(|_, r| r != &msg.recipient);
        
        // Forward unsubscription to all task actors
        for (_id, addr) in &self.tasks {
            let _ = addr.do_send(msg.clone());
        }
        
        Ok(())
    }
}

impl Handler<CheckWorkflow> for WorkflowActor {
    type Result = Result<ActorStatus>;
    
    fn handle(&mut self, _msg: CheckWorkflow, ctx: &mut Self::Context) -> Self::Result {
        // Convenience method to check if workflow is finished
        // by checking all task statuses
        
        // Update stats before checking
        let fut = self.update_stats();
        ctx.spawn(fut.into_actor(self).map(|_, _, _| ()));
        
        Ok(self.status.clone())
    }
}

// For TaskActor to reference the workflow actor
impl Handler<ProcessBatch> for WorkflowActor {
    type Result = ResponseFuture<Result<DataRecordBatch>>;
    
    fn handle(&mut self, msg: ProcessBatch, _ctx: &mut Self::Context) -> Self::Result {
        // Workflows don't process batches directly but route them
        // This helps in testing and monitoring
        info!("Workflow {} received batch with {} records", 
              self.id, msg.batch.records().len());
        
        // Just return the batch as-is
        Box::pin(async move {
            Ok(msg.batch)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix::test]
    async fn test_workflow_actor_initialization() {
        let workflow_actor = WorkflowActor::new("test-workflow");
        let addr = workflow_actor.start();

        let result = addr.send(Initialize {
            workflow_id: "test-workflow".to_string(),
            config: serde_json::json!({}),
        }).await.unwrap();

        assert!(result.is_ok());

        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert_eq!(status, ActorStatus::Initialized);
    }
}
