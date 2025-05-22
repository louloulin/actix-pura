//! Workflow Actor for DataFlare
//!
//! Implements an actor responsible for coordinating the execution of workflows
//! in the new flattened architecture.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use actix::prelude::*;
use log::{debug, error, info, warn};
use uuid::Uuid;
use std::time::{Duration, Instant};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecordBatch, WorkflowPhase, WorkflowProgress, StartExtraction},
};

// Import the correct workflow config from crate module
use crate::workflow::Workflow;

use crate::actor::{
    ProcessBatch, ActorRegistry, MessageRouter,
    TaskActor, TaskKind, GetTaskStats,
    Initialize, Finalize, Pause, Resume, GetStatus, ActorStatus,
    SubscribeProgress, UnsubscribeProgress,
    TaskCompleted, RegisterTask, RegisterSourceActor, SourceActor, ConnectToTask, RegisterDestinationActor,
    DestinationActor
};

use crate::actor::task::ErrorStrategy;

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
    /// Execution time in milliseconds
    pub execution_time_ms: Option<u64>,
}

impl Default for WorkflowStats {
    fn default() -> Self {
        Self {
            start_time: None,
            end_time: None,
            records_processed: 0,
            bytes_processed: 0,
            records_per_second: 0.0,
            current_stage: None,
            stages: HashMap::new(),
            error_count: 0,
            last_error: None,
            execution_time_ms: None,
        }
    }
}

/// Message to start a workflow
#[derive(Message, Clone)]
#[rtype(result = "Result<()>")]
pub struct StartWorkflow;

/// Message to stop a workflow
#[derive(Message, Clone)]
#[rtype(result = "Result<()>")]
pub struct StopWorkflow;

/// Message to get workflow statistics
#[derive(Message, Clone)]
#[rtype(result = "Result<WorkflowStats>")]
pub struct GetWorkflowStats;

/// Message to check workflow status
#[derive(Message, Clone)]
#[rtype(result = "Result<ActorStatus>")]
pub struct CheckWorkflow;

/// Message to add a downstream task
#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct AddDownstream {
    /// Task ID to add as downstream
    pub task_id: String,
}

/// Message to update workflow stats
#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct UpdateWorkflowStats {
    pub workflow_id: String,
}

/// Message to set workflow failure strategy
#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct SetFailureStrategy {
    /// Workflow ID
    pub workflow_id: String,
    /// Failure strategy to set
    pub strategy: FailureStrategy,
}

/// Message to execute a workflow
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct ExecuteWorkflow {
    /// Workflow ID
    pub workflow_id: String,
    /// Optional execution parameters
    pub parameters: Option<serde_json::Value>,
}

/// Message to notify workflow of task failure
#[derive(Message)]
#[rtype(result = "()")]
pub struct TaskFailed {
    /// Workflow ID
    pub workflow_id: String,
    /// Task ID
    pub task_id: String,
    /// Error message
    pub error_message: String,
    /// Whether to retry the task
    pub should_retry: bool,
}

/// Failure strategy for workflows
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureStrategy {
    /// Continue on task failure
    Continue,
    /// Abort workflow on task failure
    Abort,
    /// Retry failed tasks
    Retry(usize), // Max retries
}

impl Default for FailureStrategy {
    fn default() -> Self {
        Self::Continue
    }
}

/// Actor that manages workflow execution
pub struct WorkflowActor {
    /// ID of the workflow
    id: String,

    /// Workflow name
    name: String,

    /// Status
    status: ActorStatus,

    /// Workflow configuration
    config: Option<Workflow>,

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

    /// Completed tasks
    completed_tasks: HashSet<String>,

    /// Failed tasks
    failed_tasks: HashSet<String>,

    /// Failure strategy
    failure_strategy: FailureStrategy,

    /// Source actors
    source_actors: HashMap<String, Addr<SourceActor>>,

    /// Destination actors
    destination_actors: HashMap<String, Addr<DestinationActor>>,

    /// Task retry counts
    task_retry_counts: HashMap<String, usize>,

    /// Maximum number of task failures allowed before workflow fails
    max_task_failures: usize,

    /// Default error strategy for tasks
    default_error_strategy: ErrorStrategy,

    /// Recovery in progress flag
    recovery_in_progress: bool,

    /// Tasks in recovery
    tasks_in_recovery: HashSet<String>,
}

