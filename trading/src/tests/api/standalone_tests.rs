use std::collections::HashMap;
use actix_web::{test, web, App, HttpResponse, HttpRequest};
use actix_web::http::StatusCode;
use chrono::Utc;
use uuid::Uuid;
use serde_json::Value;

// A simplified ApiResponse structure
#[derive(serde::Serialize)]
struct ApiResponse<T> {
    success: bool,
    message: Option<String>,
    data: Option<T>,
}

// Simple Account structure
#[derive(serde::Serialize)]
struct Account {
    id: String,
    name: String,
    balance: f64,
    available: f64,
    frozen: f64,
    positions: HashMap<String, f64>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

// Simple Order structure with enum types
#[derive(serde::Serialize)]
struct Order {
    order_id: String,
    account_id: String,
    symbol: String,
    side: OrderSide,
    order_type: OrderType,
    price: Option<f64>,
    quantity: f64,
    filled_quantity: f64,
    status: OrderStatus,
    stop_price: Option<f64>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(serde::Serialize)]
enum OrderSide {
    Buy,
    Sell,
}

#[derive(serde::Serialize)]
enum OrderType {
    Market,
    Limit,
    StopLimit,
    StopMarket,
}

#[derive(serde::Serialize)]
enum OrderStatus {
    New,
    Accepted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

// Add string implementations for the enums to make them JSON serializable
impl serde::Serialize for OrderSide {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            OrderSide::Buy => serializer.serialize_str("Buy"),
            OrderSide::Sell => serializer.serialize_str("Sell"),
        }
    }
}

impl serde::Serialize for OrderType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            OrderType::Market => serializer.serialize_str("Market"),
            OrderType::Limit => serializer.serialize_str("Limit"),
            OrderType::StopLimit => serializer.serialize_str("StopLimit"),
            OrderType::StopMarket => serializer.serialize_str("StopMarket"),
        }
    }
}

impl serde::Serialize for OrderStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            OrderStatus::New => serializer.serialize_str("New"),
            OrderStatus::Accepted => serializer.serialize_str("Accepted"),
            OrderStatus::PartiallyFilled => serializer.serialize_str("PartiallyFilled"),
            OrderStatus::Filled => serializer.serialize_str("Filled"),
            OrderStatus::Cancelled => serializer.serialize_str("Cancelled"),
            OrderStatus::Rejected => serializer.serialize_str("Rejected"),
        }
    }
}

// Standalone handler function for getting accounts
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
async fn create_test_account(account_data: web::Json<Value>) -> HttpResponse {
    // Extract the name from the request body
    let name = match account_data.get("name") {
        Some(name_value) => match name_value.as_str() {
            Some(name_str) => name_str,
            None => return HttpResponse::BadRequest().json(ApiResponse::<Vec<()>> {
                success: false,
                message: Some("Invalid account name".to_string()),
                data: None,
            }),
        },
        None => return HttpResponse::BadRequest().json(ApiResponse::<Vec<()>> {
            success: false,
            message: Some("Missing account name".to_string()),
            data: None,
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

#[actix_web::test]
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
    let response: Value = serde_json::from_slice(&body).unwrap();
    
    // Verify the response format is correct
    assert_eq!(response["success"], true);
    assert!(response["data"].is_array());
}

#[actix_web::test]
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
    let response: Value = serde_json::from_slice(&body).unwrap();
    
    // Verify the response format is correct
    assert_eq!(response["success"], true);
    assert_eq!(response["message"], "Account created successfully");
    
    // Verify account data in the response
    assert!(response["data"].is_object());
    assert_eq!(response["data"]["name"], "New Test Account");
    assert_eq!(response["data"]["balance"], 0.0);
    assert!(response["data"]["id"].is_string());
}

#[actix_web::test]
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
    let response: Value = serde_json::from_slice(&body).unwrap();
    
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