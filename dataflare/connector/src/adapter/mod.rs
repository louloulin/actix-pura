//! Connector adapters for DataFlare
//!
//! This module provides adapter implementations that help existing connectors
//! adapt to the new batch-optimized interfaces.

use std::sync::Arc;
use dataflare_core::{
    error::Result,
    connector::{
        SourceConnector, DestinationConnector,
        BatchSourceConnector, BatchDestinationConnector
    }
};

// Import the source and destination adapters
mod source;
mod destination;

// Re-export the adapter types
pub use self::source::BatchSourceAdapter;
pub use self::destination::BatchDestinationAdapter;

/// Convert a legacy SourceConnector to a BatchSourceConnector
pub fn adapt_source<T: SourceConnector + 'static>(source: T) -> BatchSourceAdapter<T> {
    source::BatchSourceAdapter::new(source)
}

/// Convert a legacy DestinationConnector to a BatchDestinationConnector
pub fn adapt_destination<T: DestinationConnector + 'static>(dest: T) -> BatchDestinationAdapter<T> {
    destination::BatchDestinationAdapter::new(dest)
}

/// Register a legacy source connector as a batch connector
///
/// This function provides a safe way to register source connectors by using
/// a factory that directly creates the concrete type T.
pub fn register_adapted_source_connector<T: SourceConnector + 'static>(
    name: &str,
    factory: Arc<dyn Fn(serde_json::Value) -> Result<T> + Send + Sync>
) {
    let adapted_factory = move |config: serde_json::Value| -> Result<Box<dyn BatchSourceConnector>> {
        let source = factory(config.clone())?;
        Ok(Box::new(source::BatchSourceAdapter::new(source)))
    };

    crate::registry::register_connector::<dyn BatchSourceConnector>(
        name,
        Arc::new(adapted_factory),
    );
}

/// Register a legacy destination connector as a batch connector
///
/// This function provides a safe way to register destination connectors by using
/// a factory that directly creates the concrete type T.
pub fn register_adapted_destination_connector<T: DestinationConnector + 'static>(
    name: &str,
    factory: Arc<dyn Fn(serde_json::Value) -> Result<T> + Send + Sync>
) {
    let adapted_factory = move |config: serde_json::Value| -> Result<Box<dyn BatchDestinationConnector>> {
        let dest = factory(config.clone())?;
        Ok(Box::new(destination::BatchDestinationAdapter::new(dest)))
    };

    crate::registry::register_connector::<dyn BatchDestinationConnector>(
        name,
        Arc::new(adapted_factory),
    );
}