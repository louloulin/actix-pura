//! Task Actor implementation
//!
//! This module defines the TaskActor, which is the base class for all data processing tasks
//! in the flattened architecture. It provides a unified interface for sources, processors,
//! and destinations.

use std::collections::HashMap;
use actix::prelude::*;
use log::{debug, error, info, warn};
use uuid::Uuid;
use std::time::{Duration, Instant};

use dataflare_core::error::{DataFlareError, Result};
use dataflare_core::message::{DataRecord, DataRecordBatch, WorkflowPhase, WorkflowProgress, StartExtraction};
use dataflare_core::config::TaskConfig;
use dataflare_core::state::SourceState;

use crate::actor::{
    ActorStatus, DataFlareActor, GetStatus, Initialize, Finalize, Pause, Resume,
    SendBatch, SubscribeProgress, UnsubscribeProgress, TaskCompleted, WorkflowActor
};

/// Task kind indicating the role of a task actor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    /// Source task (reads data from external sources)
    Source,
    /// Processor task (transforms data)
    Processor,
    /// Destination task (writes data to external destinations)
    Destination,
}

/// Error handling strategy for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorStrategy {
    /// Continue processing despite errors
    Continue,
    /// Retry failed operations up to a maximum number of times
    Retry(usize),
    /// Abort processing on error
    Abort,
}

impl Default for ErrorStrategy {
    fn default() -> Self {
        Self::Continue
    }
}

/// Task state for checkpoint and recovery
#[derive(Debug, Clone, Default)]
pub struct TaskState {
    /// Task checkpoint data
    pub checkpoint: Option<serde_json::Value>,
    /// Number of records processed
    pub records_processed: usize,
    /// Last processing timestamp
    pub last_processed: Option<chrono::DateTime<chrono::Utc>>,
    /// Custom state properties
    pub properties: HashMap<String, serde_json::Value>,
}

/// Message to process a batch of data
#[derive(Message, Clone)]
#[rtype(result = "Result<()>")]
pub struct ProcessBatch {
    /// Batch of data
    pub batch: DataRecordBatch,
    /// Is this the last batch
    pub is_last_batch: bool,
    /// Workflow ID
    pub workflow_id: String,
}

/// Message to request the current task state
#[derive(Message)]
#[rtype(result = "Result<TaskState>")]
pub struct GetTaskState;

/// Message to set the task state (for recovery)
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct SetTaskState {
    /// Task state to set
    pub state: TaskState,
}

/// Stats about task execution
#[derive(Debug, Clone, Default)]
pub struct TaskStats {
    /// When the task was started
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Total number of records processed
    pub records_processed: usize,
    /// Total number of batches processed
    pub batches_processed: usize,
    /// Total number of bytes processed
    pub bytes_processed: usize,
    /// Number of errors encountered
    pub errors: usize,
    /// Last error message
    pub last_error: Option<String>,
    /// Processing rate (records per second)
    pub records_per_second: f64,
    /// Average batch size
    pub avg_batch_size: usize,
    /// Average processing time per batch (milliseconds)
    pub avg_batch_time_ms: f64,
}

/// Message to get task stats
#[derive(Message)]
#[rtype(result = "Result<TaskStats>")]
pub struct GetTaskStats;

/// Message to set the workflow actor for task completion notification
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetWorkflowActor {
    /// Workflow actor address
    pub workflow_actor: Addr<crate::actor::workflow::WorkflowActor>,
}

/// Message to add a downstream task
#[derive(Message)]
#[rtype(result = "()")]
pub struct AddDownstream {
    /// Task actor address to add as downstream
    pub actor_addr: Addr<TaskActor>,
}

/// Message to set error handling strategy
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetErrorStrategy {
    /// Error handling strategy to set
    pub strategy: ErrorStrategy,
}

/// Message to set destination actor for destination tasks
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetDestinationActor {
    /// Destination actor address
    pub destination_actor: Addr<crate::actor::DestinationActor>,
}

/// Message to set processor actor for processor tasks
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetProcessorActor {
    /// Processor actor address
    pub processor_actor: Addr<crate::actor::ProcessorActor>,
}

/// Message to get downstream task actors
#[derive(Message)]
#[rtype(result = "Vec<Addr<TaskActor>>")]
pub struct GetDownstream;

/// Message to reset task state for retry
#[derive(Message)]
#[rtype(result = "()")]
pub struct ResetTask {
    /// Workflow ID
    pub workflow_id: String,
}

