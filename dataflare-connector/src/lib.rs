//! # DataFlare Connector
//!
//! Connector system for the DataFlare data integration framework.
//! This crate provides interfaces and implementations for data source and destination connectors.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod source;
pub mod destination;
pub mod registry;
pub mod postgres;
pub mod csv;
pub mod hybrid;

// Re-exports for convenience
pub use source::SourceConnector;
pub use destination::DestinationConnector;
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
