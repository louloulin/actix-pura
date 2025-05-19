//! Storage module
//!
//! This module provides functionality for storing and retrieving state.

use std::path::PathBuf;
use async_trait::async_trait;
use dataflare_core::error::Result;
use serde::{de::DeserializeOwned, Serialize};

/// State storage trait
#[async_trait]
pub trait StateStorage: Send + Sync {
    /// Save state to storage
    async fn save<T: Serialize + Send + Sync>(&self, key: &str, state: &T) -> Result<()>;
    
    /// Load state from storage
    async fn load<T: DeserializeOwned + Send + Sync>(&self, key: &str) -> Result<Option<T>>;
    
    /// Delete state from storage
    async fn delete(&self, key: &str) -> Result<()>;
    
    /// Check if state exists in storage
    async fn exists(&self, key: &str) -> Result<bool>;
    
    /// List all keys in storage
    async fn list_keys(&self) -> Result<Vec<String>>;
}

/// File-based state storage
pub struct FileStateStorage {
    /// Base directory for storing state files
    base_dir: PathBuf,
}

impl FileStateStorage {
    /// Create a new file-based state storage
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
    
    /// Get the file path for a key
    fn get_file_path(&self, key: &str) -> PathBuf {
        self.base_dir.join(format!("{}.json", key))
    }
}

#[async_trait]
impl StateStorage for FileStateStorage {
    async fn save<T: Serialize + Send + Sync>(&self, key: &str, state: &T) -> Result<()> {
        let file_path = self.get_file_path(key);
        
        // Create directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Serialize state to JSON
        let json = serde_json::to_string_pretty(state)?;
        
        // Write to file
        tokio::fs::write(file_path, json).await?;
        
        Ok(())
    }
    
    async fn load<T: DeserializeOwned + Send + Sync>(&self, key: &str) -> Result<Option<T>> {
        let file_path = self.get_file_path(key);
        
        // Check if file exists
        if !file_path.exists() {
            return Ok(None);
        }
        
        // Read file
        let json = tokio::fs::read_to_string(file_path).await?;
        
        // Deserialize JSON
        let state = serde_json::from_str(&json)?;
        
        Ok(Some(state))
    }
    
    async fn delete(&self, key: &str) -> Result<()> {
        let file_path = self.get_file_path(key);
        
        // Delete file if it exists
        if file_path.exists() {
            tokio::fs::remove_file(file_path).await?;
        }
        
        Ok(())
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        let file_path = self.get_file_path(key);
        Ok(file_path.exists())
    }
    
    async fn list_keys(&self) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        
        // Create directory if it doesn't exist
        if !self.base_dir.exists() {
            return Ok(keys);
        }
        
        // Read directory entries
        let mut entries = tokio::fs::read_dir(&self.base_dir).await?;
        
        // Collect JSON files
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                if let Some(file_stem) = path.file_stem() {
                    if let Some(key) = file_stem.to_str() {
                        keys.push(key.to_string());
                    }
                }
            }
        }
        
        Ok(keys)
    }
}

/// In-memory state storage
pub struct MemoryStateStorage {
    /// In-memory storage
    storage: tokio::sync::RwLock<std::collections::HashMap<String, String>>,
}

impl MemoryStateStorage {
    /// Create a new in-memory state storage
    pub fn new() -> Self {
        Self {
            storage: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait]
impl StateStorage for MemoryStateStorage {
    async fn save<T: Serialize + Send + Sync>(&self, key: &str, state: &T) -> Result<()> {
        let json = serde_json::to_string(state)?;
        let mut storage = self.storage.write().await;
        storage.insert(key.to_string(), json);
        Ok(())
    }
    
    async fn load<T: DeserializeOwned + Send + Sync>(&self, key: &str) -> Result<Option<T>> {
        let storage = self.storage.read().await;
        
        if let Some(json) = storage.get(key) {
            let state = serde_json::from_str(json)?;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }
    
    async fn delete(&self, key: &str) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.remove(key);
        Ok(())
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        let storage = self.storage.read().await;
        Ok(storage.contains_key(key))
    }
    
    async fn list_keys(&self) -> Result<Vec<String>> {
        let storage = self.storage.read().await;
        Ok(storage.keys().cloned().collect())
    }
}

impl Default for MemoryStateStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;
    
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestState {
        id: String,
        value: i32,
    }
    
    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemoryStateStorage::new();
        
        let state = TestState {
            id: "test".to_string(),
            value: 42,
        };
        
        // Save state
        storage.save("test-key", &state).await.unwrap();
        
        // Check if exists
        assert!(storage.exists("test-key").await.unwrap());
        
        // Load state
        let loaded_state: TestState = storage.load("test-key").await.unwrap().unwrap();
        assert_eq!(loaded_state, state);
        
        // List keys
        let keys = storage.list_keys().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "test-key");
        
        // Delete state
        storage.delete("test-key").await.unwrap();
        
        // Check if exists after deletion
        assert!(!storage.exists("test-key").await.unwrap());
    }
    
    #[tokio::test]
    async fn test_file_storage() {
        let dir = tempdir().unwrap();
        let storage = FileStateStorage::new(dir.path().to_path_buf());
        
        let state = TestState {
            id: "test".to_string(),
            value: 42,
        };
        
        // Save state
        storage.save("test-key", &state).await.unwrap();
        
        // Check if exists
        assert!(storage.exists("test-key").await.unwrap());
        
        // Load state
        let loaded_state: TestState = storage.load("test-key").await.unwrap().unwrap();
        assert_eq!(loaded_state, state);
        
        // List keys
        let keys = storage.list_keys().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "test-key");
        
        // Delete state
        storage.delete("test-key").await.unwrap();
        
        // Check if exists after deletion
        assert!(!storage.exists("test-key").await.unwrap());
    }
}
