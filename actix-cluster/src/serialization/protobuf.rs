use std::any::Any;
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

use prost::Message;
use uuid::Uuid;

use crate::error::ClusterError;
use crate::message::{MessageEnvelope, DeliveryGuarantee, MessageType, ActorPath};
use crate::node::{NodeId, NodeInfo};
use crate::serialization::SerializerTrait;
use crate::transport::TransportMessage;

#[path = "generated/actix_cluster.rs"]
mod proto;
use proto::*;

/// Protocol Buffers serializer implementation
#[derive(Default, Clone)]
pub struct ProtobufSerializer;

impl ProtobufSerializer {
    /// Create a new ProtobufSerializer
    pub fn new() -> Self {
        ProtobufSerializer
    }
    
    /// Serialize a value with strong typing
    pub fn serialize<T>(&self, value: &T) -> Result<Vec<u8>, ClusterError>
    where
        T: ConvertToProto,
    {
        value.to_proto_bytes()
    }
    
    /// Deserialize a value with strong typing
    pub fn deserialize<T>(&self, bytes: &[u8]) -> Result<T, ClusterError>
    where
        T: ConvertFromProto,
    {
        T::from_proto_bytes(bytes)
    }
}

impl SerializerTrait for ProtobufSerializer {
    fn serialize_any(&self, value: &dyn Any) -> Result<Vec<u8>, ClusterError> {
        if let Some(msg) = value.downcast_ref::<MessageEnvelope>() {
            return self.serialize(msg);
        } else if let Some(tm) = value.downcast_ref::<TransportMessage>() {
            return self.serialize(tm);
        } else if let Some(node_info) = value.downcast_ref::<NodeInfo>() {
            return self.serialize(node_info);
        } else if let Some(s) = value.downcast_ref::<String>() {
            let proto = proto::NodeInfoProto {
                id: None,
                host: s.clone(),
                port: 0,
                roles: Vec::new(),
                metadata: std::collections::HashMap::new(),
                last_seen: None,
                status: String::new(),
                load: 0.0,
            };
            return Ok(proto.encode_to_vec());
        }
        
        Err(ClusterError::SerializationError("Unsupported type for protobuf serialization".to_string()))
    }
    
    fn deserialize_any(&self, bytes: &[u8]) -> Result<Box<dyn Any>, ClusterError> {
        // Try to decode as MessageEnvelope first
        if let Ok(envelope) = MessageEnvelope::from_proto_bytes(bytes) {
            return Ok(Box::new(envelope));
        }
        
        // Then try TransportMessage
        if let Ok(tm) = TransportMessage::from_proto_bytes(bytes) {
            return Ok(Box::new(tm));
        }
        
        // Then try NodeInfo
        if let Ok(node_info) = NodeInfo::from_proto_bytes(bytes) {
            return Ok(Box::new(node_info));
        }
        
        Err(ClusterError::DeserializationError("Failed to deserialize protobuf data".to_string()))
    }
    
    fn clone_box(&self) -> Box<dyn SerializerTrait> {
        Box::new(self.clone())
    }
}

/// Trait for types that can be converted to Protocol Buffers
pub trait ConvertToProto {
    /// Convert to Protocol Buffers binary format
    fn to_proto_bytes(&self) -> Result<Vec<u8>, ClusterError>;
}

/// Trait for types that can be converted from Protocol Buffers
pub trait ConvertFromProto: Sized {
    /// Convert from Protocol Buffers binary format
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ClusterError>;
}

// Implementation for MessageEnvelope
impl ConvertToProto for MessageEnvelope {
    fn to_proto_bytes(&self) -> Result<Vec<u8>, ClusterError> {
        // Convert timestamp to Google's timestamp format
        let timestamp = match SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_millis(self.timestamp)) {
            Some(t) => {
                let duration = t.duration_since(UNIX_EPOCH)
                    .map_err(|e| ClusterError::SerializationError(format!("Failed to convert timestamp: {}", e)))?;
                Some(prost_types::Timestamp {
                    seconds: duration.as_secs() as i64,
                    nanos: duration.subsec_nanos() as i32,
                })
            },
            None => None,
        };
        
        // Convert message type
        let (message_type, custom_type) = match &self.message_type {
            MessageType::ActorMessage => (MessageTypeProto::ActorMessage as i32, String::new()),
            MessageType::SystemControl => (MessageTypeProto::SystemControl as i32, String::new()),
            MessageType::Discovery => (MessageTypeProto::Discovery as i32, String::new()),
            MessageType::Ping => (MessageTypeProto::Ping as i32, String::new()),
            MessageType::Pong => (MessageTypeProto::Pong as i32, String::new()),
            MessageType::Custom(s) => (MessageTypeProto::Custom as i32, s.clone()),
        };
        
