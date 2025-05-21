//! Batch-optimized PostgreSQL connector for DataFlare
//!
//! Provides batch-optimized functionality for connecting to PostgreSQL databases.

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use serde_json::{Value, json};
use tokio_postgres::{NoTls, Row};
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    model::{Schema, Field, DataType},
    state::SourceState,
    connector::{
        Connector, BatchSourceConnector, 
        ExtractionMode, ConnectorCapabilities,
        Position
    },
};

use crate::source::SourceConnector;
use super::PostgresSourceConnector;

/// Batch-optimized PostgreSQL source connector
pub struct PostgresBatchSourceConnector {
    /// Base connector
    base: PostgresSourceConnector,
    
    /// Last read position
    position: Position,
    
    /// Batch size for reads
    batch_size: usize,
    
    /// Connection information
    connection_info: HashMap<String, String>,
}

impl PostgresBatchSourceConnector {
    /// Create a new batch source connector
    pub fn new(config: Value) -> Self {
        // Get batch size from config or use default
        let batch_size = config.get("batch_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;
            
        let mut connection_info = HashMap::new();
        if let Some(host) = config.get("host").and_then(|v| v.as_str()) {
            connection_info.insert("host".to_string(), host.to_string());
        }
        if let Some(database) = config.get("database").and_then(|v| v.as_str()) {
            connection_info.insert("database".to_string(), database.to_string());
        }
        
        Self {
            base: PostgresSourceConnector::new(config),
            position: Position::new(),
            batch_size,
            connection_info,
        }
    }
    
    /// Set extraction mode for the connector
    pub fn with_extraction_mode(mut self, mode: ExtractionMode) -> Self {
        // Note: The ExtractionMode enums in dataflare_core and crate::source
        // may differ, so we need to handle the conversion carefully
        let source_mode = match mode {
            ExtractionMode::Full => crate::source::ExtractionMode::Full,
            ExtractionMode::Incremental => crate::source::ExtractionMode::Incremental,
            ExtractionMode::CDC => crate::source::ExtractionMode::CDC,
            ExtractionMode::Hybrid => crate::source::ExtractionMode::Hybrid,
        };
        
        self.base = self.base.with_extraction_mode(source_mode);
        self
    }
    
    /// Build SQL query based on the current position
    async fn build_query_from_position(&self) -> Result<(String, Vec<String>)> {
        // Get table name
        let table = self.base.config.get("table")
            .and_then(|t| t.as_str())
            .ok_or_else(|| DataFlareError::Config("Table parameter is required".to_string()))?;
            
        // Check extraction mode
        let extraction_mode = self.base.get_extraction_mode();
        
        // Build query based on extraction mode and position
        match extraction_mode {
            crate::source::ExtractionMode::Full => {
                // For full extraction, we can use a limit and offset based on position
                let offset = self.position.get_data("offset")
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                    
                let query = format!(
                    "SELECT * FROM {} ORDER BY ctid LIMIT {} OFFSET {}",
                    table, self.batch_size, offset
                );
                
                Ok((query, Vec::new()))
            },
            crate::source::ExtractionMode::Incremental => {
                // For incremental extraction, use cursor field and value
                let state = self.base.get_state()?;
                let cursor_field = state.data.get("cursor_field")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| DataFlareError::Config("cursor_field is required for incremental mode".to_string()))?;
                    
                // Get cursor value from position or use default
                let cursor_value = if let Some(cv) = self.position.get_data("cursor_value") {
                    cv.clone()
                } else if let Some(field_value) = state.data.get("cursor_field").and_then(|v| v.as_str()) {
                    field_value.to_string()
                } else {
                    "0".to_string()
                };
                    
                let query = format!(
                    "SELECT * FROM {} WHERE {} > $1 ORDER BY {} ASC LIMIT {}",
                    table, cursor_field, cursor_field, self.batch_size
                );
                
                let params = vec![cursor_value];
                
                Ok((query, params))
            },
            _ => {
                // Other modes not fully supported yet in batch mode
                Err(DataFlareError::Connection(format!(
                    "Extraction mode {:?} not fully supported in batch mode yet", 
                    extraction_mode
                )))
            }
        }
    }
    
