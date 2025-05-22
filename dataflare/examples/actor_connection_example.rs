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
    SendBatch, TaskKind, TaskCompleted, StartExtraction
};

// Create a mock source connector
struct MockSourceConnector;

impl dataflare_connector::source::SourceConnector for MockSourceConnector {
    fn configure(&mut self, _config: &serde_json::Value) -> Result<()> {
        // Mock implementation
        Ok(())
    }
    
    fn read_batch(&mut self, batch_size: usize) -> Box<dyn std::future::Future<Output = Result<DataRecordBatch>> + Unpin + '_> {
        // Create a mock batch of data
        let records = vec![
            dataflare_core::message::DataRecord::new("record1"),
            dataflare_core::message::DataRecord::new("record2"),
        ];
        let batch = DataRecordBatch::new(records);
        
        Box::new(futures::future::ready(Ok(batch)))
    }
    
    fn get_schema(&self) -> Result<serde_json::Value> {
        // Mock implementation
        Ok(serde_json::json!({
            "fields": [
                {"name": "id", "type": "string"}
            ]
        }))
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
        println!("Setting workflow actor for task {}", self.id);
        self.set_workflow_actor(msg.workflow_actor);
    }
} 