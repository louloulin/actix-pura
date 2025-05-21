//! Connector interfaces for DataFlare
//!
//! This module defines the core interfaces for source and destination connectors.
//! The actual implementations are provided in the dataflare-connector crate.

use std::fmt::Debug;
use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;
use futures::Stream;

use crate::error::Result;
use crate::message::{DataRecord, DataRecordBatch};
use crate::model::Schema;
use crate::state::SourceState;

/// Position data for tracking connector progress
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Position {
    /// Position data as key-value pairs
    data: HashMap<String, String>,
}

impl Position {
    /// Create a new empty position
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
    
    /// Add a key-value pair to the position
    pub fn with_data<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }
    
    /// Get a position data value
    pub fn get_data(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }
    
    /// Get all position data
    pub fn data(&self) -> &HashMap<String, String> {
        &self.data
    }
}

/// Extraction mode for source connectors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionMode {
    /// Full extraction (extract all data)
    Full,
    /// Incremental extraction (extract only new or changed data)
    Incremental,
    /// Change Data Capture (extract changes as they occur)
    CDC,
    /// Hybrid mode (combination of multiple modes)
    Hybrid,
}

/// Write mode for destination connectors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteMode {
    /// Append data to the destination
    Append,
    /// Overwrite data in the destination
    Overwrite,
    /// Merge data with existing data in the destination
    Merge,
    /// Update existing data in the destination
    Update,
    /// Delete data from the destination
    Delete,
}

/// Write statistics for destination connectors
#[derive(Debug, Clone)]
pub struct WriteStats {
    /// Number of records written
    pub records_written: u64,
    /// Number of records that failed to write
    pub records_failed: u64,
    /// Number of bytes written
    pub bytes_written: u64,
    /// Time taken to write in milliseconds
    pub write_time_ms: u64,
}

/// Write state of destination connectors
#[derive(Debug, Clone)]
pub struct WriteState {
    /// Last successful write position
    pub last_position: Option<Position>,
    /// Total records written in current session
    pub total_records: u64,
    /// Total bytes written in current session
    pub total_bytes: u64,
    /// Last write timestamp
    pub last_write_time: chrono::DateTime<chrono::Utc>,
    /// Additional state information specific to the connector
    pub metadata: HashMap<String, String>,
}

/// Connector capabilities
#[derive(Debug, Clone, Default)]
pub struct ConnectorCapabilities {
    /// Supports batch operations
    pub supports_batch_operations: bool,
    /// Supports transactions
    pub supports_transactions: bool,
    /// Supports schema evolution
    pub supports_schema_evolution: bool,
    /// Maximum batch size supported
    pub max_batch_size: Option<usize>,
    /// Preferred batch size
    pub preferred_batch_size: Option<usize>,
    /// Supports parallel processing
    pub supports_parallel_processing: bool,
}

/// Base interface for all connectors
#[async_trait]
pub trait Connector: Send + Sync + 'static {
    /// Get the connector type identifier
    fn connector_type(&self) -> &str;
    
    /// Configure the connector with the provided parameters
    fn configure(&mut self, config: &Value) -> Result<()>;
    
    /// Check the connection
    async fn check_connection(&self) -> Result<bool>;
    
    /// Get connector capabilities
    fn get_capabilities(&self) -> ConnectorCapabilities;
    
    /// Get connector metadata
    fn get_metadata(&self) -> HashMap<String, String>;
}

/// Interface for source connectors (legacy)
#[async_trait]
pub trait SourceConnector: Connector {
    /// Discover the schema of the source
    async fn discover_schema(&self) -> Result<Schema>;
    
    /// Read data from the source
    async fn read(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>>;
    
    /// Get the current state of the source
    fn get_state(&self) -> Result<SourceState>;
    
    /// Get the extraction mode of the source
    fn get_extraction_mode(&self) -> ExtractionMode;
    
    /// Estimate the number of records that will be extracted
    async fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64>;
}

/// Interface for batch-optimized source connectors
#[async_trait]
pub trait BatchSourceConnector: Connector {
    /// Discover the schema of the source
    async fn discover_schema(&self) -> Result<Schema>;
    
    /// Read a batch of data from the source
    async fn read_batch(&mut self, max_size: usize) -> Result<DataRecordBatch>;
    
    /// Get the current state of the source
    fn get_state(&self) -> Result<SourceState>;
    
    /// Commit the current position
    async fn commit(&mut self, position: Position) -> Result<()>;
    
    /// Get the current position
    fn get_position(&self) -> Result<Position>;
    
    /// Seek to a specific position
    async fn seek(&mut self, position: Position) -> Result<()>;
    
    /// Get the extraction mode of the source
    fn get_extraction_mode(&self) -> ExtractionMode;
    
    /// Estimate the number of records that will be extracted
    async fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64>;
    
    /// Stream compatibility layer (default implementation converts batch to stream)
    async fn read_stream(&mut self) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>> {
        // Default implementation that converts batches to a stream
        // This provides backward compatibility with systems expecting streams
        todo!("Default implementation to convert batch to stream")
    }
}

/// Interface for destination connectors (legacy)
#[async_trait]
pub trait DestinationConnector: Connector {
    /// Prepare the schema in the destination
    async fn prepare_schema(&self, schema: &Schema) -> Result<()>;
    
    /// Write a batch of records to the destination
    async fn write_batch(&mut self, batch: &DataRecordBatch, mode: WriteMode) -> Result<WriteStats>;
    
    /// Write a single record to the destination
    async fn write_record(&mut self, record: &DataRecord, mode: WriteMode) -> Result<WriteStats>;
    
    /// Commit the write operation
    async fn commit(&mut self) -> Result<()>;
    
    /// Rollback the write operation
    async fn rollback(&mut self) -> Result<()>;
    
    /// Get the supported write modes
    fn get_supported_write_modes(&self) -> Vec<WriteMode>;
}

/// Interface for batch-optimized destination connectors
#[async_trait]
pub trait BatchDestinationConnector: Connector {
    /// Prepare the schema in the destination
    async fn prepare_schema(&self, schema: &Schema) -> Result<()>;
    
    /// Write a batch of records to the destination
    async fn write_batch(&mut self, batch: DataRecordBatch) -> Result<WriteStats>;
    
    /// Flush any buffered data
    async fn flush(&mut self) -> Result<()>;
    
    /// Get the current write state
    fn get_write_state(&self) -> Result<WriteState>;
    
    /// Commit the write operation
    async fn commit(&mut self) -> Result<()>;
    
    /// Rollback the write operation
    async fn rollback(&mut self) -> Result<()>;
    
    /// Get the supported write modes
    fn get_supported_write_modes(&self) -> Vec<WriteMode>;
    
    /// Record compatibility layer (default implementation uses batch with single record)
    async fn write_record(&mut self, record: DataRecord, mode: WriteMode) -> Result<WriteStats> {
        // Create a single-record batch and call write_batch
        let batch = DataRecordBatch::new(vec![record]);
        self.write_batch(batch).await
    }
}
