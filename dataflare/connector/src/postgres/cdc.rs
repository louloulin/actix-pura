//! PostgreSQL CDC (Change Data Capture) connector
//!
//! Provides batch-optimized Change Data Capture functionality for PostgreSQL databases
//! using logical replication.

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use futures::Stream;
use serde_json::{Value, json};
use tokio_postgres::{NoTls, Row};
use chrono::{DateTime, Utc};
use log::{debug, info, warn};

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

use super::PostgresSourceConnector;
use crate::source::SourceConnector;

/// CDC operation types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CDCOperation {
    /// Insert operation
    Insert,
    /// Update operation 
    Update,
    /// Delete operation
    Delete,
    /// Truncate operation
    Truncate,
}

impl From<&str> for CDCOperation {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "insert" | "i" => CDCOperation::Insert,
            "update" | "u" => CDCOperation::Update,
            "delete" | "d" => CDCOperation::Delete,
            "truncate" | "t" => CDCOperation::Truncate,
            _ => CDCOperation::Insert, // Default
        }
    }
}

/// PostgreSQL CDC connector for batch processing of change events
pub struct PostgresCDCConnector {
    /// Base connector
    base: PostgresSourceConnector,
    
    /// Position info
    position: Position,
    
    /// Replication slot name
    slot_name: String,
    
    /// Publication name
    publication_name: String,
    
    /// Connection info
    connection_info: HashMap<String, String>,
    
    /// Batch size
    batch_size: usize,
    
    /// Plugin for logical decoding
    decoder_plugin: String,
}

impl PostgresCDCConnector {
    /// Create a new PostgreSQL CDC connector
    pub fn new(config: Value) -> Self {
        // Extract slot and publication from config
        let slot_name = config.get("slot_name")
            .and_then(|s| s.as_str())
            .unwrap_or("dataflare_slot")
            .to_string();
            
        let publication_name = config.get("publication_name")
            .and_then(|p| p.as_str())
            .unwrap_or("dataflare_publication")
            .to_string();
            
        let decoder_plugin = config.get("decoder_plugin")
            .and_then(|p| p.as_str())
            .unwrap_or("pgoutput")
            .to_string();
            
        let batch_size = config.get("batch_size")
            .and_then(|b| b.as_u64())
            .unwrap_or(1000) as usize;
            
        // Extract connection info
        let mut connection_info = HashMap::new();
        if let Some(host) = config.get("host").and_then(|h| h.as_str()) {
            connection_info.insert("host".to_string(), host.to_string());
        }
        if let Some(db) = config.get("database").and_then(|d| d.as_str()) {
            connection_info.insert("database".to_string(), db.to_string());
        }
        
        Self {
            base: PostgresSourceConnector::new(config.clone()),
            position: Position::new(),
            slot_name,
            publication_name,
            decoder_plugin,
            batch_size,
            connection_info,
        }
    }
    
