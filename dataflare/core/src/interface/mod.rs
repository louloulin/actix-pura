//! # DataFlare Interfaces
//!
//! This module defines the core interfaces used throughout the DataFlare framework.
//! These interfaces establish the contract between different components and enable
//! dependency inversion.

use crate::{
    error::Result,
    message::{DataRecord, DataRecordBatch},
    state::SourceState,
};
use futures::Stream;
use std::pin::Pin;

/// Trait for components that can process data records
pub trait DataProcessor {
    /// Process a single data record
    fn process_record(&self, record: DataRecord) -> Result<DataRecord>;
    
    /// Process a batch of data records
    fn process_batch(&self, batch: DataRecordBatch) -> Result<DataRecordBatch>;
}

/// Trait for components that can read data
pub trait DataReader {
    /// Read data from the source
    fn read(&mut self, state: Option<SourceState>) -> Result<Pin<Box<dyn Stream<Item = Result<DataRecord>> + Send>>>;
    
    /// Get the current state of the reader
    fn get_state(&self) -> Result<SourceState>;
    
    /// Estimate the number of records that will be read
    fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64>;
}

/// Trait for components that can write data
pub trait DataWriter {
    /// Write a single record
    fn write_record(&mut self, record: DataRecord) -> Result<()>;
    
    /// Write a batch of records
    fn write_batch(&mut self, batch: DataRecordBatch) -> Result<()>;
    
    /// Flush any buffered data
    fn flush(&mut self) -> Result<()>;
    
    /// Close the writer and release resources
    fn close(&mut self) -> Result<()>;
}

/// Trait for components that can transform data
pub trait DataTransformer {
    /// Transform a single record
    fn transform(&self, record: DataRecord) -> Result<DataRecord>;
    
    /// Transform a batch of records
    fn transform_batch(&self, batch: DataRecordBatch) -> Result<DataRecordBatch>;
}

/// Trait for components that can be initialized
pub trait Initializable {
    /// Initialize the component
    fn initialize(&mut self) -> Result<()>;
}

/// Trait for components that can be configured
pub trait Configurable {
    /// Configure the component with the given configuration
    fn configure(&mut self, config: serde_json::Value) -> Result<()>;
}

/// Trait for components that can be monitored
pub trait Monitorable {
    /// Get metrics about the component
    fn get_metrics(&self) -> serde_json::Value;
}

/// Trait for components that can be lifecycle managed
pub trait Lifecycle: Initializable {
    /// Start the component
    fn start(&mut self) -> Result<()>;
    
    /// Stop the component
    fn stop(&mut self) -> Result<()>;
    
    /// Check if the component is running
    fn is_running(&self) -> bool;
}

/// Trait for components that can be part of a workflow
pub trait WorkflowComponent: Lifecycle + Configurable + Monitorable {
    /// Get the component ID
    fn get_id(&self) -> &str;
    
    /// Get the component type
    fn get_type(&self) -> &str;
}
