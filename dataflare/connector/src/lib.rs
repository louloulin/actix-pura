//! # DataFlare Connector
//!
//! Connector system for the DataFlare data integration framework.
//! This crate provides interfaces and implementations for data source and destination connectors.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod registry;
pub mod factory;
pub mod source;
pub mod destination;
pub mod adapter;
pub mod hybrid;

// Re-export existing connector modules
pub mod csv;
pub mod postgres;
pub mod mongodb;

// Re-exports for convenience
pub use dataflare_core::connector::{
    SourceConnector, DestinationConnector,
    ExtractionMode, WriteMode, WriteStats
};
pub use registry::{create_connector, get_connector, ConnectorRegistry, register_connector};

use std::sync::Once;
static INIT: Once = Once::new();

/// Register default connectors
pub fn register_default_connectors() {
    INIT.call_once(|| {
        // Register default source connectors
        source::register_default_sources();

        // Register default destination connectors
        destination::register_default_destinations();

        // Register PostgreSQL connector
        postgres::register_postgres_connector();

        // Register CSV connectors
        csv::register_csv_connectors();
        
        // Register MongoDB connector
        mongodb::register_mongodb_connector();
    });
}

/// Version of the DataFlare Connector module
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
pub use self::{
    source::MockSourceConnector,
    destination::MockDestinationConnector,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}

/// Initialize all connector modules
pub fn initialize() -> dataflare_core::error::Result<()> {
    // Register default connectors
    source::register_default_sources();
    destination::register_default_destinations();
    
    // Register CSV connectors
    csv::register_csv_connectors();
    
    // Register PostgreSQL connectors
    postgres::register_postgres_connectors();
    
    // Register MongoDB connectors
    mongodb::register_mongodb_connector();
    
    // Initialize the factory with registered connectors
    factory::initialize()?;
    
    Ok(())
}

/// Initialize the connector registry
pub fn init() -> dataflare_core::error::Result<()> {
    // Register PostgreSQL connector
    postgres::register_postgres_connector();
    postgres::batch::register_postgres_batch_connector();
    postgres::cdc::register_postgres_cdc_connector();
    
    // Register CSV connector (commented out until implemented)
    // csv::register_csv_connector();
    
    // Register factory
    factory::initialize()?;
    
    Ok(())
}

// Re-export main factory functions
pub use factory::{
    initialize as initialize_factory,
    create_source_connector,
    create_destination_connector,
    get_factory,
};

// Re-export adapter utilities
pub use adapter::{
    adapt_source,
    adapt_destination,
    register_adapted_source_connector,
    register_adapted_destination_connector,
    BatchSourceAdapter,
    BatchDestinationAdapter,
};
