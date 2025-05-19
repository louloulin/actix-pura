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
    async fn save_raw(&self, key: &str, data: &[u8]) -> Result<()>;

    /// Load state from storage
    async fn load_raw(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Delete state from storage
    async fn delete(&self, key: &str) -> Result<()>;

    /// Check if state exists in storage
    async fn exists(&self, key: &str) -> Result<bool>;

    /// List all keys in storage
    async fn list_keys(&self) -> Result<Vec<String>>;
}

/// Extension methods for StateStorage
#[async_trait]
pub trait StateStorageExt: StateStorage {
    /// Save state to storage
    async fn save<T: Serialize + Send + Sync>(&self, key: &str, state: &T) -> Result<()> {
        let serialized = serde_json::to_vec(state)?;
        self.save_raw(key, &serialized).await
    }

    /// Load state from storage
    async fn load<T: DeserializeOwned + Send + Sync>(&self, key: &str) -> Result<Option<T>> {
        if let Some(data) = self.load_raw(key).await? {
            let deserialized = serde_json::from_slice(&data)?;
            Ok(Some(deserialized))
        } else {
            Ok(None)
        }
    }
}

// Implement StateStorageExt for all StateStorage implementors
impl<T: StateStorage + ?Sized> StateStorageExt for T {}

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
    async fn save_raw(&self, key: &str, data: &[u8]) -> Result<()> {
        let file_path = self.get_file_path(key);

        // Create directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write to file
        tokio::fs::write(file_path, data).await?;

        Ok(())
    }

    async fn load_raw(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let file_path = self.get_file_path(key);

        // Check if file exists
        if !file_path.exists() {
            return Ok(None);
        }

        // Read file
        let data = tokio::fs::read(file_path).await?;

        Ok(Some(data))
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
    async fn save_raw(&self, key: &str, data: &[u8]) -> Result<()> {
        let data_str = String::from_utf8_lossy(data).to_string();
        let mut storage = self.storage.write().await;
        storage.insert(key.to_string(), data_str);
        Ok(())
    }

    async fn load_raw(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let storage = self.storage.read().await;

        if let Some(data_str) = storage.get(key) {
            Ok(Some(data_str.as_bytes().to_vec()))
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
