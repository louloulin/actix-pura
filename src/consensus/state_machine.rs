use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use tokio::sync::mpsc;
use log::{info, warn, error, debug};
use anyhow::Result;
use serde::{Serialize, Deserialize};

use crate::models::message::LogEntry;
use crate::models::order::{Order, OrderStatus};
use crate::models::account::Account;
use crate::models::execution::{ExecutionRecord, Trade};

/// 状态机接口，定义了应用日志条目的方法
pub trait StateMachine: Send + Sync {
    /// 应用日志条目到状态机
    fn apply(&mut self, entry: LogEntry) -> Result<()>;
    
    /// 获取最后应用的日志索引
    fn get_last_applied_index(&self) -> u64;
    
    /// 获取当前快照
    fn get_snapshot(&self) -> Result<Vec<u8>>;
    
    /// 从快照恢复状态
    fn restore_from_snapshot(&mut self, snapshot: Vec<u8>) -> Result<()>;
}

/// 交易系统状态机，维护订单、账户、执行记录的状态
pub struct TradingStateMachine {
    /// 最后应用的日志索引
    last_applied_index: u64,
    
    /// 订单状态
    orders: HashMap<String, Order>,
    
    /// 账户状态
    accounts: HashMap<String, Account>,
    
    /// 执行记录
    executions: HashMap<String, ExecutionRecord>,
    
    /// 交易记录
    trades: HashMap<String, Trade>,
}

impl TradingStateMachine {
    /// 创建新的交易系统状态机
    pub fn new() -> Self {
        Self {
            last_applied_index: 0,
            orders: HashMap::new(),
            accounts: HashMap::new(),
            executions: HashMap::new(),
            trades: HashMap::new(),
        }
    }
    
    /// 应用订单创建日志
    fn apply_order_create(&mut self, order: Order) -> Result<()> {
        debug!("Applying order create: {}", order.order_id);
        self.orders.insert(order.order_id.clone(), order);
        Ok(())
    }
    
    /// 应用订单取消日志
    fn apply_order_cancel(&mut self, order_id: String) -> Result<()> {
        debug!("Applying order cancel: {}", order_id);
        if let Some(mut order) = self.orders.get(&order_id).cloned() {
            order.status = OrderStatus::Canceled;
            self.orders.insert(order_id, order);
            Ok(())
        } else {
            warn!("Cannot cancel non-existent order: {}", order_id);
            Ok(()) // 不存在的订单也视为成功取消
        }
    }
    
    /// 应用执行记录日志
    fn apply_execution(&mut self, execution: ExecutionRecord) -> Result<()> {
        debug!("Applying execution: {}", execution.execution_id);
        
        // 更新订单状态
        if let Some(mut buy_order) = self.orders.get(&execution.buy_order_id).cloned() {
            buy_order.filled_quantity += execution.quantity;
            if buy_order.filled_quantity >= buy_order.quantity {
                buy_order.status = OrderStatus::Filled;
            } else {
                buy_order.status = OrderStatus::PartiallyFilled;
            }
            self.orders.insert(buy_order.order_id.clone(), buy_order);
        }
        
        if let Some(mut sell_order) = self.orders.get(&execution.sell_order_id).cloned() {
            sell_order.filled_quantity += execution.quantity;
            if sell_order.filled_quantity >= sell_order.quantity {
                sell_order.status = OrderStatus::Filled;
            } else {
                sell_order.status = OrderStatus::PartiallyFilled;
            }
            self.orders.insert(sell_order.order_id.clone(), sell_order);
        }
        
        // 保存执行记录
        self.executions.insert(execution.execution_id.clone(), execution);
        
        Ok(())
    }
    
    /// 应用交易记录日志
    fn apply_trade(&mut self, trade: Trade) -> Result<()> {
        debug!("Applying trade: {}", trade.trade_id);
        
        // 保存交易记录
        self.trades.insert(trade.trade_id.clone(), trade);
        
        Ok(())
    }
    
    /// 应用账户更新日志
    fn apply_account_update(&mut self, account: Account) -> Result<()> {
        debug!("Applying account update: {}", account.account_id);
        
        // 更新账户
        self.accounts.insert(account.account_id.clone(), account);
        
        Ok(())
    }
    
