use serde::{Serialize, Deserialize};
use actix::prelude::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 订单方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    /// 买入
    Buy,
    /// 卖出
    Sell,
}

/// 订单类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// 市价单 - 以当前最优价格成交
    Market,
    /// 限价单 - 指定价格或更好价格成交
    Limit,
    /// 止损单 - 当价格达到指定水平时以市价单执行
    StopLoss,
    /// 止损限价单 - 当价格达到指定水平时以限价单执行
    StopLimit,
    /// IOC单 - 立即成交或取消
    ImmediateOrCancel,
    /// FOK单 - 完全成交或完全取消
    FillOrKill,
}

/// 订单状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    /// 新建订单，尚未处理
    New,
    /// 订单已接受，正在处理中
    Accepted,
    /// 订单已部分成交
    PartiallyFilled,
    /// 订单已完全成交
    Filled,
    /// 订单已取消
    Cancelled,
    /// 订单被拒绝
    Rejected,
    /// 订单已过期
    Expired,
}

/// 订单模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// 订单ID，唯一标识
    pub order_id: String,
    /// 账户ID
    pub account_id: String,
    /// 交易的证券代码
    pub symbol: String,
    /// 订单方向（买/卖）
    pub side: OrderSide,
    /// 订单类型
    pub order_type: OrderType,
    /// 订单数量
    pub quantity: f64,
    /// 订单价格，对于限价单是必须的
    pub price: Option<f64>,
    /// 止损价格，对于止损单是必须的
    pub stop_price: Option<f64>,
    /// 订单当前状态
    pub status: OrderStatus,
    /// 已成交数量
    pub filled_quantity: f64,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl Order {
    /// 从订单请求创建新订单
    pub fn from_request(req: OrderRequest) -> Self {
        let now = Utc::now();
        Self {
            order_id: req.order_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            account_id: req.client_id,
            symbol: req.symbol,
            side: req.side,
            order_type: req.order_type,
            quantity: req.quantity as f64,
            price: req.price,
            stop_price: None, // 从请求中获取
            status: OrderStatus::New,
            filled_quantity: 0.0,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 更新订单状态
    pub fn update_status(&mut self, status: OrderStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }
    
    /// 更新已成交数量
    pub fn update_filled_quantity(&mut self, filled: f64) {
        self.filled_quantity += filled;
        
        // 自动更新订单状态
        if self.filled_quantity >= self.quantity {
            self.status = OrderStatus::Filled;
        } else if self.filled_quantity > 0.0 {
            self.status = OrderStatus::PartiallyFilled;
        }
        
        self.updated_at = Utc::now();
    }
    
    /// 检查订单是否可以取消
    pub fn can_cancel(&self) -> bool {
        match self.status {
            OrderStatus::New | OrderStatus::Accepted | OrderStatus::PartiallyFilled => true,
            _ => false,
        }
    }
    
    /// 检查订单是否活跃
    pub fn is_active(&self) -> bool {
        match self.status {
            OrderStatus::New | OrderStatus::Accepted | OrderStatus::PartiallyFilled => true,
            _ => false,
        }
    }
    
    /// 订单剩余未成交数量
    pub fn remaining_quantity(&self) -> f64 {
        self.quantity - self.filled_quantity
    }
}

/// 订单请求消息
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "OrderResult")]
pub struct OrderRequest {
    /// 可选的订单ID，如果未提供则自动生成
    pub order_id: Option<String>,
    /// 交易的证券代码
    pub symbol: String,
    /// 订单方向（买/卖）
    pub side: OrderSide,
    /// 价格，对于限价单是必须的
    pub price: Option<f64>,
    /// 数量
    pub quantity: u64,
    /// 客户ID/账户ID
    pub client_id: String,
    /// 订单类型
    pub order_type: OrderType,
}

/// 订单处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderResult {
    /// 订单被接受，返回订单ID
    Accepted(String),
    /// 订单被拒绝，包含拒绝原因
    Rejected(String),
    /// 处理过程中出现错误
    Error(String),
}

/// 订单查询请求
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "Vec<Order>")]
pub enum OrderQuery {
    /// 根据订单ID查询
    ById(String),
    /// 根据账户ID查询
    ByAccount(String),
    /// 根据证券代码查询
    BySymbol(String),
    /// 根据状态查询
    ByStatus(OrderStatus),
    /// 查询所有订单
    All,
}

/// 取消订单请求
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "OrderResult")]
pub struct CancelOrderRequest {
    /// 要取消的订单ID
    pub order_id: String,
    /// 发起取消请求的客户/账户ID
    pub client_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_order_from_request() {
        let req = OrderRequest {
            order_id: Some("test-order-1".to_string()),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            price: Some(150.0),
            quantity: 100,
            client_id: "client-1".to_string(),
            order_type: OrderType::Limit,
        };
        
        let order = Order::from_request(req.clone());
        
        assert_eq!(order.order_id, "test-order-1");
        assert_eq!(order.symbol, "AAPL");
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.price, Some(150.0));
        assert_eq!(order.quantity, 100.0);
        assert_eq!(order.account_id, "client-1");
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.status, OrderStatus::New);
        assert_eq!(order.filled_quantity, 0.0);
    }
    
    #[test]
    fn test_order_update_filled_quantity() {
        let req = OrderRequest {
            order_id: Some("test-order-2".to_string()),
            symbol: "MSFT".to_string(),
            side: OrderSide::Sell,
            price: Some(250.0),
            quantity: 50,
            client_id: "client-2".to_string(),
            order_type: OrderType::Limit,
        };
        
        let mut order = Order::from_request(req);
        
        // 部分成交
        order.update_filled_quantity(20.0);
        assert_eq!(order.filled_quantity, 20.0);
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        
        // 完全成交
        order.update_filled_quantity(30.0);
        assert_eq!(order.filled_quantity, 50.0);
        assert_eq!(order.status, OrderStatus::Filled);
    }
    
    #[test]
    fn test_order_can_cancel() {
        let mut order = Order {
            order_id: "test-order-3".to_string(),
            account_id: "client-3".to_string(),
            symbol: "GOOG".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 10.0,
            price: None,
            stop_price: None,
            status: OrderStatus::New,
            filled_quantity: 0.0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // 新订单可以取消
        assert!(order.can_cancel());
        
        // 已接受的订单可以取消
        order.update_status(OrderStatus::Accepted);
        assert!(order.can_cancel());
        
        // 部分成交的订单可以取消
        order.update_status(OrderStatus::PartiallyFilled);
        assert!(order.can_cancel());
        
        // 已完成的订单不能取消
        order.update_status(OrderStatus::Filled);
        assert!(!order.can_cancel());
        
        // 已取消的订单不能再次取消
        order.update_status(OrderStatus::Cancelled);
        assert!(!order.can_cancel());
    }
    
    #[test]
    fn test_remaining_quantity() {
        let mut order = Order {
            order_id: "test-order-4".to_string(),
            account_id: "client-4".to_string(),
            symbol: "AMZN".to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: 100.0,
            price: Some(3000.0),
            stop_price: None,
            status: OrderStatus::Accepted,
            filled_quantity: 0.0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // 初始剩余数量等于总数量
        assert_eq!(order.remaining_quantity(), 100.0);
        
        // 部分成交后剩余数量
        order.update_filled_quantity(30.0);
        assert_eq!(order.remaining_quantity(), 70.0);
        
        // 完全成交后剩余数量为0
        order.update_filled_quantity(70.0);
        assert_eq!(order.remaining_quantity(), 0.0);
    }
} 