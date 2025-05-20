//! # Workflow Engine
//!
//! This module provides a new workflow engine implementation that uses the
//! improved actor message system and follows the dependency inversion principle.

use crate::actor::{
    ActorId, ActorRole, MessageEnvelope, MessagePayload, ActorCommand,
    ActorQuery, ActorResponse, MessageRouter, NewActorStatus,
};
use actix::prelude::*;
use dataflare_core::{
    DataFlareError, DataRecord, DataRecordBatch, Result,
    WorkflowComponent, DataReader, DataWriter, DataProcessor,
    config::DataFlareConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Workflow ID
    pub id: String,
    /// Workflow name
    pub name: String,
    /// Workflow description
    pub description: Option<String>,
    /// Source component configuration
    pub source: ComponentConfig,
    /// Processor component configurations
    pub processors: Vec<ComponentConfig>,
    /// Destination component configuration
    pub destination: ComponentConfig,
    /// Workflow configuration
    pub config: serde_json::Value,
}

/// Component configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentConfig {
    /// Component ID
    pub id: String,
    /// Component type
    pub component_type: String,
    /// Component configuration
    pub config: serde_json::Value,
}

/// Workflow execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Synchronous execution
    Synchronous,
    /// Asynchronous execution
    Asynchronous,
    /// Streaming execution
    Streaming,
}

/// Workflow execution status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    /// Workflow is initializing
    Initializing,
    /// Workflow is ready to run
    Ready,
    /// Workflow is running
    Running,
    /// Workflow is paused
    Paused,
    /// Workflow is completed
    Completed,
    /// Workflow has failed
    Failed(String),
}

/// Workflow execution statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowStats {
    /// Number of records processed
    pub records_processed: u64,
    /// Number of records failed
    pub records_failed: u64,
    /// Number of batches processed
    pub batches_processed: u64,
    /// Start time
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    /// End time
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Component statistics
    pub component_stats: HashMap<String, ComponentStats>,
}

/// Component execution statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentStats {
    /// Number of records processed
    pub records_processed: u64,
    /// Number of records failed
    pub records_failed: u64,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    /// Component status
    pub status: String,
}

/// Workflow execution progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowProgress {
    /// Workflow ID
    pub workflow_id: String,
    /// Workflow status
    pub status: WorkflowStatus,
    /// Progress percentage (0.0 - 1.0)
    pub progress: f64,
    /// Current phase
    pub phase: String,
    /// Current component ID
    pub current_component: Option<String>,
    /// Message
    pub message: Option<String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Workflow execution options
#[derive(Debug, Clone)]
pub struct WorkflowOptions {
    /// Execution mode
    pub mode: ExecutionMode,
    /// Batch size
    pub batch_size: usize,
    /// Maximum concurrent components
    pub max_concurrency: usize,
    /// Timeout in seconds
    pub timeout_seconds: u64,
    /// Whether to collect metrics
    pub collect_metrics: bool,
}

impl Default for WorkflowOptions {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::Streaming,
            batch_size: 1000,
            max_concurrency: 4,
            timeout_seconds: 3600,
            collect_metrics: true,
        }
    }
}

/// Workflow engine
pub struct WorkflowEngine {
    /// System configuration
    config: DataFlareConfig,
    /// Message router
    router: MessageRouter,
    /// Active workflows
    workflows: HashMap<String, WorkflowContext>,
    /// Component registry
    component_registry: HashMap<String, Box<dyn WorkflowComponent>>,
}

/// Workflow execution context
struct WorkflowContext {
    /// Workflow definition
    definition: WorkflowDefinition,
    /// Workflow status
    status: WorkflowStatus,
    /// Workflow statistics
    stats: WorkflowStats,
    /// Workflow options
    options: WorkflowOptions,
    /// Actor IDs
    actor_ids: HashMap<String, ActorId>,
    /// Progress channel
    progress_tx: mpsc::Sender<WorkflowProgress>,
    /// Progress receiver
    progress_rx: mpsc::Receiver<WorkflowProgress>,
}

impl WorkflowEngine {
    /// Create a new workflow engine
    pub fn new(config: DataFlareConfig) -> Self {
        Self {
            config,
            router: MessageRouter::new(),
            workflows: HashMap::new(),
            component_registry: HashMap::new(),
        }
    }
    