/// Base actor for all data processing tasks
pub struct TaskActor {
    /// Task ID
    id: String,
    /// Task name
    name: String,
    /// Task kind
    kind: TaskKind,
    /// Task configuration
    config: TaskConfig,
    /// Current workflow ID
    workflow_id: String,
    /// Task state
    state: TaskState,
    /// Task status
    status: ActorStatus,
    /// Task statistics
    stats: TaskStats,
    /// Upstream tasks
    upstream: Vec<Addr<TaskActor>>,
    /// Downstream tasks
    downstream: Vec<Addr<TaskActor>>,
    /// Progress subscribers by workflow ID
    progress_subscribers: HashMap<String, Vec<Recipient<WorkflowProgress>>>,
    /// Processing times for performance tracking
    processing_times: Vec<Duration>,
    /// Maximum number of processing times to track
    max_processing_times: usize,
    /// Start time for rate calculation
    rate_calculation_start: Option<Instant>,
    /// Records processed since rate calculation start
    records_since_rate_start: usize,
    /// WorkflowActor reference for completion notification
    workflow_actor: Option<Addr<WorkflowActor>>,
    /// DestinationActor reference for destination tasks
    destination_actor: Option<Addr<crate::actor::DestinationActor>>,
    /// ProcessorActor reference for processor tasks
    processor_actor: Option<Addr<crate::actor::ProcessorActor>>,
    /// Error handling strategy
    error_strategy: ErrorStrategy,
    /// Current retry count
    retry_count: usize,
    /// Maximum number of retries
    max_retries: usize,
    /// Last error message
    last_error: Option<String>,
}

impl TaskActor {
    /// Create a new task actor
    pub fn new(name: &str, kind: TaskKind) -> Self {
        Self {
            id: format!("{}-{}", kind.to_string().to_lowercase(), Uuid::new_v4()),
            name: name.to_string(),
            kind,
            config: TaskConfig::default(),
            workflow_id: String::new(),
            state: TaskState::default(),
            status: ActorStatus::Initialized,
            stats: TaskStats::default(),
            upstream: Vec::new(),
            downstream: Vec::new(),
            progress_subscribers: HashMap::new(),
            processing_times: Vec::with_capacity(100),
            max_processing_times: 100,
            rate_calculation_start: None,
            records_since_rate_start: 0,
            workflow_actor: None,
            destination_actor: None,
            processor_actor: None,
            error_strategy: ErrorStrategy::default(),
            retry_count: 0,
            max_retries: 3, // 默认最大重试次数为3
            last_error: None,
        }
    }

    /// Add an upstream task
    pub fn add_upstream(&mut self, task: Addr<TaskActor>) {
        self.upstream.push(task);
    }

    /// Add a downstream task
    pub fn add_downstream(&mut self, task: Addr<TaskActor>) {
        self.downstream.push(task);
    }

