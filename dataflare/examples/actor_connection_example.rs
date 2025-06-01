//! Example showing how to connect SourceActor with TaskActor
//!
//! This example demonstrates the new actor connection system that allows
//! direct communication between SourceActor and TaskActor.

use std::time::Duration;
use actix::prelude::*;
use dataflare_core::error::Result;
use dataflare_core::message::DataRecordBatch;
use dataflare_runtime::actor::{
    ConnectToTask, TaskActor, SourceActor, WorkflowActor,
    TaskKind
};
use dataflare_runtime::actor::task::AddDownstream;
use dataflare_core::message::StartExtraction;
use async_trait::async_trait;

// Create a mock source connector
struct MockSourceConnector;

#[async_trait]
impl dataflare_connector::source::SourceConnector for MockSourceConnector {
    fn configure(&mut self, _config: &serde_json::Value) -> Result<()> {
        Ok(())
    }

    async fn check_connection(&self) -> Result<bool> {
        Ok(true)
    }

    async fn discover_schema(&self) -> Result<dataflare_core::model::Schema> {
        use dataflare_core::model::{Schema, Field, DataType};
        let mut schema = Schema::new();
        schema.add_field(Field::new("id".to_string(), DataType::String));
        Ok(schema)
    }

    async fn read(&mut self, _state: Option<dataflare_core::state::SourceState>) -> Result<Box<dyn futures::Stream<Item = Result<dataflare_core::message::DataRecord>> + Send + Unpin>> {
        use futures::stream;
        let records = vec![
            Ok(dataflare_core::message::DataRecord::new(serde_json::json!("record1"))),
            Ok(dataflare_core::message::DataRecord::new(serde_json::json!("record2"))),
        ];
        Ok(Box::new(stream::iter(records)))
    }

    fn get_state(&self) -> Result<dataflare_core::state::SourceState> {
        Ok(dataflare_core::state::SourceState::new("mock"))
    }

    fn get_extraction_mode(&self) -> dataflare_connector::source::ExtractionMode {
        dataflare_connector::source::ExtractionMode::Full
    }

    async fn estimate_record_count(&self, _state: Option<dataflare_core::state::SourceState>) -> Result<u64> {
        Ok(2)
    }
}

#[actix_rt::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Create workflow actor
    let workflow_actor = WorkflowActor::new("test-workflow").start();

    // Create source actor with mock connector
    let source_actor = SourceActor::new(
        "mock-source",
        Box::new(MockSourceConnector)
    ).start();

    // Create task actors
    let source_task = TaskActor::new("source-task", TaskKind::Source).start();
    let transform_task = TaskActor::new("transform-task", TaskKind::Processor).start();
    let dest_task = TaskActor::new("dest-task", TaskKind::Destination).start();

    // Set workflow actor for each task
    source_task.do_send(SetWorkflowActor {
        workflow_actor: workflow_actor.clone(),
    });

    transform_task.do_send(SetWorkflowActor {
        workflow_actor: workflow_actor.clone(),
    });

    dest_task.do_send(SetWorkflowActor {
        workflow_actor: workflow_actor.clone(),
    });

    // Connect source actor to source task
    source_actor.do_send(ConnectToTask {
        task_addr: source_task.clone(),
        task_id: "source-task".to_string(),
    });

    // Connect tasks in a pipeline
    source_task.do_send(AddDownstream {
        actor_addr: transform_task.clone(),
    });

    transform_task.do_send(AddDownstream {
        actor_addr: dest_task.clone(),
    });

    // Start extraction
    println!("Starting extraction...");
    source_actor.do_send(StartExtraction {
        workflow_id: "test-workflow".to_string(),
        source_id: "mock-source".to_string(),
        config: serde_json::json!({
            "batch_size": 10,
            "timeout": 5000,
        }),
        state: None,
    });

    // Wait a bit for processing to complete
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("Example completed");

    Ok(())
}

// Message to set the workflow actor for a task
#[derive(Message)]
#[rtype(result = "()")]
pub struct SetWorkflowActor {
    pub workflow_actor: Addr<WorkflowActor>,
}

impl Handler<SetWorkflowActor> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: SetWorkflowActor, _ctx: &mut Self::Context) -> Self::Result {
        println!("Setting workflow actor for task");
        self.set_workflow_actor(msg.workflow_actor);
    }
}