// Message handler for adding downstream tasks to a TaskActor
impl Handler<AddDownstream> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: AddDownstream, _ctx: &mut Self::Context) -> Self::Result {
        // In a real implementation, this would store the task ID and set up connections
        debug!("Adding downstream task: {}", msg.task_id);
    }
}

impl WorkflowActor {
    /// Create a new workflow actor
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            status: ActorStatus::Initialized,
            config: None,
            tasks: HashMap::new(),
            task_kinds: HashMap::new(),
            registry: None,
            router: None,
            subscribers: HashMap::new(),
            stats: WorkflowStats::default(),
            start_time: None,
            dag: HashMap::new(),
            completed_tasks: HashSet::new(),
            failed_tasks: HashSet::new(),
            failure_strategy: FailureStrategy::default(),
            source_actors: HashMap::new(),
            destination_actors: HashMap::new(),
            task_retry_counts: HashMap::new(),
            max_task_failures: 3, // 默认最多允许3个任务失败
            default_error_strategy: ErrorStrategy::Continue, // 默认继续执行
            recovery_in_progress: false,
            tasks_in_recovery: HashSet::new(),
        }
    }

    /// Set the actor registry
    pub fn set_registry(&mut self, registry: Arc<ActorRegistry>) {
        self.registry = Some(registry.clone());
        self.router = Some(MessageRouter::new(registry));
    }

    /// Initialize the workflow from configuration
    fn init_from_config(&mut self, config: Workflow, ctx: &mut <Self as Actor>::Context) -> Result<()> {
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
            let task_config = serde_json::json!({
                "id": task_id.clone(),
                "name": format!("Source: {}", id),
                "task_type": "source",
                "config": source_config.config.clone()
            });
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
            let task_config = serde_json::json!({
                "id": task_id.clone(),
                "name": format!("Processor: {}", id),
                "task_type": "processor",
                "config": proc_config.config.clone()
            });
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
            let task_config = serde_json::json!({
                "id": task_id.clone(),
                "name": format!("Destination: {}", id),
                "task_type": "destination",
                "config": dest_config.config.clone()
            });
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
                        if let Some(_proc_addr) = self.tasks.get(&proc_id) {
                            // Add processor as downstream of source
                            let source_actor = source_addr.clone();
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
                        if let Some(_proc_addr) = self.tasks.get(&proc_id) {
                            // Add processor as downstream of upstream processor
                            let upstream_actor = upstream_addr.clone();
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
                        if let Some(_dest_addr) = self.tasks.get(&dest_id) {
                            // Add destination as downstream of processor
                            let proc_actor = proc_addr.clone();
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
                        if let Some(_dest_addr) = self.tasks.get(&dest_id) {
                            // Add destination as downstream of source
                            let source_actor = source_addr.clone();
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
        // Create a progress message using the proper fields from WorkflowProgress in dataflare_core::message
            let progress_msg = WorkflowProgress {
            workflow_id: self.id.clone(),
                phase,
            progress, // Keep as f64, don't convert to f32
                message: message.to_string(),
            timestamp: chrono::Utc::now(),
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

    /// Add a source actor to the workflow
    pub fn add_source_actor<A: Actor>(&mut self, id: String, addr: Addr<A>) {
        // Convert address to string identifier and store it
        let addr_str = format!("{:?}", addr);
        debug!("Adding source actor: {} with addr: {}", id, addr_str);

        // The actual interaction will happen through the actor system message passing
        self.task_kinds.insert(id.clone(), TaskKind::Source);
        self.dag.insert(id, vec![]);
    }

    /// Add a processor actor to the workflow
    pub fn add_processor_actor<A: Actor>(&mut self, id: String, addr: Addr<A>) {
        // Convert address to string identifier and store it
        let addr_str = format!("{:?}", addr);
        debug!("Adding processor actor: {} with addr: {}", id, addr_str);

        // The actual interaction will happen through the actor system message passing
        self.task_kinds.insert(id.clone(), TaskKind::Processor);
        self.dag.insert(id, vec![]);
    }

    /// Add a destination actor to the workflow
    pub fn add_destination_actor<A: Actor>(&mut self, id: String, addr: Addr<A>) {
        // Convert address to string identifier and store it
        let addr_str = format!("{:?}", addr);
        debug!("Adding destination actor: {} with addr: {}", id, addr_str);

        // The actual interaction will happen through the actor system message passing
        self.task_kinds.insert(id.clone(), TaskKind::Destination);
        self.dag.insert(id, vec![]);
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
        let config: Workflow = match serde_json::from_value(msg.config.clone()) {
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

                // Reset the completed and failed tasks for a new run
                self.completed_tasks.clear();
                self.failed_tasks.clear();

                // First, resume all tasks
                for (id, addr) in &self.tasks {
                    info!("Resuming task {}", id);
                        let _ = addr.do_send(Resume {
                            workflow_id: self.id.clone(),
                        });
                    }

                // Connect SourceActors to their corresponding TaskActors
                for (source_id, source_addr) in &self.source_actors {
                    // Find a matching source task (with matching ID or prefix)
                    for (task_id, task_addr) in &self.tasks {
                        if task_id.contains(source_id) && self.task_kinds.get(task_id) == Some(&TaskKind::Source) {
                            info!("Connecting source actor {} to task {}", source_id, task_id);

                            // Send the ConnectToTask message to establish the connection
                            source_addr.do_send(ConnectToTask {
                                task_addr: task_addr.clone(),
                                task_id: task_id.clone(),
                            });
                        }
                    }
                }

                // Connect DestinationActors to their corresponding TaskActors
                for (dest_id, dest_addr) in &self.destination_actors {
                    // Find a matching destination task (with matching ID or prefix)
                    for (task_id, task_addr) in &self.tasks {
                        if task_id.contains(dest_id) && self.task_kinds.get(task_id) == Some(&TaskKind::Destination) {
                            info!("Connecting destination actor {} to task {}", dest_id, task_id);

                            // Send the ConnectToTask message to establish the connection
                            dest_addr.do_send(ConnectToTask {
                                task_addr: task_addr.clone(),
                                task_id: task_id.clone(),
                            });
                        }
                    }
                }

                // Start all source tasks - broadcast to all source tasks
                let source_tasks: Vec<_> = self.tasks.iter()
                    .filter(|(id, _)| {
                        // Check if this task ID exists in task_kinds and is a Source task
                        match self.task_kinds.get(id.as_str()) {
                            Some(kind) => *kind == TaskKind::Source,
                            None => false
                        }
                    })
                    .map(|(id, addr)| (id.clone(), addr.clone()))
                    .collect();

                for (task_id, task_addr) in source_tasks {
                    info!("Starting source task {}", task_id);
                    let _ = task_addr.do_send(StartExtraction {
                        workflow_id: self.id.clone(),
                        source_id: task_id.clone(),
                        config: serde_json::json!({
                            "batch_size": 1000,
                        }),
                        state: None,
                    });
                }

                // Start all source actors - directly start the source extraction
                for (source_id, source_addr) in &self.source_actors {
                    info!("Starting source actor {}", source_id);
                    let _ = source_addr.do_send(StartExtraction {
                        workflow_id: self.id.clone(),
                        source_id: source_id.clone(),
                        config: serde_json::json!({
                            "batch_size": 1000,
                        }),
                        state: None,
                    });
                }

                // Send initial progress update
                self.broadcast_progress(WorkflowPhase::Initializing, 0.0, "Workflow started");

                // Set up a timer to periodically update progress based on task status
                ctx.run_interval(Duration::from_secs(5), move |actor, _ctx| {
                    actor.broadcast_progress(WorkflowPhase::Extracting, 0.5, "Workflow running");
                });

        Ok(())
            }
            _ => Err(DataFlareError::Workflow(format!("Workflow is in wrong state for starting: {:?}", self.status))),
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
        self.subscribers.insert(id, msg.recipient.clone());

        // Forward subscription to all task actors (use clone() explicitly)
        for (_id, addr) in &self.tasks {
            let _ = addr.do_send(SubscribeProgress {
                workflow_id: msg.workflow_id.clone(),
                recipient: msg.recipient.clone(),
            });
        }

        Ok(())
    }
}

impl Handler<UnsubscribeProgress> for WorkflowActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: UnsubscribeProgress, _ctx: &mut Self::Context) -> Self::Result {
        // Remove by value since we don't have the UUID
        self.subscribers.retain(|_, r| r != &msg.recipient);

        // Forward unsubscription to all task actors (use clone() explicitly)
        for (_id, addr) in &self.tasks {
            let _ = addr.do_send(UnsubscribeProgress {
                workflow_id: msg.workflow_id.clone(),
                recipient: msg.recipient.clone(),
            });
            }

            Ok(())
    }
}

impl Handler<CheckWorkflow> for WorkflowActor {
    type Result = Result<ActorStatus>;

    fn handle(&mut self, _msg: CheckWorkflow, ctx: &mut Self::Context) -> Self::Result {
        // Convenience method to check if workflow is finished
        // by checking all task statuses

        // Clone status before spawning any futures
        let status = self.status.clone();
        let addr = ctx.address();
        let workflow_id = self.id.clone();

        // Spawn a future that will update stats asynchronously without returning a result
        ctx.spawn(async move {
            addr.do_send(UpdateWorkflowStats {
                workflow_id: workflow_id
            });
        }.into_actor(self));

        // Return the cloned status
        Ok(status)
    }
}

impl Handler<UpdateWorkflowStats> for WorkflowActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateWorkflowStats, ctx: &mut Self::Context) -> Self::Result {
        if msg.workflow_id != self.id {
            return;
        }

        let addr = ctx.address();

        let fut = async move {
            // 发送另一个消息回到actor以获取状态更新
            let _ = addr.send(InternalUpdateStats).await;

            // 返回单元值，不是Result
            ()
        };

        ctx.spawn(fut.into_actor(self));
    }
}

// 内部消息，用于更新状态
#[derive(Message)]
#[rtype(result = "()")]
struct InternalUpdateStats;

impl Handler<InternalUpdateStats> for WorkflowActor {
    type Result = ();

    fn handle(&mut self, _: InternalUpdateStats, _ctx: &mut Self::Context) -> Self::Result {
        // 在这里更新状态
        self.broadcast_progress(
            WorkflowPhase::Extracting,
            0.5,
            &format!("Workflow {} running", self.id)
        );
    }
}

// For TaskActor to reference the workflow actor
impl Handler<ProcessBatch> for WorkflowActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: ProcessBatch, _ctx: &mut Self::Context) -> Self::Result {
        // Workflows don't process batches directly but route them
        // This helps in testing and monitoring
        info!("Workflow {} received batch with {} records",
              self.id, msg.batch.records.len());

