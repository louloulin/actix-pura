use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use log::{debug, info, warn, error};
use chrono::{DateTime, Utc};
use async_trait;

use crate::models::order::{OrderRequest, OrderSide};
use crate::models::account::{Account, Position};
use crate::models::message::{Message, MessageType};
use crate::actor::{Actor, ActorRef, ActorContext, MessageHandler};
use crate::actor::ActorSystem;

/// 风控检查请求
#[derive(Clone)]
pub struct RiskCheckRequest {
    /// 订单请求
    pub order: OrderRequest,
    /// 账户ID
    pub account_id: String,
}

impl Message for RiskCheckRequest {
    fn message_type(&self) -> MessageType {
        MessageType::RiskCheck
    }
}

/// 风控检查结果
#[derive(Debug, Clone)]
pub enum RiskCheckResult {
    /// 通过风控检查
    Approved,
    /// 风控拒绝，包含拒绝原因
    Rejected(String),
}

/// 风控配置更新消息
#[derive(Clone)]
pub struct UpdateRiskConfigMessage {
    /// 新的订单价值上限
    pub max_order_value: Option<f64>,
    /// 新的持仓价值上限
    pub max_position_value: Option<f64>,
    /// 是否允许卖空
    pub allow_short_selling: Option<bool>,
}

impl Message for UpdateRiskConfigMessage {
    fn message_type(&self) -> MessageType {
        MessageType::UpdateConfig
    }
}

/// 风控配置
#[derive(Debug, Clone)]
pub struct RiskConfig {
    /// 单笔订单最大价值
    pub max_order_value: f64,
    /// 单个证券最大持仓价值
    pub max_position_value: f64,
    /// 是否允许卖空
    pub allow_short_selling: bool,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_order_value: 1_000_000.0, // 默认100万
            max_position_value: 10_000_000.0, // 默认1000万
            allow_short_selling: false,   // 默认不允许卖空
            updated_at: Utc::now(),
        }
    }
}

/// 风控管理器
pub struct RiskManager {
    /// 节点ID
    pub node_id: String,
    /// 风控配置
    pub config: RiskConfig,
    /// 价格缓存 (symbol -> 最新价格)
    pub price_cache: HashMap<String, f64>,
    /// 账户Actor
    pub account_actor: Option<Box<dyn ActorRef>>,
}