    /// Setup replication slot and publication
    async fn setup_replication(&mut self) -> Result<()> {
        // Ensure connection is established
        if self.base.client.is_none() {
            self.base.connect().await?;
        }
        
        let client = self.base.client.as_ref().ok_or_else(|| {
            DataFlareError::Connection("Failed to connect to PostgreSQL".to_string())
        })?;
        
        // Get table name from config
        let table = self.base.config.get("table")
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                DataFlareError::Config("Table parameter is required".to_string())
            })?;
            
        // Check if replication slot exists
        let slot_exists = client.query_one(
            "SELECT 1 FROM pg_replication_slots WHERE slot_name = $1",
            &[&self.slot_name],
        ).await.is_ok();
        
        // Create replication slot if it doesn't exist
        if !slot_exists {
            info!("Creating replication slot '{}'", self.slot_name);
            
            let create_slot_query = format!(
                "SELECT pg_create_logical_replication_slot('{}', '{}')",
                self.slot_name, self.decoder_plugin
            );
            
            client.execute(&create_slot_query, &[]).await
                .map_err(|e| {
                    DataFlareError::Query(format!("Failed to create replication slot: {}", e))
                })?;
                
            info!("Replication slot created successfully");
        }
        
        // Check if publication exists
        let publication_exists = client.query_one(
            "SELECT 1 FROM pg_publication WHERE pubname = $1",
            &[&self.publication_name],
        ).await.is_ok();
        
        // Create publication if it doesn't exist
        if !publication_exists {
            info!("Creating publication '{}'", self.publication_name);
            
            let create_publication_query = format!(
                "CREATE PUBLICATION {} FOR TABLE {}",
                self.publication_name, table
            );
            
            client.execute(&create_publication_query, &[]).await
                .map_err(|e| {
                    DataFlareError::Query(format!("Failed to create publication: {}", e))
                })?;
                
            info!("Publication created successfully");
        }
        
        Ok(())
    }
    
    /// Read changes from replication slot
    async fn read_changes(&mut self, max_changes: usize) -> Result<Vec<DataRecord>> {
        // Ensure setup is complete
        self.setup_replication().await?;
        
        let client = self.base.client.as_ref().ok_or_else(|| {
            DataFlareError::Connection("Failed to connect to PostgreSQL".to_string())
        })?;
        
        // Get LSN position from current state
        let lsn_position = self.position.get_data("lsn_position")
            .map(|lsn| lsn.to_string());
            
        // Build SQL query to read changes
        let query = if let Some(lsn) = lsn_position {
            format!(
                "SELECT * FROM pg_logical_slot_get_changes('{}', NULL, NULL, 'include-lsn', '1', 'include-timestamp', '1', 'skip-empty-xacts', '1', 'limit', '{}')",
                self.slot_name, max_changes
            )
        } else {
            format!(
                "SELECT * FROM pg_logical_slot_peek_changes('{}', NULL, NULL, 'include-lsn', '1', 'include-timestamp', '1', 'skip-empty-xacts', '1', 'limit', '{}')",
                self.slot_name, max_changes
            )
        };
        
        // Execute query
        let rows = client.query(&query, &[]).await
            .map_err(|e| {
                DataFlareError::Query(format!("Failed to read changes from replication slot: {}", e))
            })?;
            
        // Process rows into change records
        let mut records = Vec::with_capacity(rows.len());
        let mut latest_lsn = None;
        
        for row in rows {
            // Extract fields
            let lsn: String = row.get("lsn");
            latest_lsn = Some(lsn.clone());
            
            let data: String = row.get("data");
            
            // Parse the WAL data based on decoder plugin
            let record = match self.decoder_plugin.as_str() {
                "pgoutput" => self.parse_pgoutput_data(&lsn, &data)?,
                "wal2json" => self.parse_wal2json_data(&lsn, &data)?,
                _ => {
                    // For unknown decoders, just store the raw data
                    let mut record_data = serde_json::Map::new();
                    record_data.insert("lsn".to_string(), json!(lsn));
                    record_data.insert("data".to_string(), json!(data));
                    record_data.insert("timestamp".to_string(), json!(Utc::now().to_rfc3339()));
                    
                    DataRecord::new(Value::Object(record_data))
                }
            };
            
            records.push(record);
        }
        
        // Update position with latest LSN if present
        if let Some(lsn) = latest_lsn {
            self.position = Position::new().with_data("lsn_position", lsn);
        }
        
        Ok(records)
    }
    
    /// Parse pgoutput format data
    fn parse_pgoutput_data(&self, lsn: &str, data: &str) -> Result<DataRecord> {
        // Basic parsing for pgoutput format
        let parts: Vec<&str> = data.split(' ').collect();
        
        let mut record_data = serde_json::Map::new();
        record_data.insert("lsn".to_string(), json!(lsn));
        
        // Parse operation type
        if !parts.is_empty() {
            let operation = match parts[0] {
                "BEGIN" => "begin",
                "COMMIT" => "commit",
                "INSERT" => "insert",
                "UPDATE" => "update",
                "DELETE" => "delete",
                _ => parts[0],
            };
            
            record_data.insert("operation".to_string(), json!(operation));
            
            // For data operations, extract table and data
            if operation == "insert" || operation == "update" || operation == "delete" {
                if parts.len() > 1 {
                    record_data.insert("table".to_string(), json!(parts[1]));
                }
                
                // Attempt to parse column data
                // This is simplified and would need to be expanded for real pgoutput format
                if parts.len() > 2 {
                    let mut columns = serde_json::Map::new();
                    
                    // Extract column data
                    for i in 2..parts.len() {
                        if parts[i].contains(':') {
                            let column_parts: Vec<&str> = parts[i].split(':').collect();
                            if column_parts.len() == 2 {
                                columns.insert(column_parts[0].to_string(), json!(column_parts[1]));
                            }
                        }
                    }
                    
                    if !columns.is_empty() {
                        record_data.insert("data".to_string(), Value::Object(columns));
                    }
                }
            }
        }
        
        // Add timestamp
        record_data.insert("timestamp".to_string(), json!(Utc::now().to_rfc3339()));
        
        Ok(DataRecord::new(Value::Object(record_data)))
    }
    
    /// Parse wal2json format data
    fn parse_wal2json_data(&self, lsn: &str, data: &str) -> Result<DataRecord> {
        // wal2json outputs JSON, so we can parse it directly
        match serde_json::from_str::<Value>(data) {
            Ok(mut json_data) => {
                if let Value::Object(ref mut obj) = json_data {
                    // Add LSN to the object
                    obj.insert("lsn".to_string(), json!(lsn));
                    // Add timestamp
                    obj.insert("timestamp".to_string(), json!(Utc::now().to_rfc3339()));
                }
                
                Ok(DataRecord::new(json_data))
            },
            Err(e) => {
                // If we can't parse as JSON, create a simple record
                let mut record_data = serde_json::Map::new();
                record_data.insert("lsn".to_string(), json!(lsn));
                record_data.insert("raw_data".to_string(), json!(data));
                record_data.insert("error".to_string(), json!(format!("Failed to parse JSON: {}", e)));
                record_data.insert("timestamp".to_string(), json!(Utc::now().to_rfc3339()));
                
                Ok(DataRecord::new(Value::Object(record_data)))
            }
        }
    }
}

