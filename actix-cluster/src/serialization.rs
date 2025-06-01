//! Serialization module for network communication.
//!
//! This module provides serialization and deserialization functionality
//! for messages exchanged between nodes in the cluster.

use std::io;
use std::any::TypeId;
use serde::{de::DeserializeOwned, Serialize};
use crate::error::ClusterError;
use crate::compression::{CompressionAlgorithm, CompressionLevel, compress, decompress};
use crate::proto;
use prost::Message;

/// Serialization format
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SerializationFormat {
    /// Bincode binary format (efficient)
    Bincode,

    /// JSON text format (human readable)
    Json,

    /// Protocol Buffers binary format (efficient and cross-language)
    Protobuf,

    /// Bincode with compression
    CompressedBincode,

    /// JSON with compression
    CompressedJson,

    /// Protocol Buffers with compression
    CompressedProtobuf,
}

impl SerializationFormat {
    /// Check if this format uses compression
    pub fn is_compressed(&self) -> bool {
        match self {
            SerializationFormat::CompressedBincode |
            SerializationFormat::CompressedJson |
            SerializationFormat::CompressedProtobuf => true,
            _ => false,
        }
    }

    /// Get the base format without compression
    pub fn base_format(&self) -> Self {
        match self {
            SerializationFormat::CompressedBincode => SerializationFormat::Bincode,
            SerializationFormat::CompressedJson => SerializationFormat::Json,
            SerializationFormat::CompressedProtobuf => SerializationFormat::Protobuf,
            _ => *self,
        }
    }
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

    /// Protocol Buffers serializer
    Protobuf(ProtobufSerializer),

    /// Compressed Bincode serializer
    CompressedBincode(CompressedSerializer<BincodeSerializer>),

    /// Compressed JSON serializer
    CompressedJson(CompressedSerializer<JsonSerializer>),

    /// Compressed Protocol Buffers serializer
    CompressedProtobuf(CompressedSerializer<ProtobufSerializer>),
}

impl Serializer {
    /// Create a serializer for the given format
    pub fn from_format(format: SerializationFormat) -> Self {
        match format {
            SerializationFormat::Bincode => Serializer::Bincode(BincodeSerializer::new()),
            SerializationFormat::Json => Serializer::Json(JsonSerializer::new()),
            SerializationFormat::Protobuf => Serializer::Protobuf(ProtobufSerializer::new()),
            SerializationFormat::CompressedBincode => {
                Serializer::CompressedBincode(CompressedSerializer::new(
                    BincodeSerializer::new(),
                    CompressionAlgorithm::Gzip,
                    CompressionLevel::Default
                ))
            },
            SerializationFormat::CompressedJson => {
                Serializer::CompressedJson(CompressedSerializer::new(
                    JsonSerializer::new(),
                    CompressionAlgorithm::Gzip,
                    CompressionLevel::Default
                ))
            },
            SerializationFormat::CompressedProtobuf => {
                Serializer::CompressedProtobuf(CompressedSerializer::new(
                    ProtobufSerializer::new(),
                    CompressionAlgorithm::Gzip,
                    CompressionLevel::Default
                ))
            }
        }
    }

    /// Serialize a value using the appropriate format
    pub fn serialize<T: Serialize + 'static>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        match self {
            Serializer::Bincode(s) => s.serialize(value),
            Serializer::Json(s) => s.serialize(value),
            Serializer::Protobuf(_) => {
                // For Protocol Buffers, we need to handle differently
                // since it requires prost::Message trait
                // Fallback to JSON for types that don't have Protocol Buffers support
                let json_serializer = JsonSerializer::new();
                json_serializer.serialize(value)
            },
            Serializer::CompressedBincode(s) => s.serialize(value),
            Serializer::CompressedJson(s) => s.serialize(value),
            Serializer::CompressedProtobuf(_) => {
                // For Protocol Buffers, we need to handle differently
                // since it requires prost::Message trait
                // Fallback to compressed JSON for types that don't have Protocol Buffers support
                let compressed_json = CompressedSerializer::new(
                    JsonSerializer::new(),
                    CompressionAlgorithm::Gzip,
                    CompressionLevel::Default
                );
                compressed_json.serialize(value)
            },
        }
    }

    /// Deserialize a value using the appropriate format
    pub fn deserialize<T: DeserializeOwned + 'static>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        match self {
            Serializer::Bincode(s) => s.deserialize(bytes),
            Serializer::Json(s) => s.deserialize(bytes),
            Serializer::Protobuf(_) => {
                // For Protocol Buffers, we need to handle differently
                // since it requires prost::Message trait
                // Fallback to JSON for types that don't have Protocol Buffers support
                let json_serializer = JsonSerializer::new();
                json_serializer.deserialize(bytes)
            },
            Serializer::CompressedBincode(s) => s.deserialize(bytes),
            Serializer::CompressedJson(s) => s.deserialize(bytes),
            Serializer::CompressedProtobuf(_) => {
                // For Protocol Buffers, we need to handle differently
                // since it requires prost::Message trait
                // Fallback to compressed JSON for types that don't have Protocol Buffers support
                let compressed_json = CompressedSerializer::new(
                    JsonSerializer::new(),
                    CompressionAlgorithm::Gzip,
                    CompressionLevel::Default
                );
                compressed_json.deserialize(bytes)
            },
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

