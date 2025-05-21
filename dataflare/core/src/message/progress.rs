//! Workflow progress messages
//!
//! This module defines the workflow progress messages used to track the execution of workflows.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Workflow phase
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowPhase {
    /// Workflow is initializing
    Initializing,
    /// Workflow is extracting data
    Extracting,
    /// Workflow is transforming data
    Transforming,
    /// Workflow is loading data
    Loading,
    /// Workflow is processing data
    Processing,
    /// Workflow is finalizing
    Finalizing,
    /// Workflow completed successfully
    Completed,
    /// Workflow encountered an error
    Error,
}

/// Workflow progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowProgress {
    /// Workflow ID
    pub workflow_id: String,
    /// Current workflow phase
    pub phase: WorkflowPhase,
    /// Progress value (0.0 to 1.0)
    pub progress: f64,
    /// Progress message
    pub message: String,
    /// Timestamp of the progress update
    pub timestamp: DateTime<Utc>,
    /// Source ID (if applicable)
    pub source_id: Option<String>,
    /// Transformation ID (if applicable)
    pub transformation_id: Option<String>,
    /// Destination ID (if applicable)
    pub destination_id: Option<String>,
    /// Task ID (if applicable)
    pub task_id: Option<String>,
    /// Task name (if applicable)
    pub task_name: Option<String>,
    /// Number of records processed (if applicable)
    pub records_processed: Option<usize>,
    /// Duration in milliseconds (if completed)
    pub duration_ms: Option<u64>,
    /// Error message (if failed)
    pub error: Option<String>,
}

impl WorkflowProgress {
    /// Create a new workflow started progress
    pub fn started(workflow_id: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            phase: WorkflowPhase::Initializing,
            progress: 0.0,
            message: format!("Started workflow {}", workflow_id),
            timestamp: Utc::now(),
            source_id: None,
            transformation_id: None,
            destination_id: None,
            task_id: None,
            task_name: None,
            records_processed: None,
            duration_ms: None,
            error: None,
        }
    }
    
    /// Create a new workflow completed progress
    pub fn completed(workflow_id: &str, duration_ms: u64) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            phase: WorkflowPhase::Completed,
            progress: 1.0,
            message: format!("Completed workflow {}", workflow_id),
            timestamp: Utc::now(),
            source_id: None,
            transformation_id: None,
            destination_id: None,
            task_id: None,
            task_name: None,
            records_processed: None,
            duration_ms: Some(duration_ms),
            error: None,
        }
    }
    
    /// Create a new workflow failed progress
    pub fn failed(workflow_id: &str, error_msg: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            phase: WorkflowPhase::Error,
            progress: 0.0,
            message: format!("Failed workflow {}: {}", workflow_id, error_msg),
            timestamp: Utc::now(),
            source_id: None,
            transformation_id: None,
            destination_id: None,
            task_id: None,
            task_name: None,
            records_processed: None,
            duration_ms: None,
            error: Some(error_msg.to_string()),
        }
    }
    
    /// Create a new source started progress
    pub fn source_started(workflow_id: &str, source_id: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            phase: WorkflowPhase::Extracting,
            progress: 0.0,
            message: format!("Started source {}", source_id),
            timestamp: Utc::now(),
            source_id: Some(source_id.to_string()),
            transformation_id: None,
            destination_id: None,
            task_id: None,
            task_name: None,
            records_processed: None,
            duration_ms: None,
            error: None,
        }
    }
    
    /// Create a new source completed progress
    pub fn source_completed(workflow_id: &str, source_id: &str, records: usize) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            phase: WorkflowPhase::Extracting,
            progress: 1.0,
            message: format!("Completed source {} with {} records", source_id, records),
            timestamp: Utc::now(),
            source_id: Some(source_id.to_string()),
            transformation_id: None,
            destination_id: None,
            task_id: None,
            task_name: None,
            records_processed: Some(records),
            duration_ms: None,
            error: None,
        }
    }
    
    /// Create a new transformation started progress
    pub fn transformation_started(workflow_id: &str, transformation_id: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            phase: WorkflowPhase::Transforming,
            progress: 0.0,
            message: format!("Started transformation {}", transformation_id),
            timestamp: Utc::now(),
            source_id: None,
            transformation_id: Some(transformation_id.to_string()),
            destination_id: None,
            task_id: None,
            task_name: None,
            records_processed: None,
            duration_ms: None,
            error: None,
        }
    }
    
    /// Create a new transformation completed progress
    pub fn transformation_completed(workflow_id: &str, transformation_id: &str, records: usize) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            phase: WorkflowPhase::Transforming,
            progress: 1.0,
            message: format!("Completed transformation {} with {} records", transformation_id, records),
            timestamp: Utc::now(),
            source_id: None,
            transformation_id: Some(transformation_id.to_string()),
            destination_id: None,
            task_id: None,
            task_name: None,
            records_processed: Some(records),
            duration_ms: None,
            error: None,
        }
    }
    
    /// Create a new destination started progress
    pub fn destination_started(workflow_id: &str, destination_id: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            phase: WorkflowPhase::Loading,
            progress: 0.0,
            message: format!("Started destination {}", destination_id),
            timestamp: Utc::now(),
            source_id: None,
            transformation_id: None,
            destination_id: Some(destination_id.to_string()),
            task_id: None,
            task_name: None,
            records_processed: None,
            duration_ms: None,
            error: None,
        }
    }
    
    /// Create a new destination completed progress
    pub fn destination_completed(workflow_id: &str, destination_id: &str, records: usize) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            phase: WorkflowPhase::Loading,
            progress: 1.0,
            message: format!("Completed destination {} with {} records", destination_id, records),
            timestamp: Utc::now(),
            source_id: None,
            transformation_id: None,
            destination_id: Some(destination_id.to_string()),
            task_id: None,
            task_name: None,
            records_processed: Some(records),
            duration_ms: None,
            error: None,
        }
    }
}
