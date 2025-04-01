use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use actix::prelude::*;
use chrono::{DateTime, Utc};

/// 持仓方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    /// 多头持仓
    Long,
    /// 空头持仓
    Short,
}

/// 证券持仓
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// 证券代码
    pub symbol: String,
    /// 持仓数量
    pub quantity: f64,
    /// 平均成本价
    pub avg_price: f64,
    /// 冻结数量
    pub frozen: f64,
}

impl Position {
    /// 创建新持仓
    pub fn new(symbol: String, quantity: f64, price: f64) -> Self {
        Self {
            symbol,
            quantity,
            avg_price: price,
            frozen: 0.0,
        }
    }
    
    /// 当前持仓市值
    pub fn value(&self, current_price: f64) -> f64 {
        self.quantity * current_price
    }
    
    /// 可用持仓数量
    pub fn available(&self) -> f64 {
        self.quantity - self.frozen
    }
    
    /// 更新持仓
    pub fn update(&mut self, quantity_change: f64, price: f64) {
        if quantity_change > 0.0 {
            // 增加持仓，计算新的平均价格
            let total_cost = self.quantity * self.avg_price + quantity_change * price;
            let new_quantity = self.quantity + quantity_change;
            self.avg_price = if new_quantity > 0.0 { total_cost / new_quantity } else { 0.0 };
            self.quantity = new_quantity;
        } else {
            // 减少持仓，保持平均价格不变
            self.quantity += quantity_change;
        }
    }
    
    /// 冻结持仓
    pub fn freeze(&mut self, amount: f64) -> bool {
        if amount > self.available() {
            return false;
        }
        self.frozen += amount;
        true
    }
    
    /// 解冻持仓
    pub fn unfreeze(&mut self, amount: f64) -> bool {
        if amount > self.frozen {
            return false;
        }
        self.frozen -= amount;
        true
    }
}

