use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use std::env;
use std::sync::Arc;
use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use actix_web::middleware::Logger;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use log::{info, error};
use clap::{App as ClapApp, Arg};

use trading::{
    OrderSide, OrderType, OrderRequest, OrderResult, OrderQuery,
    TradingClusterManager, CancelOrderRequest
};

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
    cluster_manager: web::Data<Arc<TradingClusterManager>>
) -> impl Responder {
    // 将API请求转换为内部OrderRequest信息
    let side = match data.side.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => {
            return HttpResponse::BadRequest().json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid order side, must be 'buy' or 'sell'".to_string()),
            });
        }
    };
    
    let order_type = match data.order_type.to_lowercase().as_str() {
        "limit" => OrderType::Limit,
        "market" => OrderType::Market,
        _ => {
            return HttpResponse::BadRequest().json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid order type, must be 'limit' or 'market'".to_string()),
            });
        }
    };
    
    // 创建订单ID
    let order_id = Uuid::new_v4().to_string();
    
    // 简化：假设发送到订单处理器
    match cluster_manager.send_message("/user/order", "CREATE_ORDER") {
        Ok(_) => {
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(format!("Order {} submitted successfully", order_id)),
                error: None,
            })
        },
        Err(e) => {
            HttpResponse::InternalServerError().json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to submit order: {}", e)),
            })
        }
    }
}

async fn cancel_order(
    data: web::Json<CancelOrderRequestDTO>,
    cluster_manager: web::Data<Arc<TradingClusterManager>>
) -> impl Responder {
    // 简化：假设发送到订单处理器
    match cluster_manager.send_message("/user/order", "CANCEL_ORDER") {
        Ok(_) => {
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(format!("Cancel request for order {} submitted successfully", data.order_id)),
                error: None,
            })
        },
        Err(e) => {
            HttpResponse::InternalServerError().json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to submit cancel request: {}", e)),
            })
        }
    }
}

async fn query_orders(
    query: web::Query<OrderQuery>,
    cluster_manager: web::Data<Arc<TradingClusterManager>>
) -> impl Responder {
    // 简化：假设发送到订单处理器
    match cluster_manager.send_message("/user/order", "QUERY_ORDERS") {
        Ok(_) => {
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some("Query submitted successfully"),
                error: None,
            })
        },
        Err(e) => {
            HttpResponse::InternalServerError().json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to submit query: {}", e)),
            })
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 配置命令行参数
    let matches = ClapApp::new("Trading API Server")
        .version("1.0")
        .author("Trading System Team")
        .about("HTTP API for Distributed Trading System")
        .arg(Arg::new("port")
            .short('p')
            .long("port")
            .value_name("PORT")
            .help("API server port")
            .takes_value(true)
            .required(false))
        .arg(Arg::new("cluster_addr")
            .short('c')
            .long("cluster")
            .value_name("CLUSTER_ADDR")
            .help("Trading cluster address to connect")
            .takes_value(true)
            .required(true))
        .get_matches();
    
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // 获取API服务端口
    let api_port = matches.value_of("port")
        .map(|s| s.parse::<u16>().unwrap_or(8080))
        .unwrap_or(8080);
    
    // 获取集群地址
    let cluster_addr = matches.value_of("cluster_addr")
        .expect("Cluster address is required");
    
    // 创建节点标识符
    let node_id = format!("api-{}", Uuid::new_v4());
    
    // 创建绑定地址
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0); // 随机端口
    
    // 创建集群管理器
    let mut cluster_manager = TradingClusterManager::new(
        node_id.clone(), 
        bind_addr,
        "trading-cluster".to_string()
    );
    
    // 添加集群节点
    cluster_manager.add_seed_node(cluster_addr.to_string());
    
    // 初始化集群连接
    match cluster_manager.initialize() {
        Ok(_) => {
            info!("API服务器已连接到交易集群");
            
            // 创建共享的集群管理器
            let cluster_manager = Arc::new(cluster_manager);
            
            // 启动HTTP服务器
            let api_bind = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), api_port);
            
            info!("启动API服务器: {}", api_bind);
            
            HttpServer::new(move || {
                App::new()
                    .wrap(Logger::default())
                    .app_data(web::Data::new(cluster_manager.clone()))
                    .route("/api/orders", web::post().to(create_order))
                    .route("/api/orders/cancel", web::post().to(cancel_order))
                    .route("/api/orders", web::get().to(query_orders))
            })
            .bind(api_bind)?
            .run()
            .await
        },
        Err(e) => {
            error!("连接到交易集群失败: {}", e);
            Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        }
    }
} 