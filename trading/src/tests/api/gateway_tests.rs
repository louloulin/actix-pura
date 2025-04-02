use std::sync::Arc;
use std::collections::HashMap;
use actix::prelude::*;
use actix_web::{test, web, App};
use chrono::Utc;
use uuid::Uuid;

use crate::actor::{ActorSystem, LocalActorRef, ActorRef};
use crate::api::{
    ApiGatewayActor, ActixOrderActor, ActixAccountActor, ActixRiskActor,
    AccountQueryMessage, OrderQueryMessage, CreateOrderMessage, CancelOrderMessage,
};
// 直接从gateway模块导入
use crate::api::gateway::configure_routes;
use crate::models::account::{Account, Position};
use crate::models::order::{Order, OrderSide, OrderType, OrderStatus, OrderRequest};

// Mock actors for testing

#[derive(Default)]
struct MockOrderActor;

impl Actor for MockOrderActor {
    type Context = Context<Self>;
}

impl Handler<CreateOrderMessage> for MockOrderActor {
    type Result = ResponseFuture<crate::api::OrderCreateResult>;

    fn handle(&mut self, msg: CreateOrderMessage, _: &mut Self::Context) -> Self::Result {
        // Return a successful result with the same order
        let order = msg.order.clone();
        Box::pin(async move {
            crate::api::OrderCreateResult::Success(order)
        })
    }
}

impl Handler<CancelOrderMessage> for MockOrderActor {
    type Result = ResponseFuture<crate::api::OrderCancelResult>;

    fn handle(&mut self, _: CancelOrderMessage, _: &mut Self::Context) -> Self::Result {
        Box::pin(async move {
            crate::api::OrderCancelResult::Success
        })
    }
}

impl Handler<OrderQueryMessage> for MockOrderActor {
    type Result = ResponseFuture<crate::api::OrderQueryResult>;

    fn handle(&mut self, msg: OrderQueryMessage, _: &mut Self::Context) -> Self::Result {
        let result = match msg {
            OrderQueryMessage::GetOrder { order_id } => {
                // Create a dummy order with the requested ID
                let order = Order {
                    order_id: order_id,
                    account_id: "test-account".to_string(),
                    symbol: "BTC/USD".to_string(),
                    side: OrderSide::Buy,
                    order_type: OrderType::Limit,
                    price: Some(50000.0),
                    quantity: 1.0,
                    filled_quantity: 0.0,
                    status: OrderStatus::Accepted,
                    stop_price: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                crate::api::OrderQueryResult::Order(order)
            },
            OrderQueryMessage::GetAllOrders => {
                // Return an empty list
                crate::api::OrderQueryResult::Orders(Vec::new())
            },
            _ => crate::api::OrderQueryResult::Orders(Vec::new())
        };
        
        Box::pin(async move {
            result
        })
    }
}

#[derive(Default)]
struct MockAccountActor;

impl Actor for MockAccountActor {
    type Context = Context<Self>;
}

impl Handler<AccountQueryMessage> for MockAccountActor {
    type Result = ResponseFuture<crate::api::AccountQueryResult>;

    fn handle(&mut self, msg: AccountQueryMessage, _: &mut Self::Context) -> Self::Result {
        let result = match msg {
            AccountQueryMessage::GetAccount { account_id } => {
                // Create a dummy account with the requested ID
                let account = Account {
                    id: account_id,
                    name: "Test Account".to_string(),
                    balance: 100000.0,
                    available: 90000.0,
                    frozen: 10000.0,
                    positions: HashMap::new(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                crate::api::AccountQueryResult::Account(account)
            },
            AccountQueryMessage::GetAllAccounts => {
                // Return an empty list
                crate::api::AccountQueryResult::Accounts(Vec::new())
            }
        };
        
        Box::pin(async move {
            result
        })
    }
}

#[derive(Default)]
struct MockRiskActor;

impl Actor for MockRiskActor {
    type Context = Context<Self>;
}

impl Handler<crate::api::RiskCheckMessage> for MockRiskActor {
    type Result = ResponseFuture<crate::api::RiskCheckResult>;

    fn handle(&mut self, _: crate::api::RiskCheckMessage, _: &mut Self::Context) -> Self::Result {
        Box::pin(async move {
            // Always approve
            crate::api::RiskCheckResult::Approved
        })
    }
}

// 测试API网关功能
#[actix_rt::test]
async fn test_get_accounts() {
    // 创建模拟的Actor
    let account_actor = MockAccountActor::default().start();
    let order_actor = MockOrderActor::default().start();
    let risk_actor = MockRiskActor::default().start();
    
    // 创建ActorSystem
    let actor_system = Arc::new(ActorSystem::new());
    
    // 创建适配器
    let actor_ref = Box::new(LocalActorRef::new("/user/mock_order".to_string()));
    let order_adapter = ActixOrderActor::new(actor_ref, Arc::clone(&actor_system));

    let actor_ref = Box::new(LocalActorRef::new("/user/mock_account".to_string()));
    let account_adapter = ActixAccountActor::new(actor_ref, Arc::clone(&actor_system));

    let actor_ref = Box::new(LocalActorRef::new("/user/mock_risk".to_string()));
    let risk_adapter = ActixRiskActor::new(actor_ref, Arc::clone(&actor_system));
    
    // 启动适配器Actor
    let api_gateway = web::Data::new(ApiGatewayActor::new(
        "test-node".to_string(),
        order_adapter.start(),
        account_adapter.start(),
        risk_adapter.start(),
    ));
    
    // 创建测试App
    let mut app = test::init_service(
        App::new()
            .app_data(api_gateway.clone())
            .configure(configure_routes)
    ).await;
    
    // 发送获取账户列表请求
    let req = test::TestRequest::get().uri("/api/accounts").to_request();
    let resp = test::call_service(&app, req).await;
    
    // 确认状态码为200
    assert_eq!(resp.status().as_u16(), 200);
    
    // 获取响应体并解析
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // 确认响应格式正确
    assert_eq!(response["success"], true);
}

// 添加更多测试... 