/// 账户模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// 账户ID
    pub id: String,
    /// 账户名称
    pub name: String,
    /// 账户总余额
    pub balance: f64,
    /// 可用余额
    pub available: f64,
    /// 冻结金额
    pub frozen: f64,
    /// 证券持仓
    pub positions: HashMap<String, Position>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl Account {
    /// 创建新账户
    pub fn new(id: String, name: String, initial_balance: f64) -> Self {
        let now = Utc::now();
        Self {
            id,
            name,
            balance: initial_balance,
            available: initial_balance,
            frozen: 0.0,
            positions: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 获取持仓
    pub fn get_position(&self, symbol: &str) -> Option<&Position> {
        self.positions.get(symbol)
    }
    
    /// 更新或添加持仓
    pub fn update_position(&mut self, symbol: &str, quantity_change: f64, price: f64) {
        self.updated_at = Utc::now();
        
        if let Some(pos) = self.positions.get_mut(symbol) {
            pos.update(quantity_change, price);
            
            // 如果持仓降为0，则移除该持仓
            if pos.quantity <= 0.0 {
                self.positions.remove(symbol);
            }
        } else if quantity_change > 0.0 {
            // 新建持仓
            let position = Position::new(symbol.to_string(), quantity_change, price);
            self.positions.insert(symbol.to_string(), position);
        }
    }
    
    /// 冻结资金
    pub fn freeze_funds(&mut self, amount: f64) -> bool {
        if amount > self.available {
            return false;
        }
        
        self.available -= amount;
        self.frozen += amount;
        self.updated_at = Utc::now();
        true
    }
    
    /// 解冻资金
    pub fn unfreeze_funds(&mut self, amount: f64) -> bool {
        if amount > self.frozen {
            return false;
        }
        
        self.frozen -= amount;
        self.available += amount;
        self.updated_at = Utc::now();
        true
    }
    
    /// 存入资金
    pub fn deposit(&mut self, amount: f64) {
        self.balance += amount;
        self.available += amount;
        self.updated_at = Utc::now();
    }
    
    /// 提取资金
    pub fn withdraw(&mut self, amount: f64) -> bool {
        if amount > self.available {
            return false;
        }
        
        self.balance -= amount;
        self.available -= amount;
        self.updated_at = Utc::now();
        true
    }
    
    /// 计算账户总资产
    pub fn total_assets(&self, price_map: &HashMap<String, f64>) -> f64 {
        let mut total = self.balance;
        
        for (symbol, position) in &self.positions {
            if let Some(&price) = price_map.get(symbol) {
                total += position.value(price);
            } else {
                // 如果没有当前价格，使用平均成本价
                total += position.value(position.avg_price);
            }
        }
        
        total
    }
}

/// 账户查询消息
#[derive(Debug, Clone, Message)]
#[rtype(result = "AccountResult")]
pub enum AccountQuery {
    /// 根据ID查询账户
    ById(String),
    /// 查询所有账户
    All,
    /// 根据持仓查询账户
    ByPosition(String),
}

/// 账户查询结果
#[derive(Debug, Clone)]
pub enum AccountResult {
    /// 单个账户
    Account(Account),
    /// 多个账户
    Accounts(Vec<Account>),
    /// 查询错误
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_position_operations() {
        // 创建新持仓
        let mut pos = Position::new("AAPL".to_string(), 100.0, 150.0);
        assert_eq!(pos.symbol, "AAPL");
        assert_eq!(pos.quantity, 100.0);
        assert_eq!(pos.avg_price, 150.0);
        assert_eq!(pos.frozen, 0.0);
        
        // 测试可用持仓
        assert_eq!(pos.available(), 100.0);
        
        // 测试市值计算
        assert_eq!(pos.value(160.0), 16000.0);
        
        // 测试冻结持仓
        assert!(pos.freeze(50.0));
        assert_eq!(pos.frozen, 50.0);
        assert_eq!(pos.available(), 50.0);
        
        // 测试解冻持仓
        assert!(pos.unfreeze(20.0));
        assert_eq!(pos.frozen, 30.0);
        assert_eq!(pos.available(), 70.0);
        
        // 测试更新持仓 - 增加
        pos.update(50.0, 160.0);
        assert_eq!(pos.quantity, 150.0);
        // 新的平均价格: (100*150 + 50*160)/150 = 153.33...
        assert!((pos.avg_price - 153.33).abs() < 0.01);
        
        // 测试更新持仓 - 减少
        pos.update(-30.0, 170.0);
        assert_eq!(pos.quantity, 120.0);
        // 平均价格不变
        assert!((pos.avg_price - 153.33).abs() < 0.01);
    }
    
    #[test]
    fn test_account_operations() {
        // 创建新账户
        let mut account = Account::new(
            "acc-1".to_string(),
            "Test Account".to_string(),
            10000.0
        );
        
        assert_eq!(account.id, "acc-1");
        assert_eq!(account.name, "Test Account");
        assert_eq!(account.balance, 10000.0);
        assert_eq!(account.available, 10000.0);
        
        // 测试资金冻结
        assert!(account.freeze_funds(3000.0));
        assert_eq!(account.available, 7000.0);
        assert_eq!(account.frozen, 3000.0);
        
        // 测试资金解冻
        assert!(account.unfreeze_funds(1000.0));
        assert_eq!(account.available, 8000.0);
        assert_eq!(account.frozen, 2000.0);
        
        // 测试存入资金
        account.deposit(5000.0);
        assert_eq!(account.balance, 15000.0);
        assert_eq!(account.available, 13000.0);
        
        // 测试提取资金
        assert!(account.withdraw(3000.0));
        assert_eq!(account.balance, 12000.0);
        assert_eq!(account.available, 10000.0);
        
        // 测试持仓更新
        account.update_position("AAPL", 50.0, 150.0);
        let pos = account.get_position("AAPL").unwrap();
        assert_eq!(pos.quantity, 50.0);
        assert_eq!(pos.avg_price, 150.0);
        
        // 测试增加持仓
        account.update_position("AAPL", 30.0, 160.0);
        let pos = account.get_position("AAPL").unwrap();
        assert_eq!(pos.quantity, 80.0);
        // 新的平均价格: (50*150 + 30*160)/80 = 153.75
        assert!((pos.avg_price - 153.75).abs() < 0.01);
        
        // 测试减少持仓
        account.update_position("AAPL", -20.0, 170.0);
        let pos = account.get_position("AAPL").unwrap();
        assert_eq!(pos.quantity, 60.0);
        assert!((pos.avg_price - 153.75).abs() < 0.01);
        
        // 测试清空持仓
        account.update_position("AAPL", -60.0, 180.0);
        assert!(account.get_position("AAPL").is_none());
        
        // 测试总资产计算
        account.update_position("MSFT", 100.0, 250.0);
        account.update_position("GOOG", 10.0, 2000.0);
        
        let mut price_map = HashMap::new();
        price_map.insert("MSFT".to_string(), 260.0);
        price_map.insert("GOOG".to_string(), 2100.0);
        
        // 总资产 = 现金 + 持仓市值
        // 12000 + (100 * 260) + (10 * 2100) = 12000 + 26000 + 21000 = 59000
        assert_eq!(account.total_assets(&price_map), 59000.0);
    }
} 