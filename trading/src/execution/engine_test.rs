use std::sync::{Arc, RwLock};
use std::collections::HashMap;

use crate::execution::engine::ExecutionActor;
use crate::execution::matcher::OrderMatcher;
use crate::models::order::{Order, OrderSide, OrderType, OrderStatus};
use crate::models::execution::{ExecutionRecord, Trade};
use crate::models::message::{ExecuteOrderMessage, MessageType};

// 模拟Raft客户端
struct MockRaftClient {
    logs: Arc<RwLock<Vec<String>>>,
}

impl MockRaftClient {
    fn new() -> Self {
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn append_log(&self, log: String) -> Result<(), String> {
        let mut logs = self.logs.write().unwrap();
        logs.push(log);
        Ok(())
    }

    fn get_logs(&self) -> Vec<String> {
        let logs = self.logs.read().unwrap();
        logs.clone()
    }
}

// 模拟账户Actor
struct MockAccountActor {
    notifications: Arc<RwLock<Vec<String>>>,
}

impl MockAccountActor {
    fn new() -> Self {
        Self {
            notifications: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn send_notification(&self, notification: String) -> Result<(), String> {
        let mut notifications = self.notifications.write().unwrap();
        notifications.push(notification);
        Ok(())
    }

    fn get_notifications(&self) -> Vec<String> {
        let notifications = self.notifications.read().unwrap();
        notifications.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    fn create_test_order(id: &str, symbol: &str, side: OrderSide, price: Option<f64>, quantity: u64) -> Order {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        Order {
            order_id: id.to_string(),
            account_id: "test-account".to_string(),
            symbol: symbol.to_string(),
            side,
            price,
            quantity,
            filled_quantity: 0,
            order_type: if price.is_some() { OrderType::Limit } else { OrderType::Market },
            status: OrderStatus::New,
            created_at: now,
            updated_at: now,
        }
    }
    
    #[test]
    fn test_process_market_buy_order() {
        // 创建模拟依赖
        let raft_client = Arc::new(MockRaftClient::new());
        let account_actor = Arc::new(MockAccountActor::new());
        let matcher = Arc::new(OrderMatcher::new());
        
        // 创建ExecutionActor
        let mut exec_actor = ExecutionActor::new(
            "node-1".to_string(),
            matcher.clone(),
            raft_client.clone(),
            Some(account_actor.clone())
        );
        
        // 创建一个卖单并添加到订单簿
        let sell_order = create_test_order("sell-1", "AAPL", OrderSide::Sell, Some(100.0), 200);
        let msg = ExecuteOrderMessage { order: sell_order };
        let _ = exec_actor.process_order(msg);
        
        // 创建一个市价买单
        let buy_order = create_test_order("buy-1", "AAPL", OrderSide::Buy, None, 100);
        let msg = ExecuteOrderMessage { order: buy_order };
        let result = exec_actor.process_order(msg);
        
        // 验证结果
        assert!(result.is_ok());
        
        // 验证执行记录
        let executions = exec_actor.get_executions();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].order_id, "buy-1");
        assert_eq!(executions[0].price, 100.0); // 按卖单价格成交
        assert_eq!(executions[0].quantity, 100);
        
        // 验证交易记录
        let trades = exec_actor.get_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].buy_order_id, "buy-1");
        assert_eq!(trades[0].sell_order_id, "sell-1");
        assert_eq!(trades[0].quantity, 100);
        
        // 验证通知
        let notifications = account_actor.get_notifications();
        assert!(notifications.len() > 0);
    }
    
