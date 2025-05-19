//! Mapping processor module
//!
//! This module provides functionality for mapping fields from source to destination.

use dataflare_core::{
    error::{DataFlareError, Result},
    message::DataRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

use crate::processor::Processor;

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
            .map_err(|e| DataFlareError::Configuration(format!("Invalid mapping processor configuration: {}", e)))?;
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
                    Value::Number(serde_json::Number::from_f64(n).unwrap_or_default())
                } else if let Some(s) = value.as_str() {
                    if let Ok(n) = s.parse::<f64>() {
                        Value::Number(serde_json::Number::from_f64(n).unwrap_or_default())
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

impl Processor for MappingProcessor {
    fn process(&self, record: DataRecord) -> Result<DataRecord> {
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
        Ok(DataRecord::new(Value::Object(new_data)))
    }

    fn configure(&mut self, config: Value) -> Result<()> {
        self.config = serde_json::from_value(config)
            .map_err(|e| DataFlareError::Configuration(format!("Invalid mapping processor configuration: {}", e)))?;
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
        let processor = MappingProcessor::new(config);

        let record = DataRecord::new(json!({
            "id": 1,
            "name": "John Doe",
            "email": "JOHN.DOE@EXAMPLE.COM",
            "age": 25
        }));

        let result = processor.process(record).unwrap();
        
        assert_eq!(result.data.get("user_id").unwrap().as_str().unwrap(), "1");
        assert_eq!(result.data.get("full_name").unwrap().as_str().unwrap(), "John Doe");
        assert_eq!(result.data.get("email_address").unwrap().as_str().unwrap(), "john.doe@example.com");
        assert!(result.data.get("age").is_none());
    }
}
