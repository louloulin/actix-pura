//! Módulo de procesador para DataFlare
//!
//! Define las interfaces y funcionalidades para procesadores de datos.

use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;

use crate::{
    error::Result,
    message::{DataRecord, DataRecordBatch},
};

/// Estado del procesador
#[derive(Debug, Clone)]
pub struct ProcessorState {
    /// Datos de estado
    pub data: Value,
    /// Metadatos
    pub metadata: HashMap<String, String>,
}

impl ProcessorState {
    /// Crea un nuevo estado de procesador
    pub fn new() -> Self {
        Self {
            data: Value::Null,
            metadata: HashMap::new(),
        }
    }
    
    /// Crea un estado con datos
    pub fn with_data(data: Value) -> Self {
        Self {
            data,
            metadata: HashMap::new(),
        }
    }
    
    /// Agrega un metadato
    pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl Default for ProcessorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Interfaz para procesadores de datos
#[async_trait]
pub trait Processor: Send + Sync + 'static {
    /// Configura el procesador con los parámetros proporcionados
    fn configure(&mut self, config: &Value) -> Result<()>;
    
    /// Procesa un registro individual
    async fn process_record(&mut self, record: &DataRecord, state: Option<&ProcessorState>) -> Result<Vec<DataRecord>>;
    
    /// Procesa un lote de registros
    async fn process_batch(&mut self, batch: &DataRecordBatch, state: Option<&ProcessorState>) -> Result<DataRecordBatch>;
    
    /// Obtiene el estado actual del procesador
    fn get_state(&self) -> Result<ProcessorState>;
    
    /// Inicializa el procesador
    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }
    
    /// Finaliza el procesador
    async fn finalize(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Procesador de mapeo
pub struct MappingProcessor {
    /// Configuración del procesador
    config: Option<Value>,
    /// Mapeos de campos
    mappings: Vec<FieldMapping>,
    /// Estado del procesador
    state: ProcessorState,
}

/// Mapeo de campo
#[derive(Debug, Clone)]
pub struct FieldMapping {
    /// Campo de origen
    pub source: String,
    /// Campo de destino
    pub destination: String,
    /// Transformación a aplicar
    pub transform: Option<String>,
}

impl MappingProcessor {
    /// Crea un nuevo procesador de mapeo
    pub fn new() -> Self {
        Self {
            config: None,
            mappings: Vec::new(),
            state: ProcessorState::new(),
        }
    }
    
    /// Aplica un mapeo a un registro
    fn apply_mapping(&self, record: &DataRecord, mapping: &FieldMapping) -> Result<Option<(String, Value)>> {
        // Obtener el valor del campo de origen
        let source_value = if mapping.source.contains('.') {
            // Ruta anidada
            record.get_value(&mapping.source).cloned()
        } else {
            // Campo simple
            record.data.get(&mapping.source).cloned()
        };
        
        // Si no hay valor, retornar None
        let source_value = match source_value {
            Some(value) => value,
            None => return Ok(None),
        };
        
        // Aplicar transformación si existe
        let transformed_value = if let Some(transform) = &mapping.transform {
            match transform.as_str() {
                "lowercase" => {
                    if let Some(s) = source_value.as_str() {
                        Value::String(s.to_lowercase())
                    } else {
                        source_value
                    }
                },
                "uppercase" => {
                    if let Some(s) = source_value.as_str() {
                        Value::String(s.to_uppercase())
                    } else {
                        source_value
                    }
                },
                "trim" => {
                    if let Some(s) = source_value.as_str() {
                        Value::String(s.trim().to_string())
                    } else {
                        source_value
                    }
                },
                _ => source_value,
            }
        } else {
            source_value
        };
        
        Ok(Some((mapping.destination.clone(), transformed_value)))
    }
}

#[async_trait]
impl Processor for MappingProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = Some(config.clone());
        
        // Extraer mapeos de la configuración
        if let Some(mappings) = config.get("mappings").and_then(|m| m.as_array()) {
            self.mappings.clear();
            
            for mapping in mappings {
                if let (Some(source), Some(destination)) = (
                    mapping.get("source").and_then(|s| s.as_str()),
                    mapping.get("destination").and_then(|d| d.as_str()),
                ) {
                    let transform = mapping.get("transform").and_then(|t| t.as_str()).map(String::from);
                    
                    self.mappings.push(FieldMapping {
                        source: source.to_string(),
                        destination: destination.to_string(),
                        transform,
                    });
                }
            }
        }
        
        Ok(())
    }
    
    async fn process_record(&mut self, record: &DataRecord, _state: Option<&ProcessorState>) -> Result<Vec<DataRecord>> {
        // Crear un nuevo registro con los mismos metadatos
        let mut new_record = DataRecord::new(serde_json::json!({}));
        new_record.metadata = record.metadata.clone();
        new_record.created_at = record.created_at;
        
        // Aplicar mapeos
        for mapping in &self.mappings {
            if let Some((dest_path, value)) = self.apply_mapping(record, mapping)? {
                // Establecer el valor en el nuevo registro
                new_record.set_value(&dest_path, value)?;
            }
        }
        
        Ok(vec![new_record])
    }
    
    async fn process_batch(&mut self, batch: &DataRecordBatch, state: Option<&ProcessorState>) -> Result<DataRecordBatch> {
        let mut processed_records = Vec::with_capacity(batch.records.len());
        
        // Procesar cada registro
        for record in &batch.records {
            let mut new_records = self.process_record(record, state).await?;
            processed_records.append(&mut new_records);
        }
        
        // Crear un nuevo lote con los registros procesados
        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();
        
        Ok(new_batch)
    }
    
    fn get_state(&self) -> Result<ProcessorState> {
        Ok(self.state.clone())
    }
}

