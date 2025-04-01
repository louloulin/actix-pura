use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use actix::{Actor, Context, Handler, Addr};
use log::{info, warn, error, debug};
use uuid::Uuid;

use crate::models::account::{Account, Position};
use crate::models::message::{
    AccountQueryMessage, AccountQueryResult, 
    AccountUpdateMessage, AccountUpdateResult,
    FundTransferMessage, FundTransferResult,
    PositionUpdateMessage, PositionUpdateResult,
    AnyMessageWrapper, ExecutionNotificationMessage
};
use crate::models::order::OrderSide;
use crate::models::execution::Execution;
use crate::consensus::raft::RaftClient;

/// 账户管理Actor
pub struct AccountActor {
    node_id: String,
    accounts: Arc<RwLock<HashMap<String, Account>>>,
    raft_client: Option<Arc<RaftClient>>,
}

impl AccountActor {
    /// 创建新的账户管理Actor
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            accounts: Arc::new(RwLock::new(HashMap::new())),
            raft_client: None,
        }
    }
    
    /// 设置Raft客户端
    pub fn set_raft_client(&mut self, client: Arc<RaftClient>) {
        self.raft_client = Some(client);
    }
    
    /// 获取账户
    fn get_account(&self, account_id: &str) -> Option<Account> {
        self.accounts.read()
            .unwrap()
            .get(account_id)
            .cloned()
    }
    
    /// 添加或更新账户
    fn store_account(&self, account: Account) {
        let mut accounts = self.accounts.write().unwrap();
        accounts.insert(account.id.clone(), account);
    }
    
    /// 创建新账户
    async fn create_account(&self, account_id: Option<String>, name: String, initial_balance: f64) -> Account {
        let account_id = account_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        
        let account = Account {
            id: account_id.clone(),
            name,
            balance: initial_balance,
            available: initial_balance,
            frozen: 0.0,
            positions: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        // 存储账户
        self.store_account(account.clone());
        
        // 记录到Raft日志
        if let Some(client) = &self.raft_client {
            match client.append_log(format!("CREATE_ACCOUNT:{}", serde_json::to_string(&account).unwrap())).await {
                Ok(_) => debug!("Account creation logged to Raft: {}", account_id),
                Err(e) => error!("Failed to log account creation to Raft: {}", e),
            }
        }
        
        account
    }
    
    /// 更新账户信息
    async fn update_account(&self, account_id: &str, name: Option<String>) -> AccountUpdateResult {
        // 获取账户
        let mut account = match self.get_account(account_id) {
            Some(acc) => acc,
            None => return AccountUpdateResult::Error("Account not found".to_string()),
        };
        
        // 更新账户信息
        if let Some(new_name) = name {
            account.name = new_name;
        }
        
        account.updated_at = chrono::Utc::now();
        
        // 存储更新后的账户
        self.store_account(account.clone());
        
        // 记录到Raft日志
        if let Some(client) = &self.raft_client {
            match client.append_log(format!("UPDATE_ACCOUNT:{}", serde_json::to_string(&account).unwrap())).await {
                Ok(_) => debug!("Account update logged to Raft: {}", account_id),
                Err(e) => error!("Failed to log account update to Raft: {}", e),
            }
        }
        
        AccountUpdateResult::Success(account)
    }
    
    /// 账户资金转账（充值、提现、冻结、解冻）
    async fn transfer_funds(
        &self, 
        account_id: &str, 
        amount: f64, 
        transfer_type: &str,
        reference: Option<String>
    ) -> FundTransferResult {
        // 获取账户
        let mut account = match self.get_account(account_id) {
            Some(acc) => acc,
            None => return FundTransferResult::Error("Account not found".to_string()),
        };
        
        // 根据转账类型更新账户
        match transfer_type {
            "DEPOSIT" => {
                // 入金
                account.balance += amount;
                account.available += amount;
            },
            "WITHDRAW" => {
                // 出金
                if account.available < amount {
                    return FundTransferResult::Error("Insufficient available funds".to_string());
                }
                account.balance -= amount;
                account.available -= amount;
            },
            "FREEZE" => {
                // 冻结资金
                if account.available < amount {
                    return FundTransferResult::Error("Insufficient available funds to freeze".to_string());
                }
                account.available -= amount;
                account.frozen += amount;
            },
            "UNFREEZE" => {
                // 解冻资金
                if account.frozen < amount {
                    return FundTransferResult::Error("Insufficient frozen funds to unfreeze".to_string());
                }
                account.frozen -= amount;
                account.available += amount;
            },
            _ => return FundTransferResult::Error("Invalid transfer type".to_string()),
        }
        
        account.updated_at = chrono::Utc::now();
        
        // 存储更新后的账户
        self.store_account(account.clone());
        
        // 记录到Raft日志
        if let Some(client) = &self.raft_client {
            let log_entry = format!(
                "TRANSFER_FUNDS:{}:{}:{}:{}",
                account_id,
                transfer_type,
                amount,
                reference.unwrap_or_default()
            );
            
            match client.append_log(log_entry).await {
                Ok(_) => debug!("Fund transfer logged to Raft: {} {}", account_id, transfer_type),
                Err(e) => error!("Failed to log fund transfer to Raft: {}", e),
            }
        }
        
        FundTransferResult::Success(account)
    }
    
    /// 更新账户持仓
    async fn update_position(
        &self,
        account_id: &str,
        symbol: &str,
        quantity_change: f64,
        price: f64,
        reference: Option<String>
    ) -> PositionUpdateResult {
        // 获取账户
        let mut account = match self.get_account(account_id) {
            Some(acc) => acc,
            None => return PositionUpdateResult::Error("Account not found".to_string()),
        };
        
        // 获取当前持仓或创建新持仓
        let position = account.positions.entry(symbol.to_string()).or_insert_with(|| Position {
            symbol: symbol.to_string(),
            quantity: 0.0,
            avg_price: 0.0,
            frozen: 0.0,
        });
        
        // 计算新的平均价格
        if quantity_change > 0.0 && position.quantity > 0.0 {
            // 增加持仓，计算新的平均价格
            let total_cost = position.quantity * position.avg_price + quantity_change * price;
            let new_quantity = position.quantity + quantity_change;
            position.avg_price = total_cost / new_quantity;
            position.quantity = new_quantity;
        } else if quantity_change > 0.0 {
            // 新建或重新建立持仓
            position.quantity = quantity_change;
            position.avg_price = price;
        } else if quantity_change < 0.0 {
            // 减少持仓
            if position.quantity + quantity_change < 0.0 {
                return PositionUpdateResult::Error("Insufficient position quantity".to_string());
            }
            position.quantity += quantity_change;
            // 平仓时不改变平均价格
        }
        
        account.updated_at = chrono::Utc::now();
        
        // 清理零持仓
        if position.quantity == 0.0 {
            account.positions.remove(symbol);
        }
        
        // 存储更新后的账户
        self.store_account(account.clone());
        
        // 记录到Raft日志
        if let Some(client) = &self.raft_client {
            let log_entry = format!(
                "UPDATE_POSITION:{}:{}:{}:{}:{}",
                account_id,
                symbol,
                quantity_change,
                price,
                reference.unwrap_or_default()
            );
            
            match client.append_log(log_entry).await {
                Ok(_) => debug!("Position update logged to Raft: {} {}", account_id, symbol),
                Err(e) => error!("Failed to log position update to Raft: {}", e),
            }
        }
        
        PositionUpdateResult::Success(account)
    }
    
    /// 处理成交通知
    async fn process_execution(&self, execution: Execution) -> Result<(), String> {
        debug!("Processing execution: {}", execution.execution_id);
        
        // 买入方处理
        if let Some(buyer_id) = &execution.buyer_account_id {
            // 计算成交金额
            let cost = execution.price * execution.quantity;
            
            // 1. 解冻资金
            let _ = self.transfer_funds(
                buyer_id,
                cost,
                "UNFREEZE",
                Some(execution.execution_id.clone())
            ).await;
            
            // 2. 扣减资金
            let _ = self.transfer_funds(
                buyer_id,
                cost,
                "WITHDRAW",
                Some(execution.execution_id.clone())
            ).await;
            
            // 3. 增加持仓
            let _ = self.update_position(
                buyer_id,
                &execution.symbol,
                execution.quantity,
                execution.price,
                Some(execution.execution_id.clone())
            ).await;
        }
        
        // 卖出方处理
        if let Some(seller_id) = &execution.seller_account_id {
            // 计算成交金额
            let income = execution.price * execution.quantity;
            
            // 1. 解冻持仓
            // 注意：这里应该有一个专门用于冻结/解冻持仓的方法，这里简化处理
            
            // 2. 减少持仓
            let _ = self.update_position(
                seller_id,
                &execution.symbol,
                -execution.quantity,
                execution.price,
                Some(execution.execution_id.clone())
            ).await;
            
            // 3. 增加资金
            let _ = self.transfer_funds(
                seller_id,
                income,
                "DEPOSIT",
                Some(execution.execution_id.clone())
            ).await;
        }
        
        Ok(())
    }
}

