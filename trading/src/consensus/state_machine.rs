use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use log::{info, debug, warn, error};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::models::order::{Order, OrderRequest, OrderStatus};
use crate::models::account::Account;
use crate::models::execution::Execution;

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
    fn apply(&mut self, command: StateMachineCommand) -> StateMachineResponse;
    
    /// 创建快照
    fn create_snapshot(&self) -> Vec<u8>;
    
    /// 从快照恢复
    fn restore_from_snapshot(&mut self, snapshot: &[u8]) -> Result<(), String>;
}

/// 交易系统状态机实现
pub struct TradingStateMachine {
    /// 订单状态
    orders: HashMap<String, Order>,
    /// 执行记录
    executions: Vec<Execution>,
    /// 账户状态
    accounts: HashMap<String, Account>,
    /// 最后应用的索引
    last_applied_index: u64,
    /// 最后应用的任期
    last_applied_term: u64,
}

impl TradingStateMachine {
    /// 创建新的状态机
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            executions: Vec::new(),
            accounts: HashMap::new(),
            last_applied_index: 0,
            last_applied_term: 0,
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
    pub fn get_executions(&self) -> &[Execution] {
        &self.executions
    }
}

impl StateMachine for TradingStateMachine {
    fn apply(&mut self, command: StateMachineCommand) -> StateMachineResponse {
        match command {
            StateMachineCommand::CreateOrder(request) => {
                let order_id = request.order_id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
                let mut order = Order::from_request(request);
                
                // 确保订单ID存在
                if order.order_id.is_empty() {
                    order.order_id = order_id;
                }
                
                // 存储订单
                self.orders.insert(order.order_id.clone(), order.clone());
                
                info!("状态机: 创建订单 {}", order.order_id);
                StateMachineResponse::Success { 
                    result: Some(order.order_id) 
                }
            },
            StateMachineCommand::UpdateOrderStatus { order_id, new_status } => {
                if let Some(order) = self.orders.get_mut(&order_id) {
                    // 更新订单状态
                    order.status = new_status.clone();
                    info!("状态机: 更新订单 {} 状态为 {:?}", order_id, new_status);
                    StateMachineResponse::Success { result: None }
                } else {
                    warn!("状态机: 订单 {} 不存在，无法更新状态", order_id);
                    StateMachineResponse::Failure { 
                        error: format!("订单不存在: {}", order_id) 
                    }
                }
            },
            StateMachineCommand::AddExecution(execution) => {
                // 添加执行记录
                let exec_id = execution.execution_id.clone();
                self.executions.push(execution);
                
                info!("状态机: 添加执行记录 {}", exec_id);
                StateMachineResponse::Success { 
                    result: Some(exec_id) 
                }
            },
            StateMachineCommand::UpdateAccount(account) => {
                // 更新账户信息
                let account_id = account.account_id.clone();
                self.accounts.insert(account_id.clone(), account);
                
                info!("状态机: 更新账户 {}", account_id);
                StateMachineResponse::Success { 
                    result: None 
                }
            },
            StateMachineCommand::AdminCommand(admin_cmd) => {
                match admin_cmd {
                    AdminCommand::AddNode { node_id, address } => {
                        info!("状态机: 添加节点 {} 到地址 {}", node_id, address);
                        // 实际应用中应该更新集群配置
                        StateMachineResponse::Success { result: None }
                    },
                    AdminCommand::RemoveNode { node_id } => {
                        info!("状态机: 移除节点 {}", node_id);
                        // 实际应用中应该更新集群配置
                        StateMachineResponse::Success { result: None }
                    },
                    AdminCommand::ChangeConfig { key, value } => {
                        info!("状态机: 更改配置 {} = {}", key, value);
                        // 实际应用中应该更新系统配置
                        StateMachineResponse::Success { result: None }
                    }
                }
            }
        }
    }
    
    fn create_snapshot(&self) -> Vec<u8> {
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
} 