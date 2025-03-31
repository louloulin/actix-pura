// Migration module for distributed actor migration between nodes

use std::sync::Arc;
use std::time::Duration;
use actix::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{ClusterError, ClusterResult};
use crate::node::{NodeId, PlacementStrategy};
use crate::registry::ActorRegistry;
use crate::transport::{P2PTransport, TransportMessage};
use crate::serialization::{serialize, deserialize};
use crate::message::{MessageEnvelope, MessageType, DeliveryGuarantee};

/// Migration protocol message type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationProtocol {
    /// Request to migrate an actor
    MigrationRequest,
    /// Response to a migration request
    MigrationResponse,
    /// Actor state transfer
    StateTransfer,
    /// Migration completed
    MigrationComplete,
    /// Migration failed
    MigrationFailed,
}

/// Migration status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStatus {
    /// Migration is in progress
    InProgress,
    /// Migration completed successfully
    Completed,
    /// Migration failed
    Failed,
}

/// Reason for actor migration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationReason {
    /// Migration for load balancing
    LoadBalancing,
    /// Migration due to node failure
    NodeFailure,
    /// Migration for locality optimization
    LocalityOptimization,
    /// Migration requested by application
    ApplicationRequest,
}

/// Migration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationOptions {
    /// Whether to preserve the actor's state
    pub preserve_state: bool,
    /// Whether to keep the original actor running until migration is complete
    pub keep_original: bool,
    /// Maximum time allowed for migration
    pub timeout: Duration,
    /// Priority of the migration (higher value means higher priority)
    pub priority: u8,
}

impl Default for MigrationOptions {
    fn default() -> Self {
        Self {
            preserve_state: true,
            keep_original: true,
            timeout: Duration::from_secs(30),
            priority: 5,
        }
    }
}

/// Actor state for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorState {
    /// Actor type ID
    pub actor_type: String,
    /// Actor path
    pub path: String,
    /// Actor serialized state
    pub state: Vec<u8>,
}

/// Migration request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRequest {
    /// ID of the migration request
    pub id: Uuid,
    /// Source node ID
    pub source_node: NodeId,
    /// Target node ID
    pub target_node: NodeId,
    /// Actor path to migrate
    pub actor_path: String,
    /// Actor type
    pub actor_type: String,
    /// Migration reason
    pub reason: MigrationReason,
    /// Migration options
    pub options: MigrationOptions,
}

/// Migration response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResponse {
    /// ID of the migration request being responded to
    pub request_id: Uuid,
    /// Whether the migration request was accepted
    pub accepted: bool,
    /// Reason for rejection if not accepted
    pub rejection_reason: Option<String>,
}

/// Actor state transfer message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransfer {
    /// ID of the migration request
    pub request_id: Uuid,
    /// Actor state
    pub state: ActorState,
}

/// Migration complete message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationComplete {
    /// ID of the migration request
    pub request_id: Uuid,
    /// New actor reference
    pub new_actor_path: String,
}

/// Migration failed message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationFailed {
    /// ID of the migration request
    pub request_id: Uuid,
    /// Reason for failure
    pub reason: String,
}

/// Migration manager for handling actor migrations
pub struct MigrationManager {
    /// Local node ID
    local_node_id: NodeId,
    /// Actor registry reference
    registry: Arc<ActorRegistry>,
    /// Transport layer for communication
    transport: Option<Arc<tokio::sync::Mutex<P2PTransport>>>,
    /// Active migrations
    active_migrations: Vec<(Uuid, MigrationStatus)>,
}

impl MigrationManager {
    /// Create a new migration manager
    pub fn new(local_node_id: NodeId, registry: Arc<ActorRegistry>) -> Self {
        Self {
            local_node_id,
            registry,
            transport: None,
            active_migrations: Vec::new(),
        }
    }
    
    /// Set the transport layer
    pub fn set_transport(&mut self, transport: Arc<tokio::sync::Mutex<P2PTransport>>) {
        self.transport = Some(transport);
    }
    
    /// Initiate migration of an actor to another node
    pub async fn migrate_actor(
        &mut self,
        actor_path: &str,
        target_node: NodeId,
        reason: MigrationReason,
        options: MigrationOptions,
    ) -> ClusterResult<Uuid> {
        // Check if the actor exists locally
        if let Some(actor_ref) = self.registry.lookup(actor_path) {
            // Generate migration ID
            let migration_id = Uuid::new_v4();
            
            // Create migration request
            let request = MigrationRequest {
                id: migration_id,
                source_node: self.local_node_id.clone(),
                target_node: target_node.clone(),
                actor_path: actor_path.to_string(),
                actor_type: "unknown".to_string(), // This would need to be determined
                reason,
                options,
            };
            
            // Record active migration
            self.active_migrations.push((migration_id, MigrationStatus::InProgress));
            
            // Send migration request to target node
            if let Some(transport) = &self.transport {
                let request_bytes = serialize(&request)?;
                
                // Create a message envelope
                let envelope = MessageEnvelope::new(
                    self.local_node_id.clone(),
                    target_node.clone(),
                    "/system/migration-manager".to_string(), // A convention for the migration manager
                    MessageType::Custom("migration_request".to_string()),
                    DeliveryGuarantee::AtLeastOnce,
                    request_bytes,
                );
                
                let mut transport_lock = transport.lock().await;
                transport_lock.send_message(&target_node, TransportMessage::Envelope(envelope)).await?;
                
                Ok(migration_id)
            } else {
                Err(ClusterError::TransportError("Transport not set".to_string()))
            }
        } else {
            Err(ClusterError::ActorNotFound(actor_path.to_string()))
        }
    }
    