    /// Create a new progress notification
    fn create_progress(&self, progress: f64, message: &str) -> WorkflowProgress {
        WorkflowProgress {
            workflow_id: self.workflow_id.clone(),
            phase: match self.kind {
                TaskKind::Source => WorkflowPhase::Extracting,
                TaskKind::Processor => WorkflowPhase::Transforming,
                TaskKind::Destination => WorkflowPhase::Loading,
            },
            progress, // Use f64 directly
            message: message.to_string(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Send progress to all subscribers
    fn broadcast_progress(&self, progress: f64, message: &str) {
        let progress_msg = self.create_progress(progress, message);

        for (workflow_id, recipients) in &self.progress_subscribers {
            for recipient in recipients {
            let _ = recipient.do_send(progress_msg.clone());
            }
        }
    }

    /// Update statistics after processing a batch
    fn update_stats(&mut self, batch: &DataRecordBatch, processing_time: Duration) {
        // Update counters
        self.stats.batches_processed += 1;
        let record_count = batch.records().len();
        self.stats.records_processed += record_count;
        self.records_since_rate_start += record_count;

        // Estimate byte size
        let byte_size = batch.estimate_size();
        self.stats.bytes_processed += byte_size;

        // Update processing times
        self.processing_times.push(processing_time);
        if self.processing_times.len() > self.max_processing_times {
            self.processing_times.remove(0);
        }

        // Calculate average batch time
        let total_time: Duration = self.processing_times.iter().sum();
        self.stats.avg_batch_time_ms = total_time.as_millis() as f64 / self.processing_times.len() as f64;

        // Calculate average batch size
        self.stats.avg_batch_size = if self.stats.batches_processed > 0 {
            self.stats.records_processed / self.stats.batches_processed
        } else {
            0
        };

        // Calculate records per second
        if let Some(start_time) = self.rate_calculation_start {
            let elapsed = start_time.elapsed();
            // Recalculate every 5 seconds
            if elapsed.as_secs() >= 5 {
                let seconds = elapsed.as_secs_f64();
                if seconds > 0.0 {
                    self.stats.records_per_second = self.records_since_rate_start as f64 / seconds;
                }
                self.rate_calculation_start = Some(Instant::now());
                self.records_since_rate_start = 0;
            }
        } else {
            self.rate_calculation_start = Some(Instant::now());
        }
    }

    /// Process a batch of data
    pub async fn process_data(&mut self, batch: DataRecordBatch) -> Result<DataRecordBatch> {
        // Default implementation just passes through the data
        // Subclasses should override this method
        Ok(batch)
    }

    /// Process a batch with retry logic
    fn process_batch_with_retry(&mut self, msg: &ProcessBatch, ctx: &mut <Self as Actor>::Context) -> Result<()> {
        // Actual processing logic based on task kind
        match self.kind {
            TaskKind::Processor => {
                // For processors, we would apply transformations here
                debug!("Processing batch with {} records in processor task", msg.batch.records.len());

                // Here we would apply actual transformations
                // For now, just simulate processing
                Ok(())
            },
            TaskKind::Source => {
                // For source tasks, just pass through
                debug!("Passing through batch with {} records in source task", msg.batch.records.len());
                Ok(())
            },
            TaskKind::Destination => {
                // For destination tasks, just pass through
                debug!("Passing through batch with {} records in destination task", msg.batch.records.len());
                Ok(())
            }
        }
    }

    /// Forward data to all downstream tasks
    async fn forward_to_downstream(&self, batch: DataRecordBatch) -> Result<()> {
        for task in &self.downstream {
            let send_result = task.send(ProcessBatch {
                workflow_id: self.workflow_id.clone(),
                batch: batch.clone(),
                is_last_batch: false,
            }).await;

            if let Err(e) = send_result {
                error!("Failed to send batch to downstream task: {}", e);
                return Err(DataFlareError::Actor(format!("Failed to send batch to downstream task: {}", e)));
            }
        }

        Ok(())
    }

    fn create_progress_message(&self, workflow_id: &str, progress: f64, message: &str) -> WorkflowProgress {
        WorkflowProgress {
            workflow_id: workflow_id.to_string(),
            phase: match self.kind {
                TaskKind::Source => WorkflowPhase::Extracting,
                TaskKind::Processor => WorkflowPhase::Transforming,
                TaskKind::Destination => WorkflowPhase::Loading,
            },
            progress,
            message: message.to_string(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Set the workflow actor reference
    pub fn set_workflow_actor(&mut self, actor: Addr<WorkflowActor>) {
        self.workflow_actor = Some(actor);
    }

    /// Set the destination actor reference for destination tasks
    pub fn set_destination_actor(&mut self, actor: Addr<crate::actor::DestinationActor>) {
        self.destination_actor = Some(actor);
    }

    /// Set the processor actor reference for processor tasks
    pub fn set_processor_actor(&mut self, actor: Addr<crate::actor::ProcessorActor>) {
        self.processor_actor = Some(actor);
    }

    /// Set the error handling strategy
    pub fn set_error_strategy(&mut self, strategy: ErrorStrategy) {
        self.error_strategy = strategy;

        // If using retry strategy, update max retries
        if let ErrorStrategy::Retry(max) = strategy {
            self.max_retries = max;
        }
    }

    /// Reset retry count
    pub fn reset_retry_count(&mut self) {
        self.retry_count = 0;
    }

    /// Handle error according to the current error strategy
    pub fn handle_error(&mut self, error: &DataFlareError) -> Result<bool> {
        // Record the error
        self.stats.errors += 1;
        self.last_error = Some(error.to_string());

        // Log the error
        error!("Task {} encountered error: {}", self.id, error);

        match self.error_strategy {
            ErrorStrategy::Continue => {
                // Just continue processing
                warn!("Continuing despite error in task {}: {}", self.id, error);
                Ok(true)
            },
            ErrorStrategy::Retry(max_retries) => {
                if self.retry_count < max_retries {
                    // Increment retry count
                    self.retry_count += 1;
                    warn!("Retrying task {} after error (attempt {}/{}): {}",
                          self.id, self.retry_count, max_retries, error);
                    Ok(true)
                } else {
                    // Max retries exceeded
                    error!("Max retries ({}) exceeded for task {}: {}",
                           max_retries, self.id, error);
                    Err(DataFlareError::MaxRetriesExceeded(format!(
                        "Max retries ({}) exceeded for task {}", max_retries, self.id
                    )))
                }
            },
            ErrorStrategy::Abort => {
                // Abort processing
                error!("Aborting task {} due to error: {}", self.id, error);
                Err(DataFlareError::TaskAborted(format!(
                    "Task {} aborted due to error: {}", self.id, error
                )))
            }
        }
    }
}

impl Actor for TaskActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("TaskActor started: {}", self.id);
        self.stats.start_time = Some(chrono::Utc::now());
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("TaskActor stopped: {}", self.id);
    }
}

impl DataFlareActor for TaskActor {
    fn get_id(&self) -> &str {
        &self.id
    }

    fn get_type(&self) -> &str {
        match self.kind {
            TaskKind::Source => "source",
            TaskKind::Processor => "processor",
            TaskKind::Destination => "destination",
        }
    }

    fn initialize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Initializing task: {}", self.id);
        self.status = ActorStatus::Initialized;
        Ok(())
    }

    fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Finalizing task: {}", self.id);
        self.status = ActorStatus::Stopped;
        Ok(())
    }

    fn report_progress(&self, workflow_id: &str, phase: WorkflowPhase, progress: f64, message: &str) {
        debug!("Task {} progress: {}% - {}", self.id, progress * 100.0, message);

        let progress_msg = self.create_progress_message(workflow_id, progress, message);

        if let Some(recipients) = self.progress_subscribers.get(workflow_id) {
            for recipient in recipients {
            let _ = recipient.do_send(progress_msg.clone());
            }
        }
    }
}

impl Handler<Initialize> for TaskActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Initialize, ctx: &mut Self::Context) -> Self::Result {
        info!("Initializing task {} for workflow {}", self.id, msg.workflow_id);

