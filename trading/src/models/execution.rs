use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::order::{Order, OrderSide};

/// 执行记录 - 表示单个订单的执行情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    /// 执行ID
    pub execution_id: String,
    /// 关联的订单ID
    pub order_id: String,
    /// 对手方订单ID
    pub counter_order_id: Option<String>,
    /// 证券代码
    pub symbol: String,
    /// 执行价格
    pub price: f64,
    /// 执行数量
    pub quantity: f64,
    /// 交易方向
    pub side: OrderSide,
    /// 买方账户ID
    pub buyer_account_id: Option<String>,
    /// 卖方账户ID
    pub seller_account_id: Option<String>,
    /// 执行时间
    pub executed_at: DateTime<Utc>,
}

impl Execution {
    /// 创建新的执行记录
    pub fn new(
        order_id: String,
        counter_order_id: Option<String>,
        symbol: String,
        price: f64,
        quantity: f64,
        side: OrderSide,
        buyer_account_id: Option<String>,
        seller_account_id: Option<String>,
    ) -> Self {
        Self {
            execution_id: Uuid::new_v4().to_string(),
            order_id,
            counter_order_id,
            symbol,
            price,
            quantity,
            side,
            buyer_account_id,
            seller_account_id,
            executed_at: Utc::now(),
        }
    }
    
    /// 从订单创建执行记录
    pub fn from_order(order: &Order, counter_order_id: Option<String>, executed_quantity: f64, executed_price: f64) -> Self {
        let (buyer_account_id, seller_account_id) = match order.side {
            OrderSide::Buy => (Some(order.account_id.clone()), None),
            OrderSide::Sell => (None, Some(order.account_id.clone())),
        };
        
        Self {
            execution_id: Uuid::new_v4().to_string(),
            order_id: order.order_id.clone(),
            counter_order_id,
            symbol: order.symbol.clone(),
            price: executed_price,
            quantity: executed_quantity,
            side: order.side,
            buyer_account_id,
            seller_account_id,
            executed_at: Utc::now(),
        }
    }
    
    /// 计算执行的总价值
    pub fn value(&self) -> f64 {
        self.price * self.quantity
    }
}

/// 成交记录 - 表示买卖双方的匹配情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// 成交ID
    pub trade_id: String,
    /// 证券代码
    pub symbol: String,
    /// 成交价格
    pub price: f64,
    /// 成交数量
    pub quantity: f64,
    /// 买方订单ID
    pub buy_order_id: String,
    /// 卖方订单ID
    pub sell_order_id: String,
    /// 买方账户ID
    pub buyer_account_id: String,
    /// 卖方账户ID
    pub seller_account_id: String,
    /// 买方执行ID
    pub buy_execution_id: String,
    /// 卖方执行ID
    pub sell_execution_id: String,
    /// 成交时间
    pub traded_at: DateTime<Utc>,
}

impl Trade {
    /// 创建新的成交记录
    pub fn new(
        symbol: String,
        price: f64,
        quantity: f64,
        buy_order_id: String,
        sell_order_id: String,
        buyer_account_id: String,
        seller_account_id: String,
        buy_execution_id: String,
        sell_execution_id: String,
    ) -> Self {
        Self {
            trade_id: Uuid::new_v4().to_string(),
            symbol,
            price,
            quantity,
            buy_order_id,
            sell_order_id,
            buyer_account_id,
            seller_account_id,
            buy_execution_id,
            sell_execution_id,
            traded_at: Utc::now(),
        }
    }
    
    /// 从两个执行记录创建成交记录
    pub fn from_executions(buy_exec: &Execution, sell_exec: &Execution) -> Option<Self> {
        // 验证执行记录是买卖配对
        if buy_exec.side != OrderSide::Buy || sell_exec.side != OrderSide::Sell {
            return None;
        }
        
        // 验证证券代码、价格和数量是匹配的
        if buy_exec.symbol != sell_exec.symbol || 
           buy_exec.price != sell_exec.price ||
           buy_exec.quantity != sell_exec.quantity {
            return None;
        }
        
        // 买方和卖方账户ID必须存在
        if buy_exec.buyer_account_id.is_none() || sell_exec.seller_account_id.is_none() {
            return None;
        }
        
        Some(Self {
            trade_id: Uuid::new_v4().to_string(),
            symbol: buy_exec.symbol.clone(),
            price: buy_exec.price,
            quantity: buy_exec.quantity,
            buy_order_id: buy_exec.order_id.clone(),
            sell_order_id: sell_exec.order_id.clone(),
            buyer_account_id: buy_exec.buyer_account_id.clone().unwrap(),
            seller_account_id: sell_exec.seller_account_id.clone().unwrap(),
            buy_execution_id: buy_exec.execution_id.clone(),
            sell_execution_id: sell_exec.execution_id.clone(),
            traded_at: Utc::now(),
        })
    }
    
