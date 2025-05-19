//! # DataFlare Runtime
//!
//! Runtime engine for the DataFlare data integration framework.
//! This crate provides the workflow execution engine and actor system integration.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod actor;
pub mod workflow;
pub mod executor;

/// Runtime mode for DataFlare
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeModeEnum {
    /// Standalone mode (single node)
    Standalone,
    /// Edge mode (runs on edge devices)
    Edge,
    /// Cloud mode (runs in a distributed environment)
    Cloud,
}

/// Re-export RuntimeMode type
pub type RuntimeMode = RuntimeModeEnum;

// Re-exports for convenience
pub use workflow::{Workflow, WorkflowBuilder, WorkflowParser, YamlWorkflowParser};
pub use executor::WorkflowExecutor;
// RuntimeMode is already exported as a type alias

/// Version of the DataFlare Runtime module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

impl Default for RuntimeMode {
    fn default() -> Self {
        Self::Standalone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