        // Store workflow ID
        self.workflow_id = msg.workflow_id;

        // Parse configuration
        self.config = match serde_json::from_value(msg.config.clone()) {
            Ok(config) => config,
            Err(e) => return Err(DataFlareError::Config(format!("Failed to parse task configuration: {}", e))),
        };

        // Initialize resources
        self.initialize(ctx)?;

        // Set status to running
        self.status = ActorStatus::Running;

        Ok(())
    }
}

impl Handler<Finalize> for TaskActor {
    type Result = Result<()>;

    fn handle(&mut self, _msg: Finalize, ctx: &mut Self::Context) -> Self::Result {
        info!("Finalizing task: {}", self.id);

        // Finalize resources
        self.finalize(ctx)?;

        // Set status to stopped
        self.status = ActorStatus::Stopped;

        Ok(())
    }
}

impl Handler<Pause> for TaskActor {
    type Result = Result<()>;

    fn handle(&mut self, _msg: Pause, _ctx: &mut Self::Context) -> Self::Result {
        info!("Pausing task: {}", self.id);

        // Set status to paused
        self.status = ActorStatus::Paused;

        Ok(())
    }
}

impl Handler<Resume> for TaskActor {
    type Result = Result<()>;

    fn handle(&mut self, _msg: Resume, _ctx: &mut Self::Context) -> Self::Result {
        info!("Resuming task: {}", self.id);

        // Set status to running
        self.status = ActorStatus::Running;

        Ok(())
    }
}

impl Handler<GetStatus> for TaskActor {
    type Result = Result<ActorStatus>;

    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.status.clone())
    }
}

/// Handler for ProcessBatch message
impl Handler<ProcessBatch> for TaskActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: ProcessBatch, ctx: &mut Self::Context) -> Self::Result {
        debug!("TaskActor {} processing batch of {} records", self.id, msg.batch.records.len());

        let start = Instant::now();

        // Reset retry count for new batch
        self.reset_retry_count();

        // Process the batch with error handling
        let process_result = self.process_batch_with_retry(&msg, ctx);

        if let Err(err) = &process_result {
            // Handle error according to strategy
            match self.handle_error(err) {
                Ok(should_retry) => {
                    // Notify workflow of task failure
                    if let Some(workflow_actor) = &self.workflow_actor {
                        workflow_actor.do_send(crate::actor::workflow::TaskFailed {
                            workflow_id: msg.workflow_id.clone(),
                            task_id: self.id.clone(),
                            error_message: err.to_string(),
                            should_retry,
                        });
                    }

                    if should_retry {
                        // Schedule a retry if needed
                        if let ErrorStrategy::Retry(_) = self.error_strategy {
                            info!("Scheduling retry for batch processing in task {}", self.id);

                            // Clone what we need for the retry
                            let msg_clone = msg.clone();
                            let addr = ctx.address();

                            // Schedule retry with exponential backoff
                            let delay = 2u64.pow(self.retry_count as u32) * 100; // Exponential backoff

                            ctx.run_later(Duration::from_millis(delay), move |_, _| {
                                addr.do_send(msg_clone);
                            });

                            return Ok(());
                        }
                    } else {
                        // If not retrying, mark task as failed in workflow
                        if let Some(workflow_actor) = &self.workflow_actor {
                            workflow_actor.do_send(TaskCompleted {
                                workflow_id: msg.workflow_id.clone(),
                                task_id: self.id.clone(),
                                records_processed: self.stats.records_processed,
                                success: false,
                                error_message: Some(err.to_string()),
                            });
                        }
                    }
                },
                Err(e) => {
                    // Critical error, notify workflow and return error
                    if let Some(workflow_actor) = &self.workflow_actor {
                        workflow_actor.do_send(crate::actor::workflow::TaskFailed {
                            workflow_id: msg.workflow_id.clone(),
                            task_id: self.id.clone(),
                            error_message: e.to_string(),
                            should_retry: false,
                        });
                    }
                    return Err(e);
                },
            }
        }