    /// 获取订单
    pub fn get_order(&self, order_id: &str) -> Option<Order> {
        self.orders.get(order_id).cloned()
    }
    
    /// 查询订单列表
    pub fn query_orders(&self, filter: OrderFilter) -> Vec<Order> {
        self.orders.values()
            .filter(|order| filter.matches(order))
            .cloned()
            .collect()
    }
    
    /// 获取账户
    pub fn get_account(&self, account_id: &str) -> Option<Account> {
        self.accounts.get(account_id).cloned()
    }
    
    /// 获取执行记录
    pub fn get_execution(&self, execution_id: &str) -> Option<ExecutionRecord> {
        self.executions.get(execution_id).cloned()
    }
    
    /// 查询执行记录列表
    pub fn query_executions(&self, filter: ExecutionFilter) -> Vec<ExecutionRecord> {
        self.executions.values()
            .filter(|execution| filter.matches(execution))
            .cloned()
            .collect()
    }
    
    /// 获取交易记录
    pub fn get_trade(&self, trade_id: &str) -> Option<Trade> {
        self.trades.get(trade_id).cloned()
    }
    
    /// 查询交易记录列表
    pub fn query_trades(&self, filter: TradeFilter) -> Vec<Trade> {
        self.trades.values()
            .filter(|trade| filter.matches(trade))
            .cloned()
            .collect()
    }
}

impl StateMachine for TradingStateMachine {
    fn apply(&mut self, entry: LogEntry) -> Result<()> {
        match entry {
            LogEntry::OrderRequest(request) => {
                if let Some(order_id) = &request.order_id {
                    let order = Order::new(
                        order_id.clone(),
                        request.client_id.clone(),
                        request.symbol.clone(),
                        request.side,
                        request.price,
                        request.quantity,
                        request.order_type,
                    );
                    self.apply_order_create(order)
                } else {
                    warn!("Order request without order_id");
                    Ok(())
                }
            },
            LogEntry::CancelOrder(cancel_request) => {
                self.apply_order_cancel(cancel_request.order_id)
            },
            LogEntry::Execution(execution) => {
                self.apply_execution(execution)
            },
            LogEntry::Trade(trade) => {
                self.apply_trade(trade)
            },
            LogEntry::AccountUpdate(account) => {
                self.apply_account_update(account)
            },
            LogEntry::ConfigUpdate(_) => {
                // 系统配置更新目前不处理
                Ok(())
            },
            LogEntry::Custom { data_type, data: _ } => {
                warn!("Unhandled custom log entry type: {}", data_type);
                Ok(())
            },
        }
    }
    
    fn get_last_applied_index(&self) -> u64 {
        self.last_applied_index
    }
    
    fn get_snapshot(&self) -> Result<Vec<u8>> {
        // 序列化状态机状态
        let snapshot = Snapshot {
            last_applied_index: self.last_applied_index,
            orders: self.orders.clone(),
            accounts: self.accounts.clone(),
            executions: self.executions.clone(),
            trades: self.trades.clone(),
        };
        
        let serialized = bincode::serialize(&snapshot)?;
        Ok(serialized)
    }
    
    fn restore_from_snapshot(&mut self, snapshot: Vec<u8>) -> Result<()> {
        // 反序列化状态机状态
        let snapshot: Snapshot = bincode::deserialize(&snapshot)?;
        
        self.last_applied_index = snapshot.last_applied_index;
        self.orders = snapshot.orders;
        self.accounts = snapshot.accounts;
        self.executions = snapshot.executions;
        self.trades = snapshot.trades;
        
        Ok(())
    }
}

/// 状态机快照，用于序列化和反序列化状态机状态
#[derive(Serialize, Deserialize)]
struct Snapshot {
    last_applied_index: u64,
    orders: HashMap<String, Order>,
    accounts: HashMap<String, Account>,
    executions: HashMap<String, ExecutionRecord>,
    trades: HashMap<String, Trade>,
}

/// 订单过滤器，用于查询订单
pub struct OrderFilter {
    pub account_id: Option<String>,
    pub symbol: Option<String>,
    pub status: Option<OrderStatus>,
}

impl OrderFilter {
    pub fn new() -> Self {
        Self {
            account_id: None,
            symbol: None,
            status: None,
        }
    }
    
    pub fn with_account_id(mut self, account_id: String) -> Self {
        self.account_id = Some(account_id);
        self
    }
    
