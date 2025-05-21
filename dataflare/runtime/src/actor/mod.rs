//! Actor system for DataFlare
//!
//! This module provides the actor-based runtime for DataFlare.

// Module declarations
pub mod source;
pub mod processor;
pub mod destination;
pub mod workflow;
pub mod supervisor;
pub mod message_bus;
pub mod message_system;
pub mod pool;
pub mod router;
pub mod actor_ref;
pub mod cluster;
pub mod task;

// Re-exports
pub use source::SourceActor;
pub use processor::ProcessorActor;
pub use destination::DestinationActor;
pub use workflow::WorkflowActor;
pub use supervisor::SupervisorActor;
pub use message_bus::MessageBus;
pub use pool::ActorPool;
pub use actor_ref::{ActorRef, ActorRegistry, MessageRouter};
pub use cluster::{ClusterActor, ClusterConfig, RegisterNode, UnregisterNode, 
                 Heartbeat, DeployWorkflow, StopWorkflow};
pub use task::{TaskActor, TaskKind, TaskState, TaskStats, ProcessBatch,
              GetTaskState, SetTaskState, GetTaskStats};

use actix::prelude::*;
use dataflare_core::error::Result;
use dataflare_core::message::{WorkflowPhase, DataRecordBatch, WorkflowProgress};
use serde_json::Value;

/// Actor status
#[derive(Debug, Clone, PartialEq)]
pub enum ActorStatus {
    /// 初始化状态
    Initialized,
    /// 运行中
    Running,
    /// 已暂停
    Paused,
    /// 已停止
    Stopped,
    /// 已完成（最终状态）
    Finalized,
    /// 错误状态
    Error(String),
}

/// Message to initialize an actor
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Initialize {
    /// ID of the workflow
    pub workflow_id: String,
    /// Configuration values
    pub config: Value,
}

/// Message to finalize an actor
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Finalize {
    /// ID of the workflow
    pub workflow_id: String,
}

/// Message to get actor status
#[derive(Message)]
#[rtype(result = "Result<ActorStatus>")]
pub struct GetStatus;

/// Message to pause an actor
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Pause {
    /// Workflow ID
    pub workflow_id: String,
}

/// Message to resume an actor
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Resume {
    /// Workflow ID
    pub workflow_id: String,
}

/// Message to send a batch of data
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct SendBatch {
    /// Workflow ID
    pub workflow_id: String,
    /// Batch of data
    pub batch: DataRecordBatch,
}

/// Message to subscribe to progress updates
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct SubscribeProgress {
    /// Workflow ID
    pub workflow_id: String,
    /// Progress update recipient
    pub recipient: Recipient<WorkflowProgress>,
}

/// Message to unsubscribe from progress updates
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct UnsubscribeProgress {
    /// Workflow ID
    pub workflow_id: String,
    /// Recipient to cancel
    pub recipient: Recipient<WorkflowProgress>,
}

/// Message to subscribe to progress updates (alias for SubscribeProgress)
pub type SubscribeToProgress = SubscribeProgress;

/// Message to unsubscribe from progress updates (alias for UnsubscribeProgress)
pub type UnsubscribeFromProgress = UnsubscribeProgress;

/// Common trait for all DataFlare actors
pub trait DataFlareActor: Actor {
    /// Get the actor ID
    fn get_id(&self) -> &str;
    
    /// Get the actor type
    fn get_type(&self) -> &str;
    
    /// Initialize the actor
    fn initialize(&mut self, ctx: &mut Self::Context) -> Result<()>;
    
    /// Finalize the actor
    fn finalize(&mut self, ctx: &mut Self::Context) -> Result<()>;
    
    /// Report progress
    fn report_progress(&self, workflow_id: &str, phase: WorkflowPhase, progress: f64, message: &str);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test implementation of a DataFlare actor
    struct TestActor {
        id: String,
        actor_type: String,
    }

    impl Actor for TestActor {
        type Context = Context<Self>;
    }

    impl DataFlareActor for TestActor {
        fn get_id(&self) -> &str {
            &self.id
        }

        fn get_type(&self) -> &str {
            &self.actor_type
        }

        fn initialize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
            Ok(())
        }

        fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
            Ok(())
        }

        fn report_progress(&self, _workflow_id: &str, _phase: dataflare_core::message::WorkflowPhase, _progress: f64, _message: &str) {
            // Does nothing in the test
        }
    }

    #[test]
    fn test_actor_trait() {
        let actor = TestActor {
            id: "test-actor".to_string(),
            actor_type: "test".to_string(),
        };

        assert_eq!(actor.get_id(), "test-actor");
        assert_eq!(actor.get_type(), "test");
    }
}

// Re-exports
// pub use message_system::MessageSystem;
// pub use router::ActorRouter;
