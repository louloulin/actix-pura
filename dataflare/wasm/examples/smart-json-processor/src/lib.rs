//! Smart JSON Processor - DataFlare WASM Plugin Example
//! 
//! This example demonstrates a high-cohesion, low-coupling WASM plugin
//! inspired by Fluvio SmartModule and Spin Framework designs.
//! 
//! Features:
//! - JSON parsing and transformation
//! - Schema validation
//! - Field mapping and filtering
//! - Error handling and recovery
//! - State management
//! - Metrics collection

use serde::{Deserialize, Serialize};
use serde_json::{Value, Map};
use std::collections::HashMap;

// Import WIT-generated bindings
wit_bindgen::generate!({
    world: "dataflare-smart-module",
    path: "../../wit",
});

use exports::dataflare::core::smart_module::Guest as SmartModuleGuest;
use dataflare::core::types::*;

/// Plugin configuration - high cohesion: all config in one place
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonProcessorConfig {
    /// Field mappings: source -> destination
    pub field_mappings: HashMap<String, String>,
    /// Fields to filter out
    pub filter_fields: Vec<String>,
    /// Required fields for validation
    pub required_fields: Vec<String>,
    /// Default values for missing fields
    pub default_values: HashMap<String, Value>,
    /// Enable schema validation
    pub validate_schema: bool,
    /// Maximum record size in bytes
    pub max_record_size: usize,
}

impl Default for JsonProcessorConfig {
    fn default() -> Self {
        Self {
            field_mappings: HashMap::new(),
            filter_fields: Vec::new(),
            required_fields: Vec::new(),
            default_values: HashMap::new(),
            validate_schema: true,
            max_record_size: 1024 * 1024, // 1MB
        }
    }
}

/// Processing statistics - encapsulated state
#[derive(Debug, Clone, Default)]
pub struct ProcessingStats {
    pub records_processed: u64,
    pub records_success: u64,
    pub records_error: u64,
    pub records_filtered: u64,
    pub total_processing_time_ms: u64,
    pub schema_validation_errors: u64,
    pub size_limit_errors: u64,
}

/// Smart JSON Processor - high cohesion implementation
pub struct SmartJsonProcessor {
    config: JsonProcessorConfig,
    stats: ProcessingStats,
    initialized: bool,
}

impl SmartJsonProcessor {
    pub fn new() -> Self {
        Self {
            config: JsonProcessorConfig::default(),
            stats: ProcessingStats::default(),
            initialized: false,
        }
    }

    /// Initialize with configuration - single responsibility
    pub fn initialize(&mut self, config_pairs: Vec<(String, String)>) -> Result<(), String> {
        // Parse configuration from key-value pairs
        let mut config = JsonProcessorConfig::default();
        
        for (key, value) in config_pairs {
            match key.as_str() {
                "field_mappings" => {
                    config.field_mappings = serde_json::from_str(&value)
                        .map_err(|e| format!("Invalid field_mappings: {}", e))?;
                }
                "filter_fields" => {
                    config.filter_fields = serde_json::from_str(&value)
                        .map_err(|e| format!("Invalid filter_fields: {}", e))?;
                }
                "required_fields" => {
                    config.required_fields = serde_json::from_str(&value)
                        .map_err(|e| format!("Invalid required_fields: {}", e))?;
                }
                "default_values" => {
                    config.default_values = serde_json::from_str(&value)
                        .map_err(|e| format!("Invalid default_values: {}", e))?;
                }
                "validate_schema" => {
                    config.validate_schema = value.parse()
                        .map_err(|e| format!("Invalid validate_schema: {}", e))?;
                }
                "max_record_size" => {
                    config.max_record_size = value.parse()
                        .map_err(|e| format!("Invalid max_record_size: {}", e))?;
                }
                _ => {
                    return Err(format!("Unknown configuration key: {}", key));
                }
            }
        }
        
        self.config = config;
        self.initialized = true;
        Ok(())
    }

    /// Process single record - core business logic encapsulated
    pub fn process_record(&mut self, record: DataRecord) -> Result<ProcessingResult, String> {
        if !self.initialized {
            return Err("Processor not initialized".to_string());
        }

        let start_time = std::time::Instant::now();
        self.stats.records_processed += 1;

        // Size validation
        if record.payload.len() > self.config.max_record_size {
            self.stats.size_limit_errors += 1;
            self.stats.records_error += 1;
            return Ok(ProcessingResult::Error(
                format!("Record size {} exceeds limit {}", 
                       record.payload.len(), self.config.max_record_size)
            ));
        }

        // Parse JSON payload
        let json_value: Value = match serde_json::from_slice(&record.payload) {
            Ok(value) => value,
            Err(e) => {
                self.stats.records_error += 1;
                return Ok(ProcessingResult::Error(format!("JSON parse error: {}", e)));
            }
        };

        // Validate and transform
        match self.transform_json(json_value, &record) {
            Ok(Some(transformed_record)) => {
                self.stats.records_success += 1;
                Ok(ProcessingResult::Success(transformed_record))
            }
            Ok(None) => {
                self.stats.records_filtered += 1;
                Ok(ProcessingResult::Filtered)
            }
            Err(e) => {
                self.stats.records_error += 1;
                Ok(ProcessingResult::Error(e))
            }
        }
    }

