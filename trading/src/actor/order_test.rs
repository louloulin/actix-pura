use std::sync::{Arc, RwLock};
use std::collections::HashMap;

use crate::actor::order::OrderActor;
use crate::models::order::{Order, OrderSide, OrderType, OrderStatus};
use crate::models::message::{CreateOrderMessage, CancelOrderMessage, QueryOrderMessage};
use crate::risk::manager::RiskCheckResult;

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

// 模拟执行引擎
struct MockExecutionEngine {
    executions: Arc<RwLock<Vec<Order>>>,
}

impl MockExecutionEngine {
    fn new() -> Self {
        Self {
            executions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn execute_order(&self, order: Order) -> Result<(), String> {
        let mut executions = self.executions.write().unwrap();
        executions.push(order);
        Ok(())
    }

    fn get_executions(&self) -> Vec<Order> {
        let executions = self.executions.read().unwrap();
        executions.clone()
    }
}

// 模拟风控管理器
struct MockRiskManager {
    checks: Arc<RwLock<Vec<Order>>>,
    should_pass: bool,
}

impl MockRiskManager {
    fn new(should_pass: bool) -> Self {
        Self {
            checks: Arc::new(RwLock::new(Vec::new())),
            should_pass,
        }
    }

    fn check_risk(&self, order: Order) -> RiskCheckResult {
        let mut checks = self.checks.write().unwrap();
        checks.push(order);
        
        if self.should_pass {
            RiskCheckResult::Passed
        } else {
            RiskCheckResult::Failed("Risk check failed".to_string())
        }
    }

    fn get_checks(&self) -> Vec<Order> {
        let checks = self.checks.read().unwrap();
        checks.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_order_success() {
        // 创建模拟依赖
        let raft_client = Arc::new(MockRaftClient::new());
        let execution_engine = Arc::new(MockExecutionEngine::new());
        let risk_manager = Arc::new(MockRiskManager::new(true));
        
        // 创建OrderActor
        let order_actor = OrderActor::new(
            "node-1".to_string(),
            raft_client.clone(),
            execution_engine.clone(),
            risk_manager.clone()
        );
        
        // 创建订单消息
        let create_order_msg = CreateOrderMessage {
            account_id: "acc-123".to_string(),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            price: Some(150.0),
            quantity: 100,
            order_type: OrderType::Limit,
        };
        
        // 处理消息
        let result = order_actor.create_and_execute_order(create_order_msg);
        
        // 验证结果
        assert!(result.is_ok());
        
        // 验证Raft日志
        let logs = raft_client.get_logs();
        assert_eq!(logs.len(), 1);
        
        // 验证风控检查
        let checks = risk_manager.get_checks();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].symbol, "AAPL");
        
        // 验证执行
        let executions = execution_engine.get_executions();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].symbol, "AAPL");
        assert_eq!(executions[0].side, OrderSide::Buy);
    }
    
    #[test]
    fn test_create_order_risk_fail() {
        // 创建模拟依赖，风控会拒绝订单
        let raft_client = Arc::new(MockRaftClient::new());
        let execution_engine = Arc::new(MockExecutionEngine::new());
        let risk_manager = Arc::new(MockRiskManager::new(false));
        
        // 创建OrderActor
        let order_actor = OrderActor::new(
            "node-1".to_string(),
            raft_client.clone(),
            execution_engine.clone(),
            risk_manager.clone()
        );
        
        // 创建订单消息
        let create_order_msg = CreateOrderMessage {
            account_id: "acc-123".to_string(),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            price: Some(150.0),
            quantity: 100,
            order_type: OrderType::Limit,
        };
        
        // 处理消息
        let result = order_actor.create_and_execute_order(create_order_msg);
        
        // 验证结果
        assert!(result.is_err());
        
        // 验证风控检查
        let checks = risk_manager.get_checks();
        assert_eq!(checks.len(), 1);
        
        // 验证未执行
        let executions = execution_engine.get_executions();
        assert_eq!(executions.len(), 0);
    }
    
    #[test]
    fn test_cancel_order() {
        // 创建模拟依赖
        let raft_client = Arc::new(MockRaftClient::new());
        let execution_engine = Arc::new(MockExecutionEngine::new());
        let risk_manager = Arc::new(MockRiskManager::new(true));
        
        // 创建OrderActor
        let mut order_actor = OrderActor::new(
            "node-1".to_string(),
            raft_client.clone(),
            execution_engine.clone(),
            risk_manager.clone()
        );
        
        // 创建订单
        let create_order_msg = CreateOrderMessage {
            account_id: "acc-123".to_string(),
            symbol: "AAPL".to_string(),
            side: OrderSide::Buy,
            price: Some(150.0),
            quantity: 100,
            order_type: OrderType::Limit,
        };
        
        let result = order_actor.create_and_execute_order(create_order_msg);
        assert!(result.is_ok());
        let order_id = result.unwrap();
        
        // 取消订单
        let cancel_msg = CancelOrderMessage {
            account_id: "acc-123".to_string(),
            order_id: order_id.clone(),
        };
        
        let cancel_result = order_actor.cancel_order(cancel_msg);
        assert!(cancel_result.is_ok());
        
        // 验证订单状态
        let query_msg = QueryOrderMessage {
            account_id: "acc-123".to_string(),
            order_id: order_id.clone(),
        };
        
        let order = order_actor.query_order(query_msg).unwrap();
        assert_eq!(order.status, OrderStatus::Canceled);
    }
    
    #[test]
    fn test_query_orders() {
        // 创建模拟依赖
        let raft_client = Arc::new(MockRaftClient::new());
        let execution_engine = Arc::new(MockExecutionEngine::new());
        let risk_manager = Arc::new(MockRiskManager::new(true));
        
        // 创建OrderActor
        let mut order_actor = OrderActor::new(
            "node-1".to_string(),
            raft_client.clone(),
            execution_engine.clone(),
            risk_manager.clone()
        );
        
        // 创建多个订单
        for i in 0..5 {
            let create_order_msg = CreateOrderMessage {
                account_id: "acc-123".to_string(),
                symbol: if i % 2 == 0 { "AAPL" } else { "MSFT" }.to_string(),
                side: if i % 3 == 0 { OrderSide::Buy } else { OrderSide::Sell },
                price: Some(150.0 + i as f64),
                quantity: 100 + i * 10,
                order_type: OrderType::Limit,
            };
            
            let result = order_actor.create_and_execute_order(create_order_msg);
            assert!(result.is_ok());
        }
        
        // 查询特定账户所有订单
        let all_orders = order_actor.query_orders_by_account("acc-123".to_string());
        assert_eq!(all_orders.len(), 5);
        
        // 查询特定账户特定股票的订单
        let aapl_orders = order_actor.query_orders_by_account_and_symbol(
            "acc-123".to_string(), 
            "AAPL".to_string()
        );
        assert_eq!(aapl_orders.len(), 3);
        
        // 查询买单
        let buy_orders = order_actor.query_orders_by_account_and_side(
            "acc-123".to_string(), 
            OrderSide::Buy
        );
        assert_eq!(buy_orders.len(), 2);
    }
} 