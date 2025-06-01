//! Zero-copy plugin record implementation
//!
//! Provides the PluginRecord type that enables zero-copy data processing
//! by directly referencing data from DataFlare's DataRecord without cloning.

use std::collections::HashMap;
use crate::error::{PluginError, Result};

/// Zero-copy plugin record with lifetime management
///
/// This structure provides direct access to data without copying,
/// enabling high-performance plugin processing.
#[derive(Debug)]
pub struct PluginRecord<'a> {
    /// Raw data value as byte slice (zero-copy reference)
    pub value: &'a [u8],

    /// Metadata key-value pairs (zero-copy reference)
    pub metadata: &'a HashMap<String, String>,

    /// Record timestamp (Unix timestamp in milliseconds)
    pub timestamp: i64,

    /// Optional record key (zero-copy reference)
    pub key: Option<&'a [u8]>,

    /// Record offset for streaming scenarios
    pub offset: u64,
}

impl<'a> PluginRecord<'a> {
    /// Create a new PluginRecord from raw components
    pub fn new(
        value: &'a [u8],
        metadata: &'a HashMap<String, String>,
        timestamp: i64,
    ) -> Self {
        Self {
            value,
            metadata,
            timestamp,
            key: None,
            offset: 0,
        }
    }

    /// Create a PluginRecord with all fields
    pub fn with_all(
        value: &'a [u8],
        metadata: &'a HashMap<String, String>,
        timestamp: i64,
        key: Option<&'a [u8]>,
        offset: u64,
    ) -> Self {
        Self {
            value,
            metadata,
            timestamp,
            key,
            offset,
        }
    }

    /// Convert from DataFlare's DataRecord (limited zero-copy due to JSON serialization)
    pub fn from_data_record(record: &'a dataflare_core::message::DataRecord) -> Result<Self> {
        // Note: This method has a fundamental limitation for true zero-copy processing
        // because DataRecord stores data as serde_json::Value, which requires serialization
        // to get byte representation. For true zero-copy, DataRecord would need to store
        // raw bytes alongside the parsed JSON.

        let timestamp = record.created_at.timestamp_millis();

        // We'll handle different JSON value types appropriately
        let value_bytes = match &record.data {
            serde_json::Value::String(s) => s.as_bytes(),
            serde_json::Value::Number(_n) => {
                // Convert number to string bytes
                // This creates a temporary string, breaking zero-copy
                // In a real implementation, we'd need a different approach
                return Err(PluginError::processing(
                    "Number values require string conversion - not zero-copy compatible"
                ));
            }
            serde_json::Value::Bool(_b) => {
                return Err(PluginError::processing(
                    "Boolean values require string conversion - not zero-copy compatible"
                ));
            }
            serde_json::Value::Null => b"null",
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                // Complex JSON requires serialization
                return Err(PluginError::processing(
                    "Complex JSON data requires serialization - not zero-copy compatible"
                ));
            }
        };

        Ok(Self {
            value: value_bytes,
            metadata: &record.metadata,
            timestamp,
            key: None, // DataRecord doesn't currently have a key field
            offset: 0, // DataRecord doesn't currently have an offset field
        })
    }

    /// Get the value as a UTF-8 string
    pub fn value_as_str(&self) -> Result<&str> {
        std::str::from_utf8(self.value).map_err(PluginError::from)
    }

    /// Get the value as a JSON value (requires parsing)
    pub fn value_as_json(&self) -> Result<serde_json::Value> {
        let s = self.value_as_str()?;
        serde_json::from_str(s).map_err(PluginError::from)
    }

    /// Get the key as a UTF-8 string if present
    pub fn key_as_str(&self) -> Result<Option<&str>> {
        match self.key {
            Some(key_bytes) => Ok(Some(std::str::from_utf8(key_bytes)?)),
            None => Ok(None),
        }
    }

    /// Get metadata value by key
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Check if metadata contains a key
    pub fn has_metadata(&self, key: &str) -> bool {
        self.metadata.contains_key(key)
    }

    /// Get the size of the value in bytes
    pub fn value_size(&self) -> usize {
        self.value.len()
    }

    /// Get the total size including metadata (approximate)
    pub fn total_size(&self) -> usize {
        let metadata_size: usize = self.metadata
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum();

        self.value.len() + metadata_size + self.key.map_or(0, |k| k.len())
    }

    /// Check if the record is empty
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Create a clone with owned data (breaks zero-copy but sometimes necessary)
    pub fn to_owned(&self) -> OwnedPluginRecord {
        OwnedPluginRecord {
            value: self.value.to_vec(),
            metadata: self.metadata.clone(),
            timestamp: self.timestamp,
            key: self.key.map(|k| k.to_vec()),
            offset: self.offset,
        }
    }
}

/// Owned version of PluginRecord for cases where zero-copy isn't possible
#[derive(Debug, Clone)]
pub struct OwnedPluginRecord {
    pub value: Vec<u8>,
    pub metadata: HashMap<String, String>,
    pub timestamp: i64,
    pub key: Option<Vec<u8>>,
    pub offset: u64,
}

impl OwnedPluginRecord {
    /// Convert to a borrowed PluginRecord
    pub fn as_plugin_record(&self) -> PluginRecord<'_> {
        PluginRecord {
            value: &self.value,
            metadata: &self.metadata,
            timestamp: self.timestamp,
            key: self.key.as_ref().map(|k| k.as_slice()),
            offset: self.offset,
        }
    }

    /// Convert to DataFlare's DataRecord
    pub fn to_data_record(&self) -> Result<dataflare_core::message::DataRecord> {
        let value_str = std::str::from_utf8(&self.value)?;
        let json_value: serde_json::Value = serde_json::from_str(value_str)
            .unwrap_or_else(|_| serde_json::Value::String(value_str.to_string()));

        let mut record = dataflare_core::message::DataRecord::new(json_value);
        record.metadata = self.metadata.clone();
        record.created_at = chrono::DateTime::from_timestamp_millis(self.timestamp)
            .unwrap_or_else(chrono::Utc::now);

        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_plugin_record_creation() {
        let data = b"test data";
        let metadata = HashMap::new();
        let timestamp = 1234567890;

        let record = PluginRecord::new(data, &metadata, timestamp);

        assert_eq!(record.value, data);
        assert_eq!(record.timestamp, timestamp);
        assert_eq!(record.value_size(), 9);
        assert!(!record.is_empty());
    }

    #[test]
    fn test_value_as_str() {
        let data = b"hello world";
        let metadata = HashMap::new();
        let record = PluginRecord::new(data, &metadata, 0);

        assert_eq!(record.value_as_str().unwrap(), "hello world");
    }

    #[test]
    fn test_value_as_json() {
        let data = br#"{"key": "value"}"#;
        let metadata = HashMap::new();
        let record = PluginRecord::new(data, &metadata, 0);

        let json = record.value_as_json().unwrap();
        assert_eq!(json["key"], "value");
    }

    #[test]
    fn test_metadata_access() {
        let data = b"test";
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "test".to_string());

        let record = PluginRecord::new(data, &metadata, 0);

        assert_eq!(record.get_metadata("source"), Some("test"));
        assert!(record.has_metadata("source"));
        assert!(!record.has_metadata("missing"));
    }

    #[test]
    fn test_owned_conversion() {
        let data = b"test data";
        let metadata = HashMap::new();
        let record = PluginRecord::new(data, &metadata, 1234567890);

        let owned = record.to_owned();
        assert_eq!(owned.value, data);
        assert_eq!(owned.timestamp, 1234567890);

        let borrowed = owned.as_plugin_record();
        assert_eq!(borrowed.value, data);
    }
}
