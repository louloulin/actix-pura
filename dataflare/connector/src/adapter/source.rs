//! Source connector adapter for DataFlare
//!
//! This module provides an adapter that converts legacy SourceConnectors
//! to the new BatchSourceConnector interface.

use std::collections::HashMap;
use std::pin::Pin;
use async_trait::async_trait;
use futures::{Stream, StreamExt, TryStreamExt};
use log::{debug, warn};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    model::Schema,
    state::{SourceState, Position},
    connector::{
        Connector, SourceConnector, BatchSourceConnector, 
        ExtractionMode, ConnectorCapabilities
    }
};

/// Adapter that converts a SourceConnector to a BatchSourceConnector
pub struct BatchSourceAdapter<T: SourceConnector> {
    /// The wrapped source connector
    inner: T,
    
    /// Buffer for read operations
    buffer: Vec<DataRecord>,
    
    /// Current position
    position: Position,
    
    /// Buffered stream for reading
    stream: Option<Pin<Box<dyn Stream<Item = Result<DataRecord>> + Send>>>,
}

impl<T: SourceConnector> BatchSourceAdapter<T> {
    /// Create a new adapter for a source connector
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            position: Position::new(),
            stream: None,
        }
    }
    
    /// Initialize the stream if it doesn't exist
    async fn ensure_stream_initialized(&mut self) -> Result<()> {
        if self.stream.is_none() {
            let state = self.get_state()?;
            let stream = self.inner.read(Some(state)).await?;
            self.stream = Some(stream);
        }
        Ok(())
    }
}

impl<T: SourceConnector> Connector for BatchSourceAdapter<T> {
    fn connector_type(&self) -> &str {
        self.inner.connector_type()
    }
    
    fn configure(&mut self, config: &serde_json::Value) -> Result<()> {
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
        metadata.insert("adapter".to_string(), "BatchSourceAdapter".to_string());
        metadata.insert("original_type".to_string(), self.inner.connector_type().to_string());
        metadata
    }
}

#[async_trait]
impl<T: SourceConnector> BatchSourceConnector for BatchSourceAdapter<T> {
    async fn discover_schema(&self) -> Result<Schema> {
        self.inner.discover_schema().await
    }
    
    async fn read_batch(&mut self, max_size: usize) -> Result<DataRecordBatch> {
        // Ensure we have an initialized stream
        self.ensure_stream_initialized().await?;
        
        // Use the existing stream to read records up to max_size
        let mut records = Vec::with_capacity(max_size);
        let stream = self.stream.as_mut()
            .ok_or_else(|| DataFlareError::Connector("Stream not initialized".to_string()))?;
        
        // Read up to max_size records from the stream
        while records.len() < max_size {
            match stream.next().await {
                Some(Ok(record)) => {
                    records.push(record);
                },
                Some(Err(e)) => {
                    // Log error but continue reading
                    warn!("Error reading record: {}", e);
                },
                None => {
                    // End of stream
                    break;
                }
            }
        }
        
        // If we got any records, update position
        if !records.is_empty() {
            self.position = Position::new()
                .with_data("record_count", (self.position.get_data("record_count")
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0) + records.len() as u64).to_string());
        }
        
        // Create a batch from the records
        Ok(DataRecordBatch::new(records))
    }
    
    fn get_state(&self) -> Result<SourceState> {
        self.inner.get_state()
    }
    
    async fn commit(&mut self, position: Position) -> Result<()> {
        // Store the position for later use
        self.position = position;
        Ok(())
    }
    
    fn get_position(&self) -> Result<Position> {
        Ok(self.position.clone())
    }
    
    async fn seek(&mut self, position: Position) -> Result<()> {
        // Reset stream to force re-initialization
        self.stream = None;
        self.position = position;
        
        // We'll need to re-initialize the stream on next read
        Ok(())
    }
    
    fn get_extraction_mode(&self) -> ExtractionMode {
        self.inner.get_extraction_mode()
    }
    
    async fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64> {
        self.inner.estimate_record_count(state).await
    }
    
    async fn read_stream(&mut self) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>> {
        // Use the inner connector's read method directly
        let state = self.get_state()?;
        self.inner.read(Some(state)).await
    }
} 