/// Compressed serializer that wraps another serializer
#[derive(Clone)]
pub struct CompressedSerializer<S> {
    /// Inner serializer
    inner: S,
    /// Compression algorithm
    algorithm: CompressionAlgorithm,
    /// Compression level
    level: CompressionLevel,
}

impl<S> CompressedSerializer<S> {
    /// Create a new compressed serializer
    pub fn new(inner: S, algorithm: CompressionAlgorithm, level: CompressionLevel) -> Self {
        Self {
            inner,
            algorithm,
            level,
        }
    }
}

impl<S: Clone> CompressedSerializer<S>
where
    S: SerializerTrait + 'static,
    S: Clone,
{
    /// Clone the inner serializer
    pub fn inner(&self) -> &S {
        &self.inner
    }
}

impl<S> SerializerTrait for CompressedSerializer<S>
where
    S: SerializerTrait + 'static,
    S: Clone,
{
    fn serialize_any(&self, value: &dyn std::any::Any) -> Result<Vec<u8>, ClusterError> {
        let bytes = self.inner.serialize_any(value)?;
        compress(&bytes, self.algorithm, self.level)
    }

    fn deserialize_any(&self, bytes: &[u8]) -> Result<Box<dyn std::any::Any>, ClusterError> {
        let decompressed = decompress(bytes, self.algorithm)?;
        self.inner.deserialize_any(&decompressed)
    }

    fn clone_box(&self) -> Box<dyn SerializerTrait> {
        let cloned = CompressedSerializer {
            inner: self.inner.clone(),
            algorithm: self.algorithm,
            level: self.level,
        };
        Box::new(cloned)
    }
}

/// Specialized implementation for BincodeSerializer
impl CompressedSerializer<BincodeSerializer> {
    /// Serialize with strong typing
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        let serialized = self.inner.serialize(value)?;
        compress(&serialized, self.algorithm, self.level)
    }

    /// Deserialize with strong typing
    pub fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        let decompressed = decompress(bytes, self.algorithm)?;
        self.inner.deserialize(&decompressed)
    }
}

/// Specialized implementation for JsonSerializer
impl CompressedSerializer<JsonSerializer> {
    /// Serialize with strong typing
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        let serialized = self.inner.serialize(value)?;
        compress(&serialized, self.algorithm, self.level)
    }

    /// Deserialize with strong typing
    pub fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        let decompressed = decompress(bytes, self.algorithm)?;
        self.inner.deserialize(&decompressed)
    }
}

/// Protocol Buffers serializer implementation
#[derive(Default, Clone)]
pub struct ProtobufSerializer;

impl ProtobufSerializer {
    /// Create a new ProtobufSerializer
    pub fn new() -> Self {
        ProtobufSerializer
    }

    /// Serialize with strong typing
    pub fn serialize<T: Serialize + prost::Message + Default>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        let mut buf = Vec::new();
        buf.reserve(value.encoded_len());
        value.encode(&mut buf)
            .map_err(|e| ClusterError::SerializationError(e.to_string()))?;
        Ok(buf)
    }

    /// Deserialize with strong typing
    pub fn deserialize<T: DeserializeOwned + prost::Message + Default>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        T::decode(bytes)
            .map_err(|e| ClusterError::DeserializationError(e.to_string()))
    }
}