        // Convert delivery guarantee
        let delivery_guarantee = match self.delivery_guarantee {
            DeliveryGuarantee::AtMostOnce => DeliveryGuaranteeProto::AtMostOnce as i32,
            DeliveryGuarantee::AtLeastOnce => DeliveryGuaranteeProto::AtLeastOnce as i32,
            DeliveryGuarantee::ExactlyOnce => DeliveryGuaranteeProto::ExactlyOnce as i32,
        };
        
        // Create the protobuf message
        let proto = MessageEnvelopeProto {
            message_id: self.message_id.to_string(),
            sender_node: Some(NodeIdProto {
                id: self.sender_node.to_string(),
                is_local: self.sender_node.is_local(),
            }),
            target_node: Some(NodeIdProto {
                id: self.target_node.to_string(),
                is_local: self.target_node.is_local(),
            }),
            target_actor: self.target_actor.clone(),
            timestamp,
            message_type,
            custom_message_type: custom_type,
            delivery_guarantee,
            payload: self.payload.clone(),
            compressed: self.compressed,
        };
        
        Ok(proto.encode_to_vec())
    }
}

impl ConvertFromProto for MessageEnvelope {
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ClusterError> {
        let proto = MessageEnvelopeProto::decode(bytes)
            .map_err(|e| ClusterError::DeserializationError(format!("Failed to decode MessageEnvelope: {}", e)))?;
        
        // Parse message ID
        let message_id = Uuid::parse_str(&proto.message_id)
            .map_err(|e| ClusterError::DeserializationError(format!("Invalid UUID: {}", e)))?;
        
        // Parse sender node
        let sender_node = match proto.sender_node {
            Some(node) => {
                let mut id = NodeId::from_string(&node.id)
                    .map_err(|e| ClusterError::DeserializationError(format!("Invalid sender node ID: {}", e)))?;
                if node.is_local {
                    id.set_local(true);
                }
                id
            },
            None => return Err(ClusterError::DeserializationError("Missing sender node".to_string())),
        };
        
        // Parse target node
        let target_node = match proto.target_node {
            Some(node) => {
                let mut id = NodeId::from_string(&node.id)
                    .map_err(|e| ClusterError::DeserializationError(format!("Invalid target node ID: {}", e)))?;
                if node.is_local {
                    id.set_local(true);
                }
                id
            },
            None => return Err(ClusterError::DeserializationError("Missing target node".to_string())),
        };
        
        // Parse timestamp
        let timestamp = match proto.timestamp {
            Some(ts) => {
                let seconds_millis = ts.seconds * 1000;
                let nanos_millis = (ts.nanos / 1_000_000) as u64;
                seconds_millis as u64 + nanos_millis
            },
            None => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };
        
        // Parse message type
        let message_type = match proto.message_type {
            m if m == MessageTypeProto::ActorMessage as i32 => MessageType::ActorMessage,
            m if m == MessageTypeProto::SystemControl as i32 => MessageType::SystemControl,
            m if m == MessageTypeProto::Discovery as i32 => MessageType::Discovery,
            m if m == MessageTypeProto::Ping as i32 => MessageType::Ping,
            m if m == MessageTypeProto::Pong as i32 => MessageType::Pong,
            m if m == MessageTypeProto::Custom as i32 => MessageType::Custom(proto.custom_message_type),
            _ => return Err(ClusterError::DeserializationError("Invalid message type".to_string())),
        };
        
        // Parse delivery guarantee
        let delivery_guarantee = match proto.delivery_guarantee {
            g if g == DeliveryGuaranteeProto::AtMostOnce as i32 => DeliveryGuarantee::AtMostOnce,
            g if g == DeliveryGuaranteeProto::AtLeastOnce as i32 => DeliveryGuarantee::AtLeastOnce,
            g if g == DeliveryGuaranteeProto::ExactlyOnce as i32 => DeliveryGuarantee::ExactlyOnce,
            _ => return Err(ClusterError::DeserializationError("Invalid delivery guarantee".to_string())),
        };
        
        Ok(MessageEnvelope {
            message_id,
            sender_node,
            target_node,
            target_actor: proto.target_actor,
            timestamp,
            message_type,
            delivery_guarantee,
            payload: proto.payload,
            compressed: proto.compressed,
        })
    }
}

// Implementation for TransportMessage
impl ConvertToProto for TransportMessage {
    fn to_proto_bytes(&self) -> Result<Vec<u8>, ClusterError> {
        // Convert timestamp
        let sent_at = match SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_millis(self.sent_at)) {
            Some(t) => {
                let duration = t.duration_since(UNIX_EPOCH)
                    .map_err(|e| ClusterError::SerializationError(format!("Failed to convert timestamp: {}", e)))?;
                Some(prost_types::Timestamp {
                    seconds: duration.as_secs() as i64,
                    nanos: duration.subsec_nanos() as i32,
                })
            },
            None => None,
        };
        
