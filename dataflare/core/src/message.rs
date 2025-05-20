//! Messages used in DataFlare communication
//!
//! This module defines message types for communication between DataFlare components.

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

use crate::data::{DataRecord, DataRecordBatch, Schema};
use crate::state::SourceState;

/// Workflow phase
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowPhase {
    /// Workflow is initializing
    Initializing,
    /// Workflow is extracting data
    Extracting,
    /// Workflow is transforming data
    Transforming,
    /// Workflow is loading data
    Loading,
    /// Workflow is finalizing
    Finalizing,
    /// Workflow has completed
    Completed,
    /// Workflow has encountered an error
    Error,
}

/// Workflow progress message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowProgress {
    /// Workflow ID
    pub workflow_id: String,
    /// Current phase
    pub phase: WorkflowPhase,
    /// Progress value (0.0 to 1.0)
    pub progress: f64,
    /// Progress message
    pub message: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

// ... rest of the file 