/// Procesador de filtro
pub struct FilterProcessor {
    /// Configuración del procesador
    config: Option<Value>,
    /// Condición de filtro
    condition: Option<String>,
    /// Estado del procesador
    state: ProcessorState,
}

impl FilterProcessor {
    /// Crea un nuevo procesador de filtro
    pub fn new() -> Self {
        Self {
            config: None,
            condition: None,
            state: ProcessorState::new(),
        }
    }
    
    /// Evalúa una condición simple en un registro
    fn evaluate_condition(&self, record: &DataRecord, condition: &str) -> Result<bool> {
        // Implementación simple de evaluación de condiciones
        // En una implementación real, se usaría un motor de expresiones más completo
        
        // Ejemplo: "user.email != null"
        let parts: Vec<&str> = condition.split_whitespace().collect();
        if parts.len() != 3 {
            return Ok(false);
        }
        
        let field_path = parts[0];
        let operator = parts[1];
        let value = parts[2];
        
        // Obtener el valor del campo
        let field_value = record.get_value(field_path);
        
        // Evaluar la condición
        match operator {
            "==" => Ok(match field_value {
                Some(v) => match value {
                    "null" => v.is_null(),
                    _ => v.as_str().map(|s| s == value).unwrap_or(false),
                },
                None => value == "null",
            }),
            "!=" => Ok(match field_value {
                Some(v) => match value {
                    "null" => !v.is_null(),
                    _ => v.as_str().map(|s| s != value).unwrap_or(true),
                },
                None => value != "null",
            }),
            _ => Ok(false),
        }
    }
}

