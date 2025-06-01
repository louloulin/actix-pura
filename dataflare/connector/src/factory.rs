//! Connector factory for DataFlare
//! 
//! This module implements an improved connector factory system to create
//! and manage connector instances.

use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use serde_json::Value;

use dataflare_core::{
    error::{Result, DataFlareError},
    connector::{
        Connector, BatchSourceConnector, BatchDestinationConnector,
        ConnectorCapabilities
    },
};

use crate::registry;

/// Connector factory for creating connector instances
pub struct ConnectorFactory {
    /// Registry for batch source connectors
    source_registry: HashMap<String, Box<dyn Fn() -> Box<dyn BatchSourceConnector> + Send + Sync>>,
    
    /// Registry for batch destination connectors
    destination_registry: HashMap<String, Box<dyn Fn() -> Box<dyn BatchDestinationConnector> + Send + Sync>>,
    
    /// Connector capabilities cache
    capabilities_cache: HashMap<String, ConnectorCapabilities>,
}

impl Default for ConnectorFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectorFactory {
    /// Create a new connector factory
    pub fn new() -> Self {
        Self {
            source_registry: HashMap::new(),
            destination_registry: HashMap::new(),
            capabilities_cache: HashMap::new(),
        }
    }
    
    /// Register a batch source connector factory
    pub fn register_source<F>(&mut self, connector_type: &str, factory: F) 
    where 
        F: Fn() -> Box<dyn BatchSourceConnector> + Send + Sync + 'static
    {
        self.source_registry.insert(connector_type.to_string(), Box::new(factory));
    }
    
    /// Register a batch destination connector factory
    pub fn register_destination<F>(&mut self, connector_type: &str, factory: F)
    where
        F: Fn() -> Box<dyn BatchDestinationConnector> + Send + Sync + 'static
    {
        self.destination_registry.insert(connector_type.to_string(), Box::new(factory));
    }
    
    /// Create a batch source connector
    pub fn create_source(&self, connector_type: &str) -> Result<Box<dyn BatchSourceConnector>> {
        self.source_registry.get(connector_type)
            .map(|factory| factory())
            .ok_or_else(|| DataFlareError::Connection(format!("Unknown source connector type: {}", connector_type)))
    }
    
    /// Create a batch destination connector
    pub fn create_destination(&self, connector_type: &str) -> Result<Box<dyn BatchDestinationConnector>> {
        self.destination_registry.get(connector_type)
            .map(|factory| factory())
            .ok_or_else(|| DataFlareError::Connection(format!("Unknown destination connector type: {}", connector_type)))
    }
    
    /// Get connector capabilities
    pub fn get_capabilities(&mut self, connector_type: &str) -> Result<ConnectorCapabilities> {
        // Check if capabilities are cached
        if let Some(capabilities) = self.capabilities_cache.get(connector_type) {
            return Ok(capabilities.clone());
        }
        
        // Try to create a source connector to get capabilities
        if let Ok(source) = self.create_source(connector_type) {
            let capabilities = source.get_capabilities();
            self.capabilities_cache.insert(connector_type.to_string(), capabilities.clone());
            return Ok(capabilities);
        }
        
        // Try to create a destination connector to get capabilities
        if let Ok(destination) = self.create_destination(connector_type) {
            let capabilities = destination.get_capabilities();
            self.capabilities_cache.insert(connector_type.to_string(), capabilities.clone());
            return Ok(capabilities);
        }
        
        Err(DataFlareError::Connection(format!("Unknown connector type: {}", connector_type)))
    }
    
    /// Configure a source connector with parameters
    pub fn configure_source(&self, connector_type: &str, config: &Value) -> Result<Box<dyn BatchSourceConnector>> {
        let mut connector = self.create_source(connector_type)?;
        connector.configure(config)?;
        Ok(connector)
    }
    
    /// Configure a destination connector with parameters
    pub fn configure_destination(&self, connector_type: &str, config: &Value) -> Result<Box<dyn BatchDestinationConnector>> {
        let mut connector = self.create_destination(connector_type)?;
        connector.configure(config)?;
        Ok(connector)
    }
    
