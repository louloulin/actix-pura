//! Actix Distributed Cluster Extension
//!
//! This crate provides distributed computing capabilities for the Actix actor framework.
//! It supports both centralized (master-worker) and decentralized (peer-to-peer) cluster architectures.

#![deny(rust_2018_idioms, nonstandard_style, future_incompatible)]
#![warn(missing_docs)]

pub mod cluster;
pub mod config;
pub mod node;
pub mod discovery;
pub mod transport;
pub mod registry;
pub mod broker;
pub mod error;
pub mod serialization;
pub mod message;
pub mod security;
pub mod sync;

pub use cluster::{ClusterSystem, Architecture};
pub use config::{ClusterConfig, NodeRole, DiscoveryMethod};
pub use node::{Node, NodeId, NodeInfo, NodeStatus, PlacementStrategy};
pub use discovery::ServiceDiscovery;
pub use error::{ClusterError, ClusterResult};
pub use serialization::{SerializationFormat, Serializer, SerializerTrait, BincodeSerializer, JsonSerializer};
pub use message::{ActorPath, MessageEnvelope, MessageType, DeliveryGuarantee, AnyMessage};
pub use registry::{ActorRef, LocalActorRef};

/// Re-exports from the actix crate
pub mod prelude {
    pub use actix::prelude::*;
    pub use crate::{
        ClusterSystem, ClusterConfig, Architecture, NodeRole, DiscoveryMethod,
        Node, NodeId, NodeInfo, PlacementStrategy, SerializationFormat,
    };
} 