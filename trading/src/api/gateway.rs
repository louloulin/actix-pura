use actix::{Actor, Context, Addr};
use log::{info, error};
use actix_web::{web, HttpResponse, Error as ActixError};
use serde::{Serialize, Deserialize};

// 导入适配器类而不是原始actor
use crate::api::{
    ActixOrderActor, ActixAccountActor, ActixRiskActor,
    AccountQueryMessage, AccountUpdateMessage, FundTransferMessage, 
    CreateOrderMessage, CancelOrderMessage, OrderQueryMessage,
    AccountQueryResult, OrderQueryResult, AccountUpdateResult, FundTransferResult,
    OrderCreateResult, OrderCancelResult
};
use crate::models::order::{Order, OrderSide, OrderType, OrderStatus};

/// API请求处理结果
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<T>,
}

/// 订单创建请求
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub account_id: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub quantity: f64,
    pub price: Option<f64>,
    pub stop_price: Option<f64>,
}

/// API网关Actor，处理HTTP请求
pub struct ApiGatewayActor {
    node_id: String,
    // 使用适配器类型而不是原始actor类型
    order_actor: Addr<ActixOrderActor>,
    account_actor: Addr<ActixAccountActor>,
    risk_actor: Addr<ActixRiskActor>,
}

impl ApiGatewayActor {
    /// 创建新的API网关Actor
    pub fn new(
        node_id: String,
        // 更新参数类型为适配器类型
        order_actor: Addr<ActixOrderActor>,
        account_actor: Addr<ActixAccountActor>,
        risk_actor: Addr<ActixRiskActor>,
    ) -> Self {
        Self {
            node_id,
            order_actor,
            account_actor,
            risk_actor,
        }
    }
    