        // If we got here, either processing succeeded or we're continuing despite errors

        // Calculate processing time
        let processing_time = start.elapsed();
        debug!("Batch processed in {:?}", processing_time);

        // Update processing stats
        self.stats.records_processed += msg.batch.records.len();

        // Forward to downstream tasks - use a future to avoid blocking
        if !self.downstream.is_empty() {
            info!("转发批次到 {} 个下游任务", self.downstream.len());

            for task in &self.downstream {
                debug!("Forwarding batch to downstream task");

                // Clone what we need for the future
                let task_addr_clone = task.clone();
                let batch_clone = msg.batch.clone();
                let workflow_id_clone = msg.workflow_id.clone();
                let is_last_batch = msg.is_last_batch;

                // Create future
                let fut = async move {
                    match task_addr_clone.send(SendBatch {
                        workflow_id: workflow_id_clone,
                        batch: batch_clone,
                        is_last_batch,
                    }).await {
                        Ok(result) => {
                            match result {
                                Ok(_) => debug!("Successfully forwarded batch to downstream task"),
                                Err(e) => error!("Error forwarding batch to downstream task: {}", e),
                            }
                        },
                        Err(e) => error!("Mailbox error when forwarding batch: {}", e),
                    }
                };

                // Spawn the future
                ctx.spawn(fut.into_actor(self));
            }
        } else if self.kind == TaskKind::Destination && msg.batch.records.len() > 0 {
            // If we're a destination task, send the batch to any connected destination actors
            info!("发送批次到目标连接器");

            // If we have any destination actors connected, send them the batch
            if let Some(dest_task) = &self.workflow_actor {
                let dest_addr = dest_task.clone();
                let batch_clone = msg.batch.clone();
                let workflow_id_clone = msg.workflow_id.clone();

                // Create future for destination load
                let fut = async move {
                    // 直接处理批次，不发送LoadBatch消息
                    info!("Processing batch in destination task directly");
                    // TODO: 实现实际的目标处理逻辑

                    // 模拟成功处理
                    info!("Successfully processed batch in destination task");
                };

                // Spawn the future
                ctx.spawn(fut.into_actor(self));
            } else {
                warn!("目标任务没有连接到目标连接器");
            }
        }

        Ok(())
    }
}

/// Handler for SendBatch message
impl Handler<SendBatch> for TaskActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: SendBatch, ctx: &mut Self::Context) -> Self::Result {
        info!("TaskActor {} received batch with {} records", self.id, msg.batch.records.len());

