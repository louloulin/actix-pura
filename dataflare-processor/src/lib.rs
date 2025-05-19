//! # DataFlare Processor
//!
//! Processor system for the DataFlare data integration framework.
//! This crate provides interfaces and implementations for data transformation processors.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod processor;
pub mod registry;
pub mod mapping;
pub mod filter;
pub mod aggregate;
pub mod enrichment;
pub mod join;

// Re-exports for convenience
pub use processor::Processor;
pub use mapping::MappingProcessor;
pub use filter::FilterProcessor;
pub use aggregate::AggregateProcessor;
pub use enrichment::EnrichmentProcessor;
pub use join::JoinProcessor;
pub use registry::{register_processor, get_processor, ProcessorRegistry, register_default_processors};

/// Version of the DataFlare Processor module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
