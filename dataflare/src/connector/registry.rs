//! Registro de conectores para DataFlare
//!
//! Proporciona funcionalidades para registrar y obtener conectores.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use serde_json::Value;

use crate::error::{DataFlareError, Result};

/// Tipo de fábrica de conectores
pub type ConnectorFactory<T> = Box<dyn Fn(Value) -> Result<Box<T>> + Send + Sync>;

/// Registro de conectores
#[derive(Default)]
pub struct ConnectorRegistry {
    /// Mapa de fábricas de conectores
    factories: HashMap<(TypeId, String), Box<dyn Any + Send + Sync>>,
}

impl ConnectorRegistry {
    /// Crea un nuevo registro de conectores
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }
    
    /// Registra una fábrica de conectores
    pub fn register<T: ?Sized + Any>(&mut self, name: &str, factory: ConnectorFactory<T>) {
        let type_id = TypeId::of::<T>();
        self.factories.insert((type_id, name.to_string()), Box::new(factory));
    }
    
    /// Obtiene una fábrica de conectores
    pub fn get<T: ?Sized + Any>(&self, name: &str) -> Option<&ConnectorFactory<T>> {
        let type_id = TypeId::of::<T>();
        self.factories.get(&(type_id, name.to_string()))
            .and_then(|factory| factory.downcast_ref::<ConnectorFactory<T>>())
    }
    
    /// Crea una instancia de un conector
    pub fn create<T: ?Sized + Any>(&self, name: &str, config: Value) -> Result<Box<T>> {
        if let Some(factory) = self.get::<T>(name) {
            factory(config)
        } else {
            Err(DataFlareError::Registry(format!("Conector no encontrado: {}", name)))
        }
    }
    
    /// Obtiene los nombres de todos los conectores registrados para un tipo
    pub fn get_connector_names<T: ?Sized + Any>(&self) -> Vec<String> {
        let type_id = TypeId::of::<T>();
        self.factories.keys()
            .filter_map(|(tid, name)| if *tid == type_id { Some(name.clone()) } else { None })
            .collect()
    }
}

// Instancia global del registro de conectores
lazy_static::lazy_static! {
    static ref REGISTRY: RwLock<ConnectorRegistry> = RwLock::new(ConnectorRegistry::new());
}

/// Registra un conector en el registro global
pub fn register_connector<T: ?Sized + Any>(name: &str, factory: ConnectorFactory<T>) {
    let mut registry = REGISTRY.write().unwrap();
    registry.register::<T>(name, factory);
}

/// Obtiene una fábrica de conectores del registro global
pub fn get_connector<T: ?Sized + Any>(name: &str) -> Option<ConnectorFactory<T>> {
    let registry = REGISTRY.read().unwrap();
    registry.get::<T>(name).cloned()
}

/// Crea una instancia de un conector del registro global
pub fn create_connector<T: ?Sized + Any>(name: &str, config: Value) -> Result<Box<T>> {
    let registry = REGISTRY.read().unwrap();
    registry.create::<T>(name, config)
}

/// Obtiene los nombres de todos los conectores registrados para un tipo
pub fn get_connector_names<T: ?Sized + Any>() -> Vec<String> {
    let registry = REGISTRY.read().unwrap();
    registry.get_connector_names::<T>()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Trait de prueba
    trait TestConnector: Send + Sync {
        fn get_name(&self) -> &str;
    }
    
    // Implementación de prueba
    struct TestConnectorImpl {
        name: String,
    }
    
    impl TestConnector for TestConnectorImpl {
        fn get_name(&self) -> &str {
            &self.name
        }
    }
    
    #[test]
    fn test_connector_registry() {
        // Crear registro
        let mut registry = ConnectorRegistry::new();
        
        // Registrar conector
        registry.register::<dyn TestConnector>(
            "test",
            Box::new(|config: Value| -> Result<Box<dyn TestConnector>> {
                let name = config.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();
                
                Ok(Box::new(TestConnectorImpl { name }))
            }),
        );
        
        // Obtener fábrica
        let factory = registry.get::<dyn TestConnector>("test").unwrap();
        
        // Crear instancia
        let config = serde_json::json!({ "name": "test-instance" });
        let connector = factory(config).unwrap();
        
        // Verificar instancia
        assert_eq!(connector.get_name(), "test-instance");
    }
    
    #[test]
    fn test_global_registry() {
        // Registrar conector
        register_connector::<dyn TestConnector>(
            "global-test",
            Box::new(|config: Value| -> Result<Box<dyn TestConnector>> {
                let name = config.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();
                
                Ok(Box::new(TestConnectorImpl { name }))
            }),
        );
        
        // Obtener fábrica
        let factory = get_connector::<dyn TestConnector>("global-test").unwrap();
        
        // Crear instancia
        let config = serde_json::json!({ "name": "global-instance" });
        let connector = factory(config).unwrap();
        
        // Verificar instancia
        assert_eq!(connector.get_name(), "global-instance");
        
        // Verificar nombres de conectores
        let names = get_connector_names::<dyn TestConnector>();
        assert!(names.contains(&"global-test".to_string()));
    }
}
