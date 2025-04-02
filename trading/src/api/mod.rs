pub mod gateway;
pub mod adapter;

pub use gateway::ApiGatewayActor;

// 导出适配器组件
pub use adapter::order_actor::ActixOrderActor;
pub use adapter::account_actor::ActixAccountActor;
pub use adapter::risk_actor::ActixRiskActor;
pub use adapter::messages::{
    AccountQueryMessage, AccountUpdateMessage, FundTransferMessage,
    CreateOrderMessage, CancelOrderMessage, OrderQueryMessage,
    AccountQueryResult, OrderQueryResult, AccountUpdateResult, FundTransferResult,
    OrderCreateResult, OrderCancelResult, RiskCheckMessage, RiskCheckResult
}; 