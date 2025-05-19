//! State manager module
//!
//! This module provides functionality for managing state across a workflow.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use dataflare_core::error::Result;

use crate::state::SourceState;
use crate::checkpoint::CheckpointState;
use crate::storage::{StateStorage, StateStorageExt};

/// State manager for tracking and persisting workflow state
pub struct StateManager {
    /// Workflow ID
    workflow_id: String,
    /// State storage
    storage: Arc<dyn StateStorage>,
    /// Current source states
    source_states: RwLock<HashMap<String, SourceState>>,
    /// Checkpoint directory
    checkpoint_dir: Option<PathBuf>,
}

impl StateManager {
    /// Create a new state manager
    pub fn new(workflow_id: &str, storage: Arc<dyn StateStorage>) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            storage,
            source_states: RwLock::new(HashMap::new()),
            checkpoint_dir: None,
        }
    }

    /// Create a new state manager with a checkpoint directory
    pub fn with_checkpoint_dir(workflow_id: &str, storage: Arc<dyn StateStorage>, checkpoint_dir: PathBuf) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            storage,
            source_states: RwLock::new(HashMap::new()),
            checkpoint_dir: Some(checkpoint_dir),
        }
    }

    /// Get a source state
    pub async fn get_source_state(&self, source_id: &str) -> Result<Option<SourceState>> {
        let states = self.source_states.read().await;
        if let Some(state) = states.get(source_id) {
            return Ok(Some(state.clone()));
        }

        // Try to load from storage
        let key = format!("{}.source.{}", self.workflow_id, source_id);
        self.storage.load(&key).await
    }

    /// Set a source state
    pub async fn set_source_state(&self, source_id: &str, state: SourceState) -> Result<()> {
        // Save to storage
        let key = format!("{}.source.{}", self.workflow_id, source_id);
        self.storage.save(&key, &state).await?;

        // Update in-memory state
        let mut states = self.source_states.write().await;
        states.insert(source_id.to_string(), state);

        Ok(())
    }

    /// Create a checkpoint
    pub async fn create_checkpoint(&self, metadata: HashMap<String, String>) -> Result<CheckpointState> {
        let mut checkpoint = CheckpointState::new(&self.workflow_id);

        // Add metadata
        for (key, value) in metadata {
            checkpoint.add_metadata(key, value);
        }

        // Add source states
        let states = self.source_states.read().await;
        for (source_id, state) in states.iter() {
            checkpoint.add_source_state(source_id, state.clone());
        }

        // Save checkpoint to storage
        let key = format!("{}.checkpoint.{}", self.workflow_id, checkpoint.checkpoint_id);
        self.storage.save(&key, &checkpoint).await?;

        // Save to file if checkpoint directory is set
        if let Some(dir) = &self.checkpoint_dir {
            let file_path = dir.join(format!("{}.json", checkpoint.checkpoint_id));
            tokio::fs::create_dir_all(dir).await?;
            let json = serde_json::to_string_pretty(&checkpoint)?;
            tokio::fs::write(file_path, json).await?;
        }

        Ok(checkpoint)
    }

    /// Load a checkpoint
    pub async fn load_checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointState>> {
        // Try to load from storage
        let key = format!("{}.checkpoint.{}", self.workflow_id, checkpoint_id);
        let checkpoint: Option<CheckpointState> = self.storage.load(&key).await?;

        // If found, update source states
        if let Some(checkpoint) = &checkpoint {
            let mut states = self.source_states.write().await;
            for (source_id, state) in &checkpoint.source_states {
                states.insert(source_id.clone(), state.clone());
            }
        }

        Ok(checkpoint)
    }

    /// List all checkpoints
    pub async fn list_checkpoints(&self) -> Result<Vec<String>> {
        let prefix = format!("{}.checkpoint.", self.workflow_id);
        let keys = self.storage.list_keys().await?;

        let checkpoint_ids = keys.iter()
            .filter(|key| key.starts_with(&prefix))
            .map(|key| key[prefix.len()..].to_string())
            .collect();

        Ok(checkpoint_ids)
    }

    /// Delete a checkpoint
    pub async fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let key = format!("{}.checkpoint.{}", self.workflow_id, checkpoint_id);
        self.storage.delete(&key).await?;

        // Delete file if checkpoint directory is set
        if let Some(dir) = &self.checkpoint_dir {
            let file_path = dir.join(format!("{}.json", checkpoint_id));
            if file_path.exists() {
                tokio::fs::remove_file(file_path).await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStateStorage;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_state_manager() {
        let storage = Arc::new(MemoryStateStorage::new());
        let manager = StateManager::new("test-workflow", storage);

        // Test source state
        let source_state = SourceState::new()
            .with_source_name("test-source")
            .with_extraction_mode("full");
        manager.set_source_state("test-source", source_state.clone()).await.unwrap();

        let retrieved_state = manager.get_source_state("test-source").await.unwrap().unwrap();
        assert_eq!(retrieved_state.source_name, Some("test-source".to_string()));

        // Test checkpoint
        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());

        let checkpoint = manager.create_checkpoint(metadata).await.unwrap();
        assert_eq!(checkpoint.workflow_id, "test-workflow");
        assert_eq!(checkpoint.metadata.get("key1").unwrap(), "value1");

        let checkpoint_id = checkpoint.checkpoint_id.clone();
        let retrieved_checkpoint = manager.load_checkpoint(&checkpoint_id).await.unwrap().unwrap();
        assert_eq!(retrieved_checkpoint.checkpoint_id, checkpoint_id);

        let checkpoints = manager.list_checkpoints().await.unwrap();
        assert_eq!(checkpoints.len(), 1);

        manager.delete_checkpoint(&checkpoint_id).await.unwrap();
        let checkpoints = manager.list_checkpoints().await.unwrap();
        assert_eq!(checkpoints.len(), 0);
    }

    #[tokio::test]
    async fn test_state_manager_with_checkpoint_dir() {
        let dir = tempdir().unwrap();
        let storage = Arc::new(MemoryStateStorage::new());
        let manager = StateManager::with_checkpoint_dir(
            "test-workflow",
            storage,
            dir.path().to_path_buf()
        );

        // Create a checkpoint
        let checkpoint = manager.create_checkpoint(HashMap::new()).await.unwrap();
        let checkpoint_id = checkpoint.checkpoint_id.clone();

        // Check that the file was created
        let file_path = dir.path().join(format!("{}.json", checkpoint_id));
        assert!(file_path.exists());

        // Delete the checkpoint
        manager.delete_checkpoint(&checkpoint_id).await.unwrap();

        // Check that the file was deleted
        assert!(!file_path.exists());
    }
}
