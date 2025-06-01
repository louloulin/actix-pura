//! # DataFlare
//!
//! DataFlare is a data integration framework for the Actix actor system that provides
//! ETL (Extract, Transform, Load) capabilities.
//!
//! This is the main package that re-exports functionality from the modular crates.

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

// Re-exports from core crates
pub use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    model::{DataType, Field, Schema},
    config::DataFlareConfig,
};

// Re-exports from runtime crate
pub use dataflare_runtime::{
    workflow::{Workflow, WorkflowBuilder, WorkflowParser, YamlWorkflowParser, WorkflowTemplateManager, TemplateParameterValues},
    executor::WorkflowExecutor,
    RuntimeMode,
};
pub use dataflare_core::message::WorkflowProgress;

// Re-exports from connector crate
pub use dataflare_connector::{
    source::SourceConnector,
    destination::{DestinationConnector, WriteMode},
    registry::{create_connector, get_connector, ConnectorRegistry, register_connector},
};

// Re-exports from processor crate
pub use dataflare_processor::{
    processor::Processor,
    mapping::MappingProcessor,
    filter::FilterProcessor,
    aggregate::AggregateProcessor,
    enrichment::EnrichmentProcessor,
    join::JoinProcessor,
    registry::{register_processor, register_default_processors},
};

// Re-exports from plugin crate
pub use dataflare_plugin::{
    DataFlarePlugin, PluginType, PluginInfo, PluginRecord, PluginResult, PluginError,
    PluginRuntime, PluginRuntimeConfig, PluginMetrics,
    SmartPluginAdapter, OwnedPluginRecord,
};

// Re-exports from state crate
pub use dataflare_state::{
    state::SourceState,
    StateManager,
    checkpoint::CheckpointState,
    storage::StateStorage,
};

// Conditional re-exports based on features
#[cfg(feature = "edge")]
pub use dataflare_edge::{
    EdgeRuntimeConfig,
    runtime::EdgeRuntime,
    cache::OfflineCache,
    sync::SyncManager,
    resource::ResourceMonitor,
};

#[cfg(feature = "cloud")]
pub use dataflare_cloud::{
    CloudRuntimeConfig,
    runtime::CloudRuntime,
    cluster::ClusterManager,
    scheduler::TaskScheduler,
    coordinator::StateCoordinator,
};

// Export modules for direct access
/// Error types for DataFlare
pub mod error {
    pub use dataflare_core::error::*;
}

/// Message types for DataFlare
pub mod message {
    pub use dataflare_core::message::*;
}

/// Data model types for DataFlare
pub mod model {
    pub use dataflare_core::model::*;
}

/// Connector system for DataFlare
pub mod connector {
    pub use dataflare_connector::*;

    /// Source connectors
    pub mod source {
        pub use dataflare_connector::source::*;
    }

    /// Destination connectors
    pub mod destination {
        pub use dataflare_connector::destination::*;
    }

    /// CSV connectors
    pub mod csv {
        pub use dataflare_connector::csv::*;
    }

    /// PostgreSQL connectors
    pub mod postgres {
        pub use dataflare_connector::postgres::*;
    }
}

/// Data processors for DataFlare
pub mod processor {
    pub use dataflare_processor::*;
}

/// Workflow system for DataFlare
pub mod workflow {
    pub use dataflare_runtime::workflow::*;
}

/// Plugin system for DataFlare
pub mod plugin {
    pub use dataflare_plugin::*;
}

/// State management for DataFlare
pub mod state {
    pub use dataflare_state::*;
}

/// Version of the DataFlare framework
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the DataFlare system
///
/// This function configures the environment needed to run DataFlare workflows,
/// including registering connectors, initializing the plugin system, and
/// setting up the logging system.
///
/// # Examples
///
/// ```no_run
/// use dataflare::init;
///
/// #[actix::main]
/// async fn main() {
///     let config = dataflare::DataFlareConfig::default();
///     init(config).expect("Failed to initialize DataFlare");
///
///     // Now you can create and execute workflows
/// }
/// ```
pub fn init(config: DataFlareConfig) -> Result<()> {
    // Initialize logging system
    if config.init_logging {
        dataflare_core::init_logging(config.log_level)?;
    }

    // Register default connectors
    dataflare_connector::register_default_connectors();

    // Register default processors
    dataflare_processor::register_default_processors();

    // Initialize plugin system
    let plugin_config = dataflare_plugin::PluginRuntimeConfig::default();
    let _plugin_runtime = dataflare_plugin::PluginRuntime::new(plugin_config);
    log::info!("Plugin runtime initialized with directory: {:?}", config.plugin_dir);

    // Initialize edge components if enabled
    #[cfg(feature = "edge")]
    {
        log::info!("Initializing Edge mode components");
        // Edge-specific initialization
    }

    // Initialize cloud components if enabled
    #[cfg(feature = "cloud")]
    {
        log::info!("Initializing Cloud mode components");
        // Cloud-specific initialization
    }

    log::info!("DataFlare v{} initialized", VERSION);
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
