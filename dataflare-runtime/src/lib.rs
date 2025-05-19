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

// Re-exports for convenience
pub use workflow::{Workflow, WorkflowBuilder, WorkflowParser, YamlWorkflowParser};
pub use executor::WorkflowExecutor;

/// Version of the DataFlare Runtime module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runtime mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    /// Standalone mode (single node)
    Standalone,
    /// Edge mode (optimized for resource-constrained environments)
    Edge,
    /// Cloud mode (distributed processing)
    Cloud,
}

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
