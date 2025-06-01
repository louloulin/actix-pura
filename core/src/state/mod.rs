//! State module
//!
//! This module provides functionality for tracking and managing state.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Source state for tracking incremental extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceState {
    /// Source ID
    pub source_id: String,
    
    /// Last extracted timestamp
    pub last_timestamp: Option<String>,
    
    /// Last extracted position
    pub last_position: Option<String>,
    
    /// Last extracted offset
    pub last_offset: Option<i64>,
    
    /// Last extracted watermark
    pub last_watermark: Option<String>,
    
    /// Additional state data
    pub data: HashMap<String, serde_json::Value>,
    
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl SourceState {
    /// Create a new source state
    pub fn new(source_id: &str) -> Self {
        Self {
            source_id: source_id.to_string(),
            last_timestamp: None,
            last_position: None,
            last_offset: None,
            last_watermark: None,
            data: HashMap::new(),
            updated_at: Utc::now(),
        }
    }
    
    /// Create an empty source state with a placeholder ID
    pub fn empty() -> Self {
        Self {
            source_id: "empty".to_string(),
            last_timestamp: None,
            last_position: None,
            last_offset: None,
            last_watermark: None,
            data: HashMap::new(),
            updated_at: Utc::now(),
        }
    }
    
    // ... existing code ...
} 