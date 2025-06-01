/// Node identification
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NodeIdProto {
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    #[prost(bool, tag = "2")]
    pub is_local: bool,
}
/// Actor path for identifying actors in the cluster
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ActorPathProto {
    #[prost(message, optional, tag = "1")]
    pub node_id: ::core::option::Option<NodeIdProto>,
    #[prost(string, tag = "2")]
    pub path: ::prost::alloc::string::String,
}
/// Custom message type identifier
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CustomMessageTypeProto {
    #[prost(enumeration = "MessageTypeProto", tag = "1")]
    pub base_type: i32,
    #[prost(string, tag = "2")]
    pub custom_type: ::prost::alloc::string::String,
}
/// Message envelope for routing and tracking messages
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MessageEnvelopeProto {
    #[prost(string, tag = "1")]
    pub message_id: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub sender_node: ::core::option::Option<NodeIdProto>,
    #[prost(message, optional, tag = "3")]
    pub target_node: ::core::option::Option<NodeIdProto>,
    #[prost(string, tag = "4")]
    pub target_actor: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "5")]
    pub timestamp: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(enumeration = "MessageTypeProto", tag = "6")]
    pub message_type: i32,
    /// Only used when message_type is CUSTOM
    #[prost(string, tag = "7")]
    pub custom_message_type: ::prost::alloc::string::String,
    #[prost(enumeration = "DeliveryGuaranteeProto", tag = "8")]
    pub delivery_guarantee: i32,
    #[prost(bytes = "vec", tag = "9")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
    #[prost(bool, tag = "10")]
    pub compressed: bool,
}
/// Transport message for cluster communication
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TransportMessageProto {
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    #[prost(enumeration = "transport_message_proto::TransportMessageType", tag = "2")]
    pub r#type: i32,
    #[prost(message, optional, tag = "3")]
    pub source: ::core::option::Option<NodeIdProto>,
    #[prost(message, optional, tag = "4")]
    pub target: ::core::option::Option<NodeIdProto>,
    #[prost(message, optional, tag = "5")]
    pub sent_at: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(message, repeated, tag = "6")]
    pub envelopes: ::prost::alloc::vec::Vec<MessageEnvelopeProto>,
    #[prost(map = "string, string", tag = "7")]
    pub headers: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}
/// Nested message and enum types in `TransportMessageProto`.
pub mod transport_message_proto {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum TransportMessageType {
        Data = 0,
        Ack = 1,
        Discovery = 2,
        HealthCheck = 3,
        System = 4,
    }
    impl TransportMessageType {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                TransportMessageType::Data => "DATA",
                TransportMessageType::Ack => "ACK",
                TransportMessageType::Discovery => "DISCOVERY",
                TransportMessageType::HealthCheck => "HEALTH_CHECK",
                TransportMessageType::System => "SYSTEM",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "DATA" => Some(Self::Data),
                "ACK" => Some(Self::Ack),
                "DISCOVERY" => Some(Self::Discovery),
                "HEALTH_CHECK" => Some(Self::HealthCheck),
                "SYSTEM" => Some(Self::System),
                _ => None,
            }
        }
    }
}
/// Node information for discovery and clustering
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NodeInfoProto {
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<NodeIdProto>,
    #[prost(string, tag = "2")]
    pub host: ::prost::alloc::string::String,
    #[prost(int32, tag = "3")]
    pub port: i32,
    #[prost(string, repeated, tag = "4")]
    pub roles: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(map = "string, string", tag = "5")]
    pub metadata: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
    #[prost(message, optional, tag = "6")]
    pub last_seen: ::core::option::Option<::prost_types::Timestamp>,
    #[prost(string, tag = "7")]
    pub status: ::prost::alloc::string::String,
    #[prost(double, tag = "8")]
    pub load: f64,
}
/// Different message delivery guarantees
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DeliveryGuaranteeProto {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}
impl DeliveryGuaranteeProto {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            DeliveryGuaranteeProto::AtMostOnce => "AT_MOST_ONCE",
            DeliveryGuaranteeProto::AtLeastOnce => "AT_LEAST_ONCE",
            DeliveryGuaranteeProto::ExactlyOnce => "EXACTLY_ONCE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "AT_MOST_ONCE" => Some(Self::AtMostOnce),
            "AT_LEAST_ONCE" => Some(Self::AtLeastOnce),
            "EXACTLY_ONCE" => Some(Self::ExactlyOnce),
            _ => None,
        }
    }
}
/// Types of messages in the system
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum MessageTypeProto {
    ActorMessage = 0,
    SystemControl = 1,
    Discovery = 2,
    Ping = 3,
    Pong = 4,
    Custom = 5,
}
impl MessageTypeProto {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            MessageTypeProto::ActorMessage => "ACTOR_MESSAGE",
            MessageTypeProto::SystemControl => "SYSTEM_CONTROL",
            MessageTypeProto::Discovery => "DISCOVERY",
            MessageTypeProto::Ping => "PING",
            MessageTypeProto::Pong => "PONG",
            MessageTypeProto::Custom => "CUSTOM",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ACTOR_MESSAGE" => Some(Self::ActorMessage),
            "SYSTEM_CONTROL" => Some(Self::SystemControl),
            "DISCOVERY" => Some(Self::Discovery),
            "PING" => Some(Self::Ping),
            "PONG" => Some(Self::Pong),
            "CUSTOM" => Some(Self::Custom),
            _ => None,
        }
    }
}
