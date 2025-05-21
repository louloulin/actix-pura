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
/// # Safety
/// This function uses unsafe code to downcast Box<dyn SourceConnector> to Box<T>,
/// which is necessary because Rust doesn't allow direct downcasting of trait objects.
/// This conversion is safe as long as the factory actually returns instances of type T.
pub fn register_adapted_source_connector<T: SourceConnector + 'static>(
    name: &str,
    factory: Arc<dyn Fn(serde_json::Value) -> Result<Box<dyn SourceConnector>> + Send + Sync>
) {
    let adapted_factory = move |config: serde_json::Value| -> Result<Box<dyn BatchSourceConnector>> {
        let source = factory(config.clone())?;
        
        // We need to downcast Box<dyn SourceConnector> to Box<T>
        // This is safe because we know the factory returns instances of type T
        let concrete_source = unsafe {
            let ptr = Box::into_raw(source);
            Box::from_raw(ptr as *mut T)
        };
        
        Ok(Box::new(source::BatchSourceAdapter::new(*concrete_source)))
    };
    
    crate::registry::register_connector::<dyn BatchSourceConnector>(
        name,
        Arc::new(adapted_factory),
    );
}

/// Register a legacy destination connector as a batch connector
/// 
/// # Safety
/// This function uses unsafe code to downcast Box<dyn DestinationConnector> to Box<T>,
/// which is necessary because Rust doesn't allow direct downcasting of trait objects.
/// This conversion is safe as long as the factory actually returns instances of type T.
pub fn register_adapted_destination_connector<T: DestinationConnector + 'static>(
    name: &str,
    factory: Arc<dyn Fn(serde_json::Value) -> Result<Box<dyn DestinationConnector>> + Send + Sync>
) {
    let adapted_factory = move |config: serde_json::Value| -> Result<Box<dyn BatchDestinationConnector>> {
        let dest = factory(config.clone())?;
        
        // We need to downcast Box<dyn DestinationConnector> to Box<T>
        // This is safe because we know the factory returns instances of type T
        let concrete_dest = unsafe {
            let ptr = Box::into_raw(dest);
            Box::from_raw(ptr as *mut T)
        };
        
        Ok(Box::new(destination::BatchDestinationAdapter::new(*concrete_dest)))
    };
    
    crate::registry::register_connector::<dyn BatchDestinationConnector>(
        name,
        Arc::new(adapted_factory),
    );
} 