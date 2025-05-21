//! Destination connector adapter for DataFlare
//!
//! This module provides an adapter that converts legacy DestinationConnectors
//! to the new BatchDestinationConnector interface.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use async_trait::async_trait;
use log::{debug, warn};
use chrono::Utc;
use serde_json::Value;

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    model::Schema,
    connector::{
        Connector, DestinationConnector, BatchDestinationConnector,
        WriteMode, WriteStats, WriteState, Position
    },
    connector::ConnectorCapabilities
};

/// Adapter that converts a DestinationConnector to a BatchDestinationConnector
pub struct BatchDestinationAdapter<T: DestinationConnector> {
    /// The wrapped destination connector
    inner: T,
    
    /// Write statistics
    stats: Arc<BatchDestStats>,
    
    /// Current mode for writes
    mode: WriteMode,
}

/// Stats for batch destination adapter
struct BatchDestStats {
    /// Total records written
    total_records: AtomicU64,
    
    /// Total bytes written
    total_bytes: AtomicU64,
    
    /// Last write time
    last_write_time: std::sync::Mutex<chrono::DateTime<chrono::Utc>>,
}

impl<T: DestinationConnector> BatchDestinationAdapter<T> {
    /// Create a new adapter for a destination connector
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            stats: Arc::new(BatchDestStats {
                total_records: AtomicU64::new(0),
                total_bytes: AtomicU64::new(0),
                last_write_time: std::sync::Mutex::new(Utc::now()),
            }),
            mode: WriteMode::Append, // Default mode
        }
    }
    
    /// Update stats after a write operation
    fn update_stats(&self, stats: &WriteStats) {
        self.stats.total_records.fetch_add(stats.records_written, Ordering::Relaxed);
        self.stats.total_bytes.fetch_add(stats.bytes_written, Ordering::Relaxed);
        
        // Update last write time
        let mut last_write_time = self.stats.last_write_time.lock().unwrap();
        *last_write_time = Utc::now();
    }
}

#[async_trait]
impl<T: DestinationConnector> Connector for BatchDestinationAdapter<T> {
    fn connector_type(&self) -> &str {
        self.inner.connector_type()
    }
    
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.inner.configure(config)
    }
    
    async fn check_connection(&self) -> Result<bool> {
        self.inner.check_connection().await
    }
    
    fn get_capabilities(&self) -> ConnectorCapabilities {
        // Enhance capabilities based on the adaptations we provide
        let mut caps = ConnectorCapabilities::default();
        caps.supports_batch_operations = true;
        caps.preferred_batch_size = Some(1000); // Default batch size
        caps
    }
    
    fn get_metadata(&self) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("adapter".to_string(), "BatchDestinationAdapter".to_string());
        metadata.insert("original_type".to_string(), self.inner.connector_type().to_string());
        metadata
    }
}

#[async_trait]
impl<T: DestinationConnector> BatchDestinationConnector for BatchDestinationAdapter<T> {
    async fn prepare_schema(&self, schema: &Schema) -> Result<()> {
        self.inner.prepare_schema(schema).await
    }
    
    async fn write_batch(&mut self, batch: DataRecordBatch) -> Result<WriteStats> {
        // Clone the mode so we can use it
        let mode_copy = self.mode.clone();
        let stats = self.inner.write_batch(&batch, mode_copy).await?;
        
        Ok(stats)
    }
    
    async fn flush(&mut self) -> Result<()> {
        // Many legacy connectors don't have explicit flush, so we commit instead
        self.inner.commit().await
    }
    
    fn get_write_state(&self) -> Result<WriteState> {
        let total_records = self.stats.total_records.load(Ordering::Relaxed);
        let total_bytes = self.stats.total_bytes.load(Ordering::Relaxed);
        let last_write_time = *self.stats.last_write_time.lock().unwrap();
        
        Ok(WriteState {
            last_position: None, // Legacy connectors don't track position
            total_records,
            total_bytes,
            last_write_time,
            metadata: HashMap::new(),
        })
    }
    
    async fn commit(&mut self) -> Result<()> {
        self.inner.commit().await
    }
    
    async fn rollback(&mut self) -> Result<()> {
        self.inner.rollback().await
    }
    
    fn get_supported_write_modes(&self) -> Vec<WriteMode> {
        self.inner.get_supported_write_modes()
    }
    
    async fn write_record(&mut self, record: DataRecord, mode: WriteMode) -> Result<WriteStats> {
        // Save the original mode
        let old_mode = self.mode.clone();
        
        // Temporarily use the provided mode
        self.mode = mode.clone();
        let stats = self.inner.write_record(&record, mode).await?;
        
        // Restore the original mode
        self.mode = old_mode;
        
        Ok(stats)
    }
} 