    /// Convert PostgreSQL row to DataRecord
    fn row_to_record(&self, row: Row) -> Result<DataRecord> {
        let mut record_data = serde_json::Map::new();
        
        // Get column names
        let cols = row.columns();
        
        // Process each column
        for col in cols {
            let name = col.name();
            let col_type = col.type_();
            
            // Process value based on column type
            if col_type == &tokio_postgres::types::Type::INT4 || col_type == &tokio_postgres::types::Type::INT8 {
                if let Ok(value) = row.try_get::<_, i64>(name) {
                    record_data.insert(name.to_string(), json!(value));
                }
            } else if col_type == &tokio_postgres::types::Type::FLOAT4 || col_type == &tokio_postgres::types::Type::FLOAT8 {
                if let Ok(value) = row.try_get::<_, f64>(name) {
                    record_data.insert(name.to_string(), json!(value));
                }
            } else if col_type == &tokio_postgres::types::Type::TEXT || col_type == &tokio_postgres::types::Type::VARCHAR {
                if let Ok(value) = row.try_get::<_, String>(name) {
                    record_data.insert(name.to_string(), json!(value));
                }
            } else if col_type == &tokio_postgres::types::Type::BOOL {
                if let Ok(value) = row.try_get::<_, bool>(name) {
                    record_data.insert(name.to_string(), json!(value));
                }
            } else if col_type == &tokio_postgres::types::Type::TIMESTAMP || col_type == &tokio_postgres::types::Type::TIMESTAMPTZ {
                if let Ok(value) = row.try_get::<_, DateTime<Utc>>(name) {
                    record_data.insert(name.to_string(), json!(value.to_rfc3339()));
                }
            } else if col_type == &tokio_postgres::types::Type::JSON || col_type == &tokio_postgres::types::Type::JSONB {
                // For JSON types, we'll parse the string representation
                if let Ok(value) = row.try_get::<_, String>(name) {
                    if let Ok(json_value) = serde_json::from_str::<Value>(&value) {
                        record_data.insert(name.to_string(), json_value);
                    } else {
                        record_data.insert(name.to_string(), json!(value));
                    }
                }
            } else {
                // Default to string representation
                if let Ok(value) = row.try_get::<_, String>(name) {
                    record_data.insert(name.to_string(), json!(value));
                }
            }
        }
        
        Ok(DataRecord::new(Value::Object(record_data)))
    }
}

#[async_trait]
impl Connector for PostgresBatchSourceConnector {
    fn connector_type(&self) -> &str {
        "postgres-batch"
    }
    
    fn configure(&mut self, config: &Value) -> Result<()> {
        // Forward configuration to base connector
        self.base.configure(config)?;
        
        // Update batch size if provided
        if let Some(batch_size) = config.get("batch_size").and_then(|v| v.as_u64()) {
            self.batch_size = batch_size as usize;
        }
        
        Ok(())
    }
    
    async fn check_connection(&self) -> Result<bool> {
        // Use base connector to check connection
        // We need to use the trait method, so import the trait first
        use crate::source::SourceConnector;
        self.base.check_connection().await
    }
    
    fn get_capabilities(&self) -> ConnectorCapabilities {
        let mut capabilities = ConnectorCapabilities::default();
        capabilities.supports_batch_operations = true;
        capabilities.supports_parallel_processing = true;
        capabilities.preferred_batch_size = Some(self.batch_size);
        capabilities.max_batch_size = Some(10000); // Reasonable maximum for PostgreSQL
        
        capabilities
    }
    
    fn get_metadata(&self) -> HashMap<String, String> {
        let mut metadata = self.connection_info.clone();
        metadata.insert("connector_type".to_string(), "postgres-batch".to_string());
        metadata.insert("batch_size".to_string(), self.batch_size.to_string());
        
        metadata
    }
}

#[async_trait]
impl BatchSourceConnector for PostgresBatchSourceConnector {
    async fn discover_schema(&self) -> Result<Schema> {
        // Use base connector to discover schema
        self.base.discover_schema().await
    }
    
