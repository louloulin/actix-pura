use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use actix::{Actor, Context, Handler, Addr};
use log::{info, warn, error, debug};
use uuid::Uuid;

use crate::models::account::{Account, Position};
use crate::api::adapter::messages::{
    AccountQueryMessage, AccountQueryResult, 
    AccountUpdateMessage, AccountUpdateResult,
    FundTransferMessage, FundTransferResult
};
use crate::models::order::OrderSide;
use crate::models::execution::Execution;

/// 账户管理Actor
pub struct AccountActor {
    node_id: String,
    accounts: Arc<RwLock<HashMap<String, Account>>>,
    // 暂时注释掉raft_client，因为RaftClient结构体还未定义或者导入路径已变更
    // raft_client: Option<Arc<RaftClient>>,
}

impl AccountActor {
    /// 创建新的账户管理Actor
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            accounts: Arc::new(RwLock::new(HashMap::new())),
            // raft_client: None,
        }
    }
    
    // 暂时注释掉set_raft_client方法，因为RaftClient结构体还未定义或者导入路径已变更
    /*
    /// 设置Raft客户端
    pub fn set_raft_client(&mut self, client: Arc<RaftClient>) {
        self.raft_client = Some(client);
    }
    */
    
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
        
        // 记录到Raft日志 - 暂时注释掉
        /*
        if let Some(client) = &self.raft_client {
            match client.append_log(format!("CREATE_ACCOUNT:{}", serde_json::to_string(&account).unwrap())).await {
                Ok(_) => debug!("Account creation logged to Raft: {}", account_id),
                Err(e) => error!("Failed to log account creation to Raft: {}", e),
            }
        }
        */
        
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
        
        // 记录到Raft日志 - 暂时注释掉
        /*
        if let Some(client) = &self.raft_client {
            match client.append_log(format!("UPDATE_ACCOUNT:{}", serde_json::to_string(&account).unwrap())).await {
                Ok(_) => debug!("Account update logged to Raft: {}", account_id),
                Err(e) => error!("Failed to log account update to Raft: {}", e),
            }
        }
        */
        
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
        
        // 记录到Raft日志 - 暂时注释掉
        /*
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
        */
        
        FundTransferResult::Success(account)
    }
    
    // 暂时注释掉未在messages.rs中定义的方法和相关消息处理
    /*
    /// 更新账户持仓
    async fn update_position(
        &self,
        account_id: &str,
        symbol: &str,
        quantity_change: f64,
        price: f64,
        reference: Option<String>
    ) -> PositionUpdateResult {
        // ... 实现代码 ...
    }
    
    /// 处理执行通知
    async fn process_execution(&self, execution: Execution) -> Result<(), String> {
        // ... 实现代码 ...
    }
    */
}

impl Actor for AccountActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("AccountActor started, node_id: {}", self.node_id);
    }
}

impl Handler<AccountQueryMessage> for AccountActor {
    type Result = actix::MessageResult<AccountQueryMessage>;
    
    fn handle(&mut self, msg: AccountQueryMessage, _: &mut Self::Context) -> Self::Result {
        match msg {
            AccountQueryMessage::GetAccount { account_id } => {
                match self.get_account(&account_id) {
                    Some(account) => actix::MessageResult(AccountQueryResult::Account(account)),
                    None => actix::MessageResult(AccountQueryResult::Error(format!("Account not found: {}", account_id))),
                }
            },
            AccountQueryMessage::GetAllAccounts => {
                let accounts = self.accounts.read()
                    .unwrap()
                    .values()
                    .cloned()
                    .collect();
                actix::MessageResult(AccountQueryResult::Accounts(accounts))
            },
        }
    }
}

impl Handler<AccountUpdateMessage> for AccountActor {
    type Result = actix::ResponseFuture<AccountUpdateResult>;
    
    fn handle(&mut self, msg: AccountUpdateMessage, _: &mut Self::Context) -> Self::Result {
        let account_id = msg.account_id;
        let name = msg.name;
        let initial_balance = msg.initial_balance;
        
        let this = self.clone();
        
        Box::pin(async move {
            match account_id {
                Some(id) => this.update_account(&id, Some(name)).await,
                None => {
                    let account = this.create_account(None, name, initial_balance).await;
                    AccountUpdateResult::Success(account)
                }
            }
        })
    }
}

impl Handler<FundTransferMessage> for AccountActor {
    type Result = actix::ResponseFuture<FundTransferResult>;
    
    fn handle(&mut self, msg: FundTransferMessage, _: &mut Self::Context) -> Self::Result {
        let this = self.clone();
        
        Box::pin(async move {
            this.transfer_funds(
                &msg.account_id, 
                msg.amount, 
                &msg.transfer_type.to_uppercase(),
                msg.reference
            ).await
        })
    }
}

// 暂时注释掉未在messages.rs中定义的Handler实现
/*
impl Handler<PositionUpdateMessage> for AccountActor {
    type Result = actix::ResponseFuture<PositionUpdateResult>;
    
    fn handle(&mut self, msg: PositionUpdateMessage, _: &mut Self::Context) -> Self::Result {
        // ... 实现代码 ...
    }
}

impl Handler<ExecutionNotificationMessage> for AccountActor {
    type Result = actix::ResponseFuture<Result<(), String>>;
    
    fn handle(&mut self, msg: ExecutionNotificationMessage, _: &mut Self::Context) -> Self::Result {
        // ... 实现代码 ...
    }
}

impl Handler<AnyMessageWrapper> for AccountActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessageWrapper, ctx: &mut Self::Context) -> Self::Result {
        // ... 实现代码 ...
    }
}
*/

impl Clone for AccountActor {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            accounts: Arc::clone(&self.accounts),
            // raft_client: self.raft_client.clone(),
        }
    }
} 