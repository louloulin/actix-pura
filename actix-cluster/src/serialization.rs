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
pub trait SerializerTrait: Send + Sync {
    /// Serialize a value into bytes using type erasure
    fn serialize_any(&self, value: &dyn std::any::Any) -> Result<Vec<u8>, ClusterError>;
    
    /// Deserialize bytes into a dynamically typed value
    fn deserialize_any(&self, bytes: &[u8]) -> Result<Box<dyn std::any::Any>, ClusterError>;
    
    /// Clone the serializer
    fn clone_box(&self) -> Box<dyn SerializerTrait>;
}

// Allow SerializerTrait objects to be cloned
impl Clone for Box<dyn SerializerTrait> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
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
    /// Create a serializer for the given format
    pub fn from_format(format: SerializationFormat) -> Self {
        match format {
            SerializationFormat::Bincode => Serializer::Bincode(BincodeSerializer::new()),
            SerializationFormat::Json => Serializer::Json(JsonSerializer::new()),
        }
    }
    
    /// Serialize a value using the appropriate format
    pub fn serialize<T: Serialize + 'static>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        match self {
            Serializer::Bincode(s) => s.serialize(value),
            Serializer::Json(s) => s.serialize(value),
        }
    }
    
    /// Deserialize a value using the appropriate format
    pub fn deserialize<T: DeserializeOwned + 'static>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        match self {
            Serializer::Bincode(s) => s.deserialize(bytes),
            Serializer::Json(s) => s.deserialize(bytes),
        }
    }
}

/// Bincode serializer implementation
#[derive(Default, Clone)]
pub struct BincodeSerializer;

impl BincodeSerializer {
    /// Create a new BincodeSerializer
    pub fn new() -> Self {
        BincodeSerializer
    }
    
    /// Serialize with strong typing
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        bincode::serialize(value)
            .map_err(|e| ClusterError::SerializationError(e.to_string()))
    }
    
    /// Deserialize with strong typing
    pub fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        bincode::deserialize(bytes)
            .map_err(|e| ClusterError::DeserializationError(e.to_string()))
    }
}

impl SerializerTrait for BincodeSerializer {
    fn serialize_any(&self, value: &dyn std::any::Any) -> Result<Vec<u8>, ClusterError> {
        // 由于我们无法直接序列化Any，我们需要知道真实类型
        // 在实际应用中，我们可能需要为特定类型编写匹配分支或使用某种注册表
        // 这里提供一个简单示例
        if let Some(s) = value.downcast_ref::<String>() {
            return self.serialize(s);
        } else if let Some(i) = value.downcast_ref::<i32>() {
            return self.serialize(i);
        } else if let Some(msg) = value.downcast_ref::<crate::message::MessageEnvelope>() {
            return self.serialize(msg);
        } else if let Some(tm) = value.downcast_ref::<crate::transport::TransportMessage>() {
            return self.serialize(tm);
        }
        // 添加更多类型分支...
        
        Err(ClusterError::SerializationError("Unsupported type for serialization".to_string()))
    }
    
    fn deserialize_any(&self, bytes: &[u8]) -> Result<Box<dyn std::any::Any>, ClusterError> {
        // 我们可以尝试作为不同类型反序列化，取决于调用方的期望
        // 这是一个简化的实现，实际上需要知道预期的类型
        
        // 首先尝试作为MessageEnvelope反序列化
        if let Ok(value) = self.deserialize::<crate::message::MessageEnvelope>(bytes) {
            return Ok(Box::new(value));
        }
        
        // 再尝试作为TransportMessage反序列化
        if let Ok(value) = self.deserialize::<crate::transport::TransportMessage>(bytes) {
            return Ok(Box::new(value));
        }
        
        // 其他常见类型
        if let Ok(value) = self.deserialize::<String>(bytes) {
            return Ok(Box::new(value));
        }
        
        if let Ok(value) = self.deserialize::<i32>(bytes) {
            return Ok(Box::new(value));
        }
        
        Err(ClusterError::DeserializationError("Unsupported type for deserialization".to_string()))
    }
    
    fn clone_box(&self) -> Box<dyn SerializerTrait> {
        Box::new(self.clone())
    }
}

/// JSON serializer implementation
#[derive(Default, Clone)]
pub struct JsonSerializer;

impl JsonSerializer {
    /// Create a new JsonSerializer
    pub fn new() -> Self {
        JsonSerializer
    }
    