    async fn read_batch(&mut self, max_size: usize) -> Result<DataRecordBatch> {
        // Adjust batch size if needed
        let effective_batch_size = std::cmp::min(max_size, self.batch_size);
        
        // Ensure we have connection
        if self.base.client.is_none() {
            self.base.connect().await?;
        }
        
        // Get client reference
        let client = self.base.client.as_ref().ok_or_else(|| {
            DataFlareError::Connection("Failed to connect to PostgreSQL".to_string())
        })?;
        
        // Build query based on position
        let (query, param_values) = self.build_query_from_position().await?;
        
        debug!("Executing batch query: {}", query);
        
        // Execute query
        let rows = if param_values.is_empty() {
            client.query(&query, &[]).await
                .map_err(|e| DataFlareError::Query(format!("Failed to execute query: {}", e)))?
        } else {
            // Convert params to the right type for Postgres
            let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = param_values
                .iter()
                .map(|v| v as &(dyn tokio_postgres::types::ToSql + Sync))
                .collect();
            
            client.query(&query, &params).await
                .map_err(|e| DataFlareError::Query(format!("Failed to execute query: {}", e)))?
        };
        
        // Convert rows to records
        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            match self.row_to_record(row) {
                Ok(record) => records.push(record),
                Err(e) => warn!("Failed to convert row to record: {}", e),
            }
        }
        
        // Update position
        if self.base.get_extraction_mode() == crate::source::ExtractionMode::Full {
            // For full extraction, update offset
            let current_offset = self.position.get_data("offset")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
                
            let new_offset = current_offset + records.len() as u64;
            self.position = Position::new().with_data("offset", new_offset.to_string());
        } else if self.base.get_extraction_mode() == crate::source::ExtractionMode::Incremental {
            // For incremental extraction, update cursor value if records present
            if !records.is_empty() && records.len() > 0 {
                // Get state to find cursor field
                let state = self.base.get_state()?;
                if let Some(cursor_field) = state.data.get("cursor_field").and_then(|v| v.as_str()) {
                    // Get last record's cursor value
                    let last_record = &records[records.len() - 1];
                    
                    if let Some(value) = last_record.data.get(cursor_field) {
                        if let Some(s) = value.as_str() {
                            self.position = Position::new().with_data("cursor_value", s.to_string());
                        } else if let Some(n) = value.as_i64() {
                            self.position = Position::new().with_data("cursor_value", n.to_string());
                        } else if let Some(f) = value.as_f64() {
                            self.position = Position::new().with_data("cursor_value", f.to_string());
                        }
                    }
                }
            }
        }
        
        // Create batch with records
        Ok(DataRecordBatch::new(records))
    }
    
    fn get_state(&self) -> Result<SourceState> {
        self.base.get_state()
    }
    
    async fn commit(&mut self, position: Position) -> Result<()> {
        // Update base state with position data
        let mut state = self.base.get_state()?;
        
        // Copy position data to state
        for (key, value) in position.data().iter() {
            state.add_data(key, value.to_string());
        }
        
        // Save updated state back to base
        self.base.state = state;
        
        // Store position (after we've used it)
        self.position = position;
        
        Ok(())
    }
    
    fn get_position(&self) -> Result<Position> {
        Ok(self.position.clone())
    }
    
    async fn seek(&mut self, position: Position) -> Result<()> {
        // Update position
        self.position = position;
        
        Ok(())
    }
    
    fn get_extraction_mode(&self) -> ExtractionMode {
        // Convert from source extraction mode to core extraction mode
        match self.base.get_extraction_mode() {
            crate::source::ExtractionMode::Full => ExtractionMode::Full,
            crate::source::ExtractionMode::Incremental => ExtractionMode::Incremental,
            crate::source::ExtractionMode::CDC => ExtractionMode::CDC,
            crate::source::ExtractionMode::Hybrid => ExtractionMode::Hybrid,
        }
    }
    
    async fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64> {
        self.base.estimate_record_count(state).await
    }
}

/// Register batch-optimized PostgreSQL connector
pub fn register_postgres_batch_connector() {
    crate::registry::register_connector::<dyn BatchSourceConnector>(
        "postgres-batch",
        Arc::new(|config: Value| -> Result<Box<dyn BatchSourceConnector>> {
            Ok(Box::new(PostgresBatchSourceConnector::new(config)))
        }),
    );
} 