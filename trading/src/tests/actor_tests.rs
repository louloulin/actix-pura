use actix::prelude::*;
use crate::actor::account::AccountActor;
use crate::api::adapter::messages::{AccountUpdateMessage, AccountQueryMessage, AccountUpdateResult, AccountQueryResult, FundTransferMessage, FundTransferResult};
use crate::models::account::Account;
use std::time::Duration;
use chrono::Utc;
use std::collections::HashMap;

#[test]
fn test_account_actor_create_account() {
    let system = System::new();
    
    system.block_on(async {
        // Create an account actor
        let account_actor = AccountActor::new("test-node".to_string()).start();
        
        // Create a new account with auto-generated ID
        let account_name = "Test Account".to_string();
        let response = account_actor.send(AccountUpdateMessage {
            account_id: None,  // Let the system generate an ID
            name: account_name.clone(),
            initial_balance: 1000.0,
        }).await.unwrap();
        
        let account_id = match &response {
            AccountUpdateResult::Success(account) => {
                assert_eq!(account.name, account_name);
                assert_eq!(account.balance, 1000.0);
                account.id.clone()
            },
            AccountUpdateResult::Error(e) => panic!("Failed to create account: {}", e)
        };
        
        // Query the account to verify it was created
        let query_response = account_actor.send(AccountQueryMessage::GetAccount { 
            account_id: account_id.clone() 
        }).await.unwrap();
        
        match query_response {
            AccountQueryResult::Account(account) => {
                assert_eq!(account.id, account_id);
                assert_eq!(account.name, account_name);
                assert_eq!(account.balance, 1000.0);
                assert!(account.positions.is_empty());
            },
            AccountQueryResult::Accounts(_) => panic!("Unexpected accounts response"),
            AccountQueryResult::Error(e) => panic!("Failed to get account: {}", e)
        }
    });
}

#[test]
fn test_account_actor_fund_transfer() {
    let system = System::new();
    
    system.block_on(async {
        // Create an account actor
        let account_actor = AccountActor::new("test-node".to_string()).start();
        
        // Create a new account with auto-generated ID
        let account_name = "Fund Transfer Test".to_string();
        let response = account_actor.send(AccountUpdateMessage {
            account_id: None,
            name: account_name,
            initial_balance: 1000.0,
        }).await.unwrap();
        
        let account_id = match &response {
            AccountUpdateResult::Success(account) => account.id.clone(),
            AccountUpdateResult::Error(e) => panic!("Failed to create account: {}", e)
        };
        
        // Test deposit funds
        let deposit_response = account_actor.send(FundTransferMessage {
            account_id: account_id.clone(),
            amount: 500.0,
            transfer_type: "DEPOSIT".to_string(),
            reference: Some("Test deposit".to_string()),
        }).await.unwrap();
        
        match &deposit_response {
            FundTransferResult::Success(account) => {
                assert_eq!(account.balance, 1500.0);
                assert_eq!(account.available, 1500.0);
            },
            FundTransferResult::Error(e) => panic!("Failed to deposit funds: {}", e)
        }
        
        // Test freezing funds
        let freeze_response = account_actor.send(FundTransferMessage {
            account_id: account_id.clone(),
            amount: 300.0,
            transfer_type: "FREEZE".to_string(),
            reference: Some("Test freeze".to_string()),
        }).await.unwrap();
        
        match &freeze_response {
            FundTransferResult::Success(account) => {
                assert_eq!(account.balance, 1500.0);
                assert_eq!(account.available, 1200.0);
                assert_eq!(account.frozen, 300.0);
            },
            FundTransferResult::Error(e) => panic!("Failed to freeze funds: {}", e)
        }
        
        // Test unfreezing funds
        let unfreeze_response = account_actor.send(FundTransferMessage {
            account_id: account_id.clone(),
            amount: 200.0,
            transfer_type: "UNFREEZE".to_string(),
            reference: Some("Test unfreeze".to_string()),
        }).await.unwrap();
        
        match &unfreeze_response {
            FundTransferResult::Success(account) => {
                assert_eq!(account.balance, 1500.0);
                assert_eq!(account.available, 1400.0);
                assert_eq!(account.frozen, 100.0);
            },
            FundTransferResult::Error(e) => panic!("Failed to unfreeze funds: {}", e)
        }
        
        // Test withdrawing funds
        let withdraw_response = account_actor.send(FundTransferMessage {
            account_id: account_id.clone(),
            amount: 400.0,
            transfer_type: "WITHDRAW".to_string(),
            reference: Some("Test withdraw".to_string()),
        }).await.unwrap();
        
        match &withdraw_response {
            FundTransferResult::Success(account) => {
                assert_eq!(account.balance, 1100.0);
                assert_eq!(account.available, 1000.0);
                assert_eq!(account.frozen, 100.0);
            },
            FundTransferResult::Error(e) => panic!("Failed to withdraw funds: {}", e)
        }
        
        // Verify final state with query
        let query_response = account_actor.send(AccountQueryMessage::GetAccount { 
            account_id: account_id.clone() 
        }).await.unwrap();
        
        match query_response {
            AccountQueryResult::Account(account) => {
                assert_eq!(account.balance, 1100.0);
                assert_eq!(account.available, 1000.0);
                assert_eq!(account.frozen, 100.0);
            },
            AccountQueryResult::Accounts(_) => panic!("Unexpected accounts response"),
            AccountQueryResult::Error(e) => panic!("Failed to get account: {}", e)
        }
    });
}

