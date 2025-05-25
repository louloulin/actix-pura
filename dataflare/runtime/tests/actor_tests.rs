//! Actor通信单元测试
//!
//! 测试DataFlare运行时中各个Actor之间的通信

use actix::prelude::*;
use serde_json::json;
use dataflare_runtime::actor::{
    source::SourceActor,
    processor::ProcessorActor,
    destination::DestinationActor,
    Initialize,
    SendBatch,
};
use dataflare_core::{
    message::{DataRecord, DataRecordBatch},
    model::{Schema, Field, DataType},
};
use dataflare_connector::csv::{CsvSourceConnector, CsvDestinationConnector};
use dataflare_processor::mapping::{MappingProcessor, MappingProcessorConfig, FieldMapping};
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;

#[actix::test]
async fn test_source_actor_initialization() {
    // 创建CSV源连接器
    let config = json!({
        "file_path": "test.csv",
        "has_header": true,
        "delimiter": ","
    });
    let connector = CsvSourceConnector::new(config);

    // 创建源Actor
    let source_actor = SourceActor::new("test_source".to_string(), Box::new(connector)).start();

    // 创建初始化消息
    let init_msg = Initialize {
        workflow_id: "test_workflow".to_string(),
        config: json!({
            "file_path": "test.csv",
            "has_header": true,
            "delimiter": ","
        }),
    };

    // 发送初始化消息
    let result = source_actor.send(init_msg).await;

    // 验证结果
    assert!(result.is_ok(), "Actor邮箱通信失败: {:?}", result.err());

    let init_result = result.unwrap();
    // 注意：由于文件不存在，初始化可能失败，但这是预期的
    // 我们主要测试Actor通信是否正常
    println!("源Actor初始化结果: {:?}", init_result);
}

#[actix::test]
async fn test_processor_actor_initialization() {
    // 创建映射处理器
    let config = MappingProcessorConfig {
        mappings: vec![
            FieldMapping {
                source: "id".to_string(),
                destination: "user_id".to_string(),
                transform: None,
            }
        ],
    };
    let processor = MappingProcessor::new(config);

    let processor_actor = ProcessorActor::new("test_processor".to_string(), Box::new(processor)).start();

    let init_msg = Initialize {
        workflow_id: "test_workflow".to_string(),
        config: json!({
            "mappings": [
                {
                    "source": "id",
                    "destination": "user_id"
                }
            ]
        }),
    };

    let result = processor_actor.send(init_msg).await;
    assert!(result.is_ok(), "处理器Actor邮箱通信失败: {:?}", result.err());

    let init_result = result.unwrap();
    assert!(init_result.is_ok(), "处理器Actor初始化失败: {:?}", init_result.err());
}

#[actix::test]
async fn test_destination_actor_initialization() {
    // 创建CSV目标连接器
    let config = json!({
        "file_path": "output.csv",
        "delimiter": ",",
        "write_header": true
    });
    let connector = CsvDestinationConnector::new(config);

    let dest_actor = DestinationActor::new("test_dest".to_string(), Box::new(connector)).start();

    let init_msg = Initialize {
        workflow_id: "test_workflow".to_string(),
        config: json!({
            "file_path": "output.csv",
            "delimiter": ",",
            "write_header": true
        }),
    };

    let result = dest_actor.send(init_msg).await;
    assert!(result.is_ok(), "目标Actor邮箱通信失败: {:?}", result.err());

    let init_result = result.unwrap();
    // 目标Actor初始化通常会成功，因为它只是准备写入
    println!("目标Actor初始化结果: {:?}", init_result);
}

#[actix::test]
async fn test_processor_actor_batch_processing() {
    // 创建映射处理器
    let config = MappingProcessorConfig {
        mappings: vec![
            FieldMapping {
                source: "id".to_string(),
                destination: "user_id".to_string(),
                transform: None,
            },
            FieldMapping {
                source: "name".to_string(),
                destination: "full_name".to_string(),
                transform: None,
            }
        ],
    };
    let processor = MappingProcessor::new(config);
    let processor_actor = ProcessorActor::new("test_processor".to_string(), Box::new(processor)).start();

    // 初始化处理器
    let init_msg = Initialize {
        workflow_id: "test_workflow".to_string(),
        config: json!({
            "mappings": [
                {
                    "source": "id",
                    "destination": "user_id"
                },
                {
                    "source": "name",
                    "destination": "full_name"
                }
            ]
        }),
    };

    let init_result = processor_actor.send(init_msg).await;
    assert!(init_result.is_ok() && init_result.unwrap().is_ok(), "处理器初始化失败");

    // 创建测试数据批次
    let mut schema = Schema::new();
    schema.fields.push(Field {
        name: "id".to_string(),
        data_type: DataType::String,
        nullable: false,
        description: None,
        metadata: HashMap::new(),
    });
    schema.fields.push(Field {
        name: "name".to_string(),
        data_type: DataType::String,
        nullable: false,
        description: None,
        metadata: HashMap::new(),
    });

    let mut records = Vec::new();
    records.push(DataRecord::new(json!({"id": "1", "name": "Alice"})));
    records.push(DataRecord::new(json!({"id": "2", "name": "Bob"})));

    let batch = DataRecordBatch::with_schema(records, schema);

    // 发送批次处理消息
    let send_msg = SendBatch {
        workflow_id: "test_workflow".to_string(),
        batch,
        is_last_batch: true,
    };

    let result = processor_actor.send(send_msg).await;
    assert!(result.is_ok(), "批次发送失败: {:?}", result.err());

    let process_result = result.unwrap();
    assert!(process_result.is_ok(), "批次处理失败: {:?}", process_result.err());
}