        // Just acknowledge receipt
        Box::pin(async move {
            Ok(())
        })
    }
}

impl Handler<ExecuteWorkflow> for WorkflowActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: ExecuteWorkflow, ctx: &mut Self::Context) -> Self::Result {
        info!("Executing workflow {}", msg.workflow_id);

        // If the workflow is already initialized, start it
        if self.id == msg.workflow_id && self.status == ActorStatus::Initialized {
            // Include any parameters if provided
            if let Some(params) = msg.parameters {
                debug!("Workflow execution parameters: {:?}", params);
                // Store parameters for later use if needed
            }

            // Start the workflow
            return self.handle(StartWorkflow, ctx);
        } else if self.id != msg.workflow_id {
            return Err(DataFlareError::Workflow(format!("Workflow ID mismatch: expected {}, got {}",
                self.id, msg.workflow_id)));
        } else if self.status != ActorStatus::Initialized {
            return Err(DataFlareError::Workflow(format!("Workflow {} is not in initialized state (current: {:?})",
                self.id, self.status)));
        }

        Err(DataFlareError::Workflow(format!("Failed to execute workflow {}", msg.workflow_id)))
    }
}

/// Handler for TaskCompleted message
impl Handler<TaskCompleted> for WorkflowActor {
    type Result = ();

