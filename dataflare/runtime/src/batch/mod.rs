//! Batch processing system for DataFlare
//! 
//! Implements efficient batch data structures and processing mechanisms
//! for high-throughput data pipelines.

mod adaptive;
mod metrics;
mod backpressure;

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use log::{debug, info, warn};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::DataRecord,
};

// Re-export core components
pub use self::adaptive::{AdaptiveBatcher, AdaptiveBatchingConfig};
pub use self::metrics::BatchingMetrics;
pub use self::backpressure::{
    BackpressureController, CreditConfig, CreditMode,
    SharedBackpressureController, create_shared_controller,
};

/// Shared data batch with reference counting for efficient data sharing
#[derive(Debug, Clone)]
pub struct SharedDataBatch {
    /// Records in the batch, wrapped in Arc for efficient sharing
    records: Arc<Vec<DataRecord>>,
    
    /// Watermark timestamp if available
    watermark: Option<i64>,
    
    /// Batch metadata
    metadata: Arc<HashMap<String, String>>,
    
    /// Batch creation time
    creation_time: DateTime<Utc>,
    
    /// Batch ID
    id: String,
}

impl SharedDataBatch {
    /// Create a new shared data batch from records
    pub fn new(records: Vec<DataRecord>) -> Self {
        Self {
            records: Arc::new(records),
            watermark: None,
            metadata: Arc::new(HashMap::new()),
            creation_time: Utc::now(),
            id: uuid::Uuid::new_v4().to_string(),
        }
    }
    
    /// Create an empty batch
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }
    
    /// Create a batch with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            records: Arc::new(Vec::with_capacity(capacity)),
            watermark: None,
            metadata: Arc::new(HashMap::new()),
            creation_time: Utc::now(),
            id: uuid::Uuid::new_v4().to_string(),
        }
    }
    
    /// Get a reference to the records
    pub fn records(&self) -> &[DataRecord] {
        &self.records
    }
    
    /// Get the number of records in the batch
    pub fn len(&self) -> usize {
        self.records.len()
    }
    
    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    
    /// Get the batch watermark
    pub fn watermark(&self) -> Option<i64> {
        self.watermark
    }
    
    /// Set the batch watermark
    pub fn with_watermark(mut self, watermark: i64) -> Self {
        self.watermark = Some(watermark);
        self
    }
    
    /// Get a reference to the batch metadata
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
    
    /// Add metadata to the batch
    pub fn with_metadata(self, key: &str, value: &str) -> Self {
        // Create a new HashMap with the existing metadata plus the new key-value
        let mut new_metadata = HashMap::new();
        for (k, v) in self.metadata.iter() {
            new_metadata.insert(k.clone(), v.clone());
        }
        new_metadata.insert(key.to_string(), value.to_string());
        
        // Return a new SharedDataBatch with the new metadata
        Self {
            records: self.records.clone(),
            watermark: self.watermark,
            metadata: Arc::new(new_metadata),
            creation_time: self.creation_time,
            id: self.id.clone(),
        }
    }
    
    /// Create a slice of this batch without copying the data
    pub fn slice(&self, start: usize, end: usize) -> Self {
        let end = std::cmp::min(end, self.records.len());
        let start = std::cmp::min(start, end);
        
        if start == 0 && end == self.records.len() {
            // If the entire batch is being sliced, just return a clone
            return self.clone();
        }
        
        // Create a new Vec that references the subset of records
        let records_subset: Vec<DataRecord> = self.records[start..end].to_vec();
        
        Self {
            records: Arc::new(records_subset),
            watermark: self.watermark,
            metadata: self.metadata.clone(),
            creation_time: self.creation_time,
            id: uuid::Uuid::new_v4().to_string(), // New ID for the slice
        }
    }
    
    /// Get the age of the batch
    pub fn age(&self) -> Duration {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.creation_time);
        Duration::from_millis(duration.num_milliseconds() as u64)
    }
    
    /// Get the batch ID
    pub fn id(&self) -> &str {
        &self.id
    }
    
    /// Estimate the memory size of the batch in bytes
    pub fn estimate_size(&self) -> usize {
        // Estimate based on record count and average record size
        // This is an approximation and should be refined for production
        let avg_record_size = 1024; // Assume 1KB per record
        self.records.len() * avg_record_size
    }
    
    /// Convert to a new batch by applying a transformation function
    pub fn map<F>(&self, f: F) -> Self 
    where 
        F: Fn(&DataRecord) -> DataRecord
    {
        let transformed: Vec<DataRecord> = self.records
            .iter()
            .map(f)
            .collect();
            
        Self {
            records: Arc::new(transformed),
            watermark: self.watermark,
            metadata: self.metadata.clone(),
            creation_time: Utc::now(), // New creation time for the transformed batch
            id: uuid::Uuid::new_v4().to_string(), // New ID for the transformed batch
        }
    }
    
    /// Filter the batch based on a predicate
    pub fn filter<F>(&self, predicate: F) -> Self
    where
        F: Fn(&DataRecord) -> bool
    {
        let filtered: Vec<DataRecord> = self.records
            .iter()
            .filter(|record| predicate(record))
            .cloned()
            .collect();
            
        Self {
            records: Arc::new(filtered),
            watermark: self.watermark,
            metadata: self.metadata.clone(),
            creation_time: Utc::now(),
            id: uuid::Uuid::new_v4().to_string(),
        }
    }
    
    /// Merge multiple batches into one
    pub fn merge(batches: &[SharedDataBatch]) -> Self {
        if batches.is_empty() {
            return Self::empty();
        }
        
        if batches.len() == 1 {
            return batches[0].clone();
        }
        
        // Calculate total capacity needed
        let total_capacity: usize = batches.iter().map(|b| b.len()).sum();
        
        // Create a new vector with all records
        let mut all_records = Vec::with_capacity(total_capacity);
        for batch in batches {
            all_records.extend_from_slice(&batch.records);
        }
        
        // Find the highest watermark
        let max_watermark = batches.iter()
            .filter_map(|b| b.watermark)
            .max();
        
        // Create a new batch with the merged records
        let mut result = Self {
            records: Arc::new(all_records),
            watermark: max_watermark,
            metadata: Arc::new(HashMap::new()),
            creation_time: Utc::now(),
            id: uuid::Uuid::new_v4().to_string(),
        };
        
        // Merge metadata (keys from later batches override earlier ones)
        let mut merged_metadata = HashMap::new();
        for batch in batches {
            for (k, v) in batch.metadata().iter() {
                merged_metadata.insert(k.clone(), v.clone());
            }
        }
        
        result.metadata = Arc::new(merged_metadata);
        result
    }
} 