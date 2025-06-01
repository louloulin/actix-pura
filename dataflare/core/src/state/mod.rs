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
    
    /// Set the last timestamp
    pub fn with_timestamp(mut self, timestamp: &str) -> Self {
        self.last_timestamp = Some(timestamp.to_string());
        self.updated_at = Utc::now();
        self
    }
    
    /// Set the last position
    pub fn with_position(mut self, position: &str) -> Self {
        self.last_position = Some(position.to_string());
        self.updated_at = Utc::now();
        self
    }
    
    /// Set the last offset
    pub fn with_offset(mut self, offset: i64) -> Self {
        self.last_offset = Some(offset);
        self.updated_at = Utc::now();
        self
    }
    
    /// Set the last watermark
    pub fn with_watermark(mut self, watermark: &str) -> Self {
        self.last_watermark = Some(watermark.to_string());
        self.updated_at = Utc::now();
        self
    }
    
    /// Add state data
    pub fn add_data<K: Into<String>, V: Into<serde_json::Value>>(&mut self, key: K, value: V) -> &mut Self {
        self.data.insert(key.into(), value.into());
        self.updated_at = Utc::now();
        self
    }
}

/// Checkpoint state for tracking workflow progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointState {
    /// Workflow ID
    pub workflow_id: String,
    
    /// Checkpoint ID
    pub checkpoint_id: String,
    
    /// Timestamp when the checkpoint was created
    pub timestamp: DateTime<Utc>,
    
    /// Source states
    pub source_states: HashMap<String, SourceState>,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl CheckpointState {
    /// Create a new checkpoint state
    pub fn new(workflow_id: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            checkpoint_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            source_states: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
    
    /// Add a source state to the checkpoint
    pub fn add_source_state(&mut self, source_id: &str, state: SourceState) -> &mut Self {
        self.source_states.insert(source_id.to_string(), state);
        self
    }
    
    /// Get a source state from the checkpoint
    pub fn get_source_state(&self, source_id: &str) -> Option<&SourceState> {
        self.source_states.get(source_id)
    }
    
    /// Add metadata to the checkpoint
    pub fn add_metadata<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) -> &mut Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_source_state() {
        let mut state = SourceState::new("test-source")
            .with_timestamp("2023-01-01T00:00:00Z")
            .with_offset(100);
        
        state.add_data("key1", "value1");
        
        assert_eq!(state.source_id, "test-source");
        assert_eq!(state.last_timestamp, Some("2023-01-01T00:00:00Z".to_string()));
        assert_eq!(state.last_offset, Some(100));
        assert_eq!(state.data.get("key1").unwrap().as_str().unwrap(), "value1");
    }
    
    #[test]
    fn test_checkpoint_state() {
        let source_state = SourceState::new("test-source")
            .with_timestamp("2023-01-01T00:00:00Z");
        
        let mut checkpoint = CheckpointState::new("test-workflow");
        checkpoint.add_source_state("test-source", source_state);
        checkpoint.add_metadata("key1", "value1");
        
        assert_eq!(checkpoint.workflow_id, "test-workflow");
        assert_eq!(checkpoint.metadata.get("key1").unwrap(), "value1");
        
        let retrieved_state = checkpoint.get_source_state("test-source").unwrap();
        assert_eq!(retrieved_state.source_id, "test-source");
        assert_eq!(retrieved_state.last_timestamp, Some("2023-01-01T00:00:00Z".to_string()));
    }
}
