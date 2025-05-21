//! MongoDB Connector for DataFlare
//!
//! Provides functionality to connect to MongoDB databases.

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use log::error;
use mongodb::bson::{doc};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::DataRecord,
    model::Schema,
    state::SourceState,
    model::Field,
    model::DataType,
};

use crate::source::{SourceConnector, ExtractionMode};

#[cfg(test)]
mod tests;

/// MongoDB Source Connector
pub struct MongoDBSourceConnector {
    /// Configuration
    config: Value,
    /// MongoDB Client
    client: Option<mongodb::Client>,
    /// Current state
    state: SourceState,
    /// Data schema
    schema: Schema,
    /// Extraction mode
    extraction_mode: ExtractionMode,
}

impl MongoDBSourceConnector {
    /// Create a new MongoDB connector
    pub fn new(config: Value) -> Self {
        Self {
            config,
            client: None,
            state: SourceState::new("mongodb"),
            schema: Schema::new(),
            extraction_mode: ExtractionMode::Full,
        }
    }

    /// Set extraction mode
    pub fn with_extraction_mode(mut self, mode: ExtractionMode) -> Self {
        self.extraction_mode = mode;
        self
    }

    /// Connect to MongoDB
    async fn connect(&mut self) -> Result<()> {
        // Extract connection parameters
        let connection_string = self.config.get("connection_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("'connection_string' parameter is required".to_string()))?;

        // Connect to MongoDB
        let client = mongodb::Client::with_uri_str(connection_string)
            .await
            .map_err(|e| DataFlareError::Connection(format!("Failed to connect to MongoDB: {}", e)))?;

        // Test connection
        client.list_database_names(None, None)
            .await
            .map_err(|e| DataFlareError::Connection(format!("Failed to list MongoDB databases: {}", e)))?;
        
        self.client = Some(client);
        Ok(())
    }

    /// Convert a MongoDB document to a data record
    fn document_to_record(&self, doc: mongodb::bson::Document) -> Result<DataRecord> {
        // Convert BSON to JSON
        let json_value: Value = match mongodb::bson::to_document(&doc) {
            Ok(document) => {
                let json_doc = mongodb::bson::to_raw_document_buf(&document)
                    .map_err(|e| DataFlareError::Serialization(format!("Failed to convert BSON to raw document: {}", e)))?;
                
                serde_json::from_slice(json_doc.as_bytes())
                    .map_err(|e| DataFlareError::Serialization(format!("Failed to parse JSON from BSON: {}", e)))?
            },
            Err(e) => {
                return Err(DataFlareError::Serialization(format!("Failed to convert document to BSON: {}", e)));
            }
        };

        Ok(DataRecord::new(json_value))
    }
}

#[async_trait]
impl SourceConnector for MongoDBSourceConnector {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = config.clone();
        Ok(())
    }

    async fn check_connection(&self) -> Result<bool> {
        // Create a temporary client just for testing connection
        let connection_string = self.config.get("connection_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("'connection_string' parameter is required".to_string()))?;

        let client = mongodb::Client::with_uri_str(connection_string)
            .await
            .map_err(|e| DataFlareError::Connection(format!("Failed to connect to MongoDB: {}", e)))?;

        // Test connection
        client.list_database_names(None, None)
            .await
            .map_err(|e| DataFlareError::Connection(format!("Failed to list MongoDB databases: {}", e)))?;

        Ok(true)
    }

    async fn discover_schema(&self) -> Result<Schema> {
        let mut schema = Schema::new();
        
        // MongoDB schemas are dynamic, so we will sample some documents to infer schema
        let db_name = self.config.get("database")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("'database' parameter is required".to_string()))?;
            
        let collection_name = self.config.get("collection")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("'collection' parameter is required".to_string()))?;
            