    /// Transform JSON data - separated concern
    fn transform_json(&mut self, mut json: Value, original_record: &DataRecord) -> Result<Option<DataRecord>, String> {
        let obj = match json.as_object_mut() {
            Some(obj) => obj,
            None => return Err("JSON payload is not an object".to_string()),
        };

        // Schema validation
        if self.config.validate_schema {
            if let Err(e) = self.validate_schema(obj) {
                self.stats.schema_validation_errors += 1;
                return Err(e);
            }
        }

        // Apply field mappings
        self.apply_field_mappings(obj);

        // Filter fields
        self.filter_fields(obj);

        // Add default values
        self.add_default_values(obj);

        // Check if record should be filtered out
        if self.should_filter_record(obj) {
            return Ok(None);
        }

        // Create new record
        let new_payload = serde_json::to_vec(&json)
            .map_err(|e| format!("JSON serialization error: {}", e))?;

        let mut new_record = DataRecord {
            id: original_record.id.clone(),
            timestamp: original_record.timestamp,
            payload: new_payload,
            metadata: original_record.metadata.clone(),
            schema_version: original_record.schema_version.clone(),
            partition_key: original_record.partition_key.clone(),
            offset: original_record.offset,
            topic: original_record.topic.clone(),
        };

        // Add processing metadata
        new_record.metadata.push(("processed_by".to_string(), "smart-json-processor".to_string()));
        new_record.metadata.push(("processed_at".to_string(), chrono::Utc::now().to_rfc3339()));

        Ok(Some(new_record))
    }

    /// Schema validation - single responsibility
    fn validate_schema(&self, obj: &Map<String, Value>) -> Result<(), String> {
        for required_field in &self.config.required_fields {
            if !obj.contains_key(required_field) {
                return Err(format!("Missing required field: {}", required_field));
            }
        }
        Ok(())
    }

    /// Apply field mappings - single responsibility
    fn apply_field_mappings(&self, obj: &mut Map<String, Value>) {
        let mut new_fields = Vec::new();
        let mut remove_fields = Vec::new();

        for (source, destination) in &self.config.field_mappings {
            if let Some(value) = obj.get(source) {
                new_fields.push((destination.clone(), value.clone()));
                if source != destination {
                    remove_fields.push(source.clone());
                }
            }
        }

        // Add new fields
        for (key, value) in new_fields {
            obj.insert(key, value);
        }

        // Remove old fields
        for key in remove_fields {
            obj.remove(&key);
        }
    }

    /// Filter fields - single responsibility
    fn filter_fields(&self, obj: &mut Map<String, Value>) {
        for field in &self.config.filter_fields {
            obj.remove(field);
        }
    }

    /// Add default values - single responsibility
    fn add_default_values(&self, obj: &mut Map<String, Value>) {
        for (key, default_value) in &self.config.default_values {
            if !obj.contains_key(key) {
                obj.insert(key.clone(), default_value.clone());
            }
        }
    }

    /// Check if record should be filtered - single responsibility
    fn should_filter_record(&self, _obj: &Map<String, Value>) -> bool {
        // Custom filtering logic can be added here
        false
    }

    /// Get processing statistics - encapsulated state access
    pub fn get_stats(&self) -> String {
        serde_json::to_string(&self.stats).unwrap_or_else(|_| "{}".to_string())
    }
}

// Global processor instance - low coupling with external systems
static mut PROCESSOR: Option<SmartJsonProcessor> = None;

/// WIT interface implementation - minimal coupling
struct SmartJsonProcessorImpl;

impl SmartModuleGuest for SmartJsonProcessorImpl {
    fn init(config: Vec<(String, String)>) -> Result<(), String> {
        unsafe {
            let mut processor = SmartJsonProcessor::new();
            processor.initialize(config)?;
            PROCESSOR = Some(processor);
        }
        Ok(())
    }

    fn process(record: DataRecord) -> Result<ProcessingResult, String> {
        unsafe {
            match PROCESSOR.as_mut() {
                Some(processor) => processor.process_record(record),
                None => Err("Processor not initialized".to_string()),
            }
        }
    }

    fn process_batch(records: Vec<DataRecord>) -> Result<BatchResult, String> {
        let mut results = Vec::new();
        let mut success_count = 0;
        let mut error_count = 0;
        let mut skip_count = 0;
        let mut filtered_count = 0;

        for record in records {
            match Self::process(record)? {
                ProcessingResult::Success(_) => {
                    success_count += 1;
                    results.push(ProcessingResult::Success(DataRecord {
                        id: "processed".to_string(),
                        timestamp: 0,
                        payload: vec![],
                        metadata: vec![],
                        schema_version: None,
                        partition_key: None,
                        offset: None,
                        topic: None,
                    }));
                }
                ProcessingResult::Error(e) => {
                    error_count += 1;
                    results.push(ProcessingResult::Error(e));
                }
                ProcessingResult::Skip => {
                    skip_count += 1;
                    results.push(ProcessingResult::Skip);
                }
                ProcessingResult::Filtered => {
                    filtered_count += 1;
                    results.push(ProcessingResult::Filtered);
                }
                other => results.push(other),
            }
        }

        Ok(BatchResult {
            processed: results,
            total_count: success_count + error_count + skip_count + filtered_count,
            success_count,
            error_count,
            skip_count,
            filtered_count: filtered_count,
        })
    }

    fn get_state() -> String {
        unsafe {
            match PROCESSOR.as_ref() {
                Some(processor) => processor.get_stats(),
                None => "{}".to_string(),
            }
        }
    }

    fn cleanup() -> Result<(), String> {
        unsafe {
            PROCESSOR = None;
        }
        Ok(())
    }
}

// Export the implementation
export!(SmartJsonProcessorImpl);
