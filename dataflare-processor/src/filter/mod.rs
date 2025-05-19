//! Filter processor module
//!
//! This module provides functionality for filtering data records based on conditions.

use dataflare_core::{
    error::{DataFlareError, Result},
    message::DataRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

use crate::processor::Processor;

/// Configuration for the filter processor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterProcessorConfig {
    /// Filter condition expression
    pub condition: String,
}

/// Filter processor for filtering data records based on conditions
#[derive(Debug, Clone)]
pub struct FilterProcessor {
    /// Configuration for the filter processor
    config: FilterProcessorConfig,
}

impl FilterProcessor {
    /// Create a new filter processor with the given configuration
    pub fn new(config: FilterProcessorConfig) -> Self {
        Self { config }
    }

    /// Create a new filter processor from a JSON configuration
    pub fn from_json(config: Value) -> Result<Self> {
        let config: FilterProcessorConfig = serde_json::from_value(config)
            .map_err(|e| DataFlareError::Configuration(format!("Invalid filter processor configuration: {}", e)))?;
        Ok(Self::new(config))
    }

    /// Evaluate the filter condition for a data record
    fn evaluate_condition(&self, record: &DataRecord) -> Result<bool> {
        // Simple implementation for now - just check if a field exists
        // In a real implementation, this would parse and evaluate the condition expression
        let parts: Vec<&str> = self.config.condition.split_whitespace().collect();
        if parts.len() >= 3 {
            let field_name = parts[0];
            let operator = parts[1];
            let value_str = parts[2];

            if let Some(field_value) = record.data.get(field_name) {
                match operator {
                    "==" | "=" => {
                        if let Some(field_str) = field_value.as_str() {
                            return Ok(field_str == value_str);
                        } else if let Some(field_num) = field_value.as_i64() {
                            if let Ok(value_num) = value_str.parse::<i64>() {
                                return Ok(field_num == value_num);
                            }
                        } else if let Some(field_bool) = field_value.as_bool() {
                            if value_str == "true" {
                                return Ok(field_bool);
                            } else if value_str == "false" {
                                return Ok(!field_bool);
                            }
                        }
                    }
                    "!=" | "<>" => {
                        if let Some(field_str) = field_value.as_str() {
                            return Ok(field_str != value_str);
                        } else if let Some(field_num) = field_value.as_i64() {
                            if let Ok(value_num) = value_str.parse::<i64>() {
                                return Ok(field_num != value_num);
                            }
                        } else if let Some(field_bool) = field_value.as_bool() {
                            if value_str == "true" {
                                return Ok(!field_bool);
                            } else if value_str == "false" {
                                return Ok(field_bool);
                            }
                        }
                    }
                    ">" => {
                        if let Some(field_num) = field_value.as_i64() {
                            if let Ok(value_num) = value_str.parse::<i64>() {
                                return Ok(field_num > value_num);
                            }
                        } else if let Some(field_num) = field_value.as_f64() {
                            if let Ok(value_num) = value_str.parse::<f64>() {
                                return Ok(field_num > value_num);
                            }
                        }
                    }
                    ">=" => {
                        if let Some(field_num) = field_value.as_i64() {
                            if let Ok(value_num) = value_str.parse::<i64>() {
                                return Ok(field_num >= value_num);
                            }
                        } else if let Some(field_num) = field_value.as_f64() {
                            if let Ok(value_num) = value_str.parse::<f64>() {
                                return Ok(field_num >= value_num);
                            }
                        }
                    }
                    "<" => {
                        if let Some(field_num) = field_value.as_i64() {
                            if let Ok(value_num) = value_str.parse::<i64>() {
                                return Ok(field_num < value_num);
                            }
                        } else if let Some(field_num) = field_value.as_f64() {
                            if let Ok(value_num) = value_str.parse::<f64>() {
                                return Ok(field_num < value_num);
                            }
                        }
                    }
                    "<=" => {
                        if let Some(field_num) = field_value.as_i64() {
                            if let Ok(value_num) = value_str.parse::<i64>() {
                                return Ok(field_num <= value_num);
                            }
                        } else if let Some(field_num) = field_value.as_f64() {
                            if let Ok(value_num) = value_str.parse::<f64>() {
                                return Ok(field_num <= value_num);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Default to false if condition can't be evaluated
        Ok(false)
    }
}

impl Processor for FilterProcessor {
    fn process(&self, record: DataRecord) -> Result<DataRecord> {
        if self.evaluate_condition(&record)? {
            Ok(record)
        } else {
            // Return an error to indicate that the record should be filtered out
            Err(DataFlareError::Filtered("Record filtered out".to_string()))
        }
    }

    fn configure(&mut self, config: Value) -> Result<()> {
        self.config = serde_json::from_value(config)
            .map_err(|e| DataFlareError::Configuration(format!("Invalid filter processor configuration: {}", e)))?;
        Ok(())
    }
}

impl fmt::Display for FilterProcessor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FilterProcessor(condition={})", self.config.condition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_filter_processor() {
        let config = FilterProcessorConfig {
            condition: "age >= 18".to_string(),
        };
        let processor = FilterProcessor::new(config);

        // Test with a record that should pass the filter
        let record = DataRecord::new(json!({
            "id": 1,
            "name": "John Doe",
            "age": 25
        }));
        let result = processor.process(record);
        assert!(result.is_ok());

        // Test with a record that should be filtered out
        let record = DataRecord::new(json!({
            "id": 2,
            "name": "Jane Doe",
            "age": 16
        }));
        let result = processor.process(record);
        assert!(result.is_err());
        match result {
            Err(DataFlareError::Filtered(_)) => (),
            _ => panic!("Expected Filtered error"),
        }
    }
}