#[async_trait]
impl Connector for PostgresCDCConnector {
    fn connector_type(&self) -> &str {
        "postgres-cdc"
    }
    
    fn configure(&mut self, config: &Value) -> Result<()> {
        // Forward configuration to base
        self.base.configure(config)?;
        
        // Extract slot and publication from config
        if let Some(slot_name) = config.get("slot_name").and_then(|s| s.as_str()) {
            self.slot_name = slot_name.to_string();
        }
            
        if let Some(publication_name) = config.get("publication_name").and_then(|p| p.as_str()) {
            self.publication_name = publication_name.to_string();
        }
            
        if let Some(decoder_plugin) = config.get("decoder_plugin").and_then(|p| p.as_str()) {
            self.decoder_plugin = decoder_plugin.to_string();
        }
            
        if let Some(batch_size) = config.get("batch_size").and_then(|b| b.as_u64()) {
            self.batch_size = batch_size as usize;
        }
        
        Ok(())
    }
    
    async fn check_connection(&self) -> Result<bool> {
        self.base.check_connection().await
    }
    
    fn get_capabilities(&self) -> ConnectorCapabilities {
        let mut capabilities = ConnectorCapabilities::default();
        capabilities.supports_batch_operations = true;
        capabilities.supports_parallel_processing = false; // CDC typically needs sequential processing
        capabilities.preferred_batch_size = Some(self.batch_size);
        capabilities.max_batch_size = Some(10000); 
        
        capabilities
    }
    
    fn get_metadata(&self) -> HashMap<String, String> {
        let mut metadata = self.connection_info.clone();
        metadata.insert("connector_type".to_string(), "postgres-cdc".to_string());
        metadata.insert("slot_name".to_string(), self.slot_name.clone());
        metadata.insert("publication_name".to_string(), self.publication_name.clone());
        metadata.insert("decoder_plugin".to_string(), self.decoder_plugin.clone());
        
        metadata
    }
}

