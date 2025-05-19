//! # DataFlare
//!
//! DataFlare is a data integration framework for the Actix actor system that provides
//! ETL (Extract, Transform, Load) capabilities.
//!
//! This framework implements a distributed data flow system that supports:
//! - Data extraction from multiple sources
//! - Data transformation using configurable processors
//! - Data loading to various destinations
//! - Support for full, incremental, and CDC (Change Data Capture) collection modes
//! - Actor-based architecture for distributed processing
//! - Integration with WASM plugin system
//! - Edge and cloud deployment modes

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
    workflow::{Workflow, WorkflowBuilder, WorkflowParser, YamlWorkflowParser},
    executor::WorkflowExecutor,
    RuntimeMode,
};

// Re-exports from connector crate
pub use dataflare_connector::{
    source::SourceConnector,
    destination::DestinationConnector,
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
    plugin::{PluginManager, PluginConfig, PluginMetadata, PluginType, ProcessorPlugin},
    wasm::WasmProcessor,
    registry::{register_plugin, get_plugin, list_plugins},
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
    dataflare_plugin::plugin::init_plugin_system(config.plugin_dir)?;

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
