//! Offline cache module
//!
//! This module provides functionality for caching data in offline mode.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use dataflare_core::error::Result;
use serde::{de::DeserializeOwned, Serialize};

/// Offline cache for storing data in memory
pub struct OfflineCache {
    /// In-memory cache
    cache: RwLock<HashMap<String, Vec<u8>>>,
}

impl OfflineCache {
    /// Create a new offline cache
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Put an item in the cache
    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let serialized = bincode::serialize(value)?;
        let mut cache = self.cache.write().unwrap();
        cache.insert(key.to_string(), serialized);
        Ok(())
    }

    /// Get an item from the cache
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let cache = self.cache.read().unwrap();

        if let Some(serialized) = cache.get(key) {
            let value: T = bincode::deserialize(serialized)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Remove an item from the cache
    pub fn remove(&self, key: &str) -> Result<()> {
        let mut cache = self.cache.write().unwrap();
        cache.remove(key);
        Ok(())
    }

    /// Check if an item exists in the cache
    pub fn contains(&self, key: &str) -> bool {
        let cache = self.cache.read().unwrap();
        cache.contains_key(key)
    }

    /// Clear the cache
    pub fn clear(&self) -> Result<()> {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
        Ok(())
    }

    /// Get the number of items in the cache
    pub fn len(&self) -> usize {
        let cache = self.cache.read().unwrap();
        cache.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        let cache = self.cache.read().unwrap();
        cache.is_empty()
    }

    /// Get all keys in the cache
    pub fn keys(&self) -> Vec<String> {
        let cache = self.cache.read().unwrap();
        cache.keys().cloned().collect()
    }
}

impl Default for OfflineCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestData {
        id: String,
        value: i32,
    }

    #[test]
    fn test_offline_cache() {
        let cache = OfflineCache::new();

        let data = TestData {
            id: "test".to_string(),
            value: 42,
        };

        // Put data in cache
        cache.put("test-key", &data).unwrap();

        // Check if cache contains key
        assert!(cache.contains("test-key"));

        // Get data from cache
        let retrieved: TestData = cache.get("test-key").unwrap().unwrap();
        assert_eq!(retrieved, data);

        // Check cache size
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());

        // Get keys
        let keys = cache.keys();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "test-key");

        // Remove data from cache
        cache.remove("test-key").unwrap();
        assert!(!cache.contains("test-key"));

        // Check cache size after removal
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }
}
