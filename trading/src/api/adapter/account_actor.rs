use std::sync::Arc;
use actix::prelude::*;
use log::{debug, info, warn, error};
use crate::actor::account::AccountActor as CustomAccountActor;
use crate::models::account::Account;
use crate::actor::{ActorRef, ActorSystem};
use super::messages::{
    AccountQueryMessage, AccountQueryResult,
    AccountUpdateMessage, AccountUpdateResult,
    FundTransferMessage, FundTransferResult
};

/// ActixAccountActor serves as a wrapper for the existing AccountActor 
/// which already implements actix::Actor
pub struct ActixAccountActor {
    /// Internal actor reference
    internal_actor: Box<dyn ActorRef>,
    /// Actor system reference
    actor_system: Arc<ActorSystem>,
}

impl ActixAccountActor {
    /// Create a new ActixAccountActor with a reference to the internal actor
    pub fn new(internal_actor: Box<dyn ActorRef>, actor_system: Arc<ActorSystem>) -> Self {
        Self {
            internal_actor,
            actor_system,
        }
    }

    /// Create a new ActixAccountActor from an existing AccountActor
    pub fn from_custom(account_actor: CustomAccountActor, actor_system: Arc<ActorSystem>) -> Self {
        // For now, create a default LocalActorRef as a placeholder
        // In a real implementation, this would properly convert the CustomAccountActor
        // to a compatible ActorRef
        use crate::actor::LocalActorRef;
        let internal_actor = Box::new(LocalActorRef::new(
            format!("/user/account_actor_{}", uuid::Uuid::new_v4())
        ));
        
        Self {
            internal_actor,
            actor_system,
        }
    }
}

impl Actor for ActixAccountActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("ActixAccountActor started");
    }
}

impl Handler<AccountQueryMessage> for ActixAccountActor {
    type Result = ResponseFuture<AccountQueryResult>;

    fn handle(&mut self, msg: AccountQueryMessage, _ctx: &mut Self::Context) -> Self::Result {
        let actor_ref = self.internal_actor.clone();
        let actor_system = Arc::clone(&self.actor_system);
        
        // Use the actor_system to send the message to the internal actor
        Box::pin(async move {
            match actor_system.ask::<AccountQueryMessage, AccountQueryResult>(actor_ref, msg).await {
                Some(result) => result,
                None => AccountQueryResult::Error("No response from account actor".to_string())
            }
        })
    }
}

impl Handler<AccountUpdateMessage> for ActixAccountActor {
    type Result = ResponseFuture<AccountUpdateResult>;

    fn handle(&mut self, msg: AccountUpdateMessage, _ctx: &mut Self::Context) -> Self::Result {
        let actor_ref = self.internal_actor.clone();
        let actor_system = Arc::clone(&self.actor_system);
        
        Box::pin(async move {
            match actor_system.ask::<AccountUpdateMessage, AccountUpdateResult>(actor_ref, msg).await {
                Some(result) => result,
                None => AccountUpdateResult::Error("No response from account actor".to_string())
            }
        })
    }
}

impl Handler<FundTransferMessage> for ActixAccountActor {
    type Result = ResponseFuture<FundTransferResult>;

    fn handle(&mut self, msg: FundTransferMessage, _ctx: &mut Self::Context) -> Self::Result {
        let actor_ref = self.internal_actor.clone();
        let actor_system = Arc::clone(&self.actor_system);
        
        Box::pin(async move {
            match actor_system.ask::<FundTransferMessage, FundTransferResult>(actor_ref, msg).await {
                Some(result) => result,
                None => FundTransferResult::Error("No response from account actor".to_string())
            }
        })
    }
} 