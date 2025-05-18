//! Módulo de conectores para DataFlare
//!
//! Define las interfaces y funcionalidades para conectores de fuentes y destinos.

mod source;
mod destination;
mod registry;

pub use source::SourceConnector;
pub use destination::DestinationConnector;
pub use registry::{ConnectorRegistry, register_connector, get_connector};

use crate::error::Result;
use std::sync::Once;

static INIT: Once = Once::new();

/// Registra los conectores predeterminados
pub fn register_default_connectors() {
    INIT.call_once(|| {
        // Registrar conectores de fuente predeterminados
        source::register_default_sources();
        
        // Registrar conectores de destino predeterminados
        destination::register_default_destinations();
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
