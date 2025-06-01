use crate::mongodb::MongoDBSourceConnector;
use crate::source::{SourceConnector, ExtractionMode};
use dataflare_core::error::{Result, DataFlareError};
use dataflare_core::message::DataRecord;
use dataflare_core::model::Schema;
use dataflare_core::state::SourceState;
use futures::StreamExt;
use serde_json::{json, Value};
use mongodb::bson::{self, doc, Document};

// 假设本地有一个测试MongoDB服务器
// 如果不可用，这些测试将失败
const TEST_MONGODB_URI: &str = "mongodb://localhost:27017";
const TEST_DATABASE: &str = "dataflare_test";
const TEST_COLLECTION: &str = "test_documents";

// 辅助函数 - 设置测试数据
async fn setup_test_data() -> Result<()> {
    let client = mongodb::Client::with_uri_str(TEST_MONGODB_URI).await
        .map_err(|e| DataFlareError::Connection(format!("Failed to connect: {}", e)))?;
    
    let db = client.database(TEST_DATABASE);
    let collection = db.collection::<mongodb::bson::Document>(TEST_COLLECTION);
    
    // 清除任何现有数据
    collection.drop(None).await.ok();
    
    // 创建测试集合
    let test_docs = vec![
        doc! {
            "name": "Document 1",
            "value": 100,
            "tags": ["tag1", "tag2"],
            "metadata": {
                "created_at": chrono::Utc::now(),
                "author": "test_user"
            }
        },
        doc! {
            "name": "Document 2",
            "value": 200,
            "tags": ["tag2", "tag3"],
            "metadata": {
                "created_at": chrono::Utc::now(),
                "author": "test_user"
            }
        },
        doc! {
            "name": "Document 3",
            "value": 300,
            "tags": ["tag1", "tag3"],
            "metadata": {
                "created_at": chrono::Utc::now(),
                "author": "different_user"
            }
        }
    ];
    
    collection.insert_many(test_docs, None).await
        .map_err(|e| DataFlareError::Extraction(format!("Failed to insert test data: {}", e)))?;
    
    Ok(())
}

#[actix::test]
async fn test_mongodb_connection() {
    let config = json!({
        "connection_string": TEST_MONGODB_URI,
        "database": TEST_DATABASE,
        "collection": TEST_COLLECTION
    });
    
    let connector = MongoDBSourceConnector::new(config);
    
    // 测试连接检查
    let connection_result = connector.check_connection().await;
    assert!(connection_result.is_ok());
    assert!(connection_result.unwrap());
}

#[actix::test]
async fn test_mongodb_schema_discovery() -> Result<()> {
    // 设置测试数据
    setup_test_data().await?;
    
    let config = json!({
        "connection_string": TEST_MONGODB_URI,
        "database": TEST_DATABASE,
        "collection": TEST_COLLECTION,
        "sample_size": 10
    });
    
    let connector = MongoDBSourceConnector::new(config);
    
    // 发现模式
    let schema = connector.discover_schema().await?;
    
    // 验证发现的模式
    assert!(schema.has_field("name"));
    assert!(schema.has_field("value"));
    assert!(schema.has_field("tags"));
    assert!(schema.has_field("metadata"));
    assert!(schema.has_field("metadata.created_at"));
    assert!(schema.has_field("metadata.author"));
    
    Ok(())
}

#[actix::test]
async fn test_mongodb_data_extraction() -> Result<()> {
    // 设置测试数据
    setup_test_data().await?;
    
    let config = json!({
        "connection_string": TEST_MONGODB_URI,
        "database": TEST_DATABASE,
        "collection": TEST_COLLECTION,
        "batch_size": 10
    });
    
    let mut connector = MongoDBSourceConnector::new(config);
    
    // 获取数据记录
    let mut stream = connector.read(None).await?;
    
    let mut record_count = 0;
    while let Some(record_result) = stream.next().await {
        let record = record_result?;
        // 直接访问记录的data字段
        let data = &record.data;
        
        // 验证记录包含预期的字段
        assert!(data.get("name").is_some());
        assert!(data.get("value").is_some());
        assert!(data.get("tags").is_some());
        assert!(data.get("metadata").is_some());
        
        record_count += 1;
    }
    
    // 验证我们读取了所有3条记录
    assert_eq!(record_count, 3);
    
    Ok(())
}

#[actix::test]
async fn test_mongodb_incremental_extraction() -> Result<()> {
    // 设置测试数据
    setup_test_data().await?;
    
    let config = json!({
        "connection_string": TEST_MONGODB_URI,
        "database": TEST_DATABASE,
        "collection": TEST_COLLECTION
    });
    
    let mut connector = MongoDBSourceConnector::new(config)
        .with_extraction_mode(ExtractionMode::Incremental);
    
    // 首先读取所有记录
    let mut stream = connector.read(None).await?;
    let mut last_id = None;
    
    let mut record_count = 0;
    while let Some(record_result) = stream.next().await {
        let record = record_result?;
        let data = &record.data;
        
        // 保存最后一个ID用于增量查询
        if let Some(id) = data.get("_id").and_then(|v| v.get("$oid")).and_then(|v| v.as_str()) {
            last_id = Some(id.to_string());
        }
        
        record_count += 1;
    }
    
    assert_eq!(record_count, 3);
    
    // 创建带有最后ID的状态
    let mut state = SourceState::new("mongodb");
    if let Some(id) = last_id {
        state.add_data("last_id", id);
    }
    
    // 现在添加一个新文档
    let client = mongodb::Client::with_uri_str(TEST_MONGODB_URI).await
        .map_err(|e| DataFlareError::Connection(format!("Failed to connect to MongoDB: {}", e)))?;
    let db = client.database(TEST_DATABASE);
    let collection = db.collection::<mongodb::bson::Document>(TEST_COLLECTION);
    
    let new_doc = doc! {
        "name": "Document 4",
        "value": 400,
        "tags": ["tag4"],
        "metadata": {
            "created_at": chrono::Utc::now(),
            "author": "new_user"
        }
    };
    
    collection.insert_one(new_doc, None).await
        .map_err(|e| DataFlareError::Extraction(format!("Failed to insert new test document: {}", e)))?;
    
    // 使用状态再次读取，应该只获取新添加的文档
    let mut stream = connector.read(Some(state)).await?;
    
    let mut record_count = 0;
    while let Some(record_result) = stream.next().await {
        let record = record_result?;
        let data = &record.data;
        
        // 验证这是新添加的文档
        if let Some(name) = data.get("name").and_then(|v| v.as_str()) {
            assert_eq!(name, "Document 4");
        } else {
            panic!("Expected 'name' field not found in record");
        }
        
        record_count += 1;
    }
    
    // 应该只读取到一条新记录
    assert_eq!(record_count, 1);
    
    Ok(())
}

#[actix::test]
async fn test_mongodb_record_count() -> Result<()> {
    // 设置测试数据
    setup_test_data().await?;
    
    let config = json!({
        "connection_string": TEST_MONGODB_URI,
        "database": TEST_DATABASE,
        "collection": TEST_COLLECTION
    });
    
    let mut connector = MongoDBSourceConnector::new(config);
    
    // 确保连接
    connector.connect().await?;
    
    // 估计记录数
    let count = connector.estimate_record_count(None).await?;
    assert_eq!(count, 3);
    
    Ok(())
} 