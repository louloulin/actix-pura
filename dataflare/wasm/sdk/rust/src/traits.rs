//! Trait definitions for DataFlare plugin components

use crate::types::*;
use crate::error::PluginResult;

/// Data source trait for reading data
pub trait DataSource {
    /// Initialize the data source with configuration
    fn init(&mut self, config: Config) -> PluginResult<()>;
    
    /// Read the next available record
    fn read_next(&mut self) -> PluginResult<Option<DataRecord>>;
    
    /// Read a batch of records
    fn read_batch(&mut self, max_size: u32) -> PluginResult<Vec<DataRecord>>;
    
    /// Reset the data source to beginning
    fn reset(&mut self) -> PluginResult<()>;
    
    /// Check if more data is available
    fn has_more(&self) -> bool;
    
    /// Get source capabilities
    fn get_capabilities(&self) -> Capabilities;
    
    /// Get current metrics
    fn get_metrics(&self) -> Metrics;
    
    /// Cleanup and close the source
    fn close(&mut self) -> PluginResult<()>;
}

/// Data destination trait for writing data
pub trait DataDestination {
    /// Initialize the destination with configuration
    fn init(&mut self, config: Config) -> PluginResult<()>;
    
    /// Write a single record
    fn write(&mut self, record: DataRecord) -> PluginResult<()>;
    
    /// Write a batch of records
    fn write_batch(&mut self, records: Vec<DataRecord>) -> PluginResult<BatchResult>;
    
    /// Flush any buffered data
    fn flush(&mut self) -> PluginResult<()>;
    
    /// Get destination capabilities
    fn get_capabilities(&self) -> Capabilities;
    
    /// Get current metrics
    fn get_metrics(&self) -> Metrics;
    
    /// Cleanup and close the destination
    fn close(&mut self) -> PluginResult<()>;
}

/// Data processor trait for processing data
pub trait DataProcessor {
    /// Initialize the processor with configuration
    fn init(&mut self, config: Config) -> PluginResult<()>;
    
    /// Process a single record
    fn process(&mut self, input: DataRecord) -> PluginResult<ProcessingResult>;
    
    /// Process a batch of records
    fn process_batch(&mut self, inputs: Vec<DataRecord>) -> PluginResult<BatchResult>;
    
    /// Get processor capabilities
    fn get_capabilities(&self) -> Capabilities;
    
    /// Get current metrics
    fn get_metrics(&self) -> Metrics;
    
    /// Cleanup the processor
    fn cleanup(&mut self) -> PluginResult<()>;
}

/// Data transformer trait for transforming data
pub trait DataTransformer {
    /// Initialize the transformer with configuration
    fn init(&mut self, config: Config) -> PluginResult<()>;
    
    /// Transform a single record
    fn transform(&mut self, input: DataRecord) -> PluginResult<DataRecord>;
    
    /// Transform a batch of records
    fn transform_batch(&mut self, inputs: Vec<DataRecord>) -> PluginResult<Vec<DataRecord>>;
    
    /// Get transformer capabilities
    fn get_capabilities(&self) -> Capabilities;
    
    /// Get current metrics
    fn get_metrics(&self) -> Metrics;
    
    /// Cleanup the transformer
    fn cleanup(&mut self) -> PluginResult<()>;
}

/// Data filter trait for filtering data
pub trait DataFilter {
    /// Initialize the filter with configuration
    fn init(&mut self, config: Config) -> PluginResult<()>;
    
    /// Filter a single record (returns true to keep, false to discard)
    fn filter(&mut self, input: DataRecord) -> PluginResult<bool>;
    
    /// Filter a batch of records
    fn filter_batch(&mut self, inputs: Vec<DataRecord>) -> PluginResult<Vec<bool>>;
    
    /// Get filter capabilities
    fn get_capabilities(&self) -> Capabilities;
    
    /// Get current metrics
    fn get_metrics(&self) -> Metrics;
    
    /// Cleanup the filter
    fn cleanup(&mut self) -> PluginResult<()>;
}

/// Data aggregator trait for aggregating data
pub trait DataAggregator {
    /// Initialize the aggregator with configuration
    fn init(&mut self, config: Config) -> PluginResult<()>;
    
    /// Add a record to the aggregation
    fn aggregate(&mut self, input: DataRecord) -> PluginResult<()>;
    
    /// Get the current aggregation result
    fn get_result(&mut self) -> PluginResult<Option<DataRecord>>;
    
    /// Reset the aggregation state
    fn reset(&mut self) -> PluginResult<()>;
    
    /// Get aggregator capabilities
    fn get_capabilities(&self) -> Capabilities;
    
    /// Get current metrics
    fn get_metrics(&self) -> Metrics;
    
    /// Cleanup the aggregator
    fn cleanup(&mut self) -> PluginResult<()>;
}

/// Plugin lifecycle trait
pub trait PluginLifecycle {
    /// Called when the plugin is loaded
    fn on_load(&mut self) -> PluginResult<()> {
        Ok(())
    }
    