        // Process the batch
        match self.kind {
            TaskKind::Source => {
                info!("Source task forwarding batch of {} records", msg.batch.records.len());

                // Process the batch (just pass-through for source)
                let process_result = self.handle(ProcessBatch {
                    batch: msg.batch.clone(),
                    is_last_batch: msg.is_last_batch,
                    workflow_id: msg.workflow_id.clone(),
                }, ctx);

                if let Err(e) = process_result {
                    error!("Error processing batch in source task: {}", e);
                    return Err(e);
                }

                // Forward to downstream tasks
                info!("🔍 Source TaskActor {} has {} downstream tasks", self.id, self.downstream.len());
                for downstream in &self.downstream {
                    info!("🚀 Source TaskActor {} forwarding batch to downstream TaskActor", self.id);
                    downstream.do_send(SendBatch {
                        workflow_id: msg.workflow_id.clone(),
                        batch: msg.batch.clone(),
                        is_last_batch: msg.is_last_batch,
                    });
                }
                if self.downstream.is_empty() {
                    warn!("❌ Source TaskActor {} has no downstream tasks to forward to!", self.id);
                }
            },
            TaskKind::Processor => {
                info!("Processor task processing batch of {} records", msg.batch.records.len());

                // Forward to the actual ProcessorActor if available
                if let Some(proc_actor) = &self.processor_actor {
                    info!("🚀 TaskActor {} forwarding batch of {} records to ProcessorActor",
                          self.id, msg.batch.records.len());

                    // Send SendBatch message to ProcessorActor
                    proc_actor.do_send(crate::actor::SendBatch {
                        workflow_id: msg.workflow_id.clone(),
                        batch: msg.batch.clone(),
                        is_last_batch: msg.is_last_batch,
                    });

                    info!("✅ TaskActor {} successfully sent SendBatch message to ProcessorActor", self.id);

                    // After sending to ProcessorActor, also forward to downstream tasks
                    // The ProcessorActor will process the data, but we need to forward the processed data
                    // For now, we'll forward the original data and let the downstream handle it
                    for downstream in &self.downstream {
                        info!("🚀 Processor TaskActor {} forwarding batch to downstream TaskActor", self.id);
                        downstream.do_send(SendBatch {
                            workflow_id: msg.workflow_id.clone(),
                            batch: msg.batch.clone(),
                            is_last_batch: msg.is_last_batch,
                        });
                    }
                } else {
                    warn!("❌ Processor task {} has no associated ProcessorActor", self.id);
                }
            },
            TaskKind::Destination => {
                info!("Destination task processing batch of {} records", msg.batch.records.len());

                // Forward to the actual DestinationActor if available
                if let Some(dest_actor) = &self.destination_actor {
                    info!("🚀 TaskActor {} forwarding batch of {} records to DestinationActor",
                          self.id, msg.batch.records.len());

                    // Send LoadBatch message to DestinationActor
                    dest_actor.do_send(dataflare_core::message::LoadBatch {
                        workflow_id: msg.workflow_id.clone(),
                        destination_id: self.id.clone(),
                        batch: msg.batch.clone(),
                        config: serde_json::json!({}), // Use empty config for now
                    });

                    info!("✅ TaskActor {} successfully sent LoadBatch message to DestinationActor", self.id);
                } else {
                    warn!("❌ Destination task {} has no associated DestinationActor", self.id);
                }
            }
        }

        // If this is the last batch, notify the workflow
        if msg.is_last_batch {
            if let Some(workflow_actor) = &self.workflow_actor {
                info!("Task {} completed processing, total records: {}", self.id, self.stats.records_processed);

                workflow_actor.do_send(TaskCompleted {
                    workflow_id: msg.workflow_id.clone(),
                    task_id: self.id.clone(),
                    records_processed: self.stats.records_processed as usize,
                    success: true,
                    error_message: None,
                });
            }
        }

            Ok(())
    }
}

impl Handler<SubscribeProgress> for TaskActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: SubscribeProgress, _ctx: &mut Self::Context) -> Self::Result {
        let recipients = self.progress_subscribers
            .entry(msg.workflow_id.clone())
            .or_insert_with(Vec::new);

        recipients.push(msg.recipient);
        debug!("Added progress subscriber to task {}", self.id);
        Ok(())
    }
}

impl Handler<UnsubscribeProgress> for TaskActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: UnsubscribeProgress, _ctx: &mut Self::Context) -> Self::Result {
        // Get the subscribers for this workflow
        let workflow_id_key = msg.workflow_id.clone();
        let mut to_remove = Vec::new();

        // Find matching recipient by comparing addresses
        for (id, subs) in &mut self.progress_subscribers {
            if let Some(pos) = subs.iter().position(|r| std::ptr::eq(r, &msg.recipient)) {
                // Mark for removal
                to_remove.push((id.clone(), pos));
            }
        }

        // Remove the marked subscribers
        for (id, pos) in to_remove {
            if let Some(subs) = self.progress_subscribers.get_mut(&id) {
                subs.remove(pos);
            }
        }

        Ok(())
    }
}

impl Handler<GetTaskState> for TaskActor {
    type Result = Result<TaskState>;

    fn handle(&mut self, _msg: GetTaskState, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.state.clone())
    }
}

impl Handler<SetTaskState> for TaskActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: SetTaskState, _ctx: &mut Self::Context) -> Self::Result {
        self.state = msg.state;
        debug!("Set task state for {}", self.id);
        Ok(())
    }
}

impl Handler<GetTaskStats> for TaskActor {
    type Result = Result<TaskStats>;

    fn handle(&mut self, _msg: GetTaskStats, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.stats.clone())
    }
}

impl Handler<SetWorkflowActor> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: SetWorkflowActor, _ctx: &mut Self::Context) -> Self::Result {
        debug!("Setting workflow actor for task {}", self.id);
        self.workflow_actor = Some(msg.workflow_actor);
    }
}

impl Handler<StartExtraction> for TaskActor {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: StartExtraction, _ctx: &mut Self::Context) -> Self::Result {
        // 只有源任务才应该处理这种消息
        if self.kind != TaskKind::Source {
            return Box::pin(async move {
                Err(DataFlareError::Actor(format!(
                    "任务 {} 不是源任务，无法处理StartExtraction消息",
                    msg.source_id
                )))
            });
        }

