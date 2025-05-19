//! Módulo de conectores para DataFlare
//!
//! Define las interfaces y funcionalidades para conectores de fuentes y destinos.

pub mod source;
pub mod destination;
pub mod registry;
pub mod postgres;
pub mod csv;
pub mod hybrid;

pub use source::SourceConnector;
pub use destination::DestinationConnector;
pub use registry::{create_connector, get_connector, ConnectorRegistry, register_connector};

use std::sync::Once;
static INIT: Once = Once::new();

/// Registra los conectores predeterminados
pub fn register_default_connectors() {
    INIT.call_once(|| {
        // Registrar conectores de fuente predeterminados
        source::register_default_sources();

        // Registrar conectores de destino predeterminados
        destination::register_default_destinations();

        // Registrar conector PostgreSQL
        postgres::register_postgres_connector();

        // Registrar conectores CSV
        csv::register_csv_connectors();
    });
}

#[cfg(test)]
pub use self::{
    source::MockSourceConnector,
    destination::MockDestinationConnector,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_default_connectors() {
        // Registrar conectores predeterminados
        register_default_connectors();

        // Verificar que se puedan obtener conectores registrados
        let source_factory = get_connector::<dyn SourceConnector>("memory");
        assert!(source_factory.is_some());

        let destination_factory = get_connector::<dyn DestinationConnector>("memory");
        assert!(destination_factory.is_some());
    }
}