#[async_trait]
impl BatchSourceConnector for PostgresCDCConnector {
    async fn discover_schema(&self) -> Result<Schema> {
        self.base.discover_schema().await
    }
    
    async fn read_batch(&mut self, max_size: usize) -> Result<DataRecordBatch> {
        // Read a batch of changes
        let records = self.read_changes(max_size).await?;
        
        // Return batch
        Ok(DataRecordBatch::new(records))
    }
    
    fn get_state(&self) -> Result<SourceState> {
        let mut state = self.base.get_state()?;
        
        // Add CDC-specific state
        state.add_data("slot_name", self.slot_name.clone());
        if let Some(lsn) = self.position.get_data("lsn_position") {
            state.add_data("lsn_position", lsn.clone());
        }
        
        Ok(state)
    }
    
    async fn commit(&mut self, position: Position) -> Result<()> {
        // Update local position
        self.position = position.clone();
        
        // Get LSN from position
        if let Some(lsn) = position.get_data("lsn_position") {
            // Update base state
            let mut state = self.base.get_state()?;
            state.add_data("lsn_position", lsn.clone());
            self.base.state = state;
            
            // If we have a client, advance the replication slot
            if let Some(client) = &self.base.client {
                let query = format!(
                    "SELECT pg_replication_slot_advance('{}', '{}')",
                    self.slot_name, lsn
                );
                
                client.execute(&query, &[]).await
                    .map_err(|e| {
                        DataFlareError::Query(format!("Failed to advance replication slot: {}", e))
                    })?;
                    
                debug!("Advanced replication slot to LSN: {}", lsn);
            }
        }
        
        Ok(())
    }
    
    fn get_position(&self) -> Result<Position> {
        Ok(self.position.clone())
    }
    
    async fn seek(&mut self, position: Position) -> Result<()> {
        // Update position
        self.position = position.clone();
        
        Ok(())
    }
    
    fn get_extraction_mode(&self) -> ExtractionMode {
        ExtractionMode::CDC
    }
    
    async fn estimate_record_count(&self, _state: Option<SourceState>) -> Result<u64> {
        // It's hard to estimate CDC records, so return 0
        Ok(0)
    }
}

