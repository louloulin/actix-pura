use std::sync::{Arc, RwLock};
use std::collections::HashMap;

use crate::risk::manager::{RiskManager, RiskConfig, RiskCheckRequest, RiskCheckResult, UpdateRiskConfigMessage};
use crate::models::order::{Order, OrderSide, OrderType, OrderStatus};
use crate::models::account::{Account, Position};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_order(id: &str, account_id: &str, symbol: &str, side: OrderSide, price: Option<f64>, quantity: u64) -> Order {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        Order {
            order_id: id.to_string(),
            account_id: account_id.to_string(),
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
    
    fn create_test_account(id: &str, balance: f64) -> Account {
        Account {
            account_id: id.to_string(),
            balance,
            positions: HashMap::new(),
            status: "active".to_string(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    #[test]
    fn test_max_order_value_check() {
        // 创建风控配置
        let config = RiskConfig {
            max_order_value: 10000.0,
            max_position_value: 50000.0,
            max_single_order_quantity: 1000,
            enabled: true,
        };
        
        // 创建风控管理器
        let risk_manager = RiskManager::new("node-1".to_string(), config);
        
        // 创建测试账户
        let account = create_test_account("acc-1", 20000.0);
        risk_manager.update_account_info(account);
        
        // 测试订单金额超过限制
        let order = create_test_order(
            "order-1", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(105.0), 
            100, // 价值为 105 * 100 = 10500 > 10000
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Failed(reason) => {
                assert!(reason.contains("exceeds maximum order value"));
            },
            _ => panic!("Expected risk check to fail"),
        }
        
        // 测试在限制内的订单
        let order = create_test_order(
            "order-2", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(95.0), 
            100, // 价值为 95 * 100 = 9500 < 10000
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Passed => {},
            _ => panic!("Expected risk check to pass"),
        }
    }
    
    #[test]
    fn test_max_position_check() {
        // 创建风控配置
        let config = RiskConfig {
            max_order_value: 10000.0,
            max_position_value: 15000.0,
            max_single_order_quantity: 1000,
            enabled: true,
        };
        
        // 创建风控管理器
        let risk_manager = RiskManager::new("node-1".to_string(), config);
        
        // 创建带持仓的测试账户
        let mut account = create_test_account("acc-1", 20000.0);
        
        // 添加现有持仓
        let mut positions = HashMap::new();
        positions.insert("AAPL".to_string(), Position {
            symbol: "AAPL".to_string(),
            quantity: 100,
            average_price: 100.0,
        });
        account.positions = positions;
        
        risk_manager.update_account_info(account);
        
        // 测试会使持仓超限的买单
        let order = create_test_order(
            "order-1", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(100.0), 
            100, // 已有持仓价值 100 * 100 = 10000，新增 100 * 100 = 10000，总计 20000 > 15000
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Failed(reason) => {
                assert!(reason.contains("would exceed maximum position value"));
            },
            _ => panic!("Expected risk check to fail"),
        }
        
        // 测试卖单，不受持仓限制
        let order = create_test_order(
            "order-2", 
            "acc-1", 
            "AAPL", 
            OrderSide::Sell, 
            Some(100.0), 
            50,
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Passed => {},
            _ => panic!("Expected risk check to pass"),
        }
        
        // 测试在限制内的买单
        let order = create_test_order(
            "order-3", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(100.0), 
            40, // 已有持仓价值 10000，新增 100 * 40 = 4000，总计 14000 < 15000
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Passed => {},
            _ => panic!("Expected risk check to pass"),
        }
    }
    
    #[test]
    fn test_max_quantity_check() {
        // 创建风控配置
        let config = RiskConfig {
            max_order_value: 1000000.0, // 设置很高以不触发金额检查
            max_position_value: 1000000.0,
            max_single_order_quantity: 500,
            enabled: true,
        };
        
        // 创建风控管理器
        let risk_manager = RiskManager::new("node-1".to_string(), config);
        
        // 创建测试账户
        let account = create_test_account("acc-1", 20000.0);
        risk_manager.update_account_info(account);
        
        // 测试超过最大数量限制的订单
        let order = create_test_order(
            "order-1", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(100.0), 
            600, // > 500
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Failed(reason) => {
                assert!(reason.contains("exceeds maximum quantity"));
            },
            _ => panic!("Expected risk check to fail"),
        }
        
        // 测试在限制内的订单
        let order = create_test_order(
            "order-2", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(100.0), 
            450, // < 500
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Passed => {},
            _ => panic!("Expected risk check to pass"),
        }
    }
    
    #[test]
    fn test_account_balance_check() {
        // 创建风控配置
        let config = RiskConfig {
            max_order_value: 1000000.0,
            max_position_value: 1000000.0,
            max_single_order_quantity: 1000,
            enabled: true,
        };
        
        // 创建风控管理器
        let risk_manager = RiskManager::new("node-1".to_string(), config);
        
        // 创建测试账户
        let account = create_test_account("acc-1", 5000.0);
        risk_manager.update_account_info(account);
        
        // 测试超过账户余额的订单
        let order = create_test_order(
            "order-1", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(100.0), 
            60, // 价值 100 * 60 = 6000 > 账户余额 5000
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Failed(reason) => {
                assert!(reason.contains("insufficient balance"));
            },
            _ => panic!("Expected risk check to fail"),
        }
        
        // 测试在账户余额内的订单
        let order = create_test_order(
            "order-2", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(100.0), 
            40, // 价值 100 * 40 = 4000 < 账户余额 5000
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Passed => {},
            _ => panic!("Expected risk check to pass"),
        }
    }
    
    #[test]
    fn test_update_risk_config() {
        // 创建初始风控配置
        let config = RiskConfig {
            max_order_value: 10000.0,
            max_position_value: 50000.0,
            max_single_order_quantity: 500,
            enabled: true,
        };
        
        // 创建风控管理器
        let risk_manager = RiskManager::new("node-1".to_string(), config);
        
        // 创建测试账户
        let account = create_test_account("acc-1", 20000.0);
        risk_manager.update_account_info(account);
        
        // 测试原配置下会被拒绝的订单
        let order = create_test_order(
            "order-1", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(100.0), 
            600, // > 500 初始限制
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Failed(_) => {},
            _ => panic!("Expected risk check to fail with initial config"),
        }
        
        // 更新配置
        let update_msg = UpdateRiskConfigMessage {
            max_order_value: Some(20000.0),
            max_position_value: Some(100000.0),
            max_single_order_quantity: Some(1000),
            enabled: Some(true),
        };
        
        risk_manager.update_config(update_msg);
        
        // 测试新配置下同样的订单应该通过
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Passed => {},
            _ => panic!("Expected risk check to pass with updated config"),
        }
    }
    
    #[test]
    fn test_risk_check_disabled() {
        // 创建风控配置，但禁用检查
        let config = RiskConfig {
            max_order_value: 10000.0,
            max_position_value: 50000.0,
            max_single_order_quantity: 500,
            enabled: false,
        };
        
        // 创建风控管理器
        let risk_manager = RiskManager::new("node-1".to_string(), config);
        
        // 创建测试账户
        let account = create_test_account("acc-1", 5000.0);
        risk_manager.update_account_info(account);
        
        // 测试超过所有限制的订单，但因为检查被禁用，应该通过
        let order = create_test_order(
            "order-1", 
            "acc-1", 
            "AAPL", 
            OrderSide::Buy, 
            Some(100.0), 
            1000, // 超过数量限制，总金额也超过账户余额
        );
        
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Passed => {},
            _ => panic!("Expected risk check to pass when disabled"),
        }
        
        // 启用风控检查
        let update_msg = UpdateRiskConfigMessage {
            max_order_value: None,
            max_position_value: None,
            max_single_order_quantity: None,
            enabled: Some(true),
        };
        
        risk_manager.update_config(update_msg);
        
        // 再次测试同样的订单，现在应该失败
        let request = RiskCheckRequest { order: order.clone() };
        let result = risk_manager.check_risk(request);
        
        match result {
            RiskCheckResult::Failed(_) => {},
            _ => panic!("Expected risk check to fail when enabled"),
        }
    }
} 