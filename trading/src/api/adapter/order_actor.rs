use std::sync::Arc;
use actix::prelude::*;
use log::info;
use crate::actor::order::OrderActor as CustomOrderActor;
use crate::actor::{ActorRef, ActorSystem};
use crate::models::order::{Order, OrderQuery, OrderResult, CancelOrderRequest, OrderRequest};
use super::messages::{
    CreateOrderMessage, OrderCreateResult,
    CancelOrderMessage, OrderCancelResult,
    OrderQueryMessage, OrderQueryResult
};
use uuid;

/// Adapter that wraps our custom OrderActor and implements actix::Actor
pub struct ActixOrderActor {
    /// Internal actor reference
    internal_actor: Box<dyn ActorRef>,
    /// Actor system reference
    actor_system: Arc<ActorSystem>,
}

impl ActixOrderActor {
    /// Create a new ActixOrderActor with a reference to the internal actor
    pub fn new(internal_actor: Box<dyn ActorRef>, actor_system: Arc<ActorSystem>) -> Self {
        Self {
            internal_actor,
            actor_system,
        }
    }

    /// Create a new ActixOrderActor from a CustomOrderActor
    pub fn from_custom(order_actor: CustomOrderActor, actor_system: Arc<ActorSystem>) -> Self {
        // For now, create a default LocalActorRef as a placeholder
        // In a real implementation, this would properly convert the CustomOrderActor
        // to a compatible ActorRef
        use crate::actor::LocalActorRef;
        let internal_actor = Box::new(LocalActorRef::new(format!("/user/order_actor_{}", uuid::Uuid::new_v4())));
        
        Self {
            internal_actor,
            actor_system,
        }
    }
}

impl Actor for ActixOrderActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("ActixOrderActor started");
    }
}

impl Handler<CreateOrderMessage> for ActixOrderActor {
    type Result = ResponseFuture<OrderCreateResult>;

    fn handle(&mut self, msg: CreateOrderMessage, _ctx: &mut Self::Context) -> Self::Result {
        let actor_ref = self.internal_actor.clone();
        let actor_system = Arc::clone(&self.actor_system);
        
        // Convert actix message to our custom message
        let order = msg.order;
        
        // Create an order request from the order
        let request = OrderRequest {
            order_id: Some(order.order_id.clone()),
            symbol: order.symbol.clone(),
            side: order.side,
            price: order.price,
            quantity: order.quantity as u64, // Ensure proper type conversion
            client_id: order.account_id.clone(),
            order_type: order.order_type,
        };
        
        let custom_msg = crate::actor::order::CreateOrderMessage { request };
        
        Box::pin(async move {
            // Send message to internal actor
            let result = actor_system.ask::<crate::actor::order::CreateOrderMessage, OrderResult>(
                actor_ref,
                custom_msg
            ).await;
            
            // Convert result back to actix message result
            match result {
                Some(OrderResult::Accepted(order_id)) => {
                    // Create a new order with the result
                    let order = Order {
                        order_id: order_id,
                        ..order.clone()
                    };
                    OrderCreateResult::Success(order)
                },
                Some(OrderResult::Rejected(reason)) => {
                    OrderCreateResult::Error(reason)
                },
                Some(OrderResult::Error(error)) => {
                    OrderCreateResult::Error(error)
                },
                None => {
                    OrderCreateResult::Error("No response from order actor".to_string())
                }
            }
        })
    }
}

impl Handler<CancelOrderMessage> for ActixOrderActor {
    type Result = ResponseFuture<OrderCancelResult>;

    fn handle(&mut self, msg: CancelOrderMessage, _ctx: &mut Self::Context) -> Self::Result {
        let actor_ref = self.internal_actor.clone();
        let actor_system = Arc::clone(&self.actor_system);
        
        // Convert actix message to our custom message
        let cancel_request = CancelOrderRequest {
            order_id: msg.order_id,
            client_id: msg.requested_by,
        };
        
        let custom_msg = crate::actor::order::CancelOrderMessage { 
            request: cancel_request
        };
        
        Box::pin(async move {
            // Send message to internal actor
            let result = actor_system.ask::<crate::actor::order::CancelOrderMessage, OrderResult>(
                actor_ref,
                custom_msg
            ).await;
            
            // Convert result back to actix message result
            match result {
                Some(OrderResult::Accepted(_)) => {
                    OrderCancelResult::Success
                },
                Some(OrderResult::Rejected(reason)) => {
                    OrderCancelResult::Error(reason)
                },
                Some(OrderResult::Error(error)) => {
                    OrderCancelResult::Error(error)
                },
                None => {
                    OrderCancelResult::Error("No response from order actor".to_string())
                }
            }
        })
    }
}

impl Handler<OrderQueryMessage> for ActixOrderActor {
    type Result = ResponseFuture<OrderQueryResult>;

    fn handle(&mut self, msg: OrderQueryMessage, _ctx: &mut Self::Context) -> Self::Result {
        let actor_ref = self.internal_actor.clone();
        let actor_system = Arc::clone(&self.actor_system);
        
        // Convert actix message to our custom message
        let query = match msg {
            OrderQueryMessage::GetOrder { order_id } => {
                OrderQuery::ById(order_id)
            },
            OrderQueryMessage::GetOrdersByAccount { account_id } => {
                OrderQuery::ByAccount(account_id)
            },
            OrderQueryMessage::GetOrdersBySymbol { symbol } => {
                OrderQuery::BySymbol(symbol)
            },
            OrderQueryMessage::GetOrdersByStatus { status } => {
                OrderQuery::ByStatus(status)
            },
            OrderQueryMessage::GetAllOrders => {
                OrderQuery::All
            },
        };
        
        let custom_msg = crate::actor::order::QueryOrderMessage { query };
        
        Box::pin(async move {
            // Send message to internal actor
            let result = actor_system.ask::<crate::actor::order::QueryOrderMessage, Vec<Order>>(
                actor_ref,
                custom_msg
            ).await;
            
            // Convert result back to actix message result
            match result {
                Some(orders) if orders.len() == 1 => {
                    OrderQueryResult::Order(orders[0].clone())
                },
                Some(orders) => {
                    OrderQueryResult::Orders(orders)
                },
                None => {
                    OrderQueryResult::Error("No response from order actor".to_string())
                }
            }
        })
    }
} 