    /// Get all registered source connector types
    pub fn get_source_types(&self) -> Vec<String> {
        self.source_registry.keys().cloned().collect()
    }
    
    /// Get all registered destination connector types
    pub fn get_destination_types(&self) -> Vec<String> {
        self.destination_registry.keys().cloned().collect()
    }
}

// Global factory instance
lazy_static::lazy_static! {
    static ref FACTORY: Arc<RwLock<ConnectorFactory>> = Arc::new(RwLock::new(ConnectorFactory::new()));
}

/// Initialize the connector factory with all registered connectors
pub fn initialize() -> Result<()> {
    let mut factory = FACTORY.write().unwrap();
    
    // Get all batch source connectors from registry
    for connector_type in registry::get_registered_batch_source_connectors() {
        let factory_ref = connector_type.clone();
        factory.register_source(&connector_type, move || {
            // This is just a placeholder - in a real implementation we would 
            // use the registry to create the connector
            match registry::create_connector::<dyn BatchSourceConnector>(&factory_ref, Value::Null) {
                Ok(connector) => connector,
                Err(_) => panic!("Failed to create source connector: {}", factory_ref),
            }
        });
    }
    
    // Get all batch destination connectors from registry
    for connector_type in registry::get_registered_batch_destination_connectors() {
        let factory_ref = connector_type.clone();
        factory.register_destination(&connector_type, move || {
            // This is just a placeholder - in a real implementation we would 
            // use the registry to create the connector
            match registry::create_connector::<dyn BatchDestinationConnector>(&factory_ref, Value::Null) {
                Ok(connector) => connector,
                Err(_) => panic!("Failed to create destination connector: {}", factory_ref),
            }
        });
    }
    
    Ok(())
}

/// Get the global connector factory
pub fn get_factory() -> Arc<RwLock<ConnectorFactory>> {
    FACTORY.clone()
}

/// Helper function to create a configured source connector
pub fn create_source_connector(connector_type: &str, config: &Value) -> Result<Box<dyn BatchSourceConnector>> {
    let factory = FACTORY.read().unwrap();
    factory.configure_source(connector_type, config)
}

