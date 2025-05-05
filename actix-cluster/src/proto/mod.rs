//! Protocol Buffers module for message serialization.
//!
//! This module provides Protocol Buffers serialization for messages
//! exchanged between nodes in the cluster.

use prost::Message;
use uuid::Uuid;
use std::collections::HashMap;
use crate::node::{NodeId, NodeInfo, NodeStatus};
use crate::message::{MessageEnvelope, MessageType, DeliveryGuarantee, ActorPath};
use crate::transport::TransportMessage;
use crate::error::{ClusterError, ClusterResult};
use crate::broker::{BrokerMessage, SubscriptionOptions};
use crate::master::{ElectionMessage, MasterState};

// Include the generated code from prost
include!(concat!(env!("OUT_DIR"), "/actix_cluster.rs"));

impl From<MessageType> for i32 {
    fn from(message_type: MessageType) -> Self {
        match message_type {
            MessageType::ActorMessage => 0,
            MessageType::SystemControl => 1,
            MessageType::Discovery => 2,
            MessageType::Ping => 3,
            MessageType::Pong => 4,
            MessageType::Custom(_) => 5,
        }
    }
}

impl From<i32> for MessageType {
    fn from(value: i32) -> Self {
        match value {
            0 => MessageType::ActorMessage,
            1 => MessageType::SystemControl,
            2 => MessageType::Discovery,
            3 => MessageType::Ping,
            4 => MessageType::Pong,
            5 => MessageType::Custom("".to_string()),
            _ => MessageType::ActorMessage,
        }
    }
}

impl From<DeliveryGuarantee> for i32 {
    fn from(guarantee: DeliveryGuarantee) -> Self {
        match guarantee {
            DeliveryGuarantee::AtMostOnce => 0,
            DeliveryGuarantee::AtLeastOnce => 1,
            DeliveryGuarantee::ExactlyOnce => 2,
        }
    }
}

impl From<i32> for DeliveryGuarantee {
    fn from(value: i32) -> Self {
        match value {
            0 => DeliveryGuarantee::AtMostOnce,
            1 => DeliveryGuarantee::AtLeastOnce,
            2 => DeliveryGuarantee::ExactlyOnce,
            _ => DeliveryGuarantee::AtMostOnce,
        }
    }
}

impl TryFrom<MessageEnvelope> for ProtoMessageEnvelope {
    type Error = ClusterError;

    fn try_from(envelope: MessageEnvelope) -> Result<Self, Self::Error> {
        let mut proto_envelope = ProtoMessageEnvelope {
            message_id: envelope.message_id.to_string(),
            sender_actor: String::new(), // Not in our MessageEnvelope
            target_actor: envelope.target_actor,
            sender_node: envelope.sender_node.to_string(),
            target_node: envelope.target_node.to_string(),
            message_type: i32::from(envelope.message_type.clone()),
            payload: envelope.payload,
            timestamp: envelope.timestamp,
            ttl: 0, // Not in our MessageEnvelope
            delivery_guarantee: i32::from(envelope.delivery_guarantee),
            correlation_id: String::new(), // Not in our MessageEnvelope
            custom_type: String::new(),
            headers: std::collections::HashMap::new(), // Not in our MessageEnvelope
        };

        // Handle custom message type
        if let MessageType::Custom(custom_type) = envelope.message_type {
            proto_envelope.custom_type = custom_type;
        }

        Ok(proto_envelope)
    }
}

impl TryFrom<ProtoMessageEnvelope> for MessageEnvelope {
    type Error = ClusterError;

    fn try_from(proto: ProtoMessageEnvelope) -> Result<Self, Self::Error> {
        let message_id = Uuid::parse_str(&proto.message_id)
            .map_err(|e| ClusterError::DeserializationError(format!("Invalid UUID: {}", e)))?;

        let sender_node = NodeId::from_str(&proto.sender_node)
            .map_err(|e| ClusterError::DeserializationError(format!("Invalid sender node ID: {}", e)))?;

        let target_node = NodeId::from_str(&proto.target_node)
            .map_err(|e| ClusterError::DeserializationError(format!("Invalid target node ID: {}", e)))?;

        let mut message_type = MessageType::from(proto.message_type);
        if let MessageType::Custom(_) = message_type {
            message_type = MessageType::Custom(proto.custom_type);
        }

        Ok(MessageEnvelope {
            message_id,
            sender_node,
            target_node,
            target_actor: proto.target_actor,
            timestamp: proto.timestamp,
            message_type,
            delivery_guarantee: DeliveryGuarantee::from(proto.delivery_guarantee),
            payload: proto.payload,
            compressed: false, // Default value
        })
    }
}