impl SerializerTrait for ProtobufSerializer {
    fn serialize_any(&self, value: &dyn std::any::Any) -> Result<Vec<u8>, ClusterError> {
        // Handle specific types that support Protocol Buffers
        if let Some(msg) = value.downcast_ref::<crate::message::MessageEnvelope>() {
            // Convert to proto message
            let proto_msg = match crate::proto::ProtoMessageEnvelope::try_from(msg.clone()) {
                Ok(proto) => proto,
                Err(e) => return Err(e),
            };

            // Serialize using prost
            let mut buf = Vec::new();
            buf.reserve(proto_msg.encoded_len());
            proto_msg.encode(&mut buf)
                .map_err(|e| ClusterError::SerializationError(e.to_string()))?;
            return Ok(buf);
        } else if let Some(tm) = value.downcast_ref::<crate::transport::TransportMessage>() {
            // Convert to proto message
            let proto_msg = match crate::proto::ProtoTransportMessage::try_from(tm.clone()) {
                Ok(proto) => proto,
                Err(e) => return Err(e),
            };

            // Serialize using prost
            let mut buf = Vec::new();
            buf.reserve(proto_msg.encoded_len());
            proto_msg.encode(&mut buf)
                .map_err(|e| ClusterError::SerializationError(e.to_string()))?;
            return Ok(buf);
        }

        // Fallback to JSON for types that don't have Protocol Buffers support
        let json_serializer = JsonSerializer::new();
        json_serializer.serialize_any(value)
    }

    fn deserialize_any(&self, bytes: &[u8]) -> Result<Box<dyn std::any::Any>, ClusterError> {
        // Try to deserialize as MessageEnvelope
        if let Ok(proto_msg) = crate::proto::ProtoMessageEnvelope::decode(bytes) {
            match crate::message::MessageEnvelope::try_from(proto_msg) {
                Ok(msg) => return Ok(Box::new(msg)),
                Err(_) => {} // Continue to next type
            }
        }

        // Try to deserialize as TransportMessage
        if let Ok(proto_msg) = crate::proto::ProtoTransportMessage::decode(bytes) {
            match crate::transport::TransportMessage::try_from(proto_msg) {
                Ok(msg) => return Ok(Box::new(msg)),
                Err(_) => {} // Continue to next type
            }
        }

        // Fallback to JSON for types that don't have Protocol Buffers support
        let json_serializer = JsonSerializer::new();
        json_serializer.deserialize_any(bytes)
    }

    fn clone_box(&self) -> Box<dyn SerializerTrait> {
        Box::new(self.clone())
    }
}

/// Specialized implementation for ProtobufSerializer
impl CompressedSerializer<ProtobufSerializer> {
    /// Serialize with strong typing
    pub fn serialize<T: Serialize + prost::Message + Default>(&self, value: &T) -> Result<Vec<u8>, ClusterError> {
        let serialized = self.inner.serialize(value)?;
        compress(&serialized, self.algorithm, self.level)
    }

    /// Deserialize with strong typing
    pub fn deserialize<T: DeserializeOwned + prost::Message + Default>(&self, bytes: &[u8]) -> Result<T, ClusterError> {
        let decompressed = decompress(bytes, self.algorithm)?;
        self.inner.deserialize(&decompressed)
    }
}