    fn handle(&mut self, msg: TaskCompleted, _ctx: &mut Self::Context) -> Self::Result {
        if msg.success {
            info!("Task {} completed in workflow {}, processed {} records",
                 msg.task_id, msg.workflow_id, msg.records_processed);

            // Update statistics
            self.stats.records_processed += msg.records_processed;

            // Mark task as completed
            self.completed_tasks.insert(msg.task_id.clone());

            // Remove from recovery set if it was in recovery
            self.tasks_in_recovery.remove(&msg.task_id);

            // Check if all tasks in recovery are now completed
            if self.recovery_in_progress && self.tasks_in_recovery.is_empty() {
                info!("All tasks in recovery have completed for workflow {}", self.id);
                self.recovery_in_progress = false;

                // Broadcast recovery completion
                self.broadcast_progress(
                    WorkflowPhase::Transforming,
                    0.5,
                    &format!("Workflow {} recovered from failures", self.id)
                );
            }
        } else {
            error!("Task {} failed in workflow {}: {}",
                  msg.task_id, msg.workflow_id,
                  msg.error_message.unwrap_or_else(|| "Unknown error".to_string()));

            // Mark task as failed
            self.failed_tasks.insert(msg.task_id.clone());

            // If abort strategy is configured, stop the workflow
            if self.failure_strategy == FailureStrategy::Abort {
                self.status = ActorStatus::Failed;
                self.stats.end_time = Some(chrono::Utc::now());

                // Broadcast workflow failure event
                self.broadcast_progress(
                    WorkflowPhase::Error,
                    1.0,
                    &format!("Workflow {} failed due to task {}",
                            self.id, msg.task_id)
                );

                return;
            }
        }

        // Check if all tasks are completed or failed
        let total_task_count = self.tasks.len();
        let completed_count = self.completed_tasks.len();
        let failed_count = self.failed_tasks.len();

        let all_tasks_processed = (completed_count + failed_count) >= total_task_count;

        if all_tasks_processed {
            // Set final workflow status based on failed task count
            if failed_count == 0 {
                info!("All tasks completed successfully for workflow {}", self.id);
                self.status = ActorStatus::Completed;

                // Broadcast workflow completion event
                self.broadcast_progress(
                    WorkflowPhase::Completed,
                    1.0,
                    &format!("Workflow {} completed successfully", self.id)
                );
            } else {
                info!("Workflow {} completed with {} failed tasks", self.id, failed_count);
                self.status = ActorStatus::CompletedWithErrors;

                // Broadcast workflow partial completion event
                self.broadcast_progress(
                    WorkflowPhase::Error,
                    1.0,
                    &format!("Workflow {} completed with {} failed tasks",
                            self.id, failed_count)
                );
            }

            // Record end time
            self.stats.end_time = Some(chrono::Utc::now());

            // Calculate and record execution time
            if let Some(start_time) = self.start_time {
                let duration = start_time.elapsed();
                self.stats.execution_time_ms = Some(duration.as_millis() as u64);
                info!("Workflow {} executed in {:?}", self.id, duration);
            }
        } else {
            // Update progress
            let progress = (completed_count as f64) / (total_task_count as f64);
            self.broadcast_progress(
                WorkflowPhase::Transforming,
                progress,
                &format!("Workflow {} progress: {:.1}%",
                        self.id, progress * 100.0)
            );
        }
    }
}

