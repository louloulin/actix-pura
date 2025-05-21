//! Task Actor implementation
//!
//! This module defines the TaskActor, which is the base class for all data processing tasks
//! in the flattened architecture. It provides a unified interface for sources, processors,
//! and destinations.

use std::collections::HashMap;
use actix::prelude::*;
use std::sync::{Arc, Mutex};
use log::{debug, error, info, warn};
use uuid::Uuid;
use futures::StreamExt;
use std::time::{Duration, Instant};

use dataflare_core::error::{DataFlareError, Result};
use dataflare_core::message::{DataRecord, DataRecordBatch, WorkflowPhase, WorkflowProgress};
use dataflare_core::config::TaskConfig;

use crate::actor::{
    ActorStatus, DataFlareActor, GetStatus, Initialize, Finalize, Pause, Resume,
    SendBatch, SubscribeProgress, UnsubscribeProgress
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
#[derive(Message)]
#[rtype(result = "Result<DataRecordBatch>")]
pub struct ProcessBatch {
    /// Workflow ID
    pub workflow_id: String,
    /// Batch of data to process
    pub batch: DataRecordBatch,
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

/// Base actor for all data processing tasks
pub struct TaskActor {
    /// Unique ID for this task
    id: String,
    /// Task name
    name: String,
    /// Task kind (source, processor, destination)
    kind: TaskKind,
    /// Task configuration
    config: TaskConfig,
    /// Workflow this task belongs to
    workflow_id: String,
    /// Task state for checkpointing and recovery
    state: TaskState,
    /// Actor status
    status: ActorStatus,
    /// Task statistics
    stats: TaskStats,
    /// Upstream tasks
    upstream: Vec<Addr<TaskActor>>,
    /// Downstream tasks
    downstream: Vec<Addr<TaskActor>>,
    /// Progress subscribers
    progress_subscribers: HashMap<Uuid, Recipient<WorkflowProgress>>,
    /// Batch processing times (for calculating averages)
    processing_times: Vec<Duration>,
    /// Max number of processing times to keep
    max_processing_times: usize,
    /// Start time for rate calculation
    rate_calculation_start: Option<Instant>,
    /// Records processed since rate calculation started
    records_since_rate_start: usize,
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
            task_id: self.id.clone(),
            task_name: self.name.clone(),
            phase: match self.kind {
                TaskKind::Source => WorkflowPhase::Extracting,
                TaskKind::Processor => WorkflowPhase::Processing,
                TaskKind::Destination => WorkflowPhase::Loading,
            },
            progress,
            message: message.to_string(),
            timestamp: chrono::Utc::now(),
            records_processed: self.stats.records_processed,
        }
    }

    /// Send progress to all subscribers
    fn broadcast_progress(&self, progress: f64, message: &str) {
        let progress_msg = self.create_progress(progress, message);
        
        for (_, recipient) in &self.progress_subscribers {
            let _ = recipient.do_send(progress_msg.clone());
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

    /// Forward data to all downstream tasks
    async fn forward_to_downstream(&self, batch: DataRecordBatch) -> Result<()> {
        for task in &self.downstream {
            let send_result = task.send(ProcessBatch {
                workflow_id: self.workflow_id.clone(),
                batch: batch.clone(),
            }).await;
            
            if let Err(e) = send_result {
                error!("Failed to send batch to downstream task: {}", e);
                return Err(DataFlareError::Actor(format!("Failed to send batch to downstream task: {}", e)));
            }
        }
        
        Ok(())
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
        
        let progress_msg = WorkflowProgress {
            workflow_id: workflow_id.to_string(),
            task_id: self.id.clone(),
            task_name: self.name.clone(),
            phase,
            progress,
            message: message.to_string(),
            timestamp: chrono::Utc::now(),
            records_processed: self.stats.records_processed,
        };
        
        for (_, recipient) in &self.progress_subscribers {
            let _ = recipient.do_send(progress_msg.clone());
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

impl Handler<ProcessBatch> for TaskActor {
    type Result = ResponseFuture<Result<DataRecordBatch>>;
    
    fn handle(&mut self, msg: ProcessBatch, _ctx: &mut Self::Context) -> Self::Result {
        // Skip processing if not running
        if self.status != ActorStatus::Running {
            return Box::pin(async move {
                Err(DataFlareError::Actor(format!("Task is not running: {:?}", self.status)))
            });
        }
        
        let batch = msg.batch;
        let start_time = Instant::now();
        
        // Clone self fields needed in the future
        let self_id = self.id.clone();
        
        // Process batch
        let fut = self.process_data(batch);
        
        Box::pin(async move {
            match fut.await {
                Ok(processed_batch) => {
                    let processing_time = start_time.elapsed();
                    debug!("Task {} processed batch in {:?}", self_id, processing_time);
                    Ok(processed_batch)
                }
                Err(e) => {
                    error!("Task {} failed to process batch: {}", self_id, e);
                    Err(e)
                }
            }
        })
    }
}

impl Handler<SendBatch> for TaskActor {
    type Result = ResponseFuture<Result<()>>;
    
    fn handle(&mut self, msg: SendBatch, _ctx: &mut Self::Context) -> Self::Result {
        debug!("Task {} received batch to send", self.id);
        
        let start_time = Instant::now();
        let batch = msg.batch;
        let self_id = self.id.clone();
        let downstream = self.downstream.clone();
        
        Box::pin(async move {
            // Start with the current batch
            let processed_batch = batch;
            
            // Forward to downstream tasks
            for task in downstream {
                let send_result = task.send(ProcessBatch {
                    workflow_id: msg.workflow_id.clone(),
                    batch: processed_batch.clone(),
                }).await;
                
                if let Err(e) = send_result {
                    error!("Task {} failed to send batch to downstream task: {}", self_id, e);
                    return Err(DataFlareError::Actor(format!("Failed to send batch to downstream task: {}", e)));
                }
            }
            
            debug!("Task {} forwarded batch in {:?}", self_id, start_time.elapsed());
            Ok(())
        })
    }
}

impl Handler<SubscribeProgress> for TaskActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: SubscribeProgress, _ctx: &mut Self::Context) -> Self::Result {
        let sub_id = Uuid::new_v4();
        self.progress_subscribers.insert(sub_id, msg.recipient);
        debug!("Added progress subscriber to task {}", self.id);
        Ok(())
    }
}

impl Handler<UnsubscribeProgress> for TaskActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: UnsubscribeProgress, _ctx: &mut Self::Context) -> Self::Result {
        let removed = self.progress_subscribers.iter()
            .find(|(_, r)| r.recipient_id() == msg.recipient.recipient_id())
            .map(|(id, _)| id.clone());
        
        if let Some(id) = removed {
            self.progress_subscribers.remove(&id);
            debug!("Removed progress subscriber from task {}", self.id);
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
            DataRecord::new("test-record-1"),
            DataRecord::new("test-record-2"),
            DataRecord::new("test-record-3"),
        ];
        let batch = DataRecordBatch::new("test-batch", None, records);
        
        // Process batch
        let result = addr.send(ProcessBatch {
            workflow_id: "test-workflow".into(),
            batch: batch.clone(),
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
            recipient: collector_addr.recipient(),
        }).await.unwrap().unwrap();
        
        // Report progress
        addr.do_send(crate::actor::SendBatch {
            workflow_id: "test-workflow".into(),
            batch: DataRecordBatch::new("test", None, vec![DataRecord::new("test")]),
        });
        
        // Wait a moment for progress to be reported
        actix_rt::time::sleep(Duration::from_millis(100)).await;
        
        // Manually report progress
        let task_addr = addr.clone();
        actix::spawn(async move {
            let df_actor: Box<dyn DataFlareActor> = Box::new(TaskActor::new("test", TaskKind::Processor));
            df_actor.report_progress("test-workflow", WorkflowPhase::Processing, 0.5, "Half done");
            
            task_addr.do_send(crate::actor::SendBatch {
                workflow_id: "test-workflow".into(),
                batch: DataRecordBatch::new("test", None, vec![DataRecord::new("test")]),
            });
        });
        
        // Wait for progress to be reported
        actix_rt::time::sleep(Duration::from_millis(100)).await;
        
        // Unsubscribe from progress
        addr.send(UnsubscribeProgress {
            workflow_id: "test-workflow".into(),
            recipient: collector_addr.recipient(),
        }).await.unwrap().unwrap();
        
        // Finalize
        addr.send(Finalize { workflow_id: "test-workflow".into() }).await.unwrap().unwrap();
    }
} 