        let sample_size = self.config.get("sample_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;
            
        // Create a temporary client if needed
        let client = if let Some(client) = &self.client {
            client.clone()
        } else {
            let connection_string = self.config.get("connection_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| DataFlareError::Config("'connection_string' parameter is required".to_string()))?;

            mongodb::Client::with_uri_str(connection_string)
                .await
                .map_err(|e| DataFlareError::Connection(format!("Failed to connect to MongoDB: {}", e)))?
        };
        
        let db = client.database(db_name);
        let collection = db.collection::<mongodb::bson::Document>(collection_name);
        
        // Create a sample aggregation to get random documents
        let pipeline = vec![
            doc! { "$sample": { "size": sample_size as i32 } }
        ];
        
        let mut cursor = collection.aggregate(pipeline, None)
            .await
            .map_err(|e| DataFlareError::Query(format!("Failed to sample documents: {}", e)))?;
            
        // Process sample documents to infer schema
        let mut field_types: std::collections::HashMap<String, Vec<DataType>> = std::collections::HashMap::new();
        
        while let Some(result) = futures::StreamExt::next(&mut cursor).await {
            match result {
                Ok(doc) => {
                    self.process_document_for_schema(&doc, "", &mut field_types);
                },
                Err(e) => {
                    error!("Error fetching document for schema discovery: {}", e);
                }
            }
        }
        
        // Convert field types to schema
        for (field_path, types) in field_types {
            // Determine common type across samples
            let data_type = if types.contains(&DataType::Object) {
                DataType::Object
            } else if types.iter().any(|t| matches!(t, DataType::Array(_))) {
                DataType::Array(Box::new(DataType::String))
            } else if types.contains(&DataType::String) {
                DataType::String
            } else if types.contains(&DataType::Int64) || types.contains(&DataType::Float64) {
                DataType::Float64
            } else if types.contains(&DataType::Boolean) {
                DataType::Boolean
            } else if types.contains(&DataType::Timestamp) {
                DataType::Timestamp
            } else {
                DataType::String // 默认为字符串类型
            };
            
            schema.add_field(Field::new(field_path.clone(), data_type));
        }
        
        Ok(schema)
    }

    async fn read(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin + 'static>> {
        // Ensure client is connected
        if self.client.is_none() {
            self.connect().await?;
        }
        
        let client = self.client.as_ref().ok_or_else(|| {
            DataFlareError::Connection("Failed to connect to MongoDB".to_string())
        })?;
        
        let db_name = self.config.get("database")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("'database' parameter is required".to_string()))?;
            
        let collection_name = self.config.get("collection")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("'collection' parameter is required".to_string()))?;
            
        let db = client.database(db_name);
        let collection = db.collection::<mongodb::bson::Document>(collection_name);
        
        // Build query based on state (if incremental)
        let filter = if let Some(state) = state {
            if let Some(last_id) = state.data.get("last_id").and_then(|v| v.as_str()) {
                // Use ObjectId for filtering if possible
                match mongodb::bson::oid::ObjectId::parse_str(last_id) {
                    Ok(object_id) => {
                        doc! { "_id": { "$gt": object_id } }
                    },
                    Err(_) => {
                        // Fall back to string comparison
                        doc! { "_id": { "$gt": last_id } }
                    }
                }
            } else {
                doc! {}
            }
        } else {
            doc! {}
        };
        
        // Set up sort order
        let sort = doc! { "_id": 1 };
        
        // Get batch size from config
        let batch_size = self.config.get("batch_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as u32;
            
        // Create cursor
        let find_options = mongodb::options::FindOptions::builder()
            .sort(sort)
            .batch_size(batch_size)
            .build();
            
        let cursor = collection.find(filter, find_options)
            .await
            .map_err(|e| DataFlareError::Query(format!("Failed to execute MongoDB query: {}", e)))?;
            
        // Convert to DataFlare stream
        let stream = futures::StreamExt::map(cursor, move |result| {
            match result {
                Ok(doc) => {
                    // 克隆所需的数据以满足'static生命周期需求
                    let self_clone = MongoDBSourceConnector {
                        config: Value::Null, // 不需要克隆完整配置
                        client: None,        // 不需要克隆客户端
                        state: SourceState::new("mongodb"),
                        schema: Schema::new(),
                        extraction_mode: ExtractionMode::Full.clone(),
                    };
                    
                    match self_clone.document_to_record(doc) {
                        Ok(record) => Ok(record),
                        Err(e) => Err(e),
                    }
                },
                Err(e) => Err(DataFlareError::Extraction(format!("Failed to read MongoDB document: {}", e))),
            }
        });
        
        Ok(Box::new(stream))
    }

    fn get_state(&self) -> Result<SourceState> {
        Ok(self.state.clone())
    }

