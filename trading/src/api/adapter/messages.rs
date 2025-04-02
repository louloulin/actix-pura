use actix::prelude::*;
use serde::{Serialize, Deserialize};
use crate::models::order::{Order, OrderStatus, OrderSide, OrderType};
use crate::models::account::Account;
use chrono::Utc;

// Account related messages

#[derive(Debug, Clone, Message)]
#[rtype(result = "AccountQueryResult")]
pub enum AccountQueryMessage {
    GetAccount { account_id: String },
    GetAllAccounts,
}

#[derive(Debug, Clone)]
pub enum AccountQueryResult {
    Account(Account),
    Accounts(Vec<Account>),
    Error(String),
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "AccountUpdateResult")]
pub struct AccountUpdateMessage {
    pub account_id: Option<String>,
    pub name: String,
    pub initial_balance: f64,
}

#[derive(Debug, Clone)]
pub enum AccountUpdateResult {
    Success(Account),
    Error(String),
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "FundTransferResult")]
pub struct FundTransferMessage {
    pub account_id: String,
    pub amount: f64,
    pub transfer_type: String,  // "deposit" or "withdraw"
    pub reference: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FundTransferResult {
    Success(Account),
    Error(String),
}

// Order related messages

#[derive(Debug, Clone, Message)]
#[rtype(result = "OrderCreateResult")]
pub struct CreateOrderMessage {
    pub order: Order,
    pub skip_risk_check: bool,
}

#[derive(Debug, Clone)]
pub enum OrderCreateResult {
    Success(Order),
    Error(String),
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "OrderCancelResult")]
pub struct CancelOrderMessage {
    pub order_id: String,
    pub requested_by: String,
}

#[derive(Debug, Clone)]
pub enum OrderCancelResult {
    Success,
    Error(String),
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "OrderQueryResult")]
pub enum OrderQueryMessage {
    GetOrder { order_id: String },
    GetOrdersByAccount { account_id: String },
    GetOrdersBySymbol { symbol: String },
    GetOrdersByStatus { status: OrderStatus },
    GetAllOrders,
}

#[derive(Debug, Clone)]
pub enum OrderQueryResult {
    Order(Order),
    Orders(Vec<Order>),
    Error(String),
}

// Risk related messages

#[derive(Debug, Clone, Message)]
#[rtype(result = "RiskCheckResult")]
pub struct RiskCheckMessage {
    pub account_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum RiskCheckResult {
    Approved,
    Rejected(String),
} 