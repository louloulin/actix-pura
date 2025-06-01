use std::collections::HashMap;
use log::{info, warn};
use serde::{Serialize, Deserialize};
use crate::models::order::{Order, OrderRequest, OrderStatus};
use crate::models::account::Account;
use crate::models::execution::Execution;
use std::any::Any;

/// 状态机命令类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateMachineCommand {
    /// 创建订单
    CreateOrder(OrderRequest),
    /// 更新订单状态
    UpdateOrderStatus { order_id: String, new_status: OrderStatus },
    /// 添加执行记录
    AddExecution(Execution),
    /// 更新账户
    UpdateAccount(Account),
    /// 管理命令
    AdminCommand(AdminCommand),
}

/// 管理命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdminCommand {
    /// 添加节点到集群
    AddNode { node_id: String, address: String },
    /// 从集群移除节点
    RemoveNode { node_id: String },
    /// 更改配置
    ChangeConfig { key: String, value: String },
}

/// 状态机响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateMachineResponse {
    /// 操作成功
    Success { result: Option<String> },
    /// 操作失败
    Failure { error: String },
}

/// 状态机接口
pub trait StateMachine: Send + Sync {
    /// 应用命令到状态机
    fn apply(&mut self, command: &StateMachineCommand) -> Result<(), String>;
    
    /// 获取当前状态机状态的快照
    fn snapshot(&self) -> Vec<u8>;
    
    /// 从快照恢复状态机
    fn restore_from_snapshot(&mut self, snapshot: &[u8]) -> Result<(), String>;

    /// 用于类型转换的方法 - 允许将状态机转换为Any类型
    fn as_any(&self) -> &dyn Any;
    
    /// 用于类型转换的方法 - 允许将状态机转换为Any类型（可变引用）
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// 交易系统状态机实现
pub struct TradingStateMachine {
    /// 订单状态
    orders: HashMap<String, Order>,
    /// 执行记录
    executions: HashMap<String, Execution>,
    /// 账户状态
    accounts: HashMap<String, Account>,
    /// 最后应用的索引
    last_applied_index: u64,
    /// 最后应用的任期
    last_applied_term: u64,
    /// 下一个订单ID
    next_order_id: u64,
    /// 下一个执行ID
    next_execution_id: u64,
}

impl TradingStateMachine {
    /// 创建新的状态机
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            executions: HashMap::new(),
            accounts: HashMap::new(),
            last_applied_index: 0,
            last_applied_term: 0,
            next_order_id: 1,
            next_execution_id: 1,
        }
    }
    
    /// 更新应用进度
    pub fn update_applied(&mut self, index: u64, term: u64) {
        self.last_applied_index = index;
        self.last_applied_term = term;
    }
    
    /// 获取订单
    pub fn get_order(&self, order_id: &str) -> Option<&Order> {
        self.orders.get(order_id)
    }
    
    /// 获取所有订单
    pub fn get_all_orders(&self) -> Vec<Order> {
        self.orders.values().cloned().collect()
    }
    
    /// 获取账户
    pub fn get_account(&self, account_id: &str) -> Option<&Account> {
        self.accounts.get(account_id)
    }
    
    /// 获取执行记录
    pub fn get_executions(&self) -> Vec<Execution> {
        self.executions.values().cloned().collect()
    }
}

impl StateMachine for TradingStateMachine {
    fn apply(&mut self, command: &StateMachineCommand) -> Result<(), String> {
        match command {
            StateMachineCommand::CreateOrder(request) => {
                // 创建新订单
                let order_id = format!("order-{}", self.next_order_id);
                self.next_order_id += 1;
                
                // 创建订单对象
                let order = Order {
                    order_id: order_id.clone(),
                    account_id: request.client_id.clone(),
                    symbol: request.symbol.clone(),
                    side: request.side,
                    order_type: request.order_type,
                    quantity: request.quantity as f64,
                    price: request.price,
                    stop_price: None,
                    status: OrderStatus::New,
                    filled_quantity: 0.0,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };
                
                // 存储订单
                self.orders.insert(order_id.clone(), order);
                
                info!("状态机: 创建订单 {}", order_id);
                Ok(())
            },
            StateMachineCommand::UpdateOrderStatus { order_id, new_status } => {
                if let Some(order) = self.orders.get_mut(order_id) {
                    // 更新订单状态
                    order.status = new_status.clone();
                    info!("状态机: 更新订单 {} 状态为 {:?}", order_id, new_status);
                    Ok(())
                } else {
                    warn!("状态机: 订单 {} 不存在，无法更新状态", order_id);
                    Err(format!("订单不存在: {}", order_id))
                }
            },
            StateMachineCommand::AddExecution(execution) => {
                // 添加成交记录
                let exec_id = format!("exec-{}", self.next_execution_id);
                self.next_execution_id += 1;
                
                // 创建成交记录
                let mut exec = execution.clone();
                exec.execution_id = exec_id.clone();
                
                // 存储成交记录
                self.executions.insert(exec_id.clone(), exec);
                
                info!("状态机: 添加执行记录 {}", exec_id);
                Ok(())
            },
            StateMachineCommand::UpdateAccount(account) => {
                // 更新账户信息
                let account_id = account.id.clone();
                
                // 存储/更新账户
                self.accounts.insert(account_id.clone(), account.clone());
                
                info!("状态机: 更新账户 {}", account_id);
                Ok(())
            },
            StateMachineCommand::AdminCommand(admin_cmd) => {
                match admin_cmd {
                    AdminCommand::AddNode { node_id, address } => {
                        info!("状态机: 添加节点 {} 到地址 {}", node_id, address);
                        // 实际应用中应该更新集群配置
                        Ok(())
                    },
                    AdminCommand::RemoveNode { node_id } => {
                        info!("状态机: 移除节点 {}", node_id);
                        // 实际应用中应该更新集群配置
                        Ok(())
                    },
                    AdminCommand::ChangeConfig { key, value } => {
                        info!("状态机: 更改配置 {} = {}", key, value);
                        // 实际应用中应该更新系统配置
                        Ok(())
                    }
                }
            }
        }
    }
    
    fn snapshot(&self) -> Vec<u8> {
        info!("状态机: 创建快照 (索引={}, 任期={})", 
              self.last_applied_index, self.last_applied_term);
        
        // 实际实现中应该使用序列化库如bincode或serde_json
        // 这里简化实现
        Vec::new()
    }
    
    fn restore_from_snapshot(&mut self, snapshot: &[u8]) -> Result<(), String> {
        info!("状态机: 从快照恢复");
        
        // 实际实现中应该使用反序列化库
        // 这里简化实现
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
} 