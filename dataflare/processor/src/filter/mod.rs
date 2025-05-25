//! Filter processor module
//!
//! This module provides functionality for filtering data records based on conditions.

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

// Removed unused import: use dataflare_core::processor::Processor;

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
            .map_err(|e| DataFlareError::Config(format!("Invalid filter processor configuration: {}", e)))?;
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

#[async_trait::async_trait]
impl dataflare_core::processor::Processor for FilterProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = serde_json::from_value(config.clone())
            .map_err(|e| DataFlareError::Config(format!("Invalid filter processor configuration: {}", e)))?;
        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord) -> Result<DataRecord> {
        if self.evaluate_condition(record)? {
            Ok(record.clone())
        } else {
            // Filter out the record by returning an empty record
            Ok(DataRecord::new(serde_json::json!({})))
        }
    }

    async fn process_batch(&mut self, batch: &DataRecordBatch) -> Result<DataRecordBatch> {
        let mut processed_records = Vec::new();

        for record in &batch.records {
            let result = self.process_record(record).await?;
            // Only add non-empty records
            if !result.data.is_null() && !result.data.as_object().map_or(true, |obj| obj.is_empty()) {
                processed_records.push(result);
            }
        }

        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        Ok(new_batch)
    }

    fn get_state(&self) -> dataflare_core::processor::ProcessorState {
        dataflare_core::processor::ProcessorState::new("filter")
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

impl fmt::Display for FilterProcessor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FilterProcessor(condition={})", self.config.condition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio_test::block_on;
    use dataflare_core::processor::Processor;

    #[test]
    fn test_filter_processor() {
        let config = FilterProcessorConfig {
            condition: "age >= 18".to_string(),
        };
        let mut processor = FilterProcessor::new(config);

        // Test with a record that should pass the filter
        let record = DataRecord::new(json!({
            "id": 1,
            "name": "John Doe",
            "age": 25
        }));

        let result = block_on(processor.process_record(&record));
        assert!(result.is_ok());
        let record_result = result.unwrap();
        // 检查记录是否非空（未被过滤）
        assert!(!record_result.data.as_object().unwrap().is_empty());

        // Test with a record that should be filtered out
        let record = DataRecord::new(json!({
            "id": 2,
            "name": "Jane Doe",
            "age": 16
        }));

        let result = block_on(processor.process_record(&record));
        assert!(result.is_ok());
        let record_result = result.unwrap();
        // 检查记录是否为空（被过滤）
        assert!(record_result.data.as_object().unwrap().is_empty()); // 空记录表示被过滤掉
    }
}
