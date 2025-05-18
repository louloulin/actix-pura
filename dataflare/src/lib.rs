//! # DataFlare
//!
//! DataFlare es un módulo de integración de datos para el framework Actix que proporciona
//! capacidades ETL (Extract, Transform, Load) basadas en actores.
//!
//! Este módulo implementa un sistema de flujo de datos distribuido que soporta:
//! - Extracción de datos de múltiples fuentes
//! - Transformación de datos mediante procesadores configurables
//! - Carga de datos en diversos destinos
//! - Soporte para modos de recolección completos, incrementales y CDC (Change Data Capture)
//! - Arquitectura basada en actores para procesamiento distribuido
//! - Integración con el sistema de plugins WASM

#![warn(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod actor;
pub mod config;
pub mod connector;
pub mod error;
pub mod message;
pub mod model;
pub mod plugin;
pub mod processor;
pub mod registry;
pub mod schema;
pub mod state;
pub mod utils;
pub mod workflow;

// Re-exportaciones para facilitar el uso
pub use crate::{
    actor::{DestinationActor, ProcessorActor, SourceActor, WorkflowActor},
    config::DataFlareConfig,
    connector::{DestinationConnector, SourceConnector},
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    model::{DataType, Field, Schema},
    plugin::PluginManager,
    processor::{Processor, AggregateProcessor, FilterProcessor, MappingProcessor},
    registry::ConnectorRegistry,
    state::{CheckpointState, SourceState},
    workflow::{Workflow, WorkflowBuilder, WorkflowExecutor},
};

/// Versión del módulo DataFlare
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Inicializa el sistema DataFlare
///
/// Esta función configura el entorno necesario para ejecutar flujos de trabajo DataFlare,
/// incluyendo el registro de conectores, la inicialización del sistema de plugins y
/// la configuración del sistema de logging.
///
/// # Ejemplos
///
/// ```no_run
/// use dataflare::init;
///
/// #[actix::main]
/// async fn main() {
///     let config = dataflare::DataFlareConfig::default();
///     init(config).expect("Failed to initialize DataFlare");
///
///     // Ahora puedes crear y ejecutar flujos de trabajo
/// }
/// ```
pub fn init(config: DataFlareConfig) -> Result<()> {
    // Inicializar el sistema de logging
    if config.init_logging {
        init_logging(config.log_level)?;
    }

    // Registrar conectores predeterminados
    connector::register_default_connectors();

    // Inicializar el sistema de plugins
    plugin::init_plugin_system(config.plugin_dir)?;

    Ok(())
}

/// Inicializa el sistema de logging
fn init_logging(log_level: log::LevelFilter) -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp_millis()
        .init();

    log::info!("DataFlare v{} initialized", VERSION);
    Ok(())
}

/// Módulo de pruebas
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
