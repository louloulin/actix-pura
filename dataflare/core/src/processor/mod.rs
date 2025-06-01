//! Processor interfaces for DataFlare
//!
//! This module defines the core interfaces for data processors.
//! The actual implementations are provided in the dataflare-processor crate.

use std::fmt::Debug;
use async_trait::async_trait;
use serde_json::Value;

use crate::error::Result;
use crate::message::{DataRecord, DataRecordBatch};
use crate::model::Schema;

/// Processor state for tracking processing progress
#[derive(Debug, Clone)]
pub struct ProcessorState {
    /// Processor ID
    pub processor_id: String,
    
    /// Number of records processed
    pub records_processed: u64,
    
    /// Number of records that failed processing
    pub records_failed: u64,
    
    /// Processing start time
    pub start_time: chrono::DateTime<chrono::Utc>,
    
    /// Processing end time
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    
    /// Additional state data
    pub data: std::collections::HashMap<String, Value>,
}

impl ProcessorState {
    /// Create a new processor state
    pub fn new(processor_id: &str) -> Self {
        Self {
            processor_id: processor_id.to_string(),
            records_processed: 0,
            records_failed: 0,
            start_time: chrono::Utc::now(),
            end_time: None,
            data: std::collections::HashMap::new(),
        }
    }
    
    /// Mark the processor as completed
    pub fn complete(&mut self) {
        self.end_time = Some(chrono::Utc::now());
    }
    
    /// Add state data
    pub fn add_data<K: Into<String>, V: Into<Value>>(&mut self, key: K, value: V) -> &mut Self {
        self.data.insert(key.into(), value.into());
        self
    }
}

/// Interface for data processors
#[async_trait]
pub trait Processor: Send + Sync + 'static {
    /// Configure the processor with the provided parameters
    fn configure(&mut self, config: &Value) -> Result<()>;
    
    /// Initialize the processor
    async fn initialize(&mut self) -> Result<()>;
    
    /// Process a batch of records
    async fn process_batch(&mut self, batch: &DataRecordBatch) -> Result<DataRecordBatch>;
    
    /// Process a single record
    async fn process_record(&mut self, record: &DataRecord) -> Result<DataRecord>;
    
    /// Get the input schema required by the processor
    fn get_input_schema(&self) -> Option<Schema>;
    
    /// Get the output schema produced by the processor
    fn get_output_schema(&self) -> Option<Schema>;
    
    /// Get the current state of the processor
    fn get_state(&self) -> ProcessorState;
    
    /// Finalize the processor
    async fn finalize(&mut self) -> Result<()>;
}

/// Processor type enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessorType {
    /// Mapping processor (field mapping and transformation)
    Mapping,
    /// Filter processor (data filtering)
    Filter,
    /// Aggregate processor (data aggregation)
    Aggregate,
    /// Enrichment processor (data enrichment)
    Enrichment,
    /// Join processor (data joining)
    Join,
    /// Custom processor
    Custom(String),
}
