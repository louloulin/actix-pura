//! Actix集群模块，为Actix提供分布式actor功能
//!
//! 该模块提供了actor的分布式部署、消息路由、发现和容错等功能。

#![deny(unsafe_code)]
#![warn(unused_imports)]
#![warn(missing_docs)]

pub mod actor;
pub mod broker;
pub mod cluster;
pub mod compression;
pub mod config;
pub mod consensus;
pub mod consensus_network;
pub mod discovery;
pub mod error;
pub mod master;
pub mod message;
pub mod migration;
pub mod node;
pub mod placement;
pub mod registry;
pub mod security;
pub mod serialization;
pub mod supervision;
pub mod sync;
pub mod transport;

// 添加测试工具模块，用于测试和集成测试
#[doc(hidden)]
pub mod testing;

pub use actor::{ClusterSystemActor, DistributedActor};
pub use cluster::{Architecture, ClusterSystem};
pub use compression::{CompressionConfig, CompressionLevel, CompressionAlgorithm};
pub use config::{ClusterConfig, DiscoveryMethod, NodeRole};
pub use consensus::{ConsensusActor, ConsensusCommand, ConsensusResponse, ConsensusState, GetConsensusState};
pub use discovery::ServiceDiscovery;
pub use error::{ClusterError, ClusterResult};
pub use message::{ActorPath, AnyMessage, DeliveryGuarantee, MessageEnvelope, MessageType};
pub use migration::{MigratableActor, MigrationManager, MigrationOptions, MigrationReason, MigrationStatus};
pub use node::{Node, NodeId, NodeInfo, NodeStatus, PlacementStrategy};
pub use placement::{NodeSelector, PlacementStrategyImpl};
pub use registry::{ActorRef, LocalActorRef};
pub use serialization::{BincodeSerializer, JsonSerializer, SerializationFormat, Serializer, SerializerTrait};
pub use supervision::{SupervisionStrategy, SupervisedDistributedActor, DistributedSupervisor, SupervisorActor};

/// Re-exports from the actix crate
pub mod prelude {
    pub use actix::prelude::*;
    pub use crate::{
        Architecture, ClusterConfig, ClusterSystem, ConsensusActor, ConsensusCommand, ConsensusState,
        DiscoveryMethod, DistributedActor, MigratableActor, MigrationReason, Node, NodeId, NodeInfo,
        NodeRole, PlacementStrategy, SerializationFormat, SupervisionStrategy, SupervisedDistributedActor,
    };
} 