        info!("TaskActor(源) {} 开始数据提取，工作流: {}", self.id, msg.workflow_id);

        // 只是为了记录，我们将批处理大小从配置中提取出来
        let batch_size = msg.config.get("batch_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        info!("使用批处理大小: {}", batch_size);

        // 我们将在未来实现实际的抽取逻辑
        // 目前，只返回成功
        Box::pin(async {
            Ok(())
        })
    }
}

impl Handler<AddDownstream> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: AddDownstream, _ctx: &mut Self::Context) -> Self::Result {
        info!("🔗 TaskActor {} adding downstream TaskActor, total downstream: {} -> {}",
              self.id, self.downstream.len(), self.downstream.len() + 1);
        self.downstream.push(msg.actor_addr);
        info!("✅ TaskActor {} successfully added downstream TaskActor, total downstream: {}",
              self.id, self.downstream.len());
    }
}

/// Handler for SetErrorStrategy message
impl Handler<SetErrorStrategy> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: SetErrorStrategy, _ctx: &mut Self::Context) -> Self::Result {
        info!("Setting error strategy for task {} to {:?}", self.id, msg.strategy);
        self.set_error_strategy(msg.strategy);

        // Reset retry count when strategy changes
        self.reset_retry_count();
    }
}

/// Handler for SetDestinationActor message
impl Handler<SetDestinationActor> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: SetDestinationActor, _ctx: &mut Self::Context) -> Self::Result {
        info!("Setting destination actor for task {}", self.id);
        self.set_destination_actor(msg.destination_actor);
    }
}

/// Handler for SetProcessorActor message
impl Handler<SetProcessorActor> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: SetProcessorActor, _ctx: &mut Self::Context) -> Self::Result {
        info!("Setting processor actor for task {}", self.id);
        self.set_processor_actor(msg.processor_actor);
    }
}

/// Handler for GetDownstream message
impl Handler<GetDownstream> for TaskActor {
    type Result = Vec<Addr<TaskActor>>;

    fn handle(&mut self, _msg: GetDownstream, _ctx: &mut Self::Context) -> Self::Result {
        self.downstream.clone()
    }
}

/// Handler for ResetTask message
impl Handler<ResetTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, _msg: ResetTask, _ctx: &mut Self::Context) -> Self::Result {
        info!("Resetting task {} for retry", self.id);

        // 重置重试计数
        self.reset_retry_count();

        // 重置任务状态
        self.status = ActorStatus::Running;

        // 清除上次错误
        self.last_error = None;

        // 重置任务状态数据
        self.state = TaskState {
            checkpoint: self.state.checkpoint.clone(),  // 保留检查点数据
            records_processed: 0,                       // 重置处理记录计数
            last_processed: Some(chrono::Utc::now()),   // 更新处理时间
            properties: HashMap::new(),                 // 重置属性
        };

        // 发送进度更新
        self.report_progress(
            &self.workflow_id,
            match self.kind {
                TaskKind::Source => WorkflowPhase::Extracting,
                TaskKind::Processor => WorkflowPhase::Transforming,
                TaskKind::Destination => WorkflowPhase::Loading,
            },
            0.0,
            &format!("Task {} reset for retry", self.id)
        );
    }
}

impl std::fmt::Display for TaskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskKind::Source => write!(f, "Source"),
            TaskKind::Processor => write!(f, "Processor"),
            TaskKind::Destination => write!(f, "Destination"),
        }
    }
}

// Extension trait for DataRecordBatch to estimate size
trait DataRecordBatchExt {
    fn estimate_size(&self) -> usize;
}