        // Convert message type
        let message_type = match self.message_type {
            crate::transport::TransportMessageType::Data => TransportMessageProto_TransportMessageType::Data as i32,
            crate::transport::TransportMessageType::Ack => TransportMessageProto_TransportMessageType::Ack as i32,
            crate::transport::TransportMessageType::Discovery => TransportMessageProto_TransportMessageType::Discovery as i32,
            crate::transport::TransportMessageType::HealthCheck => TransportMessageProto_TransportMessageType::HealthCheck as i32,
            crate::transport::TransportMessageType::System => TransportMessageProto_TransportMessageType::System as i32,
        };
        
        // Convert envelopes
        let mut envelopes = Vec::new();
        for envelope in &self.envelopes {
            let envelope_bytes = envelope.to_proto_bytes()?;
            let proto_envelope = MessageEnvelopeProto::decode(envelope_bytes.as_slice())
                .map_err(|e| ClusterError::SerializationError(format!("Failed to decode envelope: {}", e)))?;
            envelopes.push(proto_envelope);
        }
        
        // Create protobuf message
        let proto = TransportMessageProto {
            id: self.id.to_string(),
            r#type: message_type,
            source: Some(NodeIdProto {
                id: self.source.to_string(),
                is_local: self.source.is_local(),
            }),
            target: Some(NodeIdProto {
                id: self.target.to_string(),
                is_local: self.target.is_local(),
            }),
            sent_at,
            envelopes,
            headers: self.headers.clone(),
        };
        
        Ok(proto.encode_to_vec())
    }
}

impl ConvertFromProto for TransportMessage {
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ClusterError> {
        let proto = TransportMessageProto::decode(bytes)
            .map_err(|e| ClusterError::DeserializationError(format!("Failed to decode TransportMessage: {}", e)))?;
        
        // Parse ID
        let id = Uuid::parse_str(&proto.id)
            .map_err(|e| ClusterError::DeserializationError(format!("Invalid UUID: {}", e)))?;
        
        // Parse source node
        let source = match proto.source {
            Some(node) => {
                let mut id = NodeId::from_string(&node.id)
                    .map_err(|e| ClusterError::DeserializationError(format!("Invalid source node ID: {}", e)))?;
                if node.is_local {
                    id.set_local(true);
                }
                id
            },
            None => return Err(ClusterError::DeserializationError("Missing source node".to_string())),
        };
        
        // Parse target node
        let target = match proto.target {
            Some(node) => {
                let mut id = NodeId::from_string(&node.id)
                    .map_err(|e| ClusterError::DeserializationError(format!("Invalid target node ID: {}", e)))?;
                if node.is_local {
                    id.set_local(true);
                }
                id
            },
            None => return Err(ClusterError::DeserializationError("Missing target node".to_string())),
        };
        
        // Parse timestamp
        let sent_at = match proto.sent_at {
            Some(ts) => {
                let seconds_millis = ts.seconds * 1000;
                let nanos_millis = (ts.nanos / 1_000_000) as u64;
                seconds_millis as u64 + nanos_millis
            },
            None => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };
        
        // Parse message type
        let message_type = match proto.r#type {
            t if t == TransportMessageProto_TransportMessageType::Data as i32 => 
                crate::transport::TransportMessageType::Data,
            t if t == TransportMessageProto_TransportMessageType::Ack as i32 => 
                crate::transport::TransportMessageType::Ack,
            t if t == TransportMessageProto_TransportMessageType::Discovery as i32 => 
                crate::transport::TransportMessageType::Discovery,
            t if t == TransportMessageProto_TransportMessageType::HealthCheck as i32 => 
                crate::transport::TransportMessageType::HealthCheck,
            t if t == TransportMessageProto_TransportMessageType::System as i32 => 
                crate::transport::TransportMessageType::System,
            _ => return Err(ClusterError::DeserializationError("Invalid transport message type".to_string())),
        };
        
        // Parse envelopes
        let mut envelopes = Vec::new();
        for proto_envelope in proto.envelopes {
            let encoded = proto_envelope.encode_to_vec();
            let envelope = MessageEnvelope::from_proto_bytes(&encoded)?;
            envelopes.push(envelope);
        }
        
        Ok(TransportMessage {
            id,
            message_type,
            source,
            target,
            sent_at,
            envelopes,
            headers: proto.headers,
        })
    }
}