    /// Serialize with strong typing
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        serde_json::to_vec(value)
            .map_err(|e| ClusterError::SerializationError(e.to_string()))
    }
    
    /// Deserialize with strong typing
    pub fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        serde_json::from_slice(bytes)
            .map_err(|e| ClusterError::DeserializationError(e.to_string()))
    }
}

impl SerializerTrait for JsonSerializer {
    fn serialize_any(&self, value: &dyn std::any::Any) -> Result<Vec<u8>, ClusterError> {
        // Similar implementation to BincodeSerializer
        if let Some(s) = value.downcast_ref::<String>() {
            return self.serialize(s);
        } else if let Some(i) = value.downcast_ref::<i32>() {
            return self.serialize(i);
        } else if let Some(msg) = value.downcast_ref::<crate::message::MessageEnvelope>() {
            return self.serialize(msg);
        } else if let Some(tm) = value.downcast_ref::<crate::transport::TransportMessage>() {
            return self.serialize(tm);
        }
        // 添加更多类型分支...
        
        Err(ClusterError::SerializationError("Unsupported type for serialization".to_string()))
    }
    
    fn deserialize_any(&self, bytes: &[u8]) -> Result<Box<dyn std::any::Any>, ClusterError> {
        // Similar implementation to BincodeSerializer
        // 首先尝试作为MessageEnvelope反序列化
        if let Ok(value) = self.deserialize::<crate::message::MessageEnvelope>(bytes) {
            return Ok(Box::new(value));
        }
        
        // 再尝试作为TransportMessage反序列化
        if let Ok(value) = self.deserialize::<crate::transport::TransportMessage>(bytes) {
            return Ok(Box::new(value));
        }
        
        // 其他常见类型
        if let Ok(value) = self.deserialize::<String>(bytes) {
            return Ok(Box::new(value));
        }
        
        if let Ok(value) = self.deserialize::<i32>(bytes) {
            return Ok(Box::new(value));
        }
        
        Err(ClusterError::DeserializationError("Unsupported type for deserialization".to_string()))
    }
    
    fn clone_box(&self) -> Box<dyn SerializerTrait> {
        Box::new(self.clone())
    }
}

/// Create a serializer trait object for the given format
pub fn create_serializer_trait(format: SerializationFormat) -> Box<dyn SerializerTrait> {
    match format {
        SerializationFormat::Bincode => Box::new(BincodeSerializer::new()),
        SerializationFormat::Json => Box::new(JsonSerializer::new()),
    }
}

/// Convenience function to serialize a value using Bincode
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, ClusterError> {
    BincodeSerializer::new().serialize(value)
}

/// Convenience function to deserialize a value using Bincode
pub fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ClusterError> {
    BincodeSerializer::new().deserialize(bytes)
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
        // 使用简单的字符串代替TestMessage，因为字符串是serialize_any支持的类型
        let test_str = "test_serializer".to_string();
        
        // Test Bincode serializer
        let bincode_serializer = create_serializer_trait(SerializationFormat::Bincode);
        let serialized = bincode_serializer.serialize_any(&test_str).unwrap();
        
        // 直接测试序列化和反序列化结果
        let serializer = BincodeSerializer::new();
        let deserialized: String = serializer.deserialize(&serialized).unwrap();
        assert_eq!(test_str, deserialized);
        
        // Test JSON serializer
        let json_serializer = create_serializer_trait(SerializationFormat::Json);
        let serialized = json_serializer.serialize_any(&test_str).unwrap();
        
        // 直接测试序列化和反序列化结果
        let serializer = JsonSerializer::new();
        let deserialized: String = serializer.deserialize(&serialized).unwrap();
        assert_eq!(test_str, deserialized);
    }
    
    #[test]
    fn test_serialization_error_handling() {
        // Test invalid data deserialization
        let serializer = BincodeSerializer::new();
        let invalid_data = vec![0, 1, 2]; // Invalid data for TestMessage
        
        let result: Result<TestMessage, _> = serializer.deserialize(&invalid_data);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ClusterError::DeserializationError(_msg) => {
                // 只检查是否是DeserializationError类型，不检查具体错误信息
                // bincode在不同版本和平台上可能有不同的错误消息格式
                assert!(true);
            },
            e => panic!("Expected DeserializationError, got {:?}", e),
        }
    }
} 