//! Mapping processor module
//!
//! This module provides functionality for mapping fields from source to destination.

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

use dataflare_core::processor::Processor;

/// Field mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    /// Source field name
    pub source: String,
    /// Destination field name
    pub destination: String,
    /// Optional transformation type
    #[serde(default)]
    pub transform: Option<String>,
}

/// Configuration for the mapping processor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingProcessorConfig {
    /// List of field mappings
    pub mappings: Vec<FieldMapping>,
}

/// Mapping processor for transforming data records
#[derive(Debug, Clone)]
pub struct MappingProcessor {
    /// Configuration for the mapping processor
    config: MappingProcessorConfig,
}

impl MappingProcessor {
    /// Create a new mapping processor with the given configuration
    pub fn new(config: MappingProcessorConfig) -> Self {
        Self { config }
    }

    /// Create a new mapping processor from a JSON configuration
    pub fn from_json(config: Value) -> Result<Self> {
        let config: MappingProcessorConfig = serde_json::from_value(config)
            .map_err(|e| DataFlareError::Config(format!("Invalid mapping processor configuration: {}", e)))?;
        Ok(Self::new(config))
    }

    /// Apply a transformation to a field value
    fn apply_transform(&self, value: &Value, transform: &str) -> Value {
        match transform {
            "string" => {
                if let Some(s) = value.as_str() {
                    Value::String(s.to_string())
                } else {
                    Value::String(value.to_string())
                }
            }
            "integer" => {
                if let Some(n) = value.as_i64() {
                    Value::Number(serde_json::Number::from(n))
                } else if let Some(s) = value.as_str() {
                    if let Ok(n) = s.parse::<i64>() {
                        Value::Number(serde_json::Number::from(n))
                    } else {
                        value.clone()
                    }
                } else {
                    value.clone()
                }
            }
            "float" => {
                if let Some(n) = value.as_f64() {
                    if let Some(num) = serde_json::Number::from_f64(n) {
                        Value::Number(num)
                    } else {
                        value.clone()
                    }
                } else if let Some(s) = value.as_str() {
                    if let Ok(n) = s.parse::<f64>() {
                        if let Some(num) = serde_json::Number::from_f64(n) {
                            Value::Number(num)
                        } else {
                            value.clone()
                        }
                    } else {
                        value.clone()
                    }
                } else {
                    value.clone()
                }
            }
            "boolean" => {
                if let Some(b) = value.as_bool() {
                    Value::Bool(b)
                } else if let Some(s) = value.as_str() {
                    Value::Bool(s.to_lowercase() == "true" || s == "1")
                } else {
                    value.clone()
                }
            }
            "lowercase" => {
                if let Some(s) = value.as_str() {
                    Value::String(s.to_lowercase())
                } else {
                    value.clone()
                }
            }
            "uppercase" => {
                if let Some(s) = value.as_str() {
                    Value::String(s.to_uppercase())
                } else {
                    value.clone()
                }
            }
            _ => value.clone(),
        }
    }
}

#[async_trait::async_trait]
impl dataflare_core::processor::Processor for MappingProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = serde_json::from_value(config.clone())
            .map_err(|e| DataFlareError::Config(format!("Invalid mapping processor configuration: {}", e)))?;
        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord) -> Result<DataRecord> {
        let mut new_data = Map::new();

        // Apply mappings
        for mapping in &self.config.mappings {
            if let Some(value) = record.data.get(&mapping.source) {
                let transformed_value = if let Some(transform) = &mapping.transform {
                    self.apply_transform(value, transform)
                } else {
                    value.clone()
                };
                new_data.insert(mapping.destination.clone(), transformed_value);
            }
        }

        // Create new record with mapped data
        let mut new_record = DataRecord::new(Value::Object(new_data));
        new_record.metadata = record.metadata.clone();
        new_record.created_at = record.created_at;
        new_record.updated_at = record.updated_at;

        Ok(new_record)
    }

    async fn process_batch(&mut self, batch: &DataRecordBatch) -> Result<DataRecordBatch> {
        let mut processed_records = Vec::with_capacity(batch.records.len());

        // Process each record
        for record in &batch.records {
            let new_record = self.process_record(record).await?;
            processed_records.push(new_record);
        }

        // Create new batch with processed records
        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        Ok(new_batch)
    }

    fn get_state(&self) -> dataflare_core::processor::ProcessorState {
        dataflare_core::processor::ProcessorState::new("mapping")
    }

    fn get_input_schema(&self) -> Option<dataflare_core::model::Schema> {
        None
    }

    fn get_output_schema(&self) -> Option<dataflare_core::model::Schema> {
        None
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn finalize(&mut self) -> Result<()> {
        Ok(())
    }
}

impl fmt::Display for MappingProcessor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MappingProcessor(mappings={})", self.config.mappings.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio_test::block_on;

    #[test]
    fn test_mapping_processor() {
        let config = MappingProcessorConfig {
            mappings: vec![
                FieldMapping {
                    source: "id".to_string(),
                    destination: "user_id".to_string(),
                    transform: Some("string".to_string()),
                },
                FieldMapping {
                    source: "name".to_string(),
                    destination: "full_name".to_string(),
                    transform: None,
                },
                FieldMapping {
                    source: "email".to_string(),
                    destination: "email_address".to_string(),
                    transform: Some("lowercase".to_string()),
                },
            ],
        };
        let mut processor = MappingProcessor::new(config);

        let record = DataRecord::new(json!({
            "id": 1,
            "name": "John Doe",
            "email": "JOHN.DOE@EXAMPLE.COM",
            "age": 25
        }));

        let processed_record = block_on(processor.process_record(&record)).unwrap();
        assert_eq!(processed_record.data.get("user_id").unwrap().as_str().unwrap(), "1");
        assert_eq!(processed_record.data.get("full_name").unwrap().as_str().unwrap(), "John Doe");
        assert_eq!(processed_record.data.get("email_address").unwrap().as_str().unwrap(), "john.doe@example.com");
        assert!(processed_record.data.get("age").is_none());
    }
}