impl Actor for AccountActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("AccountActor started on node {}", self.node_id);
    }
}

/// 处理账户查询消息
impl Handler<AccountQueryMessage> for AccountActor {
    type Result = AccountQueryResult;
    
    fn handle(&mut self, msg: AccountQueryMessage, _: &mut Self::Context) -> Self::Result {
        match msg {
            AccountQueryMessage::GetAccount { account_id } => {
                match self.get_account(&account_id) {
                    Some(account) => AccountQueryResult::Account(account),
                    None => AccountQueryResult::Error("Account not found".to_string()),
                }
            },
            AccountQueryMessage::GetAllAccounts => {
                let accounts = self.accounts.read().unwrap().values().cloned().collect();
                AccountQueryResult::Accounts(accounts)
            },
            AccountQueryMessage::GetAccountsWithPosition { symbol } => {
                let accounts = self.accounts.read().unwrap();
                let filtered = accounts.values()
                    .filter(|acc| acc.positions.contains_key(&symbol))
                    .cloned()
                    .collect();
                AccountQueryResult::Accounts(filtered)
            }
        }
    }
}

/// 处理账户更新消息
impl Handler<AccountUpdateMessage> for AccountActor {
    type Result = actix::ResponseFuture<AccountUpdateResult>;
    
    fn handle(&mut self, msg: AccountUpdateMessage, _: &mut Self::Context) -> Self::Result {
        match msg {
            AccountUpdateMessage::CreateAccount { account_id, name, initial_balance } => {
                let account_id_clone = account_id.clone();
                let name_clone = name.clone();
                let balance = initial_balance;
                let this = self.clone();
                
                Box::pin(async move {
                    let account = this.create_account(account_id_clone, name_clone, balance).await;
                    AccountUpdateResult::Success(account)
                })
            },
            AccountUpdateMessage::UpdateAccount { account_id, name } => {
                let account_id_clone = account_id.clone();
                let name_clone = name.clone();
                let this = self.clone();
                
                Box::pin(async move {
                    this.update_account(&account_id_clone, name_clone).await
                })
            }
        }
    }
}

