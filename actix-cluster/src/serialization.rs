//! Serialization module for network communication.
//!
//! This module provides serialization and deserialization functionality
//! for messages exchanged between nodes in the cluster.

use std::io;
use serde::{de::DeserializeOwned, Serialize};
use crate::error::ClusterError;

/// Serialization format
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SerializationFormat {
    /// Bincode binary format (efficient)
    Bincode,
    
    /// JSON text format (human readable)
    Json,
}

/// Serializer trait for different serialization formats
pub trait SerializerTrait {
    /// Serialize a value into bytes
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, ClusterError>;
    
    /// Deserialize bytes into a value
    fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, ClusterError>;
}

/// Concrete serializer enum that can be used as a trait object
#[derive(Clone)]
pub enum Serializer {
    /// Bincode serializer
    Bincode(BincodeSerializer),
    
    /// JSON serializer
    Json(JsonSerializer),
}

impl Serializer {
    /// Serialize a value into bytes
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        match self {
            Serializer::Bincode(serializer) => serializer.serialize(value),
            Serializer::Json(serializer) => serializer.serialize(value),
        }
    }
    
    /// Deserialize bytes into a value
    pub fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        match self {
            Serializer::Bincode(serializer) => serializer.deserialize(bytes),
            Serializer::Json(serializer) => serializer.deserialize(bytes),
        }
    }
}

/// Bincode serializer implementation
#[derive(Default, Clone)]
pub struct BincodeSerializer;

impl SerializerTrait for BincodeSerializer {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        bincode::serialize(value)
            .map_err(|e| ClusterError::SerializationError(e.to_string()))
    }
    
    fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        bincode::deserialize(bytes)
            .map_err(|e| ClusterError::DeserializationError(e.to_string()))
    }
}

/// JSON serializer implementation
#[derive(Default, Clone)]
pub struct JsonSerializer;

impl SerializerTrait for JsonSerializer {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        serde_json::to_vec(value)
            .map_err(|e| ClusterError::SerializationError(e.to_string()))
    }
    
    fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        serde_json::from_slice(bytes)
            .map_err(|e| ClusterError::DeserializationError(e.to_string()))
    }
}

/// Create a serializer for the given format
pub fn create_serializer(format: SerializationFormat) -> Serializer {
    match format {
        SerializationFormat::Bincode => Serializer::Bincode(BincodeSerializer),
        SerializationFormat::Json => Serializer::Json(JsonSerializer),
        // Default to bincode for any other format
        _ => Serializer::Bincode(BincodeSerializer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestMessage {
        id: u64,
        name: String,
        data: Vec<u8>,
    }
    
    #[test]
    fn test_bincode_serialization() {
        let serializer = BincodeSerializer;
        let message = TestMessage {
            id: 42,
            name: "test".to_string(),
            data: vec![1, 2, 3, 4],
        };
        
        let serialized = serializer.serialize(&message).unwrap();
        let deserialized: TestMessage = serializer.deserialize(&serialized).unwrap();
        
        assert_eq!(message, deserialized);
    }
    
    #[test]
    fn test_json_serialization() {
        let serializer = JsonSerializer;
        let message = TestMessage {
            id: 42,
            name: "test".to_string(),
            data: vec![1, 2, 3, 4],
        };
        
        let serialized = serializer.serialize(&message).unwrap();
        let deserialized: TestMessage = serializer.deserialize(&serialized).unwrap();
        
        assert_eq!(message, deserialized);
    }
    
    #[test]
    fn test_factory_creates_correct_serializer() {
        let message = TestMessage {
            id: 42,
            name: "test".to_string(),
            data: vec![1, 2, 3, 4],
        };
        
        // Test Bincode serializer
        let bincode_serializer = create_serializer(SerializationFormat::Bincode);
        let serialized = bincode_serializer.serialize(&message).unwrap();
        let deserialized: TestMessage = bincode_serializer.deserialize(&serialized).unwrap();
        assert_eq!(message, deserialized);
        
        // Test JSON serializer
        let json_serializer = create_serializer(SerializationFormat::Json);
        let serialized = json_serializer.serialize(&message).unwrap();
        let deserialized: TestMessage = json_serializer.deserialize(&serialized).unwrap();
        assert_eq!(message, deserialized);
    }
    
    #[test]
    fn test_serialization_error_handling() {
        // Test invalid data deserialization
        let serializer = BincodeSerializer;
        let invalid_data = vec![0, 1, 2]; // Invalid data for TestMessage
        
        let result: Result<TestMessage, _> = serializer.deserialize(&invalid_data);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ClusterError::SerializationError(msg) => {
                assert!(msg.contains("Bincode deserialization error"));
            },
            _ => panic!("Expected SerializationError"),
        }
    }
} 