#[async_trait]
impl Processor for FilterProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = Some(config.clone());
        
        // Extraer condición de la configuración
        if let Some(condition) = config.get("condition").and_then(|c| c.as_str()) {
            self.condition = Some(condition.to_string());
        }
        
        Ok(())
    }
    
    async fn process_record(&mut self, record: &DataRecord, _state: Option<&ProcessorState>) -> Result<Vec<DataRecord>> {
        // Si no hay condición, pasar el registro sin cambios
        if self.condition.is_none() {
            return Ok(vec![record.clone()]);
        }
        
        // Evaluar la condición
        let condition = self.condition.as_ref().unwrap();
        let passes_filter = self.evaluate_condition(record, condition)?;
        
        // Si pasa el filtro, incluir el registro; de lo contrario, excluirlo
        if passes_filter {
            Ok(vec![record.clone()])
        } else {
            Ok(vec![])
        }
    }
    
    async fn process_batch(&mut self, batch: &DataRecordBatch, state: Option<&ProcessorState>) -> Result<DataRecordBatch> {
        let mut processed_records = Vec::new();
        
        // Procesar cada registro
        for record in &batch.records {
            let mut new_records = self.process_record(record, state).await?;
            processed_records.append(&mut new_records);
        }
        
        // Crear un nuevo lote con los registros procesados
        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();
        
        Ok(new_batch)
    }
    
    fn get_state(&self) -> Result<ProcessorState> {
        Ok(self.state.clone())
    }
}

#[cfg(test)]
use mockall::{mock, predicate};

#[cfg(test)]
mock! {
    pub Processor {}
    
    #[async_trait]
    impl Processor for Processor {
        fn configure(&mut self, config: &Value) -> Result<()>;
        async fn process_record(&mut self, record: &DataRecord, state: Option<&ProcessorState>) -> Result<Vec<DataRecord>>;
        async fn process_batch(&mut self, batch: &DataRecordBatch, state: Option<&ProcessorState>) -> Result<DataRecordBatch>;
        fn get_state(&self) -> Result<ProcessorState>;
        async fn initialize(&mut self) -> Result<()>;
        async fn finalize(&mut self) -> Result<()>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mapping_processor() {
        // Crear procesador
        let mut processor = MappingProcessor::new();
        
        // Configurar procesador
        let config = serde_json::json!({
            "mappings": [
                {
                    "source": "first_name",
                    "destination": "user.firstName",
                    "transform": "uppercase"
                },
                {
                    "source": "last_name",
                    "destination": "user.lastName",
                    "transform": "uppercase"
                },
                {
                    "source": "email",
                    "destination": "user.email",
                    "transform": "lowercase"
                }
            ]
        });
        
        processor.configure(&config).unwrap();
        
        // Crear registro de prueba
        let record = DataRecord::new(serde_json::json!({
            "first_name": "John",
            "last_name": "Doe",
            "email": "John.Doe@Example.com"
        }));
        
        // Procesar registro
        let processed = processor.process_record(&record, None).await.unwrap();
        
        // Verificar resultado
        assert_eq!(processed.len(), 1);
        let processed_record = &processed[0];
        
        assert_eq!(processed_record.get_value("user.firstName").unwrap().as_str().unwrap(), "JOHN");
        assert_eq!(processed_record.get_value("user.lastName").unwrap().as_str().unwrap(), "DOE");
        assert_eq!(processed_record.get_value("user.email").unwrap().as_str().unwrap(), "john.doe@example.com");
    }
    
    #[tokio::test]
    async fn test_filter_processor() {
        // Crear procesador
        let mut processor = FilterProcessor::new();
        
        // Configurar procesador
        let config = serde_json::json!({
            "condition": "email != null"
        });
        
        processor.configure(&config).unwrap();
        
        // Crear registros de prueba
        let record1 = DataRecord::new(serde_json::json!({
            "name": "John",
            "email": "john@example.com"
        }));
        
        let record2 = DataRecord::new(serde_json::json!({
            "name": "Jane"
            // Sin email
        }));
        
        // Procesar registros
        let processed1 = processor.process_record(&record1, None).await.unwrap();
        let processed2 = processor.process_record(&record2, None).await.unwrap();
        
        // Verificar resultados
        assert_eq!(processed1.len(), 1); // Pasa el filtro
        assert_eq!(processed2.len(), 0); // No pasa el filtro
    }
}