/// 处理资金转账消息
impl Handler<FundTransferMessage> for AccountActor {
    type Result = actix::ResponseFuture<FundTransferResult>;
    
    fn handle(&mut self, msg: FundTransferMessage, _: &mut Self::Context) -> Self::Result {
        let account_id = msg.account_id.clone();
        let amount = msg.amount;
        let transfer_type = msg.transfer_type.clone();
        let reference = msg.reference.clone();
        let this = self.clone();
        
        Box::pin(async move {
            this.transfer_funds(&account_id, amount, &transfer_type, reference).await
        })
    }
}

/// 处理持仓更新消息
impl Handler<PositionUpdateMessage> for AccountActor {
    type Result = actix::ResponseFuture<PositionUpdateResult>;
    
    fn handle(&mut self, msg: PositionUpdateMessage, _: &mut Self::Context) -> Self::Result {
        let account_id = msg.account_id.clone();
        let symbol = msg.symbol.clone();
        let quantity_change = msg.quantity_change;
        let price = msg.price;
        let reference = msg.reference.clone();
        let this = self.clone();
        
        Box::pin(async move {
            this.update_position(&account_id, &symbol, quantity_change, price, reference).await
        })
    }
}

/// 处理成交通知
impl Handler<ExecutionNotificationMessage> for AccountActor {
    type Result = actix::ResponseFuture<Result<(), String>>;
    
    fn handle(&mut self, msg: ExecutionNotificationMessage, _: &mut Self::Context) -> Self::Result {
        let execution = msg.execution.clone();
        let this = self.clone();
        
        Box::pin(async move {
            this.process_execution(execution).await
        })
    }
}

/// 处理通用消息 - 用于集群通信
impl Handler<AnyMessageWrapper> for AccountActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessageWrapper, ctx: &mut Self::Context) -> Self::Result {
        if let Some(query_msg) = msg.0.downcast::<AccountQueryMessage>() {
            let _ = self.handle(query_msg, ctx);
        } else if let Some(update_msg) = msg.0.downcast::<AccountUpdateMessage>() {
            let _ = self.handle(update_msg, ctx);
        } else if let Some(transfer_msg) = msg.0.downcast::<FundTransferMessage>() {
            let _ = self.handle(transfer_msg, ctx);
        } else if let Some(position_msg) = msg.0.downcast::<PositionUpdateMessage>() {
            let _ = self.handle(position_msg, ctx);
        } else if let Some(exec_msg) = msg.0.downcast::<ExecutionNotificationMessage>() {
            let _ = self.handle(exec_msg, ctx);
        } else {
            warn!("AccountActor received unknown message type");
        }
    }
}

impl Clone for AccountActor {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            accounts: Arc::clone(&self.accounts),
            raft_client: self.raft_client.clone(),
        }
    }
} 