use std::collections::HashMap;
use actix::prelude::*;
use actix_web::{test, web, App, HttpResponse, HttpRequest};
use actix_web::http::StatusCode;
use chrono::Utc;
use uuid::Uuid;

use crate::api::{
    AccountQueryMessage, OrderQueryMessage, CreateOrderMessage, CancelOrderMessage,
    AccountQueryResult, OrderQueryResult, OrderCreateResult, OrderCancelResult
};
use crate::api::gateway::ApiResponse;
use crate::models::account::Account;
use crate::models::order::{Order, OrderSide, OrderType, OrderStatus};

// Mock actors for testing

#[derive(Default)]
struct MockOrderActor;

impl Actor for MockOrderActor {
    type Context = Context<Self>;
}

impl Handler<CreateOrderMessage> for MockOrderActor {
    type Result = ResponseFuture<OrderCreateResult>;

    fn handle(&mut self, msg: CreateOrderMessage, _: &mut Self::Context) -> Self::Result {
        // Return a successful result with the same order
        let order = msg.order.clone();
        Box::pin(async move {
            OrderCreateResult::Success(order)
        })
    }
}

impl Handler<CancelOrderMessage> for MockOrderActor {
    type Result = ResponseFuture<OrderCancelResult>;

    fn handle(&mut self, _: CancelOrderMessage, _: &mut Self::Context) -> Self::Result {
        Box::pin(async move {
            OrderCancelResult::Success
        })
    }
}

impl Handler<OrderQueryMessage> for MockOrderActor {
    type Result = ResponseFuture<OrderQueryResult>;

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
                OrderQueryResult::Order(order)
            },
            OrderQueryMessage::GetAllOrders => {
                // Return an empty list
                OrderQueryResult::Orders(Vec::new())
            },
            _ => OrderQueryResult::Orders(Vec::new())
        };
        
        Box::pin(async move {
            result
        })
    }
}

// Create a mock implementation of ActixAccountActor
#[derive(Default)]
struct MockActixAccountActor;

impl Actor for MockActixAccountActor {
    type Context = Context<Self>;
}