    /// 计算成交的总价值
    pub fn value(&self) -> f64 {
        self.price * self.quantity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_execution_creation() {
        let exec = Execution::new(
            "order-1".to_string(),
            Some("order-2".to_string()),
            "AAPL".to_string(),
            150.0,
            10.0,
            OrderSide::Buy,
            Some("acc-1".to_string()),
            None,
        );
        
        assert_eq!(exec.order_id, "order-1");
        assert_eq!(exec.symbol, "AAPL");
        assert_eq!(exec.price, 150.0);
        assert_eq!(exec.quantity, 10.0);
        assert_eq!(exec.side, OrderSide::Buy);
        assert_eq!(exec.buyer_account_id, Some("acc-1".to_string()));
        assert_eq!(exec.seller_account_id, None);
        assert_eq!(exec.value(), 1500.0);
    }
    
    #[test]
    fn test_execution_from_order() {
        let order = Order {
            order_id: "order-2".to_string(),
            account_id: "acc-2".to_string(),
            symbol: "MSFT".to_string(),
            side: OrderSide::Sell,
            order_type: crate::models::order::OrderType::Limit,
            quantity: 20.0,
            price: Some(250.0),
            stop_price: None,
            status: crate::models::order::OrderStatus::New,
            filled_quantity: 0.0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        let exec = Execution::from_order(&order, Some("order-3".to_string()), 20.0, 250.0);
        
        assert_eq!(exec.order_id, "order-2");
        assert_eq!(exec.symbol, "MSFT");
        assert_eq!(exec.price, 250.0);
        assert_eq!(exec.quantity, 20.0);
        assert_eq!(exec.side, OrderSide::Sell);
        assert_eq!(exec.buyer_account_id, None);
        assert_eq!(exec.seller_account_id, Some("acc-2".to_string()));
        assert_eq!(exec.value(), 5000.0);
    }
    
    #[test]
    fn test_trade_creation() {
        let trade = Trade::new(
            "GOOG".to_string(),
            2000.0,
            5.0,
            "buy-order-1".to_string(),
            "sell-order-1".to_string(),
            "buyer-acc-1".to_string(),
            "seller-acc-1".to_string(),
            "buy-exec-1".to_string(),
            "sell-exec-1".to_string(),
        );
        
        assert_eq!(trade.symbol, "GOOG");
        assert_eq!(trade.price, 2000.0);
        assert_eq!(trade.quantity, 5.0);
        assert_eq!(trade.buy_order_id, "buy-order-1");
        assert_eq!(trade.sell_order_id, "sell-order-1");
        assert_eq!(trade.buyer_account_id, "buyer-acc-1");
        assert_eq!(trade.seller_account_id, "seller-acc-1");
        assert_eq!(trade.buy_execution_id, "buy-exec-1");
        assert_eq!(trade.sell_execution_id, "sell-exec-1");
        assert_eq!(trade.value(), 10000.0);
    }
    
    #[test]
    fn test_trade_from_executions() {
        let buy_exec = Execution::new(
            "buy-order-2".to_string(),
            Some("sell-order-2".to_string()),
            "AMZN".to_string(),
            3000.0,
            2.0,
            OrderSide::Buy,
            Some("buyer-acc-2".to_string()),
            None,
        );
        
        let sell_exec = Execution::new(
            "sell-order-2".to_string(),
            Some("buy-order-2".to_string()),
            "AMZN".to_string(),
            3000.0,
            2.0,
            OrderSide::Sell,
            None,
            Some("seller-acc-2".to_string()),
        );
        
        let trade = Trade::from_executions(&buy_exec, &sell_exec).unwrap();
        
        assert_eq!(trade.symbol, "AMZN");
        assert_eq!(trade.price, 3000.0);
        assert_eq!(trade.quantity, 2.0);
        assert_eq!(trade.buy_order_id, "buy-order-2");
        assert_eq!(trade.sell_order_id, "sell-order-2");
        assert_eq!(trade.buyer_account_id, "buyer-acc-2");
        assert_eq!(trade.seller_account_id, "seller-acc-2");
        assert_eq!(trade.buy_execution_id, buy_exec.execution_id);
        assert_eq!(trade.sell_execution_id, sell_exec.execution_id);
        assert_eq!(trade.value(), 6000.0);
    }
    
    #[test]
    fn test_trade_from_executions_mismatch() {
        // 不匹配的方向
        let exec1 = Execution::new(
            "order-3".to_string(),
            Some("order-4".to_string()),
            "NVDA".to_string(),
            500.0,
            10.0,
            OrderSide::Buy,
            Some("acc-3".to_string()),
            None,
        );
        
        let exec2 = Execution::new(
            "order-4".to_string(),
            Some("order-3".to_string()),
            "NVDA".to_string(),
            500.0,
            10.0,
            OrderSide::Buy, // 两个都是买单，应该不匹配
            Some("acc-4".to_string()),
            None,
        );
        
        assert!(Trade::from_executions(&exec1, &exec2).is_none());
        
        // 不匹配的证券代码
        let exec3 = Execution::new(
            "order-5".to_string(),
            Some("order-6".to_string()),
            "TSLA".to_string(),
            700.0,
            5.0,
            OrderSide::Buy,
            Some("acc-5".to_string()),
            None,
        );
        
        let exec4 = Execution::new(
            "order-6".to_string(),
            Some("order-5".to_string()),
            "FB".to_string(), // 不同的证券代码
           700.0,
            5.0,
            OrderSide::Sell,
            None,
            Some("acc-6".to_string()),
        );
        
        assert!(Trade::from_executions(&exec3, &exec4).is_none());
        
        // 不匹配的价格
        let exec5 = Execution::new(
            "order-7".to_string(),
            Some("order-8".to_string()),
            "AMD".to_string(),
            100.0,
            20.0,
            OrderSide::Buy,
            Some("acc-7".to_string()),
            None,
        );
        
        let exec6 = Execution::new(
            "order-8".to_string(),
            Some("order-7".to_string()),
            "AMD".to_string(),
            105.0, // 不同的价格
            20.0,
            OrderSide::Sell,
            None,
            Some("acc-8".to_string()),
        );
        
        assert!(Trade::from_executions(&exec5, &exec6).is_none());
    }
} 