    /// Handle incoming migration request
    pub async fn handle_migration_request(&mut self, request: MigrationRequest) -> ClusterResult<()> {
        // Check if we can accept the migration
        let can_accept = true; // In a real implementation, this would check resources
        
        let response = MigrationResponse {
            request_id: request.id,
            accepted: can_accept,
            rejection_reason: if can_accept { None } else { Some("Not enough resources".to_string()) },
        };
        
        // Send response back to the source node
        if let Some(transport) = &self.transport {
            let response_bytes = serialize(&response)?;
            
            // Create a message envelope 
            let envelope = MessageEnvelope::new(
                self.local_node_id.clone(),
                request.source_node.clone(),
                "/system/migration-manager".to_string(), // A convention for the migration manager
                MessageType::Custom("migration_response".to_string()),
                DeliveryGuarantee::AtLeastOnce,
                response_bytes,
            );
            
            let mut transport_lock = transport.lock().await;
            transport_lock.send_message(&request.source_node, TransportMessage::Envelope(envelope)).await?;
            
            if can_accept {
                // Record active migration
                self.active_migrations.push((request.id, MigrationStatus::InProgress));
            }
            
            Ok(())
        } else {
            Err(ClusterError::TransportError("Transport not set".to_string()))
        }
    }
    
    /// Handle migration response
    pub async fn handle_migration_response(&mut self, response: MigrationResponse) -> ClusterResult<()> {
        if let Some(position) = self.active_migrations.iter().position(|(id, _)| *id == response.request_id) {
            if response.accepted {
                // If accepted, prepare to transfer state
                // In a real implementation, this would extract the actor state and send it
                
                // For now, just update the status
                self.active_migrations[position].1 = MigrationStatus::InProgress;
            } else {
                // If rejected, mark migration as failed
                self.active_migrations[position].1 = MigrationStatus::Failed;
            }
            
            Ok(())
        } else {
            Err(ClusterError::ProtocolError(format!("Unknown migration request ID: {}", response.request_id)))
        }
    }
    
    /// Complete a migration
    pub async fn complete_migration(&mut self, request_id: Uuid, new_actor_path: String) -> ClusterResult<()> {
        if let Some(position) = self.active_migrations.iter().position(|(id, _)| *id == request_id) {
            self.active_migrations[position].1 = MigrationStatus::Completed;
            
            // In a real implementation, this would update references and clean up
            
            Ok(())
        } else {
            Err(ClusterError::ProtocolError(format!("Unknown migration request ID: {}", request_id)))
        }
    }
    
    /// Fail a migration
    pub async fn fail_migration(&mut self, request_id: Uuid, reason: String) -> ClusterResult<()> {
        if let Some(position) = self.active_migrations.iter().position(|(id, _)| *id == request_id) {
            self.active_migrations[position].1 = MigrationStatus::Failed;
            
            // In a real implementation, this would clean up and possibly retry
            
            Ok(())
        } else {
            Err(ClusterError::ProtocolError(format!("Unknown migration request ID: {}", request_id)))
        }
    }
    
    /// Get migration status
    pub fn get_migration_status(&self, migration_id: Uuid) -> Option<MigrationStatus> {
        self.active_migrations
            .iter()
            .find(|(id, _)| *id == migration_id)
            .map(|(_, status)| *status)
    }
}

/// An actor that implements migration capabilities
pub trait MigratableActor: Actor {
    /// Get the current state for migration
    fn get_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
    
    /// Restore from migrated state
    fn restore_state(&mut self, state: Vec<u8>) -> Result<(), Box<dyn std::error::Error>>;
    
    /// Called before migration starts
    fn before_migration(&mut self, ctx: &mut Self::Context) {}
    
    /// Called after migration completes
    fn after_migration(&mut self, ctx: &mut Self::Context) {}
    
    /// Whether this actor can be migrated
    fn can_migrate(&self) -> bool {
        true
    }
}

/// Message to initiate actor migration
#[derive(Message)]
#[rtype(result = "ClusterResult<Uuid>")]
pub struct MigrateActor {
    /// Target node for migration
    pub target_node: NodeId,
    /// Reason for migration
    pub reason: MigrationReason,
    /// Migration options
    pub options: MigrationOptions,
}

// Messages for the migration protocol would be defined and handled here 