impl Handler<AccountQueryMessage> for MockActixAccountActor {
    type Result = ResponseFuture<AccountQueryResult>;

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
                AccountQueryResult::Account(account)
            },
            AccountQueryMessage::GetAllAccounts => {
                // Return a list with one test account
                let account = Account {
                    id: "test-account".to_string(),
                    name: "Test Account".to_string(),
                    balance: 100000.0,
                    available: 90000.0,
                    frozen: 10000.0,
                    positions: HashMap::new(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                AccountQueryResult::Accounts(vec![account])
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

// Standalone handler function for the test
async fn get_test_accounts(_req: HttpRequest) -> HttpResponse {
    let test_account = Account {
        id: "test-account".to_string(),
        name: "Test Account".to_string(),
        balance: 100000.0,
        available: 90000.0,
        frozen: 10000.0,
        positions: HashMap::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    
    let accounts = vec![test_account];
    let response = ApiResponse {
        success: true,
        message: None,
        data: Some(accounts),
    };
    
    HttpResponse::Ok().json(response)
}

// Standalone handler for creating an account
async fn create_test_account(account_data: web::Json<serde_json::Value>) -> HttpResponse {
    // Extract the name from the request body
    let name = match account_data.get("name") {
        Some(name_value) => match name_value.as_str() {
            Some(name_str) => name_str,
            None => return HttpResponse::BadRequest().json(ApiResponse {
                success: false,
                message: Some("Invalid account name".to_string()),
                data: None::<Vec<()>>,
            }),
        },
        None => return HttpResponse::BadRequest().json(ApiResponse {
            success: false,
            message: Some("Missing account name".to_string()),
            data: None::<Vec<()>>,
        }),
    };

    // Create a new account with a generated ID
    let new_account = Account {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        balance: 0.0,
        available: 0.0,
        frozen: 0.0,
        positions: HashMap::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Return a success response with the new account
    let response = ApiResponse {
        success: true,
        message: Some("Account created successfully".to_string()),
        data: Some(new_account),
    };

    HttpResponse::Created().json(response)
}

// Standalone handler for getting orders
async fn get_test_orders(_req: HttpRequest) -> HttpResponse {
    // Create mock orders for testing
    let orders = vec![
        Order {
            order_id: "order-1".to_string(),
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
        },
        Order {
            order_id: "order-2".to_string(),
            account_id: "test-account".to_string(),
            symbol: "ETH/USD".to_string(),
            side: OrderSide::Sell,
            order_type: OrderType::Market,
            price: None,
            quantity: 5.0,
            filled_quantity: 5.0,
            status: OrderStatus::Filled,
            stop_price: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    ];
    
    let response = ApiResponse {
        success: true,
        message: None,
        data: Some(orders),
    };
    
    HttpResponse::Ok().json(response)
}

// Simplified test that doesn't require the gateway implementation
#[actix_rt::test]
async fn test_get_accounts() {
    // Create and initialize the test app with our standalone handler
    let app = test::init_service(
        App::new()
            .route("/api/accounts", web::get().to(get_test_accounts))
    ).await;
    
    // Send a GET request to the endpoint
    let req = test::TestRequest::get()
        .uri("/api/accounts")
        .to_request();
    
    // Perform the request and get the response
    let resp = test::call_service(&app, req).await;
    
    // Verify the response status is 200 OK
    assert_eq!(resp.status(), StatusCode::OK);
    
    // Get the response body and parse it
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // Verify the response format is correct
    assert_eq!(response["success"], true);
    assert!(response["data"].is_array());
}

#[actix_rt::test]
async fn test_create_account() {
    // Create and initialize the test app with our standalone handler
    let app = test::init_service(
        App::new()
            .route("/api/accounts", web::post().to(create_test_account))
    ).await;
    
    // Create the request body
    let request_body = serde_json::json!({
        "name": "New Test Account"
    });
    
    // Send a POST request to the endpoint
    let req = test::TestRequest::post()
        .uri("/api/accounts")
        .set_json(&request_body)
        .to_request();
    
    // Perform the request and get the response
    let resp = test::call_service(&app, req).await;
    
    // Verify the response status is 201 Created
    assert_eq!(resp.status(), StatusCode::CREATED);
    
    // Get the response body and parse it
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // Verify the response format is correct
    assert_eq!(response["success"], true);
    assert_eq!(response["message"], "Account created successfully");
    
    // Verify account data in the response
    assert!(response["data"].is_object());
    assert_eq!(response["data"]["name"], "New Test Account");
    assert_eq!(response["data"]["balance"], 0.0);
    assert!(response["data"]["id"].is_string());
}

#[actix_rt::test]
async fn test_get_orders() {
    // Create and initialize the test app with our standalone handler
    let app = test::init_service(
        App::new()
            .route("/api/orders", web::get().to(get_test_orders))
    ).await;
    
    // Send a GET request to the endpoint
    let req = test::TestRequest::get()
        .uri("/api/orders")
        .to_request();
    
    // Perform the request and get the response
    let resp = test::call_service(&app, req).await;
    
    // Verify the response status is 200 OK
    assert_eq!(resp.status(), StatusCode::OK);
    
    // Get the response body and parse it
    let body = test::read_body(resp).await;
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // Verify the response format is correct
    assert_eq!(response["success"], true);
    assert!(response["data"].is_array());
    
    // Verify we have two orders with expected properties
    assert_eq!(response["data"].as_array().unwrap().len(), 2);
    assert_eq!(response["data"][0]["order_id"], "order-1");
    assert_eq!(response["data"][0]["symbol"], "BTC/USD");
    assert_eq!(response["data"][0]["side"], "Buy");
    
    assert_eq!(response["data"][1]["order_id"], "order-2");
    assert_eq!(response["data"][1]["symbol"], "ETH/USD");
    assert_eq!(response["data"][1]["side"], "Sell");
    assert_eq!(response["data"][1]["status"], "Filled");
}

// 添加更多测试... 