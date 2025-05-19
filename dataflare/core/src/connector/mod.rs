//! Connector interfaces for DataFlare
//!
//! This module defines the core interfaces for source and destination connectors.
//! The actual implementations are provided in the dataflare-connector crate.

use std::fmt::Debug;
use async_trait::async_trait;
use serde_json::Value;
use futures::Stream;

use crate::error::Result;
use crate::message::{DataRecord, DataRecordBatch};
use crate::model::Schema;
use crate::state::SourceState;

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

/// Interface for source connectors
#[async_trait]
pub trait SourceConnector: Send + Sync + 'static {
    /// Configure the connector with the provided parameters
    fn configure(&mut self, config: &Value) -> Result<()>;
    
    /// Check the connection to the source
    async fn check_connection(&self) -> Result<bool>;
    
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

/// Interface for destination connectors
#[async_trait]
pub trait DestinationConnector: Send + Sync + 'static {
    /// Configure the connector with the provided parameters
    fn configure(&mut self, config: &Value) -> Result<()>;
    
    /// Check the connection to the destination
    async fn check_connection(&self) -> Result<bool>;
    
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