    /// 处理账户查询请求
    pub async fn handle_account_query(&self, account_id: String) -> HttpResponse {
        let msg = AccountQueryMessage::GetAccount { account_id };
        
        match self.account_actor.send(msg).await {
            Ok(result) => match result {
                AccountQueryResult::Account(account) => {
                    HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        message: None,
                        data: Some(account),
                    })
                },
                AccountQueryResult::Error(err) => {
                    HttpResponse::NotFound().json(ApiResponse::<()> {
                        success: false,
                        message: Some(err),
                        data: None,
                    })
                },
                _ => {
                    HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        message: Some("Unexpected response type".to_string()),
                        data: None,
                    })
                }
            },
            Err(e) => {
                error!("Error sending account query message: {}", e);
                HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Internal error: {}", e)),
                    data: None,
                })
            }
        }
    }
    
    /// 处理账户列表查询请求
    pub async fn handle_accounts_query(&self) -> HttpResponse {
        let msg = AccountQueryMessage::GetAllAccounts;
        
        match self.account_actor.send(msg).await {
            Ok(result) => match result {
                AccountQueryResult::Accounts(accounts) => {
                    HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        message: None,
                        data: Some(accounts),
                    })
                },
                _ => {
                    HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        message: Some("Unexpected response type".to_string()),
                        data: None,
                    })
                }
            },
            Err(e) => {
                error!("Error sending accounts query message: {}", e);
                HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Internal error: {}", e)),
                    data: None,
                })
            }
        }
    }
    
    /// 处理创建账户请求
    pub async fn handle_create_account(
        &self, 
        name: String, 
        initial_balance: Option<f64>
    ) -> HttpResponse {
        // 创建请求消息，使用正确的结构体形式，而不是枚举变体
        let msg = AccountUpdateMessage {
            account_id: None,
            name,
            initial_balance: initial_balance.unwrap_or(0.0),
        };
        
        match self.account_actor.send(msg).await {
            Ok(result) => match result {
                AccountUpdateResult::Success(account) => {
                    HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        message: Some("Account created successfully".to_string()),
                        data: Some(account),
                    })
                },
                AccountUpdateResult::Error(err) => {
                    HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        message: Some(err),
                        data: None,
                    })
                }
            },
            Err(e) => {
                error!("Error sending create account message: {}", e);
                HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Internal error: {}", e)),
                    data: None,
                })
            }
        }
    }
    
    /// 处理资金转账请求
    pub async fn handle_fund_transfer(
        &self,
        account_id: String,
        amount: f64,
        transfer_type: String,
        reference: Option<String>,
    ) -> HttpResponse {
        let msg = FundTransferMessage {
            account_id,
            amount,
            transfer_type,
            reference,
        };
        
        match self.account_actor.send(msg).await {
            Ok(result) => match result {
                FundTransferResult::Success(account) => {
                    HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        message: Some("Funds transferred successfully".to_string()),
                        data: Some(account),
                    })
                },
                FundTransferResult::Error(err) => {
                    HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        message: Some(err),
                        data: None,
                    })
                }
            },
            Err(e) => {
                error!("Error sending fund transfer message: {}", e);
                HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Internal error: {}", e)),
                    data: None,
                })
            }
        }
    }
    
    /// 处理创建订单请求
    pub async fn handle_create_order(&self, req: CreateOrderRequest) -> HttpResponse {
        // 解析订单类型
        let order_type = match req.order_type.to_uppercase().as_str() {
            "MARKET" => OrderType::Market,
            "LIMIT" => OrderType::Limit,
            "STOP" => OrderType::StopLoss,
            "STOP_LIMIT" => OrderType::StopLimit,
            _ => {
                return HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Invalid order type: {}", req.order_type)),
                    data: None,
                })
            }
        };
        
        // 解析订单方向
        let side = match req.side.to_uppercase().as_str() {
            "BUY" => OrderSide::Buy,
            "SELL" => OrderSide::Sell,
            _ => {
                return HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Invalid order side: {}", req.side)),
                    data: None,
                })
            }
        };
        
        // 检查订单参数
        if req.quantity <= 0.0 {
            return HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                message: Some("Quantity must be greater than zero".to_string()),
                data: None,
            });
        }
        
        if order_type == OrderType::Limit && req.price.is_none() {
            return HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                message: Some("Limit order requires a price".to_string()),
                data: None,
            });
        }
        
        if (order_type == OrderType::StopLoss || order_type == OrderType::StopLimit) && req.stop_price.is_none() {
            return HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                message: Some("Stop order requires a stop price".to_string()),
                data: None,
            });
        }
        
        // 创建订单对象
        let order = Order {
            order_id: uuid::Uuid::new_v4().to_string(),
            account_id: req.account_id,
            symbol: req.symbol,
            side,
            order_type,
            quantity: req.quantity,
            price: req.price,
            stop_price: req.stop_price,
            status: OrderStatus::New,
            filled_quantity: 0.0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        // 发送创建订单消息
        let msg = CreateOrderMessage {
            order,
            skip_risk_check: false,
        };
        
        match self.order_actor.send(msg).await {
            Ok(result) => match result {
                OrderCreateResult::Success(order) => {
                    HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        message: Some("Order created successfully".to_string()),
                        data: Some(order),
                    })
                },
                OrderCreateResult::Error(err) => {
                    HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        message: Some(err),
                        data: None,
                    })
                }
            },
            Err(e) => {
                error!("Error sending create order message: {}", e);
                HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Internal error: {}", e)),
                    data: None,
                })
            }
        }
    }
    
    /// 处理取消订单请求
    pub async fn handle_cancel_order(
        &self,
        order_id: String,
        account_id: String,
    ) -> HttpResponse {
        let msg = CancelOrderMessage {
            order_id,
            requested_by: account_id,
        };
        
        match self.order_actor.send(msg).await {
            Ok(result) => match result {
                OrderCancelResult::Success => {
                    HttpResponse::Ok().json(ApiResponse::<()> {
                        success: true,
                        message: Some("Order cancelled successfully".to_string()),
                        data: None,
                    })
                },
                OrderCancelResult::Error(err) => {
                    HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        message: Some(err),
                        data: None,
                    })
                }
            },
            Err(e) => {
                error!("Error sending cancel order message: {}", e);
                HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Internal error: {}", e)),
                    data: None,
                })
            }
        }
    }
    
    /// 处理订单查询请求
    pub async fn handle_order_query(&self, order_id: String) -> HttpResponse {
        let msg = OrderQueryMessage::GetOrder { order_id };
        
        match self.order_actor.send(msg).await {
            Ok(result) => match result {
                OrderQueryResult::Order(order) => {
                    HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        message: None,
                        data: Some(order),
                    })
                },
                OrderQueryResult::Error(err) => {
                    HttpResponse::NotFound().json(ApiResponse::<()> {
                        success: false,
                        message: Some(err),
                        data: None,
                    })
                },
                _ => {
                    HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        message: Some("Unexpected response type".to_string()),
                        data: None,
                    })
                }
            },
            Err(e) => {
                error!("Error sending order query message: {}", e);
                HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Internal error: {}", e)),
                    data: None,
                })
            }
        }
    }
    
    /// 处理账户订单查询请求
    pub async fn handle_account_orders_query(&self, account_id: String) -> HttpResponse {
        let msg = OrderQueryMessage::GetOrdersByAccount { account_id };
        
        match self.order_actor.send(msg).await {
            Ok(result) => match result {
                OrderQueryResult::Orders(orders) => {
                    HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        message: None,
                        data: Some(orders),
                    })
                },
                OrderQueryResult::Error(err) => {
                    HttpResponse::NotFound().json(ApiResponse::<()> {
                        success: false,
                        message: Some(err),
                        data: None,
                    })
                },
                _ => {
                    HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        message: Some("Unexpected response type".to_string()),
                        data: None,
                    })
                }
            },
            Err(e) => {
                error!("Error sending account orders query message: {}", e);
                HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    message: Some(format!("Internal error: {}", e)),
                    data: None,
                })
            }
        }
    }
}