/// Helper function to create a configured destination connector
pub fn create_destination_connector(connector_type: &str, config: &Value) -> Result<Box<dyn BatchDestinationConnector>> {
    let factory = FACTORY.read().unwrap();
    factory.configure_destination(connector_type, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dataflare_core::model::Schema;
    use dataflare_core::message::DataRecordBatch;
    use dataflare_core::state::SourceState;
    use dataflare_core::connector::{ExtractionMode, WriteMode, WriteState, WriteStats, Position};
    
    struct TestSourceConnector {
        config: Value,
        capabilities: ConnectorCapabilities,
    }
    
    #[async_trait]
    impl Connector for TestSourceConnector {
        fn connector_type(&self) -> &str {
            "test-source"
        }
        
        fn configure(&mut self, config: &Value) -> Result<()> {
            self.config = config.clone();
            Ok(())
        }
        
        async fn check_connection(&self) -> Result<bool> {
            Ok(true)
        }
        
        fn get_capabilities(&self) -> ConnectorCapabilities {
            self.capabilities.clone()
        }
        
        fn get_metadata(&self) -> HashMap<String, String> {
            HashMap::new()
        }
    }
    
    #[async_trait]
    impl BatchSourceConnector for TestSourceConnector {
        async fn discover_schema(&self) -> Result<Schema> {
            Ok(Schema::new())
        }
        
        async fn read_batch(&mut self, _max_size: usize) -> Result<DataRecordBatch> {
            Ok(DataRecordBatch::new(Vec::new()))
        }
        
        fn get_state(&self) -> Result<SourceState> {
            Ok(SourceState::new("test"))
        }
        
        async fn commit(&mut self, _position: Position) -> Result<()> {
            Ok(())
        }
        
        fn get_position(&self) -> Result<Position> {
            Ok(Position::new())
        }
        
        async fn seek(&mut self, _position: Position) -> Result<()> {
            Ok(())
        }
        
        fn get_extraction_mode(&self) -> ExtractionMode {
            ExtractionMode::Full
        }
        
        async fn estimate_record_count(&self, _state: Option<SourceState>) -> Result<u64> {
            Ok(0)
        }
    }
    
    struct TestDestConnector {
        config: Value,
        capabilities: ConnectorCapabilities,
    }
    
    #[async_trait]
    impl Connector for TestDestConnector {
        fn connector_type(&self) -> &str {
            "test-dest"
        }
        
        fn configure(&mut self, config: &Value) -> Result<()> {
            self.config = config.clone();
            Ok(())
        }
        
        async fn check_connection(&self) -> Result<bool> {
            Ok(true)
        }
        
        fn get_capabilities(&self) -> ConnectorCapabilities {
            self.capabilities.clone()
        }
        
        fn get_metadata(&self) -> HashMap<String, String> {
            HashMap::new()
        }
    }
    
    #[async_trait]
    impl BatchDestinationConnector for TestDestConnector {
        async fn prepare_schema(&self, _schema: &Schema) -> Result<()> {
            Ok(())
        }
        
        async fn write_batch(&mut self, _batch: DataRecordBatch) -> Result<WriteStats> {
            Ok(WriteStats {
                records_written: 0,
                records_failed: 0,
                bytes_written: 0,
                write_time_ms: 0,
            })
        }
        
        async fn flush(&mut self) -> Result<()> {
            Ok(())
        }
        
        fn get_write_state(&self) -> Result<WriteState> {
            Ok(WriteState {
                last_position: None,
                total_records: 0,
                total_bytes: 0,
                last_write_time: chrono::Utc::now(),
                metadata: HashMap::new(),
            })
        }
        
        async fn commit(&mut self) -> Result<()> {
            Ok(())
        }
        
        async fn rollback(&mut self) -> Result<()> {
            Ok(())
        }
        
        fn get_supported_write_modes(&self) -> Vec<WriteMode> {
            vec![WriteMode::Append]
        }
    }
    
    #[test]
    fn test_connector_factory() {
        let mut factory = ConnectorFactory::new();
        
        // Register test source connector
        factory.register_source("test-source", || {
            let capabilities = ConnectorCapabilities {
                supports_batch_operations: true,
                supports_transactions: false,
                supports_schema_evolution: false,
                max_batch_size: Some(1000),
                preferred_batch_size: Some(100),
                supports_parallel_processing: false,
            };
            
            Box::new(TestSourceConnector {
                config: Value::Null,
                capabilities,
            })
        });
        
        // Register test destination connector
        factory.register_destination("test-dest", || {
            let capabilities = ConnectorCapabilities {
                supports_batch_operations: true,
                supports_transactions: true,
                supports_schema_evolution: false,
                max_batch_size: Some(500),
                preferred_batch_size: Some(100),
                supports_parallel_processing: false,
            };
            
            Box::new(TestDestConnector {
                config: Value::Null,
                capabilities,
            })
        });
        
        // Test source connector creation
        let source = factory.create_source("test-source").unwrap();
        assert_eq!(source.connector_type(), "test-source");
        
        // Test destination connector creation
        let dest = factory.create_destination("test-dest").unwrap();
        assert_eq!(dest.connector_type(), "test-dest");
        
        // Test capabilities retrieval
        let source_caps = factory.get_capabilities("test-source").unwrap();
        assert_eq!(source_caps.max_batch_size, Some(1000));
        
        let dest_caps = factory.get_capabilities("test-dest").unwrap();
        assert_eq!(dest_caps.max_batch_size, Some(500));
        
        // Test configured connector creation
        let config = serde_json::json!({"param": "value"});
        let configured_source = factory.configure_source("test-source", &config).unwrap();
        
        // Test connector type enumeration
        let source_types = factory.get_source_types();
        assert!(source_types.contains(&"test-source".to_string()));
        
        let dest_types = factory.get_destination_types();
        assert!(dest_types.contains(&"test-dest".to_string()));
    }
} 