//! Source connector adapter for DataFlare
//!
//! This module provides an adapter that converts legacy SourceConnectors
//! to the new BatchSourceConnector interface.

use std::pin::Pin;
use std::sync::Arc;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use log::warn;

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    model::Schema,
    state::SourceState,
    connector::{
        Connector, SourceConnector, BatchSourceConnector, 
        ExtractionMode, ConnectorCapabilities, Position
    },
};

/// Adapter that transforms a legacy SourceConnector to a BatchSourceConnector
pub struct BatchSourceAdapter<T: SourceConnector> {
    /// Wrapped source connector
    inner: T,
    
    /// Batch size
    batch_size: usize,
    
    /// Current position
    position: Position,
}

impl<T: SourceConnector> BatchSourceAdapter<T> {
    /// Create a new batch source adapter
    pub fn new(source: T) -> Self {
        Self {
            inner: source,
            batch_size: 1000,
            position: Position::new(),
        }
    }
    
    /// Set batch size
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }
    
    /// Initialize a stream from the inner connector
    async fn create_stream(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>> {
        self.inner.read(state).await
    }
}

#[async_trait]
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
        let mut capabilities = self.inner.get_capabilities();
        capabilities.supports_batch_operations = true;
        capabilities.preferred_batch_size = Some(self.batch_size);
        capabilities
    }
    
    fn get_metadata(&self) -> std::collections::HashMap<String, String> {
        let mut metadata = self.inner.get_metadata();
        metadata.insert("adapted".to_string(), "true".to_string());
        metadata.insert("batch_size".to_string(), self.batch_size.to_string());
        metadata
    }
}

#[async_trait]
impl<T: SourceConnector> BatchSourceConnector for BatchSourceAdapter<T> {
    async fn discover_schema(&self) -> Result<Schema> {
        self.inner.discover_schema().await
    }
    
    async fn read_batch(&mut self, max_size: usize) -> Result<DataRecordBatch> {
        let state = self.get_state()?;
        
        // Create a new stream for reading
        let mut stream = self.create_stream(Some(state.clone())).await?;
        
        // Use the stream to read records up to max_size
        let mut records = Vec::with_capacity(max_size);
        
        // Read up to max_size records from the stream
        while let Some(result) = stream.next().await {
            match result {
                Ok(record) => records.push(record),
                Err(e) => {
                    warn!("Error reading record: {}", e);
                    return Err(e);
                }
            }
            
            if records.len() >= max_size {
                break;
            }
        }
        
        // Update position based on last processed record
        // In a real implementation, we would extract the position from the last record
        
        // Return batch
        Ok(DataRecordBatch::new(records))
    }
    
    fn get_state(&self) -> Result<SourceState> {
        self.inner.get_state()
    }
    
    async fn commit(&mut self, position: Position) -> Result<()> {
        // Update position
        self.position = position;
        
        Ok(())
    }
    
    fn get_position(&self) -> Result<Position> {
        Ok(self.position.clone())
    }
    
    async fn seek(&mut self, position: Position) -> Result<()> {
        // Store position
        self.position = position;
        
        Ok(())
    }
    
    fn get_extraction_mode(&self) -> ExtractionMode {
        self.inner.get_extraction_mode()
    }
    
    async fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64> {
        self.inner.estimate_record_count(state).await
    }
    
    async fn read_stream(&mut self) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>> {
        let state = self.get_state()?;
        self.inner.read(Some(state)).await
    }
} 