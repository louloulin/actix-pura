pub mod models;
pub mod actor;
pub mod execution;
pub mod risk;
pub mod cluster;
pub mod consensus;
pub mod persistence;
pub mod api;

#[cfg(test)]
mod tests;

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
    ExecutionNotificationMessage, TradeNotificationMessage,
    LogEntry
};
pub use execution::{
    OrderBook, OrderMatcher, MatchResult, 
    ExecutionEngine, ExecuteOrderMessage
};
pub use actor::OrderActor;
pub use risk::RiskManager;
pub use cluster::TradingClusterManager; 
pub use consensus::state_machine::{StateMachine, TradingStateMachine}; 
pub use persistence::{
    storage::{StorageProvider, FileSystemStorage, InMemoryStorage, StorageType},
    log::{LogProvider, FileSystemLog, InMemoryLog, LogEntry as PersistenceLogEntry},
    snapshot::{SnapshotProvider, FileSystemSnapshot, InMemorySnapshot, SystemSnapshot}
};
pub use api::gateway::ApiGatewayActor; 