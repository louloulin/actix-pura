use std::collections::HashMap;
use actix_web::{web, App, HttpResponse, HttpRequest, HttpServer};
use actix_web::http::StatusCode;
use chrono::Utc;
use uuid::Uuid;
use serde_json::Value;
use trading::models::order::{OrderSide, OrderType, OrderStatus};

// A simplified ApiResponse structure
#[derive(serde::Serialize)]
struct ApiResponse<T> {
    success: bool,
    message: Option<String>,
    data: Option<T>,
}

// Simple Account structure
#[derive(serde::Serialize, Clone)]
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
#[derive(serde::Serialize, Clone)]
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

// Handler function for getting accounts
async fn get_accounts(_req: HttpRequest) -> HttpResponse {
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

// Handler for creating an account
async fn create_account(account_data: web::Json<Value>) -> HttpResponse {
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

// Handler for getting orders
async fn get_orders(_req: HttpRequest) -> HttpResponse {
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

// Handler for creating an order
async fn create_order(order_data: web::Json<Value>) -> HttpResponse {
    // Extract order details from the request body
    let symbol = match order_data.get("symbol") {
        Some(symbol_value) => match symbol_value.as_str() {
            Some(symbol_str) => symbol_str,
            None => return HttpResponse::BadRequest().json(ApiResponse::<Vec<()>> {
                success: false,
                message: Some("Invalid symbol".to_string()),
                data: None,
            }),
        },
        None => return HttpResponse::BadRequest().json(ApiResponse::<Vec<()>> {
            success: false,
            message: Some("Missing symbol".to_string()),
            data: None,
        }),
    };

    let side_str = match order_data.get("side") {
        Some(side_value) => match side_value.as_str() {
            Some(side_str) => side_str,
            None => return HttpResponse::BadRequest().json(ApiResponse::<Vec<()>> {
                success: false,
                message: Some("Invalid side".to_string()),
                data: None,
            }),
        },
        None => return HttpResponse::BadRequest().json(ApiResponse::<Vec<()>> {
            success: false,
            message: Some("Missing side".to_string()),
            data: None,
        }),
    };

    let side = match side_str.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => return HttpResponse::BadRequest().json(ApiResponse::<Vec<()>> {
            success: false,
            message: Some("Invalid side value, must be 'buy' or 'sell'".to_string()),
            data: None,
        }),
    };

    let quantity = match order_data.get("quantity") {
        Some(qty_value) => match qty_value.as_f64() {
            Some(qty) => qty,
            None => return HttpResponse::BadRequest().json(ApiResponse::<Vec<()>> {
                success: false,
                message: Some("Invalid quantity".to_string()),
                data: None,
            }),
        },
        None => return HttpResponse::BadRequest().json(ApiResponse::<Vec<()>> {
            success: false,
            message: Some("Missing quantity".to_string()),
            data: None,
        }),
    };

    // Create a new order
    let new_order = Order {
        order_id: Uuid::new_v4().to_string(),
        account_id: "test-account".to_string(), // Using a fixed account for simplicity
        symbol: symbol.to_string(),
        side,
        order_type: OrderType::Market, // Default to market order for simplicity
        price: None,
        quantity,
        filled_quantity: 0.0,
        status: OrderStatus::New,
        stop_price: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Return success response with the new order
    let response = ApiResponse {
        success: true,
        message: Some("Order created successfully".to_string()),
        data: Some(new_order),
    };

    HttpResponse::Created().json(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting API test server on http://127.0.0.1:8080");
    
    HttpServer::new(|| {
        App::new()
            .route("/api/accounts", web::get().to(get_accounts))
            .route("/api/accounts", web::post().to(create_account))
            .route("/api/orders", web::get().to(get_orders))
            .route("/api/orders", web::post().to(create_order))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
} 