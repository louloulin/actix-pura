//! Actor adapters to bridge custom actor implementations with actix actors
//! 
//! This module contains adapter implementations that make our custom Actor
//! implementations work with the actix actor system. These adapters are used
//! by the API Gateway to interact with our internal actor system.

pub mod order_actor;
pub mod account_actor;
pub mod risk_actor;

pub use order_actor::ActixOrderActor;
pub use account_actor::ActixAccountActor;
pub use risk_actor::ActixRiskActor;

// Message types for the adapters
pub mod messages; 