//! # DataFlare Core
//!
//! Core components for the DataFlare data integration framework.
//! This crate provides the fundamental types, traits, and utilities used by all other DataFlare crates.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod error;
pub mod message;
pub mod model;
pub mod config;
pub mod utils;
pub mod state;
pub mod connector;
pub mod processor;
pub mod interface;

// Re-exports for convenience
pub use error::{DataFlareError, Result};
pub use message::{DataRecord, DataRecordBatch};
pub use model::{DataType, Field, Schema};
pub use config::DataFlareConfig;
pub use state::{SourceState, CheckpointState};
pub use connector::{
    SourceConnector, DestinationConnector,
    ExtractionMode, WriteMode, WriteStats
};
pub use processor::{
    Processor, ProcessorState, ProcessorType
};

// Interface re-exports
pub use interface::{
    DataProcessor, DataReader, DataWriter, DataTransformer,
    Initializable, Configurable, Monitorable, Lifecycle, WorkflowComponent
};

/// Version of the DataFlare Core module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize logging system
pub fn init_logging(log_level: log::LevelFilter) -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp_millis()
        .init();

    log::info!("DataFlare Core v{} initialized", VERSION);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
