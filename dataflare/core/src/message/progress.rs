//! Workflow progress messages
//!
//! This module defines the workflow progress messages used to track the execution of workflows.

use serde::{Deserialize, Serialize};

/// Workflow progress events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowProgress {
    /// Workflow started
    Started {
        /// Workflow ID
        workflow_id: String,
    },
    
    /// Source started
    SourceStarted {
        /// Source ID
        source_id: String,
        /// Workflow ID
        workflow_id: String,
    },
    
    /// Source completed
    SourceCompleted {
        /// Source ID
        source_id: String,
        /// Workflow ID
        workflow_id: String,
        /// Number of records processed
        records_processed: usize,
    },
    
    /// Transformation started
    TransformationStarted {
        /// Transformation ID
        transformation_id: String,
        /// Workflow ID
        workflow_id: String,
    },
    
    /// Transformation completed
    TransformationCompleted {
        /// Transformation ID
        transformation_id: String,
        /// Workflow ID
        workflow_id: String,
        /// Number of records processed
        records_processed: usize,
    },
    
    /// Destination started
    DestinationStarted {
        /// Destination ID
        destination_id: String,
        /// Workflow ID
        workflow_id: String,
    },
    
    /// Destination completed
    DestinationCompleted {
        /// Destination ID
        destination_id: String,
        /// Workflow ID
        workflow_id: String,
        /// Number of records processed
        records_processed: usize,
    },
    
    /// Workflow completed
    Completed {
        /// Workflow ID
        workflow_id: String,
        /// Duration in milliseconds
        duration_ms: u64,
    },
    
    /// Workflow failed
    Failed {
        /// Workflow ID
        workflow_id: String,
        /// Error message
        error: String,
    },
}
