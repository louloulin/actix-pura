//! # DataFlare State
//!
//! State management system for the DataFlare data integration framework.
//! This crate provides functionality for tracking and recovering workflow state.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod state;
pub mod checkpoint;
pub mod storage;
pub mod manager;

// Re-exports for convenience
pub use state::SourceState;
pub use checkpoint::CheckpointState;
pub use storage::{StateStorage, StateStorageExt};
pub use manager::StateManager;

/// Version of the DataFlare State module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