/// Handler for RegisterTask message
impl Handler<RegisterTask> for WorkflowActor {
    type Result = ();

    fn handle(&mut self, msg: RegisterTask, _ctx: &mut Self::Context) -> Self::Result {
        info!("Registering task {} with workflow {}", msg.task_id, self.id);

        // Store the task actor address
        self.tasks.insert(msg.task_id.clone(), msg.task_addr.clone());

        // Store the task kind
        self.task_kinds.insert(msg.task_id.clone(), msg.task_kind);

        // Set the workflow actor reference in the task
        msg.task_addr.do_send(Initialize {
            workflow_id: self.id.clone(),
            config: serde_json::json!({
                "task_id": msg.task_id,
                "workflow_id": self.id
            }),
        });

        // Provide the workflow actor reference to the task for completion notification
        if let Ok(addr) = _ctx.address().try_into() {
            msg.task_addr.do_send(crate::actor::task::SetWorkflowActor {
                workflow_actor: addr,
            });
        }
    }
}

/// Handler for RegisterSourceActor message
impl Handler<RegisterSourceActor> for WorkflowActor {
    type Result = ();

    fn handle(&mut self, msg: RegisterSourceActor, _ctx: &mut Self::Context) -> Self::Result {
        info!("Registering source actor {} with workflow {}", msg.source_id, self.id);

        // Store the source actor in a separate map
        self.source_actors.insert(msg.source_id.clone(), msg.source_addr.clone());

        // Get the actual source configuration from workflow config if available
        let source_config = if let Some(workflow_config) = &self.config {
            if let Some(source_cfg) = workflow_config.sources.get(&msg.source_id) {
                // Use the source's actual configuration from workflow
                source_cfg.config.clone()
            } else {
                // Fallback to basic configuration
                serde_json::json!({
                    "source_id": msg.source_id,
                    "workflow_id": self.id
                })
            }
        } else {
            // Fallback to basic configuration
            serde_json::json!({
                "source_id": msg.source_id,
                "workflow_id": self.id
            })
        };

        // Initialize the source actor with this workflow and proper configuration
        msg.source_addr.do_send(Initialize {
            workflow_id: self.id.clone(),
            config: source_config,
        });
    }
}

