use std::sync::Arc;
use actix::prelude::*;
use log::info;
use crate::risk::manager::RiskManager;
use crate::risk::manager::{RiskCheckRequest, RiskCheckResult as CustomRiskCheckResult};
use crate::actor::{ActorRef, ActorSystem};
use crate::models::order::{OrderRequest};
use super::messages::{RiskCheckMessage, RiskCheckResult};
use uuid;

/// Adapter that wraps our custom RiskManager and implements actix::Actor
pub struct ActixRiskActor {
    /// Internal actor reference
    internal_actor: Box<dyn ActorRef>,
    /// Actor system reference
    actor_system: Arc<ActorSystem>,
}

impl ActixRiskActor {
    /// Create a new ActixRiskActor with a reference to the internal actor
    pub fn new(internal_actor: Box<dyn ActorRef>, actor_system: Arc<ActorSystem>) -> Self {
        Self {
            internal_actor,
            actor_system,
        }
    }

    /// Create a new ActixRiskActor from a CustomRiskManager
    pub fn from_custom(risk_manager: RiskManager, actor_system: Arc<ActorSystem>) -> Self {
        // For now, create a default LocalActorRef as a placeholder
        // In a real implementation, this would properly convert the RiskManager
        // to a compatible ActorRef
        use crate::actor::LocalActorRef;
        let internal_actor = Box::new(LocalActorRef::new(format!("/user/risk_manager_{}", uuid::Uuid::new_v4())));
        
        Self {
            internal_actor,
            actor_system,
        }
    }
}

impl Actor for ActixRiskActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("ActixRiskActor started");
    }
}

impl Handler<RiskCheckMessage> for ActixRiskActor {
    type Result = ResponseFuture<RiskCheckResult>;

    fn handle(&mut self, msg: RiskCheckMessage, _ctx: &mut Self::Context) -> Self::Result {
        let actor_ref = self.internal_actor.clone();
        let actor_system = Arc::clone(&self.actor_system);
        
        // Convert actix message to our custom message
        let order_request = OrderRequest {
            order_id: None,
            symbol: msg.symbol.clone(),
            side: msg.side,
            price: msg.price,
            quantity: msg.quantity as u64,
            client_id: msg.account_id.clone(),
            order_type: msg.order_type,
        };
        
        let custom_msg = RiskCheckRequest {
            order: order_request,
            account_id: msg.account_id,
        };
        
        Box::pin(async move {
            // Send message to internal actor
            let result = actor_system.ask::<RiskCheckRequest, CustomRiskCheckResult>(
                actor_ref,
                custom_msg
            ).await;
            
            // Convert result back to actix message result
            match result {
                Some(CustomRiskCheckResult::Approved) => {
                    RiskCheckResult::Approved
                },
                Some(CustomRiskCheckResult::Rejected(reason)) => {
                    RiskCheckResult::Rejected(reason)
                },
                None => {
                    RiskCheckResult::Rejected("No response from risk manager".to_string())
                }
            }
        })
    }
} 