/// Register the CDC connector
pub fn register_postgres_cdc_connector() {
    crate::registry::register_connector::<dyn BatchSourceConnector>(
        "postgres-cdc",
        Arc::new(|config: Value| -> Result<Box<dyn BatchSourceConnector>> {
            Ok(Box::new(PostgresCDCConnector::new(config)))
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_cdc_operation_from_str() {
        assert_eq!(CDCOperation::from("insert"), CDCOperation::Insert);
        assert_eq!(CDCOperation::from("UPDATE"), CDCOperation::Update);
        assert_eq!(CDCOperation::from("Delete"), CDCOperation::Delete);
        assert_eq!(CDCOperation::from("t"), CDCOperation::Truncate);
        assert_eq!(CDCOperation::from("unknown"), CDCOperation::Insert); // Default
    }
    
    #[test]
    fn test_parse_pgoutput_data() {
        let connector = PostgresCDCConnector::new(json!({
            "host": "localhost",
            "database": "test_db",
            "user": "postgres",
            "password": "postgres",
        }));
        
        // Test parsing BEGIN message
        let lsn = "0/1620838";
        let data = "BEGIN 739 1622578 1622578";
        
        let record = connector.parse_pgoutput_data(lsn, data).unwrap();
        let record_data = record.data.as_object().unwrap();
        
        assert_eq!(record_data.get("lsn").unwrap().as_str().unwrap(), lsn);
        assert_eq!(record_data.get("operation").unwrap().as_str().unwrap(), "begin");
        
        // Test parsing INSERT message
        let lsn = "0/1620838";
        let data = "INSERT table public.test: id[integer]:1 name[text]:'Test'";
        
        let record = connector.parse_pgoutput_data(lsn, data).unwrap();
        let record_data = record.data.as_object().unwrap();
        
        assert_eq!(record_data.get("lsn").unwrap().as_str().unwrap(), lsn);
        assert_eq!(record_data.get("operation").unwrap().as_str().unwrap(), "insert");
        assert_eq!(record_data.get("table").unwrap().as_str().unwrap(), "table");
    }
    
    #[test]
    fn test_parse_wal2json_data() {
        let connector = PostgresCDCConnector::new(json!({
            "host": "localhost",
            "database": "test_db",
            "user": "postgres",
            "password": "postgres",
        }));
        
        // Test parsing valid JSON
        let lsn = "0/1620838";
        let data = r#"{"change":[{"kind":"insert","schema":"public","table":"test","columnnames":["id","name"],"columnvalues":[1,"Test"]}]}"#;
        
        let record = connector.parse_wal2json_data(lsn, data).unwrap();
        let record_data = record.data.as_object().unwrap();
        
        assert_eq!(record_data.get("lsn").unwrap().as_str().unwrap(), lsn);
        assert!(record_data.contains_key("change"));
        assert!(record_data.contains_key("timestamp"));
        
        // Test parsing invalid JSON
        let lsn = "0/1620838";
        let data = "This is not valid JSON";
        
        let record = connector.parse_wal2json_data(lsn, data).unwrap();
        let record_data = record.data.as_object().unwrap();
        
        assert_eq!(record_data.get("lsn").unwrap().as_str().unwrap(), lsn);
        assert_eq!(record_data.get("raw_data").unwrap().as_str().unwrap(), data);
        assert!(record_data.contains_key("error"));
        assert!(record_data.contains_key("timestamp"));
    }
    
    #[test]
    fn test_create_cdc_connector() {
        let config = json!({
            "host": "localhost",
            "database": "test_db",
            "user": "postgres",
            "password": "postgres",
            "slot_name": "custom_slot",
            "publication_name": "custom_publication",
            "decoder_plugin": "wal2json",
            "batch_size": 500
        });
        
        let connector = PostgresCDCConnector::new(config.clone());
        
        // Check extracted configuration
        assert_eq!(connector.slot_name, "custom_slot");
        assert_eq!(connector.publication_name, "custom_publication");
        assert_eq!(connector.decoder_plugin, "wal2json");
        assert_eq!(connector.batch_size, 500);
        
        // Check default values on empty config
        let connector_default = PostgresCDCConnector::new(json!({}));
        
        assert_eq!(connector_default.slot_name, "dataflare_slot");
        assert_eq!(connector_default.publication_name, "dataflare_publication");
        assert_eq!(connector_default.decoder_plugin, "pgoutput");
        assert_eq!(connector_default.batch_size, 1000);
    }
    
    #[test]
    fn test_connector_capabilities() {
        let connector = PostgresCDCConnector::new(json!({
            "host": "localhost",
            "database": "test_db"
        }));
        
        let capabilities = connector.get_capabilities();
        
        assert!(capabilities.supports_batch_operations);
        assert!(!capabilities.supports_parallel_processing); // CDC should be sequential
        assert_eq!(capabilities.preferred_batch_size, Some(1000));
        assert_eq!(capabilities.max_batch_size, Some(10000));
    }
    
    #[test]
    fn test_extraction_mode() {
        let connector = PostgresCDCConnector::new(json!({}));
        
        assert_eq!(connector.get_extraction_mode(), ExtractionMode::CDC);
    }
    
    #[test]
    fn test_metadata() {
        let connector = PostgresCDCConnector::new(json!({
            "host": "db.example.com",
            "database": "test_db",
            "slot_name": "test_slot",
            "publication_name": "test_pub"
        }));
        
        let metadata = connector.get_metadata();
        
        assert_eq!(metadata.get("connector_type").unwrap(), "postgres-cdc");
        assert_eq!(metadata.get("host").unwrap(), "db.example.com");
        assert_eq!(metadata.get("database").unwrap(), "test_db");
        assert_eq!(metadata.get("slot_name").unwrap(), "test_slot");
        assert_eq!(metadata.get("publication_name").unwrap(), "test_pub");
    }
} 