/// Handler for RegisterDestinationActor message
impl Handler<RegisterDestinationActor> for WorkflowActor {
    type Result = ();

    fn handle(&mut self, msg: RegisterDestinationActor, _ctx: &mut Self::Context) -> Self::Result {
        info!("Registering destination actor {} with workflow {}", msg.destination_id, self.id);

        // Store the destination actor in the destination actors map
        self.destination_actors.insert(msg.destination_id.clone(), msg.destination_addr.clone());

        // Get the actual destination configuration from workflow config if available
        let dest_config = if let Some(workflow_config) = &self.config {
            if let Some(dest_cfg) = workflow_config.destinations.get(&msg.destination_id) {
                // Use the destination's actual configuration from workflow
                dest_cfg.config.clone()
            } else {
                // Fallback to basic configuration
                serde_json::json!({
                    "destination_id": msg.destination_id,
                    "workflow_id": self.id
                })
            }
        } else {
            // Fallback to basic configuration
            serde_json::json!({
                "destination_id": msg.destination_id,
                "workflow_id": self.id
            })
        };

        // Initialize the destination actor with this workflow and proper configuration
        msg.destination_addr.do_send(Initialize {
            workflow_id: self.id.clone(),
            config: dest_config,
        });
    }
}

/// Handler for TaskFailed message
impl Handler<SetFailureStrategy> for WorkflowActor {
    type Result = ();

    fn handle(&mut self, msg: SetFailureStrategy, _ctx: &mut Self::Context) -> Self::Result {
        info!("Setting failure strategy for workflow {} to {:?}", self.id, msg.strategy);

        if self.id == msg.workflow_id {
            self.failure_strategy = msg.strategy;

            // Apply the strategy to all tasks
            for (task_id, task_addr) in &self.tasks {
                // Set error strategy based on workflow failure strategy
                let error_strategy = match msg.strategy {
                    FailureStrategy::Continue => ErrorStrategy::Continue,
                    FailureStrategy::Retry(max_retries) => ErrorStrategy::Retry(max_retries),
                    FailureStrategy::Abort => ErrorStrategy::Abort,
                };

                // Send message to task
                task_addr.do_send(crate::actor::task::SetErrorStrategy {
                    strategy: error_strategy,
                });

                info!("Applied error strategy {:?} to task {}", error_strategy, task_id);
            }
        } else {
            warn!("Workflow ID mismatch: expected {}, got {}", self.id, msg.workflow_id);
        }
    }
}

impl Handler<TaskFailed> for WorkflowActor {
    type Result = ();