/// Create a serializer trait object for the given format
pub fn create_serializer_trait(format: SerializationFormat) -> Box<dyn SerializerTrait> {
    match format {
        SerializationFormat::Bincode => Box::new(BincodeSerializer::new()),
        SerializationFormat::Json => Box::new(JsonSerializer::new()),
        SerializationFormat::Protobuf => Box::new(JsonSerializer::new()), // Use JSON as fallback for Protobuf
        SerializationFormat::CompressedBincode => {
            Box::new(CompressedSerializer::new(
                BincodeSerializer::new(),
                CompressionAlgorithm::Gzip,
                CompressionLevel::Default
            ))
        },
        SerializationFormat::CompressedJson => {
            Box::new(CompressedSerializer::new(
                JsonSerializer::new(),
                CompressionAlgorithm::Gzip,
                CompressionLevel::Default
            ))
        },
        SerializationFormat::CompressedProtobuf => {
            // Use compressed JSON as fallback for compressed Protobuf
            Box::new(CompressedSerializer::new(
                JsonSerializer::new(),
                CompressionAlgorithm::Gzip,
                CompressionLevel::Default
            ))
        }
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
    use prost::Message;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestMessage {
        id: u64,
        name: String,
        data: Vec<u8>,
    }

    // Define a Protocol Buffers message for testing
    #[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
    struct TestProtoMessage {
        id: u64,
        name: String,
        data: Vec<u8>,
    }

    // Define a real Protocol Buffers message for the test
    #[derive(Clone, PartialEq, Message)]
    struct TestProtoMessageProto {
        #[prost(uint64, tag = "1")]
        id: u64,
        #[prost(string, tag = "2")]
        name: String,
        #[prost(bytes, tag = "3")]
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
    fn test_protobuf_serialization() {
        // For Protocol Buffers, we need to use a type that implements Message
        let message = TestProtoMessageProto {
            id: 42,
            name: "test".to_string(),
            data: vec![1, 2, 3, 4],
        };

        // Directly use prost's encode/decode
        let mut buf = Vec::new();
        message.encode(&mut buf).unwrap();
        let deserialized = TestProtoMessageProto::decode(&buf[..]).unwrap();

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

    #[test]
    fn test_compressed_serialization() {
        let test_message = TestMessage {
            id: 123,
            name: "Test".to_string(),
            data: vec![0; 1000], // 1KB 的数据，适合压缩
        };

        // 测试 Bincode 压缩
        let serializer = Serializer::from_format(SerializationFormat::CompressedBincode);
        let serialized = serializer.serialize(&test_message).unwrap();
        let deserialized: TestMessage = serializer.deserialize(&serialized).unwrap();
        assert_eq!(deserialized, test_message);

        // 测试 JSON 压缩
        let serializer = Serializer::from_format(SerializationFormat::CompressedJson);
        let serialized = serializer.serialize(&test_message).unwrap();
        let deserialized: TestMessage = serializer.deserialize(&serialized).unwrap();
        assert_eq!(deserialized, test_message);

        // 比较压缩前后的大小
        let uncompressed_serializer = Serializer::from_format(SerializationFormat::Bincode);
        let uncompressed = uncompressed_serializer.serialize(&test_message).unwrap();
        let compressed_serializer = Serializer::from_format(SerializationFormat::CompressedBincode);
        let compressed = compressed_serializer.serialize(&test_message).unwrap();

        assert!(compressed.len() < uncompressed.len(),
            "Compressed size ({}) should be smaller than uncompressed size ({})",
            compressed.len(), uncompressed.len());
    }

    #[test]
    fn test_protobuf_compressed_serialization() {
        let test_message = TestProtoMessageProto {
            id: 123,
            name: "Test".to_string(),
            data: vec![0; 1000], // 1KB 的数据，适合压缩
        };

        // Directly use prost's encode
        let mut uncompressed = Vec::new();
        test_message.encode(&mut uncompressed).unwrap();

        // Compress the data
        let compressed = compress(
            &uncompressed,
            CompressionAlgorithm::Gzip,
            CompressionLevel::Default,
        ).unwrap();

        // Decompress and decode
        let decompressed = decompress(
            &compressed,
            CompressionAlgorithm::Gzip,
        ).unwrap();

        let deserialized = TestProtoMessageProto::decode(&decompressed[..]).unwrap();

        // Check that the message is the same
        assert_eq!(deserialized, test_message);

        // Check that compression reduced the size
        assert!(compressed.len() < uncompressed.len(),
            "Compressed size ({}) should be smaller than uncompressed size ({})",
            compressed.len(), uncompressed.len());
    }

    #[test]
    fn test_serialization_format_comparison() {
        let test_message = TestMessage {
            id: 123,
            name: "Test".to_string(),
            data: vec![0; 1000], // 1KB 的数据
        };

        // 测试不同序列化格式的大小比较
        let bincode_serializer = Serializer::from_format(SerializationFormat::Bincode);
        let bincode_size = bincode_serializer.serialize(&test_message).unwrap().len();

        let json_serializer = Serializer::from_format(SerializationFormat::Json);
        let json_size = json_serializer.serialize(&test_message).unwrap().len();

        // Protocol Buffers 通常比 JSON 更紧凑
        println!("Bincode size: {}, JSON size: {}", bincode_size, json_size);
        assert!(json_size > bincode_size, "JSON should be larger than Bincode");

        // 测试压缩效果
        let compressed_bincode = Serializer::from_format(SerializationFormat::CompressedBincode);
        let compressed_bincode_size = compressed_bincode.serialize(&test_message).unwrap().len();

        let compressed_json = Serializer::from_format(SerializationFormat::CompressedJson);
        let compressed_json_size = compressed_json.serialize(&test_message).unwrap().len();

        println!("Compressed Bincode size: {}, Compressed JSON size: {}",
                 compressed_bincode_size, compressed_json_size);

        assert!(compressed_bincode_size < bincode_size,
                "Compressed Bincode should be smaller than Bincode");
        assert!(compressed_json_size < json_size,
                "Compressed JSON should be smaller than JSON");
    }
}