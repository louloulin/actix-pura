use std::any::Any;
use actix::prelude::*;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::order::{Order, OrderRequest, OrderResult, OrderQuery, CancelOrderRequest, OrderSide};
use crate::models::execution::{Execution, Trade};
use crate::models::account::{Account, AccountQuery, AccountResult};

/// 定义Actor消息，确保实现了actix::Message
#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct AnyMessage(pub Box<dyn Any + Send>);

impl AnyMessage {
    pub fn new<T: Any + Send>(msg: T) -> Self {
        AnyMessage(Box::new(msg))
    }
    
    pub fn downcast<T: Any>(&self) -> Option<T>
    where
        T: Clone,
    {
        self.0.downcast_ref::<T>().cloned()
    }
}

/// 执行订单的消息
#[derive(Debug, Clone, Message)]
#[rtype(result = "OrderResult")]
pub struct ExecuteOrder {
    pub order: Order,
}

impl ExecuteOrder {
    pub fn new(order: Order) -> Self {
        Self { order }
    }
}

/// 订单撮合结果
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// 原始订单
    pub order: Order,
    /// 匹配生成的执行记录列表
    pub executions: Vec<Execution>,
    /// 匹配的交易记录列表
    pub trades: Vec<Trade>,
}

/// 风控检查消息
#[derive(Debug, Clone, Message)]
#[rtype(result = "RiskCheckResult")]
pub struct RiskCheck {
    /// 需要检查的订单
    pub order: OrderRequest,
}

impl RiskCheck {
    pub fn new(order: OrderRequest) -> Self {
        Self { order }
    }
}

/// 风控检查结果
#[derive(Debug, Clone)]
pub enum RiskCheckResult {
    /// 通过风控检查
    Approved,
    /// 风控拒绝，包含拒绝原因
    Rejected(String),
}

/// 系统配置更新消息
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct SystemConfig {
    pub config_id: String,
    pub max_order_value: f64,
    pub max_position_value: f64,
    pub allow_short_selling: bool,
    pub trading_hours_start: String,
    pub trading_hours_end: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
}

impl SystemConfig {
    pub fn new() -> Self {
        Self {
            config_id: Uuid::new_v4().to_string(),
            max_order_value: 1_000_000.0, // 默认最大订单价值
            max_position_value: 10_000_000.0, // 默认最大持仓价值
            allow_short_selling: true,
            trading_hours_start: "09:30:00".to_string(),
            trading_hours_end: "16:00:00".to_string(),
            updated_at: Utc::now(),
            updated_by: "system".to_string(),
        }
    }
}

/// 消息特性
pub trait Message: Send + Sync + 'static {
    /// 获取消息类型
    fn message_type(&self) -> MessageType;
}

/// 消息类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// 创建订单
    CreateOrder,
    /// 取消订单
    CancelOrder,
    /// 查询订单
    QueryOrder,
    /// 执行订单
    ExecuteOrder,
    /// 风控检查
    RiskCheck,
    /// 更新配置
    UpdateConfig,
    /// 执行通知
    ExecutionNotification,
    /// 成交通知
    TradeNotification,
    /// 集群消息
    ClusterMessage,
    /// 日志追加
    AppendLog,
    /// 其他消息
    Other,
}

/// 执行通知消息
#[derive(Debug, Clone)]
pub struct ExecutionNotificationMessage {
    /// 执行ID
    pub execution_id: String,
    /// 账户ID
    pub account_id: String,
    /// 订单ID
    pub order_id: String,
    /// 证券代码
    pub symbol: String,
    /// 价格
    pub price: f64,
    /// 数量
    pub quantity: f64,
    /// 方向
    pub side: OrderSide,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

impl Message for ExecutionNotificationMessage {
    fn message_type(&self) -> MessageType {
        MessageType::ExecutionNotification
    }
}

/// 成交通知消息
#[derive(Debug, Clone)]
pub struct TradeNotificationMessage {
    /// 成交ID
    pub trade_id: String,
    /// 证券代码
    pub symbol: String,
    /// 价格
    pub price: f64,
    /// 数量
    pub quantity: f64,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

impl Message for TradeNotificationMessage {
    fn message_type(&self) -> MessageType {
        MessageType::TradeNotification
    }
}

/// 集群节点消息
#[derive(Debug, Clone, Message, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct ClusterMessage {
    /// 消息ID
    pub message_id: String,
    /// 发送者节点ID
    pub sender_node: String,
    /// 目标节点ID，None表示广播给所有节点
    pub target_node: Option<String>,
    /// 消息类型
    pub message_type: String,
    /// 消息内容（JSON序列化后的数据）
    pub content: String,
    /// 发送时间
    pub sent_at: DateTime<Utc>,
}

impl ClusterMessage {
    pub fn new(sender_node: String, message_type: String, content: String) -> Self {
        Self {
            message_id: Uuid::new_v4().to_string(),
            sender_node,
            target_node: None,
            message_type,
            content,
            sent_at: Utc::now(),
        }
    }
    
    pub fn with_target(sender_node: String, target_node: String, message_type: String, content: String) -> Self {
        Self {
            message_id: Uuid::new_v4().to_string(),
            sender_node,
            target_node: Some(target_node),
            message_type,
            content,
            sent_at: Utc::now(),
        }
    }
}

/// Raft日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogEntry {
    /// 订单请求
    OrderRequest(OrderRequest),
    /// 订单取消请求
    CancelOrder(CancelOrderRequest),
    /// 执行记录
    Execution(Execution),
    /// 交易记录
    Trade(Trade),
    /// 账户更新
    AccountUpdate(Account),
    /// 系统配置更新
    ConfigUpdate(SystemConfig),
    /// 自定义数据
    Custom { data_type: String, data: String },
} 