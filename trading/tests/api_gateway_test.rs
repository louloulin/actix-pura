use std::sync::Arc;
use actix_web::{test, web, App, http::StatusCode};
use trading::{
    OrderSide, OrderType, OrderRequest, OrderResult, TradingClusterManager, CancelOrderRequest
};
use uuid::Uuid;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use serde::{Serialize, Deserialize};

// Mock TradingClusterManager for testing
struct MockTradingClusterManager {
    node_id: String,
}

impl MockTradingClusterManager {
    fn new(node_id: String) -> Self {
        Self { node_id }
    }
    
    fn send_message(&self, _target_path: &str, _msg: &str) -> Result<(), Box<dyn std::error::Error>> {
        // 模拟成功发送消息
        Ok(())
    }
    
    fn send_typed_message<T: serde::Serialize>(
        &self,
        _target_path: &str,
        _msg_type: &str,
        _message: T
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 模拟成功发送类型化消息
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateOrderRequest {
    symbol: String,
    side: String,
    price: Option<f64>,
    quantity: u64,
    client_id: String,
    order_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CancelOrderRequestDTO {
    order_id: String,
    client_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

async fn create_order(
    data: web::Json<CreateOrderRequest>,
    cluster_manager: web::Data<Arc<MockTradingClusterManager>>
) -> actix_web::Result<web::Json<ApiResponse<String>>> {
    let side = match data.side.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => {
            return Ok(web::Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid order side, must be 'buy' or 'sell'".to_string()),
            }));
        }
    };
    
    let order_type = match data.order_type.to_lowercase().as_str() {
        "limit" => OrderType::Limit,
        "market" => OrderType::Market,
        _ => {
            return Ok(web::Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid order type, must be 'limit' or 'market'".to_string()),
            }));
        }
    };
    
    // 创建订单ID
    let order_id = Uuid::new_v4().to_string();
    
    // 创建订单请求对象
    let order_request = OrderRequest {
        order_id: Some(order_id.clone()),
        symbol: data.symbol.clone(),
        side,
        price: data.price,
        quantity: data.quantity,
        client_id: data.client_id.clone(),
        order_type,
    };
    
    // 发送到订单处理器
    match cluster_manager.send_typed_message("/user/order", "CREATE_ORDER", order_request) {
        Ok(_) => {
            Ok(web::Json(ApiResponse {
                success: true,
                data: Some(format!("Order {} submitted successfully", order_id)),
                error: None,
            }))
        },
        Err(e) => {
            Ok(web::Json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to submit order: {}", e)),
            }))
        }
    }
}

async fn cancel_order(
    data: web::Json<CancelOrderRequestDTO>,
    cluster_manager: web::Data<Arc<MockTradingClusterManager>>
) -> actix_web::Result<web::Json<ApiResponse<String>>> {
    // 创建取消订单请求
    let cancel_request = CancelOrderRequest {
        order_id: data.order_id.clone(),
        client_id: data.client_id.clone(),
    };
    
    match cluster_manager.send_typed_message("/user/order", "CANCEL_ORDER", cancel_request) {
        Ok(_) => {
            Ok(web::Json(ApiResponse {
                success: true,
                data: Some(format!("Cancel request for order {} submitted successfully", data.order_id)),
                error: None,
            }))
        },
        Err(e) => {
            Ok(web::Json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to submit cancel request: {}", e)),
            }))
        }
    }
}

#[actix_rt::test]
async fn test_create_order_valid() {
    // 创建模拟的集群管理器
    let cluster_manager = Arc::new(MockTradingClusterManager::new("test-node".to_string()));
    
    // 创建测试应用
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(cluster_manager.clone()))
            .route("/api/orders", web::post().to(create_order))
    ).await;
    
    // 创建有效的订单请求
    let order_req = CreateOrderRequest {
        symbol: "AAPL".to_string(),
        side: "buy".to_string(),
        price: Some(150.0),
        quantity: 100,
        client_id: "test-client".to_string(),
        order_type: "limit".to_string(),
    };
    
    // 发送请求
    let req = test::TestRequest::post()
        .uri("/api/orders")
        .set_json(&order_req)
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    
    // 验证响应状态
    assert_eq!(resp.status(), StatusCode::OK);
    
    // 解析响应内容
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.success);
    assert!(body.data.is_some());
    assert!(body.error.is_none());
}

#[actix_rt::test]
async fn test_create_order_invalid_side() {
    // 创建模拟的集群管理器
    let cluster_manager = Arc::new(MockTradingClusterManager::new("test-node".to_string()));
    
    // 创建测试应用
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(cluster_manager.clone()))
            .route("/api/orders", web::post().to(create_order))
    ).await;
    
    // 创建带有无效 side 的订单请求
    let order_req = CreateOrderRequest {
        symbol: "AAPL".to_string(),
        side: "invalid".to_string(), // 无效的 side
        price: Some(150.0),
        quantity: 100,
        client_id: "test-client".to_string(),
        order_type: "limit".to_string(),
    };
    
    // 发送请求
    let req = test::TestRequest::post()
        .uri("/api/orders")
        .set_json(&order_req)
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    
    // 验证响应状态 (这里仍然是 200 OK，但 success 字段是 false)
    assert_eq!(resp.status(), StatusCode::OK);
    
    // 解析响应内容
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
    assert!(body.data.is_none());
    assert!(body.error.is_some());
    assert!(body.error.unwrap().contains("Invalid order side"));
}

#[actix_rt::test]
async fn test_cancel_order() {
    // 创建模拟的集群管理器
    let cluster_manager = Arc::new(MockTradingClusterManager::new("test-node".to_string()));
    
    // 创建测试应用
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(cluster_manager.clone()))
            .route("/api/orders/cancel", web::post().to(cancel_order))
    ).await;
    
    // 创建取消订单请求
    let cancel_req = CancelOrderRequestDTO {
        order_id: "test-order-123".to_string(),
        client_id: "test-client".to_string(),
    };
    
    // 发送请求
    let req = test::TestRequest::post()
        .uri("/api/orders/cancel")
        .set_json(&cancel_req)
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    
    // 验证响应状态
    assert_eq!(resp.status(), StatusCode::OK);
    
    // 解析响应内容
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.success);
    assert!(body.data.is_some());
    assert!(body.error.is_none());
    assert!(body.data.unwrap().contains("test-order-123"));
} 