    pub fn with_symbol(mut self, symbol: String) -> Self {
        self.symbol = Some(symbol);
        self
    }
    
    pub fn with_status(mut self, status: OrderStatus) -> Self {
        self.status = Some(status);
        self
    }
    
    pub fn matches(&self, order: &Order) -> bool {
        if let Some(account_id) = &self.account_id {
            if order.account_id != *account_id {
                return false;
            }
        }
        
        if let Some(symbol) = &self.symbol {
            if order.symbol != *symbol {
                return false;
            }
        }
        
        if let Some(status) = &self.status {
            if order.status != *status {
                return false;
            }
        }
        
        true
    }
}

/// 执行记录过滤器，用于查询执行记录
pub struct ExecutionFilter {
    pub account_id: Option<String>,
    pub symbol: Option<String>,
    pub order_id: Option<String>,
}

impl ExecutionFilter {
    pub fn new() -> Self {
        Self {
            account_id: None,
            symbol: None,
            order_id: None,
        }
    }
    
    pub fn with_account_id(mut self, account_id: String) -> Self {
        self.account_id = Some(account_id);
        self
    }
    
    pub fn with_symbol(mut self, symbol: String) -> Self {
        self.symbol = Some(symbol);
        self
    }
    
    pub fn with_order_id(mut self, order_id: String) -> Self {
        self.order_id = Some(order_id);
        self
    }
    
    pub fn matches(&self, execution: &ExecutionRecord) -> bool {
        if let Some(symbol) = &self.symbol {
            if execution.symbol != *symbol {
                return false;
            }
        }
        
        if let Some(order_id) = &self.order_id {
            if execution.buy_order_id != *order_id && execution.sell_order_id != *order_id {
                return false;
            }
        }
        
        true
    }
}

/// 交易记录过滤器，用于查询交易记录
pub struct TradeFilter {
    pub symbol: Option<String>,
    pub buy_order_id: Option<String>,
    pub sell_order_id: Option<String>,
}

impl TradeFilter {
    pub fn new() -> Self {
        Self {
            symbol: None,
            buy_order_id: None,
            sell_order_id: None,
        }
    }
    
    pub fn with_symbol(mut self, symbol: String) -> Self {
        self.symbol = Some(symbol);
        self
    }
    
    pub fn with_buy_order_id(mut self, buy_order_id: String) -> Self {
        self.buy_order_id = Some(buy_order_id);
        self
    }
    
    pub fn with_sell_order_id(mut self, sell_order_id: String) -> Self {
        self.sell_order_id = Some(sell_order_id);
        self
    }
    
    pub fn matches(&self, trade: &Trade) -> bool {
        if let Some(symbol) = &self.symbol {
            if trade.symbol != *symbol {
                return false;
            }
        }
        
        if let Some(buy_order_id) = &self.buy_order_id {
            if trade.buy_order_id != *buy_order_id {
                return false;
            }
        }
        
        if let Some(sell_order_id) = &self.sell_order_id {
            if trade.sell_order_id != *sell_order_id {
                return false;
            }
        }
        
        true
    }
}

/// 运行状态机应用器，从通道接收日志条目并应用到状态机
pub async fn run_state_machine(
    mut state_machine: impl StateMachine,
    mut rx: mpsc::Receiver<LogEntry>,
) -> Result<()> {
    info!("State machine runner started");
    
    while let Some(entry) = rx.recv().await {
        debug!("Applying log entry to state machine");
        
        if let Err(e) = state_machine.apply(entry) {
            error!("Failed to apply log entry: {}", e);
        }
    }
    
    info!("State machine runner stopped");
    Ok(())
}

/// 创建并启动状态机，返回状态机Arc引用
pub fn start_state_machine() -> (mpsc::Sender<LogEntry>, Arc<RwLock<TradingStateMachine>>) {
    let (tx, rx) = mpsc::channel(100);
    
    let state_machine = Arc::new(RwLock::new(TradingStateMachine::new()));
    let state_machine_clone = state_machine.clone();
    
    tokio::spawn(async move {
        let mut lock = state_machine_clone.write().unwrap();
        if let Err(e) = run_state_machine(&mut *lock, rx).await {
            error!("State machine runner error: {}", e);
        }
    });
    
    (tx, state_machine)
} 