    /// Register a component
    pub fn register_component<T: WorkflowComponent + 'static>(&mut self, component: T) {
        let component_type = component.get_type().to_string();
        self.component_registry.insert(component_type, Box::new(component));
    }
    
    /// Create a workflow
    pub fn create_workflow(&mut self, definition: WorkflowDefinition, options: WorkflowOptions) -> Result<()> {
        if self.workflows.contains_key(&definition.id) {
            return Err(DataFlareError::Workflow(format!(
                "Workflow with ID {} already exists", definition.id
            )));
        }
        
        // Create progress channels
        let (progress_tx, progress_rx) = mpsc::channel(100);
        
        // Create workflow context
        let context = WorkflowContext {
            definition: definition.clone(),
            status: WorkflowStatus::Initializing,
            stats: WorkflowStats::default(),
            options,
            actor_ids: HashMap::new(),
            progress_tx,
            progress_rx,
        };
        
        self.workflows.insert(definition.id.clone(), context);
        
        Ok(())
    }
    
    /// Initialize a workflow
    pub async fn initialize_workflow(&mut self, workflow_id: &str) -> Result<()> {
        let context = self.workflows.get_mut(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        // Create actors for each component
        self.create_source_actor(workflow_id, &context.definition.source).await?;
        
        for processor_config in &context.definition.processors {
            self.create_processor_actor(workflow_id, processor_config).await?;
        }
        
        self.create_destination_actor(workflow_id, &context.definition.destination).await?;
        
        // Update workflow status
        context.status = WorkflowStatus::Ready;
        
        // Send progress update
        let progress = WorkflowProgress {
            workflow_id: workflow_id.to_string(),
            status: WorkflowStatus::Ready,
            progress: 0.0,
            phase: "initialized".to_string(),
            current_component: None,
            message: Some("Workflow initialized".to_string()),
            timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = context.progress_tx.send(progress).await {
            log::warn!("Failed to send progress update: {}", e);
        }
        
        Ok(())
    }
    
    /// Start a workflow
    pub async fn start_workflow(&mut self, workflow_id: &str) -> Result<()> {
        let context = self.workflows.get_mut(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        if context.status != WorkflowStatus::Ready && context.status != WorkflowStatus::Paused {
            return Err(DataFlareError::Workflow(format!(
                "Workflow {} is not in a startable state: {:?}", workflow_id, context.status
            )));
        }
        
        // Update workflow status
        context.status = WorkflowStatus::Running;
        context.stats.start_time = Some(chrono::Utc::now());
        
        // Send start command to source actor
        let source_id = context.actor_ids.get(&context.definition.source.id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Source actor for workflow {} not found", workflow_id))
        })?;
        
        let msg = MessageEnvelope::new(
            ActorId::new("workflow_engine"),
            source_id.clone(),
            MessagePayload::Command(ActorCommand::Start),
        );
        
        self.router.send::<crate::actor::NewDataFlareActor<_>>(msg).await?;
        
        // Send progress update
        let progress = WorkflowProgress {
            workflow_id: workflow_id.to_string(),
            status: WorkflowStatus::Running,
            progress: 0.0,
            phase: "started".to_string(),
            current_component: Some(context.definition.source.id.clone()),
            message: Some("Workflow started".to_string()),
            timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = context.progress_tx.send(progress).await {
            log::warn!("Failed to send progress update: {}", e);
        }
        
        Ok(())
    }
    
    /// Pause a workflow
    pub async fn pause_workflow(&mut self, workflow_id: &str) -> Result<()> {
        let context = self.workflows.get_mut(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        if context.status != WorkflowStatus::Running {
            return Err(DataFlareError::Workflow(format!(
                "Workflow {} is not running", workflow_id
            )));
        }
        
        // Update workflow status
        context.status = WorkflowStatus::Paused;
        
        // Send pause command to all actors
        for (component_id, actor_id) in &context.actor_ids {
            let msg = MessageEnvelope::new(
                ActorId::new("workflow_engine"),
                actor_id.clone(),
                MessagePayload::Command(ActorCommand::Pause),
            );
            
            self.router.send::<crate::actor::NewDataFlareActor<_>>(msg).await?;
        }
        
        // Send progress update
        let progress = WorkflowProgress {
            workflow_id: workflow_id.to_string(),
            status: WorkflowStatus::Paused,
            progress: 0.0,
            phase: "paused".to_string(),
            current_component: None,
            message: Some("Workflow paused".to_string()),
            timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = context.progress_tx.send(progress).await {
            log::warn!("Failed to send progress update: {}", e);
        }
        
        Ok(())
    }
    
    /// Stop a workflow
    pub async fn stop_workflow(&mut self, workflow_id: &str) -> Result<()> {
        let context = self.workflows.get_mut(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        if context.status == WorkflowStatus::Completed || 
           matches!(context.status, WorkflowStatus::Failed(_)) {
            return Err(DataFlareError::Workflow(format!(
                "Workflow {} is already stopped", workflow_id
            )));
        }
        
        // Send stop command to all actors
        for (component_id, actor_id) in &context.actor_ids {
            let msg = MessageEnvelope::new(
                ActorId::new("workflow_engine"),
                actor_id.clone(),
                MessagePayload::Command(ActorCommand::Stop),
            );
            
            self.router.send::<crate::actor::NewDataFlareActor<_>>(msg).await?;
        }
        
        // Update workflow status
        context.status = WorkflowStatus::Completed;
        context.stats.end_time = Some(chrono::Utc::now());
        
        // Send progress update
        let progress = WorkflowProgress {
            workflow_id: workflow_id.to_string(),
            status: WorkflowStatus::Completed,
            progress: 1.0,
            phase: "completed".to_string(),
            current_component: None,
            message: Some("Workflow completed".to_string()),
            timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = context.progress_tx.send(progress).await {
            log::warn!("Failed to send progress update: {}", e);
        }
        
        Ok(())
    }
    
    /// Get workflow status
    pub fn get_workflow_status(&self, workflow_id: &str) -> Result<WorkflowStatus> {
        let context = self.workflows.get(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        Ok(context.status.clone())
    }
    
    /// Get workflow statistics
    pub fn get_workflow_stats(&self, workflow_id: &str) -> Result<WorkflowStats> {
        let context = self.workflows.get(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        Ok(context.stats.clone())
    }
    
    /// Get workflow progress receiver
    pub fn get_progress_receiver(&mut self, workflow_id: &str) -> Result<mpsc::Receiver<WorkflowProgress>> {
        let context = self.workflows.get_mut(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        // Create a new channel
        let (tx, rx) = mpsc::channel(100);
        context.progress_tx = tx;
        
        Ok(rx)
    }
    
    // Private methods
    
    /// Create a source actor
    async fn create_source_actor(&mut self, workflow_id: &str, config: &ComponentConfig) -> Result<()> {
        let context = self.workflows.get_mut(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        // Create actor ID
        let actor_id = ActorId::new(format!("{}_source_{}", workflow_id, config.id));
        
        // TODO: Create and register the actor
        
        // Store actor ID
        context.actor_ids.insert(config.id.clone(), actor_id);
        
        Ok(())
    }
    
    /// Create a processor actor
    async fn create_processor_actor(&mut self, workflow_id: &str, config: &ComponentConfig) -> Result<()> {
        let context = self.workflows.get_mut(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        // Create actor ID
        let actor_id = ActorId::new(format!("{}_processor_{}", workflow_id, config.id));
        
        // TODO: Create and register the actor
        
        // Store actor ID
        context.actor_ids.insert(config.id.clone(), actor_id);
        
        Ok(())
    }
    
    /// Create a destination actor
    async fn create_destination_actor(&mut self, workflow_id: &str, config: &ComponentConfig) -> Result<()> {
        let context = self.workflows.get_mut(workflow_id).ok_or_else(|| {
            DataFlareError::Workflow(format!("Workflow with ID {} not found", workflow_id))
        })?;
        
        // Create actor ID
        let actor_id = ActorId::new(format!("{}_destination_{}", workflow_id, config.id));
        
        // TODO: Create and register the actor
        
        // Store actor ID
        context.actor_ids.insert(config.id.clone(), actor_id);
        
        Ok(())
    }
}
