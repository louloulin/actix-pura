//! SmartPluginAdapter - Bridge between plugins and DataFlare Processor system
//!
//! This adapter allows DataFlarePlugin implementations to be used as standard
//! DataFlare processors, maintaining full compatibility with the existing
//! Actor system and workflow engine.

use std::collections::HashMap;
use async_trait::async_trait;
use tracing::{debug, error, info, warn};

use dataflare_core::{
    error::{DataFlareError, Result as DataFlareResult},
    message::{DataRecord, DataRecordBatch},
    processor::{Processor, ProcessorState},
    model::Schema,
};

// Import from new interface module
use crate::interface::{DataFlarePlugin, PluginRecord, PluginResult, PluginError};
// Legacy compatibility
use crate::core::{Result};

/// 拥有的插件记录（用于从DataRecord转换）
#[derive(Debug)]
pub struct OwnedPluginRecord {
    pub value: Vec<u8>,
    pub metadata: HashMap<String, String>,
    pub timestamp: i64,
    pub partition: u32,
    pub offset: u64,
}

impl OwnedPluginRecord {
    /// 从DataRecord创建OwnedPluginRecord
    pub fn from_data_record(record: &DataRecord) -> Self {
        let value = serde_json::to_vec(&record.data).unwrap_or_default();
        let metadata = record.metadata.clone();

        Self {
            value,
            metadata,
            timestamp: chrono::Utc::now().timestamp(),
            partition: 0,
            offset: 0,
        }
    }

    /// 获取PluginRecord引用
    pub fn as_plugin_record(&self) -> PluginRecord {
        PluginRecord::new(
            &self.value,
            &self.metadata,
            self.timestamp,
            self.partition,
            self.offset,
        )
    }
}

/// Adapter that bridges DataFlarePlugin to DataFlare's Processor interface
pub struct SmartPluginAdapter {
    /// The wrapped plugin instance
    plugin: Box<dyn DataFlarePlugin>,

    /// Processor state for DataFlare integration
    state: ProcessorState,

    /// Plugin configuration
    config: HashMap<String, serde_json::Value>,

    /// Performance metrics
    metrics: PluginMetrics,

    /// Plugin name for logging
    name: String,
}

/// Performance metrics for plugin execution
#[derive(Debug, Default)]
pub struct PluginMetrics {
    /// Total records processed
    pub records_processed: u64,

    /// Total processing time
    pub total_processing_time: std::time::Duration,

    /// Number of errors encountered
    pub error_count: u64,

    /// Number of filtered records
    pub filtered_count: u64,

    /// Average processing time per record
    pub avg_processing_time: std::time::Duration,

    /// Last processing time
    pub last_processing_time: std::time::Duration,
}

impl PluginMetrics {
    /// Update metrics with a new processing time
    pub fn update(&mut self, processing_time: std::time::Duration, filtered: bool) {
        self.records_processed += 1;
        self.total_processing_time += processing_time;
        self.last_processing_time = processing_time;

        if filtered {
            self.filtered_count += 1;
        }

        // Calculate average
        if self.records_processed > 0 {
            self.avg_processing_time = self.total_processing_time / self.records_processed as u32;
        }
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    /// Get processing rate (records per second)
    pub fn processing_rate(&self) -> f64 {
        if self.total_processing_time.is_zero() {
            0.0
        } else {
            self.records_processed as f64 / self.total_processing_time.as_secs_f64()
        }
    }
}

impl SmartPluginAdapter {
    /// Create a new adapter with the given plugin
    pub fn new(plugin: Box<dyn DataFlarePlugin>) -> Self {
        let name = plugin.name().to_string();
        Self {
            plugin,
            state: ProcessorState::new("smart_plugin"),
            config: HashMap::new(),
            metrics: PluginMetrics::default(),
            name,
        }
    }

    /// Create an adapter with configuration
    pub fn with_config(
        mut plugin: Box<dyn DataFlarePlugin>,
        config: HashMap<String, serde_json::Value>,
    ) -> DataFlareResult<Self> {
        // Convert config to string map for plugin initialization
        let string_config: HashMap<String, String> = config
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect();

        // Initialize plugin with configuration
        plugin.initialize(&config)
            .map_err(|e| DataFlareError::Processing(format!("插件初始化失败: {}", e)))?;

        let name = plugin.name().to_string();
        Ok(Self {
            plugin,
            state: ProcessorState::new("smart_plugin"),
            config,
            metrics: PluginMetrics::default(),
            name,
        })
    }

    /// Get plugin metrics
    pub fn metrics(&self) -> &PluginMetrics {
        &self.metrics
    }