    fn get_extraction_mode(&self) -> ExtractionMode {
        self.extraction_mode.clone()
    }

    async fn estimate_record_count(&self, _state: Option<SourceState>) -> Result<u64> {
        if self.client.is_none() {
            return Err(DataFlareError::Connection("Not connected to MongoDB".to_string()));
        }
        
        let client = self.client.as_ref().unwrap();
        let db_name = self.config.get("database")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("'database' parameter is required".to_string()))?;
            
        let collection_name = self.config.get("collection")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("'collection' parameter is required".to_string()))?;
            
        let db = client.database(db_name);
        let collection = db.collection::<mongodb::bson::Document>(collection_name);
        
        // Count documents
        let count = collection.count_documents(None, None)
            .await
            .map_err(|e| DataFlareError::Query(format!("Failed to count MongoDB documents: {}", e)))?;
            
        Ok(count)
    }
}

impl Clone for MongoDBSourceConnector {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            client: None, // Client cannot be cloned, needs to be recreated
            state: self.state.clone(),
            schema: self.schema.clone(),
            extraction_mode: self.extraction_mode.clone(),
        }
    }
}

// Helper methods
impl MongoDBSourceConnector {
    fn process_document_for_schema(&self, 
        doc: &mongodb::bson::Document, 
        prefix: &str, 
        field_types: &mut std::collections::HashMap<String, Vec<DataType>>) {
        
        for (key, value) in doc {
            let field_path = if prefix.is_empty() {
                key.to_string()
            } else {
                format!("{}.{}", prefix, key)
            };
            
            match value {
                mongodb::bson::Bson::Document(subdoc) => {
                    // Add object type and process subdocument
                    field_types.entry(field_path.clone())
                        .or_default()
                        .push(DataType::Object);
                    
                    self.process_document_for_schema(subdoc, &field_path, field_types);
                },
                mongodb::bson::Bson::Array(array) => {
                    // Add array type
                    field_types.entry(field_path.clone())
                        .or_default()
                        .push(DataType::Array(Box::new(DataType::String)));
                    
                    // Process array elements (sample first few elements if there are many)
                    for (i, element) in array.iter().take(5).enumerate() {
                        if let mongodb::bson::Bson::Document(subdoc) = element {
                            let array_path = format!("{}[{}]", field_path, i);
                            self.process_document_for_schema(subdoc, &array_path, field_types);
                        }
                    }
                },
                mongodb::bson::Bson::String(_) => {
                    field_types.entry(field_path)
                        .or_default()
                        .push(DataType::String);
                },
                mongodb::bson::Bson::ObjectId(_) => {
                    field_types.entry(field_path)
                        .or_default()
                        .push(DataType::String);
                },
                mongodb::bson::Bson::Int32(_) => {
                    field_types.entry(field_path)
                        .or_default()
                        .push(DataType::Int32);
                },
                mongodb::bson::Bson::Int64(_) => {
                    field_types.entry(field_path)
                        .or_default()
                        .push(DataType::Int64);
                },
                mongodb::bson::Bson::Double(_) => {
                    field_types.entry(field_path)
                        .or_default()
                        .push(DataType::Float64);
                },
                mongodb::bson::Bson::Decimal128(_) => {
                    field_types.entry(field_path)
                        .or_default()
                        .push(DataType::Float64);
                },
                mongodb::bson::Bson::Boolean(_) => {
                    field_types.entry(field_path)
                        .or_default()
                        .push(DataType::Boolean);
                },
                mongodb::bson::Bson::DateTime(_) => {
                    field_types.entry(field_path)
                        .or_default()
                        .push(DataType::Timestamp);
                },
                mongodb::bson::Bson::Null => {
                    // Skip null values for schema inference
                },
                _ => {
                    // Handle other BSON types
                    field_types.entry(field_path)
                        .or_default()
                        .push(DataType::String); // Default to string for unknown types
                }
            }
        }
    }
}

/// Register MongoDB connector
pub fn register_mongodb_connector() {
    use std::sync::Arc;
    
    crate::registry::register_connector::<dyn crate::source::SourceConnector>("mongodb", 
        Arc::new(|config| -> Result<Box<dyn crate::source::SourceConnector>> {
            Ok(Box::new(MongoDBSourceConnector::new(config)))
        })
    );
} 