// Implementation for NodeInfo
impl ConvertToProto for NodeInfo {
    fn to_proto_bytes(&self) -> Result<Vec<u8>, ClusterError> {
        // Convert timestamp
        let last_seen = match SystemTime::UNIX_EPOCH.checked_add(std::time::Duration::from_millis(self.last_seen)) {
            Some(t) => {
                let duration = t.duration_since(UNIX_EPOCH)
                    .map_err(|e| ClusterError::SerializationError(format!("Failed to convert timestamp: {}", e)))?;
                Some(prost_types::Timestamp {
                    seconds: duration.as_secs() as i64,
                    nanos: duration.subsec_nanos() as i32,
                })
            },
            None => None,
        };
        
        // Create protobuf message
        let proto = NodeInfoProto {
            id: Some(NodeIdProto {
                id: self.id.to_string(),
                is_local: self.id.is_local(),
            }),
            host: self.host.clone(),
            port: self.port as i32,
            roles: self.roles.clone(),
            metadata: self.metadata.clone(),
            last_seen,
            status: self.status.to_string(),
            load: self.load,
        };
        
        Ok(proto.encode_to_vec())
    }
}

impl ConvertFromProto for NodeInfo {
    fn from_proto_bytes(bytes: &[u8]) -> Result<Self, ClusterError> {
        let proto = NodeInfoProto::decode(bytes)
            .map_err(|e| ClusterError::DeserializationError(format!("Failed to decode NodeInfo: {}", e)))?;
        
        // Parse node ID
        let id = match proto.id {
            Some(node) => {
                let mut id = NodeId::from_string(&node.id)
                    .map_err(|e| ClusterError::DeserializationError(format!("Invalid node ID: {}", e)))?;
                if node.is_local {
                    id.set_local(true);
                }
                id
            },
            None => return Err(ClusterError::DeserializationError("Missing node ID".to_string())),
        };
        
        // Parse timestamp
        let last_seen = match proto.last_seen {
            Some(ts) => {
                let seconds_millis = ts.seconds * 1000;
                let nanos_millis = (ts.nanos / 1_000_000) as u64;
                seconds_millis as u64 + nanos_millis
            },
            None => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };
        
        // Parse status
        let status = match proto.status.as_str() {
            "Up" => crate::node::NodeStatus::Up,
            "Down" => crate::node::NodeStatus::Down,
            "Starting" => crate::node::NodeStatus::Starting,
            "Stopping" => crate::node::NodeStatus::Stopping,
            "Unknown" => crate::node::NodeStatus::Unknown,
            _ => crate::node::NodeStatus::Unknown,
        };
        
        Ok(NodeInfo {
            id,
            host: proto.host,
            port: proto.port as u16,
            roles: proto.roles,
            metadata: proto.metadata,
            last_seen,
            status,
            load: proto.load,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::DeliveryGuarantee;
    use crate::node::NodeStatus;
    
    #[test]
    fn test_message_envelope_serialization() {
        let sender = NodeId::default();
        let target = NodeId::new();
        let target_actor = "test_actor".to_string();
        let payload = vec![1, 2, 3, 4];
        
        let envelope = MessageEnvelope::new(
            sender.clone(),
            target.clone(),
            target_actor.clone(),
            MessageType::ActorMessage,
            DeliveryGuarantee::AtLeastOnce,
            payload.clone(),
        );
        
        let serializer = ProtobufSerializer::new();
        let serialized = serializer.serialize(&envelope).unwrap();
        let deserialized: MessageEnvelope = serializer.deserialize(&serialized).unwrap();
        
        assert_eq!(deserialized.message_id, envelope.message_id);
        assert_eq!(deserialized.sender_node.to_string(), sender.to_string());
        assert_eq!(deserialized.target_node.to_string(), target.to_string());
        assert_eq!(deserialized.target_actor, target_actor);
        assert_eq!(deserialized.message_type, MessageType::ActorMessage);
        assert_eq!(deserialized.delivery_guarantee, DeliveryGuarantee::AtLeastOnce);
        assert_eq!(deserialized.payload, payload);
    }
    
    #[test]
    fn test_node_info_serialization() {
        let node_id = NodeId::new();
        let node_info = NodeInfo {
            id: node_id.clone(),
            host: "localhost".to_string(),
            port: 8080,
            roles: vec!["master".to_string(), "worker".to_string()],
            metadata: {
                let mut map = std::collections::HashMap::new();
                map.insert("version".to_string(), "1.0".to_string());
                map
            },
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            status: NodeStatus::Up,
            load: 0.5,
        };
        
        let serializer = ProtobufSerializer::new();
        let serialized = serializer.serialize(&node_info).unwrap();
        let deserialized: NodeInfo = serializer.deserialize(&serialized).unwrap();
        
        assert_eq!(deserialized.id.to_string(), node_id.to_string());
        assert_eq!(deserialized.host, "localhost");
        assert_eq!(deserialized.port, 8080);
        assert_eq!(deserialized.roles, vec!["master".to_string(), "worker".to_string()]);
        assert_eq!(deserialized.metadata.get("version"), Some(&"1.0".to_string()));
        assert_eq!(deserialized.status, NodeStatus::Up);
        assert_eq!(deserialized.load, 0.5);
    }
} 