    /// Get plugin information
    pub fn plugin_info(&self) -> crate::core::PluginInfo {
        let info = self.plugin.info();
        // Convert from interface::PluginInfo to core::PluginInfo
        crate::core::PluginInfo {
            name: info.name,
            version: info.version,
            plugin_type: match info.plugin_type {
                crate::interface::PluginType::Source => crate::core::PluginType::Source,
                crate::interface::PluginType::Destination => crate::core::PluginType::Sink,
                crate::interface::PluginType::Processor => crate::core::PluginType::Processor,
                crate::interface::PluginType::Transformer => crate::core::PluginType::Transform,
                crate::interface::PluginType::Filter => crate::core::PluginType::Filter,
                crate::interface::PluginType::Aggregator => crate::core::PluginType::Aggregate,
                crate::interface::PluginType::Custom(_) => crate::core::PluginType::Processor, // 映射到Processor
            },
            description: Some(info.description),
            author: info.author,
            api_version: info.api_version,
        }
    }

    /// Process a single record synchronously (internal method)
    fn process_record_sync(&mut self, record: &DataRecord) -> Result<Option<DataRecord>> {
        let start_time = std::time::Instant::now();

        // Convert DataRecord to OwnedPluginRecord
        let owned_plugin_record = self.convert_to_plugin_record(record)?;
        let plugin_record = owned_plugin_record.as_plugin_record();

        // Process with the plugin
        let result = self.plugin.process(&plugin_record)
            .map_err(|e| crate::core::PluginError::Execution(format!("Interface plugin error: {}", e)))?;

        // Update metrics
        let processing_time = start_time.elapsed();
        let filtered = result.is_filtered();
        self.metrics.update(processing_time, filtered);

        // Convert result back to DataRecord
        self.convert_result_to_record(result, record)
    }

    /// Convert DataRecord to PluginRecord (zero-copy for strings only)
    fn convert_to_plugin_record(&self, record: &DataRecord) -> Result<OwnedPluginRecord> {
        // Due to the current DataRecord structure storing data as serde_json::Value,
        // we can only achieve zero-copy for string values. For other types, we create
        // an owned version to avoid lifetime issues.

        let timestamp = record.created_at.timestamp_millis();

        // Handle different JSON value types
        let value_bytes = match &record.data {
            serde_json::Value::String(s) => s.as_bytes().to_vec(),
            _ => {
                // For non-string data, we serialize to JSON
                warn!("Non-string data requires serialization, creating owned copy");
                let serialized = serde_json::to_string(&record.data)
                    .map_err(|e| crate::core::PluginError::Serialization(format!("Serialization failed: {}", e)))?;
                serialized.into_bytes()
            }
        };

        Ok(OwnedPluginRecord {
            value: value_bytes,
            metadata: record.metadata.clone(),
            timestamp,
            partition: 0,
            offset: 0,
        })
    }

    /// Convert PluginResult back to DataRecord
    fn convert_result_to_record(
        &self,
        result: PluginResult,
        original: &DataRecord,
    ) -> Result<Option<DataRecord>> {
        match result {
            PluginResult::Success { data, metadata } => {
                let mut new_record = original.clone();

                // Try to parse as JSON, fall back to string
                let json_value = if let Ok(s) = std::str::from_utf8(&data) {
                    serde_json::from_str(s).unwrap_or_else(|_| {
                        serde_json::Value::String(s.to_string())
                    })
                } else {
                    // Binary data - encode as base64
                    use base64::Engine;
                    serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(&data))
                };

                new_record.data = json_value;
                new_record.updated_at = chrono::Utc::now();

                // Update metadata if provided
                if let Some(meta) = metadata {
                    for (key, value) in meta {
                        new_record.metadata.insert(key, value);
                    }
                }

                Ok(Some(new_record))
            }
            PluginResult::Filtered => {
                debug!("Record filtered out by plugin {}", self.name);
                Ok(None)
            }
            PluginResult::Skip => {
                debug!("Record skipped by plugin {}", self.name);
                Ok(None)
            }
            PluginResult::Error { message, code: _ } => {
                error!("Plugin {} returned error: {}", self.name, message);
                Err(crate::core::PluginError::Execution(message))
            }
        }
    }
}

/// Implement DataFlare's Processor trait
#[async_trait]
impl Processor for SmartPluginAdapter {
    fn configure(&mut self, config: &serde_json::Value) -> DataFlareResult<()> {
        // Convert JSON config to HashMap
        if let serde_json::Value::Object(map) = config {
            self.config = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

            // Re-initialize plugin with new config
            self.plugin.initialize(&self.config)
                .map_err(|e| DataFlareError::Processing(format!("插件重新初始化失败: {}", e)))?;

            info!("Plugin {} configured successfully", self.name);
            Ok(())
        } else {
            Err(DataFlareError::Config(
                "Plugin configuration must be an object".to_string()
            ))
        }
    }

