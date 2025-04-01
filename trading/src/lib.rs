pub mod models;
pub mod actor;
pub mod execution;
pub mod risk;

// 重新导出常用类型
pub use models::order::{
    Order, OrderSide, OrderType, OrderStatus, 
    OrderRequest, OrderResult, OrderQuery, CancelOrderRequest
};
pub use models::account::{
    Account, Position, PositionSide, 
    AccountQuery, AccountResult
};
pub use models::execution::{
    Execution, Trade
};
pub use models::message::{
    Message, MessageType,
    ExecutionNotificationMessage, TradeNotificationMessage
};
pub use execution::{
    OrderBook, OrderMatcher, MatchResult, 
    ExecutionEngine, ExecuteOrderMessage
};
pub use actor::OrderActor;
pub use risk::RiskManager;

// 重新导出actix-cluster的Raft实现
pub use actix_cluster::raft::{RaftActor, AppendLogRequest, LogEntry}; 