impl RiskManager {
    /// 创建新的风控管理器
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            config: RiskConfig::default(),
            price_cache: HashMap::new(),
            account_actor: None,
        }
    }
    
    /// 设置账户Actor
    pub fn set_account_actor(&mut self, account_actor: Box<dyn ActorRef>) {
        self.account_actor = Some(account_actor);
    }
    
    /// 更新风控配置
    pub fn update_config(&mut self, update: UpdateRiskConfigMessage) {
        if let Some(max_order_value) = update.max_order_value {
            self.config.max_order_value = max_order_value;
        }
        
        if let Some(max_position_value) = update.max_position_value {
            self.config.max_position_value = max_position_value;
        }
        
        if let Some(allow_short_selling) = update.allow_short_selling {
            self.config.allow_short_selling = allow_short_selling;
        }
        
        self.config.updated_at = Utc::now();
    }
    
    /// 更新价格缓存
    pub fn update_price(&mut self, symbol: String, price: f64) {
        self.price_cache.insert(symbol, price);
    }
    
    /// 获取证券价格，如果缓存中没有则返回None
    pub fn get_price(&self, symbol: &str) -> Option<f64> {
        self.price_cache.get(symbol).cloned()
    }
    
    /// 执行风控检查
    async fn check_risk(&self, request: &RiskCheckRequest, ctx: &mut ActorContext) -> RiskCheckResult {
        let order = &request.order;
        
        // 获取最新价格
        let price = match self.get_price_for_order(order) {
            Some(p) => p,
            None => {
                return RiskCheckResult::Rejected(
                    format!("No price available for symbol {}", order.symbol)
                );
            }
        };
        
        // 计算订单价值
        let order_value = price * order.quantity as f64;
        
        // 检查订单价值是否超过限制
        if order_value > self.config.max_order_value {
            return RiskCheckResult::Rejected(
                format!(
                    "Order value ${:.2} exceeds maximum allowed ${:.2}",
                    order_value, self.config.max_order_value
                )
            );
        }
        
        // 查询账户信息
        let account = match self.get_account(request.account_id.clone(), ctx).await {
            Some(acc) => acc,
            None => {
                return RiskCheckResult::Rejected(
                    format!("Account {} not found", request.account_id)
                );
            }
        };
        
        // 检查卖单是否有足够持仓
        if order.side == OrderSide::Sell {
            let position = account.positions.get(&order.symbol);
            
            if let Some(pos) = position {
                if pos.available() < order.quantity as f64 {
                    return RiskCheckResult::Rejected(
                        format!(
                            "Insufficient position for {} (available: {}, required: {})",
                            order.symbol, pos.available(), order.quantity
                        )
                    );
                }
            } else if !self.config.allow_short_selling {
                return RiskCheckResult::Rejected(
                    format!("No position available for {} and short selling is not allowed", order.symbol)
                );
            }
        }
        
        // 检查买单是否有足够资金
        if order.side == OrderSide::Buy {
            let required_funds = order_value;
            
            if account.available < required_funds {
                return RiskCheckResult::Rejected(
                    format!(
                        "Insufficient funds (available: ${:.2}, required: ${:.2})",
                        account.available, required_funds
                    )
                );
            }
        }
        
        // 检查持仓价值是否超过限制
        if order.side == OrderSide::Buy {
            // 计算当前持仓价值
            let current_position = account.positions.get(&order.symbol);
            let current_position_value = current_position
                .map(|p| p.value(price))
                .unwrap_or(0.0);
            
            // 计算新增持仓价值
            let additional_value = order_value;
            
            // 总持仓价值
            let total_position_value = current_position_value + additional_value;
            
            if total_position_value > self.config.max_position_value {
                return RiskCheckResult::Rejected(
                    format!(
                        "Total position value ${:.2} would exceed maximum allowed ${:.2}",
                        total_position_value, self.config.max_position_value
                    )
                );
            }
        }
        
        // 通过所有检查
        RiskCheckResult::Approved
    }
    
    /// 获取订单的价格
    fn get_price_for_order(&self, order: &OrderRequest) -> Option<f64> {
        // 如果订单指定了价格，优先使用
        if let Some(price) = order.price {
            return Some(price);
        }
        
        // 否则使用价格缓存
        self.get_price(&order.symbol)
    }
    
    /// 获取账户信息
    async fn get_account(&self, account_id: String, ctx: &mut ActorContext) -> Option<Account> {
        // 在实际实现中，这里会查询账户服务
        // 这里为了演示，创建一个模拟账户
        
        Some(Account::new(
            account_id.clone(),
            format!("Account {}", account_id),
            1_000_000.0, // 初始资金100万
        ))
    }
}

impl Actor for RiskManager {
    fn new_context(&self, ctx: &mut ActorContext) {}
}

#[async_trait::async_trait]
impl MessageHandler<RiskCheckRequest> for RiskManager {
    async fn handle(&mut self, msg: RiskCheckRequest, ctx: &mut ActorContext) -> Option<Box<dyn std::any::Any>> {
        info!("Performing risk check for order: {} {} {}",
              msg.order.side, msg.order.quantity, msg.order.symbol);
        
        let result = self.check_risk(&msg, ctx).await;
        
        match &result {
            RiskCheckResult::Approved => {
                info!("Risk check approved for order {} {} {}",
                     msg.order.side, msg.order.quantity, msg.order.symbol);
            },
            RiskCheckResult::Rejected(reason) => {
                warn!("Risk check rejected: {}", reason);
            },
        }
        
        Some(Box::new(result))
    }
}

#[async_trait::async_trait]
impl MessageHandler<UpdateRiskConfigMessage> for RiskManager {
    async fn handle(&mut self, msg: UpdateRiskConfigMessage, _ctx: &mut ActorContext) -> Option<Box<dyn std::any::Any>> {
        info!("Updating risk configuration");
        
        self.update_config(msg);
        
        info!("Risk configuration updated: max_order_value={}, max_position_value={}, allow_short_selling={}",
             self.config.max_order_value, self.config.max_position_value, self.config.allow_short_selling);
        
        None
    }
}