    async fn initialize(&mut self) -> DataFlareResult<()> {
        info!("Initializing plugin {}", self.name);

        // Plugin is already initialized in constructor or configure
        self.state = ProcessorState::new("smart_plugin");

        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord) -> DataFlareResult<DataRecord> {
        match self.process_record_sync(record) {
            Ok(Some(processed_record)) => Ok(processed_record),
            Ok(None) => {
                // Record was filtered out or consumed
                Err(DataFlareError::Processing(
                    "Record filtered out by plugin".to_string()
                ))
            }
            Err(e) => {
                self.metrics.record_error();
                error!("Plugin {} processing error: {}", self.name, e);
                Err(DataFlareError::Processing(format!("插件处理失败: {}", e)))
            }
        }
    }

    async fn process_batch(&mut self, batch: &DataRecordBatch) -> DataFlareResult<DataRecordBatch> {
        let mut processed_records = Vec::new();
        let mut errors = 0;

        for record in &batch.records {
            match self.process_record_sync(record) {
                Ok(Some(processed_record)) => {
                    processed_records.push(processed_record);
                }
                Ok(None) => {
                    // Record filtered out - continue processing
                    debug!("Record filtered out in batch processing");
                }
                Err(e) => {
                    errors += 1;
                    warn!("Error processing record in batch: {}", e);

                    // Continue processing other records
                    // TODO: Make error handling configurable
                }
            }
        }

        if errors > 0 {
            warn!("Plugin {} had {} errors in batch of {} records",
                  self.name, errors, batch.records.len());
        }

        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        Ok(new_batch)
    }

    fn get_input_schema(&self) -> Option<Schema> {
        // Plugins don't currently define schemas
        None
    }

    fn get_output_schema(&self) -> Option<Schema> {
        // Plugins don't currently define schemas
        None
    }

    fn get_state(&self) -> ProcessorState {
        self.state.clone()
    }

    async fn finalize(&mut self) -> DataFlareResult<()> {
        info!("Finalizing plugin {}", self.name);

        // Log final metrics
        info!(
            "Plugin {} metrics: {} records processed, {} errors, {:.2} records/sec",
            self.name,
            self.metrics.records_processed,
            self.metrics.error_count,
            self.metrics.processing_rate()
        );

        // Cleanup the plugin
        self.plugin.cleanup()
            .map_err(|e| DataFlareError::Processing(format!("插件清理失败: {}", e)))?;

        Ok(())
    }
}

// Add base64 dependency for binary data encoding
use base64;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::{DataFlarePlugin, PluginRecord, PluginResult, PluginType};

    // Test plugin implementation
    struct TestPlugin;

    impl DataFlarePlugin for TestPlugin {
        fn process(&self, record: &PluginRecord) -> Result<PluginResult, crate::interface::PluginError> {
            let data = std::str::from_utf8(record.value).map_err(|e| {
                crate::interface::PluginError::processing(format!("UTF-8 parsing failed: {}", e))
            })?;
            if data.contains("filter") {
                Ok(PluginResult::filtered())
            } else {
                Ok(PluginResult::success(data.to_uppercase().into_bytes()))
            }
        }

        fn name(&self) -> &str { "test_plugin" }
        fn version(&self) -> &str { "1.0.0" }
        fn plugin_type(&self) -> PluginType { PluginType::Transformer }
    }

    // Note: Async tests are commented out due to tokio dependency issues
    // These would be enabled when tokio feature is available

    /*
    #[tokio::test]
    async fn test_adapter_basic_processing() {
        let plugin = Box::new(TestPlugin);
        let mut adapter = SmartPluginAdapter::new(plugin);

        let record = DataRecord::new(serde_json::Value::String("hello world".to_string()));

        let result = adapter.process_record(&record).await.unwrap();

        if let serde_json::Value::String(s) = &result.data {
            assert_eq!(s, "HELLO WORLD");
        } else {
            panic!("Expected string result");
        }
    }

    #[tokio::test]
    async fn test_adapter_filtering() {
        let plugin = Box::new(TestPlugin);
        let mut adapter = SmartPluginAdapter::new(plugin);

        let record = DataRecord::new(serde_json::Value::String("filter this".to_string()));

        let result = adapter.process_record(&record).await;
        assert!(result.is_err()); // Record should be filtered out
    }
    */

    #[test]
    fn test_metrics() {
        let mut metrics = PluginMetrics::default();

        metrics.update(std::time::Duration::from_millis(10), false);
        metrics.update(std::time::Duration::from_millis(20), true);

        assert_eq!(metrics.records_processed, 2);
        assert_eq!(metrics.filtered_count, 1);
        assert!(metrics.processing_rate() > 0.0);
    }

    #[test]
    fn test_plugin_adapter_creation() {
        let plugin = Box::new(TestPlugin);
        let adapter = SmartPluginAdapter::new(plugin);

        let info = adapter.plugin_info();
        assert_eq!(info.name, "test_plugin");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.plugin_type, crate::core::PluginType::Transform);
    }
}
