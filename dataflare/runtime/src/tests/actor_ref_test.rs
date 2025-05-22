//! Integration tests for the actor reference system
//!
//! Tests the new actor reference system and flattened architecture.

use actix::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde_json::json;

use dataflare_core::error::{DataFlareError, Result};
use crate::actor::{
    ActorRef, ActorRegistry, MessageRouter,
    Initialize, Finalize, GetStatus, ActorStatus,
    ClusterActor, ClusterConfig, DeployWorkflow, StopWorkflow, GetWorkflowStatus
};
use crate::actor::actor_ref::ActorHandler;
use crate::actor::cluster::WorkflowStatus;

/// Test message
#[derive(Message)]
#[rtype(result = "Result<String>")]
struct TestMessage {
    content: String,
}

/// Test actor
struct TestActor {
    id: String,
    data: Arc<Mutex<Vec<String>>>,
}

impl TestActor {
    fn new(id: String, data: Arc<Mutex<Vec<String>>>) -> Self {
        Self { id, data }
    }
}

impl Actor for TestActor {
    type Context = Context<Self>;
}

impl Handler<TestMessage> for TestActor {
    type Result = Result<String>;

    fn handle(&mut self, msg: TestMessage, _: &mut Context<Self>) -> Self::Result {
        // Store the message
        self.data.lock().unwrap().push(msg.content.clone());

        // Return a response
        Ok(format!("Processed: {}", msg.content))
    }
}

/// Integration test for the actor reference system
#[actix::test]
async fn test_actor_reference_system() {
    // Setup a shared data store to verify processing
    let shared_data = Arc::new(Mutex::new(Vec::<String>::new()));

    // Create a test actor
    let test_actor = TestActor::new("test-actor".to_string(), shared_data.clone());
    let test_addr = test_actor.start();

    // Create an actor reference
    let actor_ref = ActorRef::<TestMessage>::new("test-actor".to_string(), test_addr);

    // Send a message via actor reference
    let result = actor_ref.send(TestMessage { content: "Hello from actor ref".to_string() }).await.unwrap();
    assert_eq!(result.unwrap(), "Processed: Hello from actor ref");

    // Verify that the message was processed
    let data = shared_data.lock().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0], "Hello from actor ref");
}

/// Integration test for the actor registry and message router
#[actix::test]
async fn test_registry_and_router() {
    // Setup a shared data store
    let shared_data = Arc::new(Mutex::new(Vec::<String>::new()));

    // Create actors
    let actor1 = TestActor::new("actor1".to_string(), shared_data.clone());
    let addr1 = actor1.start();

    let actor2 = TestActor::new("actor2".to_string(), shared_data.clone());
    let addr2 = actor2.start();

    // Create actor references
    let ref1 = ActorRef::<TestMessage>::new("actor1".to_string(), addr1);
    let ref2 = ActorRef::<TestMessage>::new("actor2".to_string(), addr2);

    // Create registry
    let mut registry = ActorRegistry::new();
    registry.register(ref1);
    registry.register(ref2);

    // Create router
    let router = MessageRouter::new(Arc::new(registry));

    // Route messages to both actors
    let result1 = router.route("actor1", TestMessage { content: "Message for actor1".to_string() }).await.unwrap();
    let result2 = router.route("actor2", TestMessage { content: "Message for actor2".to_string() }).await.unwrap();

    assert_eq!(result1.unwrap(), "Processed: Message for actor1");
    assert_eq!(result2.unwrap(), "Processed: Message for actor2");

    // Verify that both messages were processed
    let data = shared_data.lock().unwrap();
    assert_eq!(data.len(), 2);
    assert_eq!(data[0], "Message for actor1");
    assert_eq!(data[1], "Message for actor2");
}

/// Integration test for the ClusterActor and flattened architecture
#[actix::test]
async fn test_cluster_actor() {
    // Create a ClusterActor with default config
    let config = ClusterConfig::default();
    let cluster_actor = ClusterActor::new(config.clone());
    let cluster_addr = cluster_actor.start();

    // Deploy a test workflow
    let workflow_id = "test-workflow".to_string();
    let workflow_config = json!({
        "name": "Test Workflow",
        "description": "Test workflow for integration test",
        "version": "1.0"
    });

    let result = cluster_addr.send(DeployWorkflow {
        id: workflow_id.clone(),
        config: workflow_config,
        node_id: None
    }).await.unwrap();

    // Verify deployment was successful
    assert!(result.is_ok());

    // Wait for workflow to initialize
    actix_rt::time::sleep(Duration::from_millis(100)).await;

    // Check workflow status
    let status = cluster_addr.send(GetWorkflowStatus { id: workflow_id.clone() }).await.unwrap().unwrap();
    assert!(matches!(status, WorkflowStatus::Initialized));

    // Stop the workflow
    let result = cluster_addr.send(StopWorkflow { id: workflow_id.clone() }).await.unwrap();
    assert!(result.is_ok());

    // Wait for workflow to stop
    actix_rt::time::sleep(Duration::from_millis(100)).await;

    // Check workflow status again
    let status = cluster_addr.send(GetWorkflowStatus { id: workflow_id }).await.unwrap().unwrap();
    assert!(matches!(status, WorkflowStatus::Stopped));
}