#[cfg(test)]
mod order_tests {
    use actix::System;
    use futures::executor::block_on;
    use uuid::Uuid;
    use crate::actor::order::{OrderActor, CreateOrderMessage, QueryOrderMessage, CancelOrderMessage};
    use crate::models::order::{OrderRequest, OrderType, OrderSide, OrderQuery, CancelOrderRequest, OrderResult, Order};
    use crate::actor::{Actor, ActorContext, MessageHandler};

    #[test]
    fn test_order_actor_create_and_query() {
        let system = System::new();
        
        system.block_on(async {
            // Create actor context and order actor
            let mut ctx = ActorContext::new("/user/order".to_string());
            let mut order_actor = OrderActor::new("test-node".to_string());
            
            // Initialize the actor
            order_actor.new_context(&mut ctx);
            
            // Create an order request
            let order_request = OrderRequest {
                symbol: "BTC/USD".to_string(),
                quantity: 1,
                price: Some(50000.0),
                order_type: OrderType::Limit,
                side: OrderSide::Buy,
                client_id: "test-client".to_string(),
                order_id: Some("test-order-id".to_string()),
            };
            
            // Send create order message
            let create_msg = CreateOrderMessage { request: order_request.clone() };
            let create_result = order_actor.handle(create_msg, &mut ctx).await;
            
            // Assert order was created
            assert!(create_result.is_some());
            let order_result = create_result.unwrap().downcast::<OrderResult>().unwrap();
            
            match &*order_result {
                OrderResult::Accepted(order_id) => {
                    assert_eq!(order_id, "test-order-id");
                    
                    // Query the order using OrderQuery::ById
                    let query_msg = QueryOrderMessage { 
                        query: OrderQuery::ById(order_id.clone()) 
                    };
                    
                    let query_result = order_actor.handle(query_msg, &mut ctx).await;
                    
                    // Assert query returns the order
                    assert!(query_result.is_some());
                    let orders = query_result.unwrap().downcast::<Vec<Order>>().unwrap();
                    assert_eq!(orders.len(), 1);
                    
                    let queried_order = &orders[0];
                    assert_eq!(queried_order.order_id, *order_id);
                    assert_eq!(queried_order.symbol, order_request.symbol);
                    assert_eq!(queried_order.quantity, order_request.quantity as f64);
                    
                    // Cancel the order
                    let cancel_req = CancelOrderRequest {
                        order_id: order_id.clone(),
                        client_id: "test-client".to_string(),
                    };
                    
                    let cancel_msg = CancelOrderMessage { request: cancel_req };
                    let cancel_result = order_actor.handle(cancel_msg, &mut ctx).await;
                    
                    // Assert order was cancelled
                    assert!(cancel_result.is_some());
                    let cancel_res = cancel_result.unwrap().downcast::<OrderResult>().unwrap();
                    
                    match &*cancel_res {
                        OrderResult::Accepted(_) => {
                            // Query again to verify it's cancelled
                            let query_msg = QueryOrderMessage { 
                                query: OrderQuery::ById(order_id.clone()) 
                            };
                            
                            let query_result = order_actor.handle(query_msg, &mut ctx).await;
                            assert!(query_result.is_some());
                            
                            let orders = query_result.unwrap().downcast::<Vec<Order>>().unwrap();
                            assert_eq!(orders.len(), 1);
                            assert_eq!(orders[0].status, crate::models::order::OrderStatus::Cancelled);
                        },
                        _ => panic!("Expected OrderResult::Accepted for cancellation"),
                    }
                },
                _ => panic!("Expected OrderResult::Accepted on order creation"),
            }
            
            System::current().stop();
        });
    }
} 