    fn handle(&mut self, msg: TaskFailed, ctx: &mut Self::Context) -> Self::Result {
        error!("Task {} failed in workflow {}: {}", msg.task_id, msg.workflow_id, msg.error_message);

        // Add to failed tasks
        self.failed_tasks.insert(msg.task_id.clone());

        // Update workflow statistics
        self.stats.error_count += 1;
        self.stats.last_error = Some(format!("Task {} failed: {}", msg.task_id, msg.error_message));

        // Handle based on failure strategy
        match self.failure_strategy {
            FailureStrategy::Continue => {
                warn!("Continuing workflow {} despite task failure", self.id);

                // Broadcast progress update
                self.broadcast_progress(
                    WorkflowPhase::Error,
                    0.0,
                    &format!("Task {} failed but workflow continues: {}",
                            msg.task_id, msg.error_message)
                );
            },
            FailureStrategy::Retry(max_retries) => {
                // Get current retry count and make a copy
                let retry_count_entry = self.task_retry_counts
                    .entry(msg.task_id.clone())
                    .or_insert(0);

                let current_retry_count = *retry_count_entry;

                if current_retry_count < max_retries && msg.should_retry {
                    // Increment retry count
                    *retry_count_entry += 1;
                    let new_retry_count = current_retry_count + 1;

                    // Add to recovery set
                    self.tasks_in_recovery.insert(msg.task_id.clone());
                    self.recovery_in_progress = true;

                    info!("Retrying task {} (attempt {}/{})",
                          msg.task_id, new_retry_count, max_retries);

                    // Broadcast retry progress
                    self.broadcast_progress(
                        WorkflowPhase::Transforming,
                        0.0,
                        &format!("Retrying task {} (attempt {}/{})",
                                msg.task_id, new_retry_count, max_retries)
                    );

                    // Schedule task retry
                    if let Some(task_addr) = self.tasks.get(&msg.task_id) {
                        let task_addr_clone = task_addr.clone();
                        let task_id_clone = msg.task_id.clone();

                        // Exponential backoff
                        let delay = 2u64.pow(new_retry_count as u32) * 100;

                        // 创建 workflow_id 的副本，以便在闭包中使用
                        let workflow_id_clone = msg.workflow_id.clone();

                        ctx.run_later(Duration::from_millis(delay), move |_actor, _| {
                            info!("Executing retry for task {}", task_id_clone);

                            // 重置任务状态
                            task_addr_clone.do_send(crate::actor::task::ResetTask {
                                workflow_id: workflow_id_clone.clone(),
                            });

                            // 恢复任务执行
                            task_addr_clone.do_send(Resume {
                                workflow_id: workflow_id_clone.clone(),
                            });

                            // 如果是源任务，重新触发数据提取
                            task_addr_clone.do_send(StartExtraction {
                                workflow_id: workflow_id_clone.clone(),
                                source_id: task_id_clone.clone(),
                                config: serde_json::json!({}),
                                state: None,
                            });
                        });
                    }
                } else {
                    // Max retries exceeded or retry not requested
                    error!("Max retries exceeded or retry not requested for task {}", msg.task_id);

                    if self.failed_tasks.len() >= self.max_task_failures {
                        // Too many failures, abort workflow
                        error!("Too many task failures ({}/{}), aborting workflow {}",
                               self.failed_tasks.len(), self.max_task_failures, self.id);

                        self.status = ActorStatus::Failed;

                        // Broadcast failure
                        self.broadcast_progress(
                            WorkflowPhase::Error,
                            0.0,
                            &format!("Workflow {} aborted due to too many task failures", self.id)
                        );
                    }
                }
            },
            FailureStrategy::Abort => {
                // Abort workflow immediately
                error!("Aborting workflow {} due to task failure", self.id);

                self.status = ActorStatus::Failed;

                // Broadcast failure
                self.broadcast_progress(
                    WorkflowPhase::Error,
                    0.0,
                    &format!("Workflow {} aborted due to task failure: {}",
                            self.id, msg.error_message)
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::prelude::*;
    use std::time::Duration;

    #[actix::test]
    async fn test_workflow_actor_initialization() {
        let workflow_actor = WorkflowActor::new("test-workflow".to_string());
        let addr = workflow_actor.start();

        let result = addr.send(Initialize {
            workflow_id: "test-workflow".to_string(),
            config: serde_json::json!({}),
        }).await.unwrap();

        assert!(result.is_ok());

        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert!(matches!(status, ActorStatus::Initialized));
    }

    #[actix::test]
    async fn test_workflow_lifecycle() {
        // 创建一个完整的工作流及其任务
        let workflow_actor = WorkflowActor::new("test-workflow-lifecycle".to_string());
        let addr = workflow_actor.start();

        // 初始化工作流
        let config = serde_json::json!({
            "name": "测试工作流",
            "sources": {
                "source1": {
                    "type": "test-source",
                    "config": { "test": "data" }
                }
            },
            "transformations": {
                "transform1": {
                    "inputs": ["source1"],
                    "type": "test-transformation",
                    "config": { "test": "data" }
                }
            },
            "destinations": {
                "dest1": {
                    "inputs": ["transform1"],
                    "type": "test-destination",
                    "config": { "test": "data" }
                }
            }
        });

        let result = addr.send(Initialize {
            workflow_id: "test-workflow-lifecycle".to_string(),
            config,
        }).await.unwrap();

        assert!(result.is_ok());

        // 获取初始化后的状态
        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert!(matches!(status, ActorStatus::Initialized));

        // 启动工作流
        let result = addr.send(StartWorkflow).await.unwrap();
        assert!(result.is_ok());

        // 验证工作流已经启动
        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert!(matches!(status, ActorStatus::Running));

        // 等待一段时间让工作流处理任务
        actix_rt::time::sleep(Duration::from_millis(100)).await;

        // 获取工作流统计信息
        let stats = addr.send(GetWorkflowStats).await.unwrap().unwrap();

        // 检查统计信息
        assert!(stats.start_time.is_some());
        assert!(stats.error_count == 0);

        // 停止工作流
        let result = addr.send(StopWorkflow).await.unwrap();
        assert!(result.is_ok());

        // 确认工作流已停止
        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert!(matches!(status, ActorStatus::Stopped));
    }
}