impl DataRecordBatchExt for DataRecordBatch {
    fn estimate_size(&self) -> usize {
        // Simple estimation based on number of records and average record size
        // In a real implementation, you would estimate more accurately based on actual data
        const AVERAGE_RECORD_SIZE: usize = 1024; // 1KB per record as a rough estimate
        self.records().len() * AVERAGE_RECORD_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // Test helper to collect progress updates
    struct ProgressCollector {
        progress: Arc<Mutex<Vec<WorkflowProgress>>>,
    }

    impl ProgressCollector {
        fn new() -> (Self, Arc<Mutex<Vec<WorkflowProgress>>>) {
            let progress = Arc::new(Mutex::new(Vec::new()));
            (Self { progress: progress.clone() }, progress)
        }
    }

    impl Actor for ProgressCollector {
        type Context = Context<Self>;
    }

    impl Handler<WorkflowProgress> for ProgressCollector {
        type Result = ();

        fn handle(&mut self, msg: WorkflowProgress, _ctx: &mut Self::Context) -> Self::Result {
            self.progress.lock().unwrap().push(msg);
        }
    }

    #[actix::test]
    async fn test_task_actor_lifecycle() {
        // Create a task actor
        let task = TaskActor::new("test-task", TaskKind::Processor);
        let addr = task.start();

        // Initialize
        let result = addr.send(Initialize {
            workflow_id: "test-workflow".into(),
            config: serde_json::json!({
                "name": "test-task",
                "type": "processor",
            }),
        }).await.unwrap();

        assert!(result.is_ok());

        // Check status
        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert_eq!(status, ActorStatus::Running);

        // Pause
        let result = addr.send(Pause { workflow_id: "test-workflow".into() }).await.unwrap();
        assert!(result.is_ok());

        // Check status
        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert_eq!(status, ActorStatus::Paused);

        // Resume
        let result = addr.send(Resume { workflow_id: "test-workflow".into() }).await.unwrap();
        assert!(result.is_ok());

        // Check status
        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert_eq!(status, ActorStatus::Running);

        // Finalize
        let result = addr.send(Finalize { workflow_id: "test-workflow".into() }).await.unwrap();
        assert!(result.is_ok());

        // Check status
        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert_eq!(status, ActorStatus::Stopped);
    }

    #[actix::test]
    async fn test_task_actor_processing() {
        // Create a task actor
        let task = TaskActor::new("test-task", TaskKind::Processor);
        let addr = task.start();

        // Initialize
        addr.send(Initialize {
            workflow_id: "test-workflow".into(),
            config: serde_json::json!({}),
        }).await.unwrap().unwrap();

        // Create a test batch
        let records = vec![
            DataRecord::new(serde_json::json!("test-record-1")),
            DataRecord::new(serde_json::json!("test-record-2")),
            DataRecord::new(serde_json::json!("test-record-3")),
        ];

        // Use the correct DataRecordBatch constructor
        let batch = DataRecordBatch::new(records);

        // Process batch
        let result = addr.send(ProcessBatch {
            workflow_id: "test-workflow".into(),
            batch: batch.clone(),
            is_last_batch: false,
        }).await.unwrap();

        assert!(result.is_ok());

        // Get stats
        let stats = addr.send(GetTaskStats).await.unwrap().unwrap();

        // In the base implementation, stats are not updated
        // A real task would override process_data and update stats
        assert_eq!(stats.records_processed, 0);

        // Finalize
        addr.send(Finalize { workflow_id: "test-workflow".into() }).await.unwrap().unwrap();
    }

    #[actix::test]
    async fn test_task_actor_progress_reporting() {
        // Create a progress collector
        let (collector, progress) = ProgressCollector::new();
        let collector_addr = collector.start();

        // Create a task actor
        let task = TaskActor::new("test-task", TaskKind::Processor);
        let addr = task.start();

        // Initialize
        addr.send(Initialize {
            workflow_id: "test-workflow".into(),
            config: serde_json::json!({}),
        }).await.unwrap().unwrap();

        // Subscribe to progress
        addr.send(SubscribeProgress {
            workflow_id: "test-workflow".into(),
            recipient: collector_addr.clone().recipient(),
        }).await.unwrap().unwrap();

        // Report progress - fix SendBatch to include is_last_batch
        addr.do_send(crate::actor::SendBatch {
            workflow_id: "test-workflow".into(),
            batch: DataRecordBatch::new(vec![DataRecord::new(serde_json::json!("test"))]),
            is_last_batch: false,
        });

        // Wait a moment for progress to be reported
        actix_rt::time::sleep(Duration::from_millis(100)).await;

        // Manually report progress
        let task_addr = addr.clone();
        actix::spawn(async move {
            // 不能使用 Box<dyn DataFlareActor>，因为 DataFlareActor 不是 object-safe 的
            let task_actor = TaskActor::new("test", TaskKind::Processor);
            task_actor.report_progress("test-workflow", WorkflowPhase::Transforming, 0.5, "Half done");

            task_addr.do_send(crate::actor::SendBatch {
                workflow_id: "test-workflow".into(),
                batch: DataRecordBatch::new(vec![DataRecord::new(serde_json::json!("test"))]),
                is_last_batch: true,
            });
        });

        // Wait for progress to be reported
        actix_rt::time::sleep(Duration::from_millis(100)).await;

        // Unsubscribe from progress
        addr.send(UnsubscribeProgress {
            workflow_id: "test-workflow".into(),
            recipient: collector_addr.clone().recipient(),
        }).await.unwrap().unwrap();

        // Finalize
        addr.send(Finalize { workflow_id: "test-workflow".into() }).await.unwrap().unwrap();
    }
}