    #[test]
    fn test_process_limit_orders_match() {
        // 创建模拟依赖
        let raft_client = Arc::new(MockRaftClient::new());
        let account_actor = Arc::new(MockAccountActor::new());
        let matcher = Arc::new(OrderMatcher::new());
        
        // 创建ExecutionActor
        let mut exec_actor = ExecutionActor::new(
            "node-1".to_string(),
            matcher.clone(),
            raft_client.clone(),
            Some(account_actor.clone())
        );
        
        // 创建一个买单
        let buy_order = create_test_order("buy-1", "AAPL", OrderSide::Buy, Some(110.0), 100);
        let msg = ExecuteOrderMessage { order: buy_order };
        let _ = exec_actor.process_order(msg);
        
        // 创建一个卖单，价格低于买单价格，应该立即成交
        let sell_order = create_test_order("sell-1", "AAPL", OrderSide::Sell, Some(100.0), 50);
        let msg = ExecuteOrderMessage { order: sell_order };
        let result = exec_actor.process_order(msg);
        
        // 验证结果
        assert!(result.is_ok());
        
        // 验证执行记录
        let executions = exec_actor.get_executions();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].buy_order_id, "buy-1");
        assert_eq!(executions[0].sell_order_id, "sell-1");
        assert_eq!(executions[0].price, 110.0); // 按买单价格成交
        assert_eq!(executions[0].quantity, 50);
        
        // 验证交易记录
        let trades = exec_actor.get_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, 110.0);
        assert_eq!(trades[0].quantity, 50);
        
        // 验证部分成交的买单仍在订单簿
        let order_book = exec_actor.get_or_create_order_book("AAPL".to_string());
        let buy_orders = order_book.get_buy_orders();
        assert_eq!(buy_orders.len(), 1);
        assert_eq!(buy_orders[0].filled_quantity, 50);
        assert_eq!(buy_orders[0].status, OrderStatus::PartiallyFilled);
    }
    
    #[test]
    fn test_process_no_match() {
        // 创建模拟依赖
        let raft_client = Arc::new(MockRaftClient::new());
        let account_actor = Arc::new(MockAccountActor::new());
        let matcher = Arc::new(OrderMatcher::new());
        
        // 创建ExecutionActor
        let mut exec_actor = ExecutionActor::new(
            "node-1".to_string(),
            matcher.clone(),
            raft_client.clone(),
            Some(account_actor.clone())
        );
        
        // 创建一个买单
        let buy_order = create_test_order("buy-1", "AAPL", OrderSide::Buy, Some(100.0), 100);
        let msg = ExecuteOrderMessage { order: buy_order };
        let _ = exec_actor.process_order(msg);
        
        // 创建一个卖单，价格高于买单价格，不会立即成交
        let sell_order = create_test_order("sell-1", "AAPL", OrderSide::Sell, Some(120.0), 100);
        let msg = ExecuteOrderMessage { order: sell_order };
        let result = exec_actor.process_order(msg);
        
        // 验证结果
        assert!(result.is_ok());
        
        // 验证没有执行记录
        let executions = exec_actor.get_executions();
        assert_eq!(executions.len(), 0);
        
        // 验证没有交易记录
        let trades = exec_actor.get_trades();
        assert_eq!(trades.len(), 0);
        
        // 验证两个订单都在订单簿
        let order_book = exec_actor.get_or_create_order_book("AAPL".to_string());
        let buy_orders = order_book.get_buy_orders();
        assert_eq!(buy_orders.len(), 1);
        let sell_orders = order_book.get_sell_orders();
        assert_eq!(sell_orders.len(), 1);
    }
    
    #[test]
    fn test_process_multiple_matches() {
        // 创建模拟依赖
        let raft_client = Arc::new(MockRaftClient::new());
        let account_actor = Arc::new(MockAccountActor::new());
        let matcher = Arc::new(OrderMatcher::new());
        
        // 创建ExecutionActor
        let mut exec_actor = ExecutionActor::new(
            "node-1".to_string(),
            matcher.clone(),
            raft_client.clone(),
            Some(account_actor.clone())
        );
        
        // 创建多个买单
        let buy_order1 = create_test_order("buy-1", "AAPL", OrderSide::Buy, Some(100.0), 50);
        let msg = ExecuteOrderMessage { order: buy_order1 };
        let _ = exec_actor.process_order(msg);
        
        let buy_order2 = create_test_order("buy-2", "AAPL", OrderSide::Buy, Some(105.0), 30);
        let msg = ExecuteOrderMessage { order: buy_order2 };
        let _ = exec_actor.process_order(msg);
        
        let buy_order3 = create_test_order("buy-3", "AAPL", OrderSide::Buy, Some(110.0), 20);
        let msg = ExecuteOrderMessage { order: buy_order3 };
        let _ = exec_actor.process_order(msg);
        
        // 创建一个大的卖单，应该匹配多个买单
        let sell_order = create_test_order("sell-1", "AAPL", OrderSide::Sell, Some(95.0), 90);
        let msg = ExecuteOrderMessage { order: sell_order };
        let result = exec_actor.process_order(msg);
        
        // 验证结果
        assert!(result.is_ok());
        
        // 验证执行记录 - 应该有3条，分别对应3个买单
        let executions = exec_actor.get_executions();
        assert_eq!(executions.len(), 3);
        
        // 验证交易记录
        let trades = exec_actor.get_trades();
        assert_eq!(trades.len(), 3);
        
        // 计算总成交量
        let total_volume: u64 = trades.iter().map(|t| t.quantity).sum();
        assert_eq!(total_volume, 90);
        
        // 验证最高价格优先
        assert_eq!(trades[0].buy_order_id, "buy-3");
        assert_eq!(trades[1].buy_order_id, "buy-2");
        assert_eq!(trades[2].buy_order_id, "buy-1");
        
        // 验证买单部分成交
        let order_book = exec_actor.get_or_create_order_book("AAPL".to_string());
        let buy_orders = order_book.get_buy_orders();
        assert_eq!(buy_orders.len(), 1);  // 只剩下buy-1的一部分
        assert_eq!(buy_orders[0].order_id, "buy-1");
        assert_eq!(buy_orders[0].filled_quantity, 40);  // 50 - (90 - 20 - 30) = 50 - 40 = 10 剩余
    }
} 