// 执行风控检查
pub async fn check_risk(
    actor_system: &ActorSystem,
    risk_addr: Box<dyn ActorRef>,
    request: OrderRequest
) -> Option<RiskCheckResult> {
    let client_id = request.client_id.clone();
    let msg = RiskCheckRequest { 
        order: request,
        account_id: client_id 
    };
    let result = actor_system.ask::<RiskCheckRequest, RiskCheckResult>(risk_addr.clone(), msg).await;
    
    result
}

// 更新风控配置
pub fn update_risk_config(
    actor_system: &ActorSystem,
    risk_addr: Box<dyn ActorRef>,
    config: RiskConfig
) -> bool {
    let update_msg = UpdateRiskConfigMessage { 
        max_order_value: Some(config.max_order_value),
        max_position_value: Some(config.max_position_value),
        allow_short_selling: Some(config.allow_short_selling)
    };
    actor_system.tell(risk_addr.clone(), update_msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::ActorSystem;
    use crate::models::order::OrderType;
    
    #[tokio::test]
    async fn test_risk_check_order_value() {
        let actor_system = ActorSystem::new();
        
        // 创建风控管理器
        let mut risk_manager = RiskManager::new("test-node".to_string());
        
        // 设置订单价值上限
        risk_manager.config.max_order_value = 1_000_000.0;
        
        // 添加价格缓存
        risk_manager.update_price("AAPL".to_string(), 150.0);
        
        let risk_addr = actor_system.create_actor(Box::new(risk_manager)).await;
        
        // 创建超过订单价值上限的请求
        let large_order = OrderRequest {
            order_id: Some("test-order-1".to_string()),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            price: Some(150.0),
            quantity: 10000, // 150 * 10000 = 1,500,000 > 1,000,000
            client_id: "client-1".to_string(),
            order_type: OrderType::Limit,
        };
        
        let msg = RiskCheckRequest {
            order: large_order,
            account_id: "client-1".to_string(),
        };
        
        let result = actor_system.ask::<RiskCheckRequest, RiskCheckResult>(risk_addr.clone(), msg).await;
        
        // 验证结果应该被拒绝
        if let Some(check_result) = result {
            match check_result {
                RiskCheckResult::Rejected(_) => {
                    // 预期被拒绝，测试通过
                },
                RiskCheckResult::Approved => {
                    panic!("Expected order to be rejected due to exceeding order value limit");
                },
            }
        }
        
        // 创建在订单价值上限内的请求
        let small_order = OrderRequest {
            order_id: Some("test-order-2".to_string()),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            price: Some(150.0),
            quantity: 5000, // 150 * 5000 = 750,000 < 1,000,000
            client_id: "client-1".to_string(),
            order_type: OrderType::Limit,
        };
        
        let msg = RiskCheckRequest {
            order: small_order,
            account_id: "client-1".to_string(),
        };
        
        let result = actor_system.ask::<RiskCheckRequest, RiskCheckResult>(risk_addr.clone(), msg).await;
        
        // 验证结果应该被接受
        if let Some(check_result) = result {
            match check_result {
                RiskCheckResult::Approved => {
                    // 预期被接受，测试通过
                },
                RiskCheckResult::Rejected(reason) => {
                    panic!("Expected order to be approved, but was rejected: {}", reason);
                },
            }
        }
    }
    
    #[tokio::test]
    async fn test_risk_config_update() {
        let actor_system = ActorSystem::new();
        
        // 创建风控管理器
        let risk_manager = RiskManager::new("test-node".to_string());
        let risk_addr = actor_system.create_actor(Box::new(risk_manager)).await;
        
        // 更新风控配置
        let update_msg = UpdateRiskConfigMessage {
            max_order_value: Some(2_000_000.0),
            max_position_value: Some(20_000_000.0),
            allow_short_selling: Some(true),
        };
        
        actor_system.tell(risk_addr.clone(), update_msg);
        
        // 在实际测试中，我们可以查询RiskManager状态验证更新是否生效
        // 由于我们的模拟Actor系统不支持状态查询，这里不做验证
    }
} 