    /// Called when the plugin is unloaded
    fn on_unload(&mut self) -> PluginResult<()> {
        Ok(())
    }
    
    /// Called when the plugin is paused
    fn on_pause(&mut self) -> PluginResult<()> {
        Ok(())
    }
    
    /// Called when the plugin is resumed
    fn on_resume(&mut self) -> PluginResult<()> {
        Ok(())
    }
}

/// Configuration trait for plugins
pub trait Configurable {
    /// Validate the configuration
    fn validate_config(&self, config: &Config) -> PluginResult<()>;
    
    /// Get the default configuration
    fn default_config(&self) -> Config;
    
    /// Get configuration schema
    fn config_schema(&self) -> ConfigSchema;
}

/// Metrics trait for plugins
pub trait MetricsProvider {
    /// Get current metrics
    fn get_metrics(&self) -> Metrics;
    
    /// Reset metrics
    fn reset_metrics(&mut self);
    
    /// Get metric names
    fn metric_names(&self) -> Vec<String>;
}

/// Health check trait for plugins
pub trait HealthCheck {
    /// Check plugin health
    fn health_check(&self) -> HealthStatus;
    
    /// Get health check details
    fn health_details(&self) -> Vec<HealthDetail>;
}

/// Async data source trait
#[cfg(feature = "async")]
pub trait AsyncDataSource {
    /// Read the next available record asynchronously
    async fn read_next_async(&mut self) -> PluginResult<Option<DataRecord>>;
    
    /// Read a batch of records asynchronously
    async fn read_batch_async(&mut self, max_size: u32) -> PluginResult<Vec<DataRecord>>;
}

/// Async data destination trait
#[cfg(feature = "async")]
pub trait AsyncDataDestination {
    /// Write a single record asynchronously
    async fn write_async(&mut self, record: DataRecord) -> PluginResult<()>;
    
    /// Write a batch of records asynchronously
    async fn write_batch_async(&mut self, records: Vec<DataRecord>) -> PluginResult<BatchResult>;
    
    /// Flush any buffered data asynchronously
    async fn flush_async(&mut self) -> PluginResult<()>;
}

/// Streaming data processor trait
pub trait StreamingProcessor {
    /// Process a stream of records
    fn process_stream<I>(&mut self, input_stream: I) -> PluginResult<Box<dyn Iterator<Item = ProcessingResult>>>
    where
        I: Iterator<Item = DataRecord>;
    
    /// Process a stream with backpressure
    fn process_stream_with_backpressure<I>(&mut self, input_stream: I, max_buffer_size: usize) -> PluginResult<Box<dyn Iterator<Item = ProcessingResult>>>
    where
        I: Iterator<Item = DataRecord>;
}

/// Windowed aggregator trait
pub trait WindowedAggregator: DataAggregator {
    /// Set window size
    fn set_window_size(&mut self, size: usize) -> PluginResult<()>;
    
    /// Set window duration
    fn set_window_duration(&mut self, duration_ms: u64) -> PluginResult<()>;
    
    /// Get windowed results
    fn get_windowed_results(&mut self) -> PluginResult<Vec<DataRecord>>;
}

/// Schema-aware transformer trait
pub trait SchemaAwareTransformer: DataTransformer {
    /// Transform schema
    fn transform_schema(&self, input_schema: &Schema) -> PluginResult<Schema>;
    
    /// Validate input against schema
    fn validate_input(&self, input: &DataRecord, schema: &Schema) -> PluginResult<()>;
    
    /// Get output schema
    fn get_output_schema(&self, input_schema: &Schema) -> PluginResult<Schema>;
}

/// State management trait
pub trait StatefulProcessor {
    /// Get current state
    fn get_state(&self) -> PluginResult<ProcessorState>;
    
    /// Set state
    fn set_state(&mut self, state: ProcessorState) -> PluginResult<()>;
    
    /// Clear state
    fn clear_state(&mut self) -> PluginResult<()>;
    
    /// Serialize state
    fn serialize_state(&self) -> PluginResult<Vec<u8>>;
    
    /// Deserialize state
    fn deserialize_state(&mut self, data: &[u8]) -> PluginResult<()>;
}

/// Error recovery trait
pub trait ErrorRecovery {
    /// Handle error and attempt recovery
    fn handle_error(&mut self, error: &crate::error::PluginError) -> PluginResult<RecoveryAction>;
    
    /// Get error recovery strategy
    fn get_recovery_strategy(&self) -> ErrorRecoveryStrategy;
    
    /// Set error recovery strategy
    fn set_recovery_strategy(&mut self, strategy: ErrorRecoveryStrategy);
}

/// Plugin factory trait
pub trait PluginFactory {
    /// Create a new plugin instance
    fn create_plugin(&self, config: Config) -> PluginResult<Box<dyn DataProcessor>>;
    
    /// Get supported plugin types
    fn supported_types(&self) -> Vec<String>;
    
    /// Validate plugin configuration
    fn validate_plugin_config(&self, plugin_type: &str, config: &Config) -> PluginResult<()>;
}
