//! Tests for PostgreSQL batch connector implementation

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use std::sync::Arc;
    
    use serde_json::{json, Value};
    use tokio::runtime::Runtime;
    
    use dataflare_core::{
        error::Result,
        message::{DataRecord, DataRecordBatch},
        connector::{Connector, BatchSourceConnector, Position, ExtractionMode},
    };
    
    use crate::postgres::batch::PostgresBatchSourceConnector;
    
    /// 创建测试配置
    fn create_test_config() -> Value {
        json!({
            "host": "localhost",
            "port": 5432,
            "database": "test_db",
            "username": "test_user",
            "password": "test_pass",
            "table": "test_data",
            "batch_size": 1000,
            "query": "SELECT * FROM test_data"
        })
    }
    
    /// 创建模拟的批量数据
    fn create_mock_data(count: usize) -> Vec<DataRecord> {
        let mut records = Vec::with_capacity(count);
        for i in 0..count {
            records.push(DataRecord::new(json!({"id": i})));
        }
        records
    }
    
    #[test]
    fn test_batch_connector_config() {
        let config = create_test_config();
        let connector = PostgresBatchSourceConnector::new(config.clone());
        
        // 验证批处理大小
        let metadata = connector.get_metadata();
        assert_eq!(metadata.get("batch_size"), Some(&"1000".to_string()));
        
        // 验证提取模式
        assert_eq!(connector.get_extraction_mode(), ExtractionMode::Full);
        
        // 验证连接器类型
        assert_eq!(connector.connector_type(), "postgres-batch");
        
        // 验证连接器能力
        let capabilities = connector.get_capabilities();
        assert!(capabilities.supports_batch_operations);
        assert_eq!(capabilities.preferred_batch_size, Some(1000));
    }
    
    #[test]
    fn test_batch_connector_position() {
        let config = create_test_config();
        let mut connector = PostgresBatchSourceConnector::new(config);
        
        // 创建测试位置
        let position = Position::new()
            .with_data("cursor", "100")
            .with_data("timestamp", "2023-01-01T00:00:00Z");
        
        // 测试位置操作
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // 设置位置
            connector.seek(position.clone()).await.unwrap();
            
            // 获取位置
            let retrieved = connector.get_position().unwrap();
            
            // 验证位置数据
            assert_eq!(retrieved.get_data("cursor"), Some(&"100".to_string()));
            assert_eq!(retrieved.get_data("timestamp"), Some(&"2023-01-01T00:00:00Z".to_string()));
        });
    }
    
    #[test]
    fn test_batch_connector_metadata() {
        let config = create_test_config();
        let connector = PostgresBatchSourceConnector::new(config);
        
        // 获取元数据
        let metadata = connector.get_metadata();
        
        // 验证元数据
        assert_eq!(metadata.get("connector_type"), Some(&"postgres-batch".to_string()));
        assert_eq!(metadata.get("batch_size"), Some(&"1000".to_string()));
        assert!(metadata.contains_key("host"));
        assert!(metadata.contains_key("database"));
    }
    
    #[test]
    fn test_batch_size_estimation() {
        // 创建不同大小的批次
        let small_batch = DataRecordBatch::new(create_mock_data(100));
        let medium_batch = DataRecordBatch::new(create_mock_data(1000));
        let large_batch = DataRecordBatch::new(create_mock_data(10000));
        
        // 使用估算器估算大小
        let small_size = crate::util::estimate_batch_size(&small_batch);
        let medium_size = crate::util::estimate_batch_size(&medium_batch);
        let large_size = crate::util::estimate_batch_size(&large_batch);
        
        // 验证估算大小与记录数成比例
        assert!(small_size < medium_size);
        assert!(medium_size < large_size);
        assert!(medium_size > small_size * 5); // 大约10倍关系
    }
    
    #[test]
    fn test_batch_slicing() {
        // 创建原始批次
        let original_batch = DataRecordBatch::new(create_mock_data(1000));
        
        // 获取切片
        let slice = crate::util::slice_batch(&original_batch, 200, 500);
        
        // 验证切片大小
        assert_eq!(slice.records.len(), 300);
        
        // 验证切片内容
        assert_eq!(slice.records[0].as_i64("id").unwrap(), 200);
        assert_eq!(slice.records[299].as_i64("id").unwrap(), 499);
        
        // 验证切片保留了原始批次的元数据
        assert_eq!(slice.metadata, original_batch.metadata);
    }
    
    // 集成测试需要实际数据库连接，这里添加标记，以便可以选择性运行
    #[test]
    #[ignore]
    fn test_batch_read_performance() {
        let config = create_test_config();
        let mut connector = PostgresBatchSourceConnector::new(config);
        
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // 测量不同批大小的性能
            let test_sizes = [100, 1000, 10000];
            
            for size in &test_sizes {
                // 设置批大小
                let mut config = create_test_config();
                config["batch_size"] = json!(*size);
                connector.configure(&config).unwrap();
                
                // 测量读取时间
                let start_time = Instant::now();
                let result = connector.read_batch(*size).await;
                let elapsed = start_time.elapsed();
                
                if let Ok(batch) = result {
                    println!("批大小 {}: 读取 {} 条记录, 耗时 {:?}, 每秒处理 {:.2} 条记录",
                        size, 
                        batch.records.len(), 
                        elapsed, 
                        batch.records.len() as f64 / elapsed.as_secs_f64()
                    );
                } else {
                    println!("读取失败: {:?}", result.err());
                }
            }
        });
    }
    
    // 仅当存在PostgreSQL逻辑复制设置时运行此测试
    #[test]
    #[ignore]
    fn test_batch_cdc_connector() {
        let config = json!({
            "host": "localhost",
            "port": 5432,
            "database": "test_db",
            "username": "test_user",
            "password": "test_pass",
            "table": "test_data",
            "slot_name": "dataflare_test_slot",
            "publication_name": "dataflare_test_pub",
            "decoder_plugin": "pgoutput",
            "batch_size": 100
        });
        
        let mut connector = crate::postgres::cdc::PostgresCDCConnector::new(config);
        
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // 尝试读取变更事件
            match connector.read_batch(100).await {
                Ok(batch) => {
                    println!("CDC 批处理: 读取 {} 条变更记录", batch.records.len());
                    
                    // 如果有记录，输出第一条以进行检查
                    if !batch.records.is_empty() {
                        println!("示例记录: {:?}", batch.records[0]);
                    }
                    
                    // 提交位置
                    let position = connector.get_position().unwrap();
                    connector.commit(position).await.unwrap();
                },
                Err(e) => {
                    println!("CDC 读取失败: {:?}", e);
                }
            }
        });
    }
    
    // 测试边缘情况
    #[test]
    fn test_batch_edge_cases() {
        let config = create_test_config();
        let mut connector = PostgresBatchSourceConnector::new(config);
        
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // 测试读取空批次
            let mut zero_config = create_test_config();
            zero_config["batch_size"] = json!(0);
            connector.configure(&zero_config).unwrap();
            
            let result = connector.read_batch(0).await;
            assert!(result.is_ok(), "应该能处理零大小的批处理请求");
            assert_eq!(result.unwrap().records.len(), 0, "空批处理应该返回零记录");
            
            // 测试极大批大小
            let mut large_config = create_test_config();
            large_config["batch_size"] = json!(usize::MAX / 2);
            connector.configure(&large_config).unwrap();
            
            let result = connector.read_batch(usize::MAX).await;
            // 这个测试应该能处理或至少优雅地失败
            if result.is_ok() {
                println!("能够处理极大批处理请求");
            } else {
                println!("极大批处理请求优雅失败: {:?}", result.err());
            }
        });
    }
} 