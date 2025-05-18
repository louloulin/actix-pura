//! Conector de origen para DataFlare
//!
//! Define la interfaz y funcionalidades para conectores de fuentes de datos.

use std::sync::Arc;
use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;

use crate::{
    error::Result,
    message::DataRecord,
    model::Schema,
    state::SourceState,
};

/// Modos de extracción de datos
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionMode {
    /// Extracción completa (todos los datos)
    Full,
    /// Extracción incremental (solo datos nuevos o modificados)
    Incremental,
    /// Captura de datos de cambio (CDC)
    CDC,
    /// Modo híbrido (combinación de modos)
    Hybrid,
}

/// Interfaz para conectores de fuentes de datos
#[async_trait]
pub trait SourceConnector: Send + Sync + 'static {
    /// Configura el conector con los parámetros proporcionados
    fn configure(&mut self, config: &Value) -> Result<()>;
    
    /// Verifica la conexión con la fuente de datos
    async fn check_connection(&self) -> Result<bool>;
    
    /// Descubre el esquema de la fuente de datos
    async fn discover_schema(&self) -> Result<Schema>;
    
    /// Lee datos de la fuente
    async fn read(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>>;
    
    /// Obtiene el estado actual de la fuente
    fn get_state(&self) -> Result<SourceState>;
    
    /// Obtiene el modo de extracción soportado
    fn get_extraction_mode(&self) -> ExtractionMode;
    
    /// Estima el número de registros que se leerán
    async fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64>;
}

/// Registra los conectores de fuente predeterminados
pub fn register_default_sources() {
    // Registrar conector de memoria
    crate::connector::register_connector::<dyn SourceConnector>(
        "memory",
        Box::new(|config: Value| -> Result<Box<dyn SourceConnector>> {
            Ok(Box::new(MemorySourceConnector::new(config)))
        }),
    );
}

/// Conector de fuente de memoria (para pruebas)
pub struct MemorySourceConnector {
    /// Configuración del conector
    config: Value,
    /// Datos en memoria
    data: Vec<DataRecord>,
    /// Esquema de los datos
    schema: Schema,
    /// Estado actual
    state: SourceState,
}

impl MemorySourceConnector {
    /// Crea un nuevo conector de fuente de memoria
    pub fn new(config: Value) -> Self {
        Self {
            config,
            data: Vec::new(),
            schema: Schema::new(),
            state: SourceState::new(),
        }
    }
}

#[async_trait]
impl SourceConnector for MemorySourceConnector {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = config.clone();
        
        // Si hay datos en la configuración, cargarlos
        if let Some(data) = config.get("data").and_then(|d| d.as_array()) {
            self.data = data.iter()
                .map(|v| DataRecord::new(v.clone()))
                .collect();
        }
        
        Ok(())
    }
    
    async fn check_connection(&self) -> Result<bool> {
        // Siempre está conectado
        Ok(true)
    }
    
    async fn discover_schema(&self) -> Result<Schema> {
        // Devolver el esquema almacenado
        Ok(self.schema.clone())
    }
    
    async fn read(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>> {
        // Crear un stream a partir de los datos en memoria
        let data = self.data.clone();
        let stream = futures::stream::iter(data.into_iter().map(Ok));
        
        Ok(Box::new(stream))
    }
    
    fn get_state(&self) -> Result<SourceState> {
        Ok(self.state.clone())
    }
    
    fn get_extraction_mode(&self) -> ExtractionMode {
        // Por defecto, modo completo
        ExtractionMode::Full
    }
    
    async fn estimate_record_count(&self, _state: Option<SourceState>) -> Result<u64> {
        Ok(self.data.len() as u64)
    }
}

#[cfg(test)]
use mockall::{mock, predicate};

#[cfg(test)]
mock! {
    pub SourceConnector {}
    
    #[async_trait]
    impl SourceConnector for SourceConnector {
        fn configure(&mut self, config: &Value) -> Result<()>;
        async fn check_connection(&self) -> Result<bool>;
        async fn discover_schema(&self) -> Result<Schema>;
        async fn read(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>>;
        fn get_state(&self) -> Result<SourceState>;
        fn get_extraction_mode(&self) -> ExtractionMode;
        async fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    
    #[tokio::test]
    async fn test_memory_source_connector() {
        // Crear datos de prueba
        let data = serde_json::json!([
            {"id": 1, "name": "Test 1"},
            {"id": 2, "name": "Test 2"},
            {"id": 3, "name": "Test 3"}
        ]);
        
        // Crear configuración
        let config = serde_json::json!({
            "data": data
        });
        
        // Crear conector
        let mut connector = MemorySourceConnector::new(config.clone());
        
        // Configurar conector
        connector.configure(&config).unwrap();
        
        // Verificar conexión
        let connected = connector.check_connection().await.unwrap();
        assert!(connected);
        
        // Leer datos
        let stream = connector.read(None).await.unwrap();
        let records: Vec<_> = stream.collect().await;
        
        // Verificar que se leyeron todos los datos
        assert_eq!(records.len(), 3);
        
        // Verificar contenido de los datos
        let record = records[0].as_ref().unwrap();
        assert_eq!(record.data.get("id").unwrap().as_i64().unwrap(), 1);
        assert_eq!(record.data.get("name").unwrap().as_str().unwrap(), "Test 1");
    }
}
