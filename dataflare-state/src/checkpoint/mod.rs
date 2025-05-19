//! Checkpoint module
//!
//! This module provides functionality for creating and managing checkpoints.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use dataflare_core::error::{DataFlareError, Result};

/// Checkpoint state for tracking workflow progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointState {
    /// Workflow ID
    pub workflow_id: String,
    /// Checkpoint ID
    pub checkpoint_id: String,
    /// Timestamp when the checkpoint was created
    pub timestamp: u64,
    /// Source states
    pub source_states: HashMap<String, super::state::SourceState>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl CheckpointState {
    /// Create a new checkpoint state
    pub fn new(workflow_id: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Self {
            workflow_id: workflow_id.to_string(),
            checkpoint_id: format!("{}-{}", workflow_id, timestamp),
            timestamp,
            source_states: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
    
    /// Add a source state to the checkpoint
    pub fn add_source_state(&mut self, source_id: &str, state: super::state::SourceState) {
        self.source_states.insert(source_id.to_string(), state);
    }
    
    /// Get a source state from the checkpoint
    pub fn get_source_state(&self, source_id: &str) -> Option<&super::state::SourceState> {
        self.source_states.get(source_id)
    }
    
    /// Add metadata to the checkpoint
    pub fn add_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }
    
    /// Save the checkpoint to a file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| DataFlareError::Serialization(format!("Failed to serialize checkpoint: {}", e)))?;
        
        std::fs::write(path, json)
            .map_err(|e| DataFlareError::IO(format!("Failed to write checkpoint to file: {}", e)))?;
        
        Ok(())
    }
    
    /// Load a checkpoint from a file
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| DataFlareError::IO(format!("Failed to read checkpoint from file: {}", e)))?;
        
        let checkpoint: Self = serde_json::from_str(&json)
            .map_err(|e| DataFlareError::Serialization(format!("Failed to deserialize checkpoint: {}", e)))?;
        
        Ok(checkpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_checkpoint_state() {
        let mut checkpoint = CheckpointState::new("test-workflow");
        
        assert_eq!(checkpoint.workflow_id, "test-workflow");
        assert!(checkpoint.checkpoint_id.starts_with("test-workflow-"));
        assert!(checkpoint.source_states.is_empty());
        assert!(checkpoint.metadata.is_empty());
        
        let source_state = super::super::state::SourceState::new("test-source");
        checkpoint.add_source_state("test-source", source_state.clone());
        
        assert_eq!(checkpoint.source_states.len(), 1);
        assert_eq!(checkpoint.get_source_state("test-source").unwrap().source_id, "test-source");
        
        checkpoint.add_metadata("key", "value");
        assert_eq!(checkpoint.metadata.get("key").unwrap(), "value");
    }
    
    #[test]
    fn test_checkpoint_file_io() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("checkpoint.json");
        
        let mut checkpoint = CheckpointState::new("test-workflow");
        let source_state = super::super::state::SourceState::new("test-source");
        checkpoint.add_source_state("test-source", source_state);
        
        checkpoint.save_to_file(&file_path).unwrap();
        
        let loaded_checkpoint = CheckpointState::load_from_file(&file_path).unwrap();
        
        assert_eq!(loaded_checkpoint.workflow_id, checkpoint.workflow_id);
        assert_eq!(loaded_checkpoint.checkpoint_id, checkpoint.checkpoint_id);
        assert_eq!(loaded_checkpoint.source_states.len(), checkpoint.source_states.len());
    }
}