impl Actor for ApiGatewayActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("ApiGatewayActor started on node {}", self.node_id);
    }
}

/// 配置API路由
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // 账户相关API
        .route("/api/accounts", web::get().to(get_accounts))
        .route("/api/accounts", web::post().to(create_account))
        .route("/api/accounts/{account_id}", web::get().to(get_account))
        .route("/api/accounts/{account_id}/transfer", web::post().to(transfer_funds))
        // 订单相关API
        .route("/api/orders", web::post().to(create_order))
        .route("/api/orders/{order_id}", web::get().to(get_order))
        .route("/api/orders/{order_id}/cancel", web::post().to(cancel_order))
        .route("/api/accounts/{account_id}/orders", web::get().to(get_account_orders));
}

// API处理函数
async fn get_accounts(
    api_gateway: web::Data<ApiGatewayActor>,
) -> Result<HttpResponse, ActixError> {
    Ok(api_gateway.handle_accounts_query().await)
}

async fn get_account(
    api_gateway: web::Data<ApiGatewayActor>,
    path: web::Path<String>,
) -> Result<HttpResponse, ActixError> {
    let account_id = path.into_inner();
    Ok(api_gateway.handle_account_query(account_id).await)
}

async fn create_account(
    api_gateway: web::Data<ApiGatewayActor>,
    json: web::Json<serde_json::Value>,
) -> Result<HttpResponse, ActixError> {
    let name = json.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("New Account")
        .to_string();
    
    let initial_balance = json.get("initial_balance")
        .and_then(|v| v.as_f64());
    
    Ok(api_gateway.handle_create_account(name, initial_balance).await)
}

async fn transfer_funds(
    api_gateway: web::Data<ApiGatewayActor>,
    path: web::Path<String>,
    json: web::Json<serde_json::Value>,
) -> Result<HttpResponse, ActixError> {
    let account_id = path.into_inner();
    
    let amount = match json.get("amount").and_then(|v| v.as_f64()) {
        Some(amount) => amount,
        None => return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
            success: false,
            message: Some("Amount is required".to_string()),
            data: None,
        })),
    };
    
    let transfer_type = match json.get("type").and_then(|v| v.as_str()) {
        Some(transfer_type) => transfer_type.to_string(),
        None => return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
            success: false,
            message: Some("Transfer type is required".to_string()),
            data: None,
        })),
    };
    
    let reference = json.get("reference")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    Ok(api_gateway.handle_fund_transfer(account_id, amount, transfer_type, reference).await)
}

async fn create_order(
    api_gateway: web::Data<ApiGatewayActor>,
    json: web::Json<CreateOrderRequest>,
) -> Result<HttpResponse, ActixError> {
    Ok(api_gateway.handle_create_order(json.into_inner()).await)
}

async fn get_order(
    api_gateway: web::Data<ApiGatewayActor>,
    path: web::Path<String>,
) -> Result<HttpResponse, ActixError> {
    let order_id = path.into_inner();
    Ok(api_gateway.handle_order_query(order_id).await)
}

async fn cancel_order(
    api_gateway: web::Data<ApiGatewayActor>,
    path: web::Path<String>,
    json: web::Json<serde_json::Value>,
) -> Result<HttpResponse, ActixError> {
    let order_id = path.into_inner();
    
    let account_id = match json.get("account_id").and_then(|v| v.as_str()) {
        Some(account_id) => account_id.to_string(),
        None => return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
            success: false,
            message: Some("Account ID is required".to_string()),
            data: None,
        })),
    };
    
    Ok(api_gateway.handle_cancel_order(order_id, account_id).await)
}

async fn get_account_orders(
    api_gateway: web::Data<ApiGatewayActor>,
    path: web::Path<String>,
) -> Result<HttpResponse, ActixError> {
    let account_id = path.into_inner();
    Ok(api_gateway.handle_account_orders_query(account_id).await)
} 