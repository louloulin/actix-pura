//! Workflow Connection Test Example
//!
//! This example demonstrates the improved actor model with proper connection
//! between SourceActor and TaskActor, and shows the workflow completion mechanism.

use std::time::Duration;
use actix::prelude::*;
use log::{debug, error, info, warn};
use futures::stream::{self, StreamExt};

use dataflare_core::error::Result;
use dataflare_core::message::{DataRecord, DataRecordBatch, StartExtraction, WorkflowPhase, WorkflowProgress};
use dataflare_core::state::SourceState;

use dataflare_connector::source::{SourceConnector, ExtractionMode};

use dataflare_runtime::actor::{
    SourceActor, TaskActor, WorkflowActor, ConnectToTask,
    TaskKind, GetStatus
};
use dataflare_runtime::actor::task::AddDownstream;
use dataflare_runtime::actor::workflow::StartWorkflow;
use dataflare_runtime::actor::SubscribeProgress;
use dataflare_runtime::actor::ActorStatus;
use async_trait::async_trait;

/// Simple mock connector that produces sample data
struct MockConnector {
    records: Vec<DataRecord>,
}

impl MockConnector {
    fn new(count: usize) -> Self {
        let mut records = Vec::with_capacity(count);
        for i in 0..count {
            records.push(DataRecord::new(serde_json::json!(format!("record-{}", i))));
        }
        Self { records }
    }
}

#[async_trait]
impl SourceConnector for MockConnector {
    fn configure(&mut self, _config: &serde_json::Value) -> Result<()> {
        Ok(())
    }

    async fn check_connection(&self) -> Result<bool> {
        Ok(true)
    }

    async fn discover_schema(&self) -> Result<dataflare_core::model::Schema> {
        Ok(dataflare_core::model::Schema::default())
    }

    async fn read(&mut self, _state: Option<SourceState>) -> Result<Box<dyn futures::Stream<Item = Result<DataRecord>> + Send + Unpin>> {
        // Create a stream from our records
        let records = self.records.clone();
        let stream = stream::iter(records.into_iter().map(Ok));
        Ok(Box::new(stream))
    }

    fn get_state(&self) -> Result<SourceState> {
        Ok(SourceState::new("mock"))
    }

    fn get_extraction_mode(&self) -> ExtractionMode {
        ExtractionMode::Full
    }

    async fn estimate_record_count(&self, _state: Option<SourceState>) -> Result<u64> {
        Ok(self.records.len() as u64)
    }
}

// Clone implementation needed for actor usage
impl Clone for MockConnector {
    fn clone(&self) -> Self {
        Self {
            records: self.records.clone(),
        }
    }
}

#[actix_rt::main]
async fn main() -> Result<()> {
    // Initialize logger with debug level for our example
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));

    info!("Starting workflow connection test example");

    // Create a system for our actors
    let system = actix::System::new();

    // Create the workflow actor
    let workflow_actor = WorkflowActor::new("test-workflow").start();

    // Create a mock connector with 100 records
    let connector = Box::new(MockConnector::new(100));

    // Create a source actor with the mock connector
    let source_actor = SourceActor::new("mock-source", connector).start();

    // Create task actors for our pipeline
    let source_task = TaskActor::new("source-task", TaskKind::Source).start();
    let transform_task = TaskActor::new("transform-task", TaskKind::Processor).start();
    let dest_task = TaskActor::new("dest-task", TaskKind::Destination).start();

    // Connect source actor to the source task
    info!("Connecting source actor to source task");
    source_actor.do_send(ConnectToTask {
        task_addr: source_task.clone(),
        task_id: "source-task".to_string(),
    });

    // Create the data pipeline by connecting tasks
    info!("Building data pipeline: source -> transform -> dest");
    source_task.do_send(AddDownstream {
        actor_addr: transform_task.clone(),
    });

    transform_task.do_send(AddDownstream {
        actor_addr: dest_task.clone(),
    });

    // Set up workflow progress monitoring
    let (tx, rx) = std::sync::mpsc::channel::<WorkflowProgress>();

    // Create a progress handler
    let progress_handler = move |progress: WorkflowProgress| {
        info!("Progress update: {:?} {:.1}% - {}",
             progress.phase, progress.progress * 100.0, progress.message);

        // Check for completion or error
        match progress.phase {
            WorkflowPhase::Completed => {
                info!("Workflow completed successfully!");
            },
            WorkflowPhase::Error => {
                error!("Workflow failed: {}", progress.message);
            },
            _ => {} // Other phases
        }

        let _ = tx.send(progress);
        async {}
    };

    // Subscribe to progress updates (simplified for this example)
    // Note: In a real implementation, you would properly set up progress monitoring

    // Start the workflow
    info!("Starting workflow");
    let _ = workflow_actor.send(StartWorkflow {}).await?;

    // Now trigger the data extraction
    info!("Starting data extraction");
    source_actor.do_send(StartExtraction {
        workflow_id: "test-workflow".to_string(),
        source_id: "mock-source".to_string(),
        config: serde_json::json!({
            "batch_size": 10,
            "timeout": 5000,
        }),
        state: None,
    });

    // Wait a bit to give the workflow time to process
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Check workflow status
    let status = workflow_actor.send(GetStatus).await??;
    info!("Workflow final status: {:?}", status);

    info!("Workflow connection test completed");

    // Clean shutdown
    actix::System::current().stop();

    Ok(())
}