impl TryFrom<TransportMessage> for ProtoTransportMessage {
    type Error = ClusterError;

    fn try_from(message: TransportMessage) -> Result<Self, Self::Error> {
        let proto_message = match message {
            TransportMessage::ActorMessage(envelope) => {
                ProtoTransportMessage {
                    message: Some(proto_transport_message::Message::ActorMessage(envelope.try_into()?))
                }
            },
            TransportMessage::Envelope(envelope) => {
                ProtoTransportMessage {
                    message: Some(proto_transport_message::Message::Envelope(envelope.try_into()?))
                }
            },
            // Implement other message types as needed
            _ => return Err(ClusterError::SerializationError("Unsupported message type".to_string())),
        };

        Ok(proto_message)
    }
}

impl TryFrom<ProtoTransportMessage> for TransportMessage {
    type Error = ClusterError;

    fn try_from(proto: ProtoTransportMessage) -> Result<Self, Self::Error> {
        match proto.message {
            Some(proto_transport_message::Message::ActorMessage(envelope)) => {
                Ok(TransportMessage::ActorMessage(envelope.try_into()?))
            },
            Some(proto_transport_message::Message::Envelope(envelope)) => {
                Ok(TransportMessage::Envelope(envelope.try_into()?))
            },
            // Implement other message types as needed
            _ => Err(ClusterError::DeserializationError("Unsupported message type".to_string())),
        }
    }
}

/// Serialize a message using Protocol Buffers
pub fn serialize<T: prost::Message>(message: &T) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(message.encoded_len());
    message.encode(&mut buf).expect("Failed to encode message");
    buf
}

/// Deserialize a message using Protocol Buffers
pub fn deserialize<T: prost::Message + Default>(bytes: &[u8]) -> Result<T, ClusterError> {
    T::decode(bytes).map_err(|e| ClusterError::DeserializationError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeId;

    #[test]
    fn test_message_envelope_conversion() {
        let sender_node = NodeId::new();
        let target_node = NodeId::new();

        let envelope = MessageEnvelope::new(
            sender_node.clone(),
            target_node.clone(),
            "/user/target".to_string(),
            MessageType::ActorMessage,
            DeliveryGuarantee::AtLeastOnce,
            vec![1, 2, 3, 4],
        );

        // Convert to proto
        let proto_envelope: ProtoMessageEnvelope = envelope.clone().try_into().unwrap();

        // Convert back to envelope
        let converted_envelope: MessageEnvelope = proto_envelope.try_into().unwrap();

        // Check fields
        assert_eq!(converted_envelope.message_id, envelope.message_id);
        assert_eq!(converted_envelope.target_actor, envelope.target_actor);
        assert_eq!(converted_envelope.sender_node, envelope.sender_node);
        assert_eq!(converted_envelope.target_node, envelope.target_node);
        assert_eq!(converted_envelope.message_type, envelope.message_type);
        assert_eq!(converted_envelope.payload, envelope.payload);
        assert_eq!(converted_envelope.timestamp, envelope.timestamp);
        assert_eq!(converted_envelope.delivery_guarantee, envelope.delivery_guarantee);
    }

    #[test]
    fn test_custom_message_type() {
        let sender_node = NodeId::new();
        let target_node = NodeId::new();

        let envelope = MessageEnvelope::new(
            sender_node,
            target_node,
            "/user/target".to_string(),
            MessageType::Custom("TestCustomType".to_string()),
            DeliveryGuarantee::AtLeastOnce,
            vec![1, 2, 3, 4],
        );

        // Convert to proto
        let proto_envelope: ProtoMessageEnvelope = envelope.clone().try_into().unwrap();

        // Check custom type
        assert_eq!(proto_envelope.message_type, 5); // CUSTOM
        assert_eq!(proto_envelope.custom_type, "TestCustomType");

        // Convert back to envelope
        let converted_envelope: MessageEnvelope = proto_envelope.try_into().unwrap();

        // Check custom type was preserved
        if let MessageType::Custom(custom_type) = converted_envelope.message_type {
            assert_eq!(custom_type, "TestCustomType");
        } else {
            panic!("Expected Custom message type");
        }
    }
}
