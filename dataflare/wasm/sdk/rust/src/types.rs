//! Type definitions for DataFlare WASM plugins

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Re-export WIT-generated types
pub use crate::dataflare::core::types::*;

/// Extended data record with additional helper methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedDataRecord {
    pub inner: DataRecord,
}

impl ExtendedDataRecord {
    /// Create a new data record
    pub fn new(id: String, payload: Vec<u8>) -> Self {
        Self {
            inner: DataRecord {
                id,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_micros() as u64,
                payload,
                metadata: vec![],
                schema_version: None,
            },
        }
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.inner.metadata.push((key, value));
        self
    }
    
    /// Set schema version
    pub fn with_schema_version(mut self, version: String) -> Self {
        self.inner.schema_version = Some(version);
        self
    }
    
    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.inner.metadata.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    
    /// Set metadata value
    pub fn set_metadata(&mut self, key: String, value: String) {
        if let Some(pos) = self.inner.metadata.iter().position(|(k, _)| k == &key) {
            self.inner.metadata[pos] = (key, value);
        } else {
            self.inner.metadata.push((key, value));
        }
    }
    
    /// Remove metadata
    pub fn remove_metadata(&mut self, key: &str) {
        self.inner.metadata.retain(|(k, _)| k != key);
    }
    
    /// Get payload as string
    pub fn payload_as_string(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.inner.payload.clone())
    }
    
    /// Set payload from string
    pub fn set_payload_from_string(&mut self, data: String) {
        self.inner.payload = data.into_bytes();
    }
    
    /// Get payload as JSON
    pub fn payload_as_json<T>(&self) -> Result<T, serde_json::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_slice(&self.inner.payload)
    }
    
    /// Set payload from JSON
    pub fn set_payload_from_json<T>(&mut self, data: &T) -> Result<(), serde_json::Error>
    where
        T: Serialize,
    {
        self.inner.payload = serde_json::to_vec(data)?;
        Ok(())
    }
}

impl From<DataRecord> for ExtendedDataRecord {
    fn from(inner: DataRecord) -> Self {
        Self { inner }
    }
}

impl From<ExtendedDataRecord> for DataRecord {
    fn from(extended: ExtendedDataRecord) -> Self {
        extended.inner
    }
}

/// Configuration helper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigHelper {
    pub inner: Config,
}

impl ConfigHelper {
    /// Create new configuration
    pub fn new() -> Self {
        Self {
            inner: vec![],
        }
    }
    
    /// Add string configuration
    pub fn with_string(mut self, key: String, value: String) -> Self {
        self.inner.push((key, ConfigValue::StringVal(value)));
        self
    }
    
    /// Add integer configuration
    pub fn with_int(mut self, key: String, value: i64) -> Self {
        self.inner.push((key, ConfigValue::IntVal(value)));
        self
    }
    
    /// Add float configuration
    pub fn with_float(mut self, key: String, value: f64) -> Self {
        self.inner.push((key, ConfigValue::FloatVal(value)));
        self
    }
    
    /// Add boolean configuration
    pub fn with_bool(mut self, key: String, value: bool) -> Self {
        self.inner.push((key, ConfigValue::BoolVal(value)));
        self
    }
    
    /// Get string value
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.inner.iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                ConfigValue::StringVal(s) => Some(s.as_str()),
                _ => None,
            })
    }
    
    /// Get integer value
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.inner.iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                ConfigValue::IntVal(i) => Some(*i),
                _ => None,
            })
    }
    
    /// Get float value
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.inner.iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                ConfigValue::FloatVal(f) => Some(*f),
                _ => None,
            })
    }
    
    /// Get boolean value
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.inner.iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                ConfigValue::BoolVal(b) => Some(*b),
                _ => None,
            })
    }
}

impl From<Config> for ConfigHelper {
    fn from(inner: Config) -> Self {
        Self { inner }
    }
}

impl From<ConfigHelper> for Config {
    fn from(helper: ConfigHelper) -> Self {
        helper.inner
    }
}

/// Schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub version: String,
    pub fields: Vec<SchemaField>,
}

/// Schema field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    pub field_type: SchemaFieldType,
    pub required: bool,
    pub description: Option<String>,
}

/// Schema field types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaFieldType {
    String,
    Integer,
    Float,
    Boolean,
    Bytes,
    Array(Box<SchemaFieldType>),
    Object(Vec<SchemaField>),
}

/// Configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    pub properties: Vec<ConfigProperty>,
    pub required: Vec<String>,
}

/// Configuration property
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigProperty {
    pub name: String,
    pub property_type: ConfigPropertyType,
    pub description: String,
    pub default: Option<ConfigValue>,
}

/// Configuration property types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigPropertyType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Health detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDetail {
    pub component: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub timestamp: u64,
}

/// Processor state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorState {
    pub data: HashMap<String, serde_json::Value>,
    pub version: u32,
    pub timestamp: u64,
}

/// Recovery action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    Retry,
    Skip,
    Abort,
    Reset,
}

/// Error recovery strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecoveryStrategy {
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub fallback_action: RecoveryAction,
}

/// Plugin capabilities builder
#[derive(Debug, Clone)]
pub struct CapabilitiesBuilder {
    capabilities: Capabilities,
}

impl CapabilitiesBuilder {
    pub fn new() -> Self {
        Self {
            capabilities: Capabilities {
                supports_async: false,
                supports_streaming: false,
                supports_batch: false,
                supports_backpressure: false,
                max_batch_size: None,
            },
        }
    }
    
    pub fn with_async(mut self, supports: bool) -> Self {
        self.capabilities.supports_async = supports;
        self
    }
    
    pub fn with_streaming(mut self, supports: bool) -> Self {
        self.capabilities.supports_streaming = supports;
        self
    }
    
    pub fn with_batch(mut self, supports: bool) -> Self {
        self.capabilities.supports_batch = supports;
        self
    }
    
    pub fn with_backpressure(mut self, supports: bool) -> Self {
        self.capabilities.supports_backpressure = supports;
        self
    }
    
    pub fn with_max_batch_size(mut self, size: u32) -> Self {
        self.capabilities.max_batch_size = Some(size);
        self
    }
    
    pub fn build(self) -> Capabilities {
        self.capabilities
    }
}

/// Metrics builder
#[derive(Debug, Clone)]
pub struct MetricsBuilder {
    metrics: Metrics,
}

impl MetricsBuilder {
    pub fn new() -> Self {
        Self {
            metrics: Metrics {
                execution_time_ms: 0.0,
                memory_usage_bytes: 0,
                throughput_per_sec: 0.0,
                error_rate: 0.0,
            },
        }
    }
    
    pub fn with_execution_time(mut self, time_ms: f64) -> Self {
        self.metrics.execution_time_ms = time_ms;
        self
    }
    
    pub fn with_memory_usage(mut self, bytes: u64) -> Self {
        self.metrics.memory_usage_bytes = bytes;
        self
    }
    
    pub fn with_throughput(mut self, per_sec: f64) -> Self {
        self.metrics.throughput_per_sec = per_sec;
        self
    }
    
    pub fn with_error_rate(mut self, rate: f64) -> Self {
        self.metrics.error_rate = rate;
        self
    }
    
    pub fn build(self) -> Metrics {
        self.metrics
    }
}

/// Batch result builder
#[derive(Debug, Clone)]
pub struct BatchResultBuilder {
    processed: Vec<ProcessingResult>,
}

impl BatchResultBuilder {
    pub fn new() -> Self {
        Self {
            processed: vec![],
        }
    }
    
    pub fn add_success(mut self, record: DataRecord) -> Self {
        self.processed.push(ProcessingResult::Success(record));
        self
    }
    
    pub fn add_error(mut self, message: String) -> Self {
        self.processed.push(ProcessingResult::Error(message));
        self
    }
    
    pub fn add_skip(mut self) -> Self {
        self.processed.push(ProcessingResult::Skip);
        self
    }
    
    pub fn add_retry(mut self, message: String) -> Self {
        self.processed.push(ProcessingResult::Retry(message));
        self
    }
    
    pub fn build(self) -> BatchResult {
        let total_count = self.processed.len() as u32;
        let success_count = self.processed.iter()
            .filter(|r| matches!(r, ProcessingResult::Success(_)))
            .count() as u32;
        let error_count = self.processed.iter()
            .filter(|r| matches!(r, ProcessingResult::Error(_)))
            .count() as u32;
        let skip_count = self.processed.iter()
            .filter(|r| matches!(r, ProcessingResult::Skip))
            .count() as u32;
        
        BatchResult {
            processed: self.processed,
            total_count,
            success_count,
            error_count,
            skip_count,
        }
    }
}
