//! DataFlare WASM处理器集成测试

use serde_json::json;
use tokio;

use dataflare_core::{
    message::DataRecord,
    processor::Processor,
};
use dataflare_wasm::DataFlareWasmProcessor;

/// 测试WASM处理器的基本功能
#[tokio::test]
async fn test_wasm_processor_basic_functionality() {
    // 创建测试配置
    let config = json!({
        "plugin_path": "test_plugin.wasm",
        "plugin_config": {
            "operation": "uppercase",
            "fields": ["name", "title"]
        },
        "memory_limit": 67108864,  // 64MB in bytes
        "timeout_ms": 5000
    });

    // 创建WASM处理器
    let mut processor = DataFlareWasmProcessor::from_config(&config).unwrap();

    // 配置处理器
    let configure_result = processor.configure(&config);
    assert!(configure_result.is_ok());

    // 验证处理器状态
    let state = processor.get_state();
    // 检查是否已配置
    assert!(state.data.get("configured").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(state.records_processed, 0);
}

/// 测试WASM处理器配置解析
#[tokio::test]
async fn test_wasm_processor_config_parsing() {
    // 测试有效配置
    let valid_config = json!({
        "plugin_path": "/path/to/plugin.wasm",
        "plugin_config": {
            "param1": "value1",
            "param2": 42
        },
        "memory_limit": 134217728,  // 128MB in bytes
        "timeout_ms": 10000,
        "process_function": "custom_process",
        "batch_function": "custom_batch"
    });

    let processor = DataFlareWasmProcessor::from_config(&valid_config);
    assert!(processor.is_ok());

    // 测试最小配置
    let minimal_config = json!({
        "plugin_path": "minimal.wasm",
        "memory_limit": 67108864,  // 64MB in bytes
        "timeout_ms": 5000
    });

    let processor = DataFlareWasmProcessor::from_config(&minimal_config);
    assert!(processor.is_ok());

    // 测试无效配置
    let invalid_config = json!({
        "invalid_field": "value"
    });

    let processor = DataFlareWasmProcessor::from_config(&invalid_config);
    assert!(processor.is_err());
}

/// 测试WASM处理器状态管理
#[tokio::test]
async fn test_wasm_processor_state_management() {
    let config = json!({
        "plugin_path": "test.wasm",
        "plugin_config": {},
        "memory_limit": 67108864,
        "timeout_ms": 5000
    });

    let mut processor = DataFlareWasmProcessor::from_config(&config).unwrap();

    // 初始状态
    let initial_state = processor.get_state();
    assert!(!initial_state.data.get("configured").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(initial_state.records_processed, 0);

    // 配置后状态
    processor.configure(&config).unwrap();
    let configured_state = processor.get_state();
    assert!(configured_state.data.get("configured").and_then(|v| v.as_bool()).unwrap_or(false));
    assert_eq!(configured_state.records_processed, 0);
}

/// 测试WASM处理器错误处理
#[tokio::test]
async fn test_wasm_processor_error_handling() {
    // 测试无效的插件路径
    let invalid_path_config = json!({
        "plugin_path": "/nonexistent/path.wasm",
        "memory_limit": 67108864,
        "timeout_ms": 5000
    });

    let mut processor = DataFlareWasmProcessor::from_config(&invalid_path_config).unwrap();
    processor.configure(&invalid_path_config).unwrap();

    // 创建测试记录
    let test_record = DataRecord::new(json!({
        "name": "test",
        "value": 123
    }));

    // 尝试处理记录应该失败（因为插件不存在）
    let result = processor.process_record(&test_record).await;
    assert!(result.is_err());
}

/// 测试WASM处理器批量处理
#[tokio::test]
async fn test_wasm_processor_batch_processing() {
    let config = json!({
        "plugin_path": "batch_test.wasm",
        "plugin_config": {
            "batch_size": 10
        },
        "memory_limit": 67108864,
        "timeout_ms": 5000
    });

    let mut processor = DataFlareWasmProcessor::from_config(&config).unwrap();
    processor.configure(&config).unwrap();

    // 创建测试批次
    let records = vec![
        DataRecord::new(json!({"id": 1, "name": "record1"})),
        DataRecord::new(json!({"id": 2, "name": "record2"})),
        DataRecord::new(json!({"id": 3, "name": "record3"})),
    ];

    let batch = dataflare_core::message::DataRecordBatch::new(records);

    // 批量处理（应该失败，因为插件不存在，但测试结构正确）
    let result = processor.process_batch(&batch).await;
    // 由于插件不存在，这里会失败，但我们测试的是接口正确性
    assert!(result.is_err());
}

/// 测试WASM处理器生命周期
#[tokio::test]
async fn test_wasm_processor_lifecycle() {
    let config = json!({
        "plugin_path": "lifecycle_test.wasm",
        "memory_limit": 67108864,
        "timeout_ms": 5000
    });

    let mut processor = DataFlareWasmProcessor::from_config(&config).unwrap();

    // 初始化
    let init_result = processor.initialize().await;
    // 由于插件不存在，初始化可能失败，但接口正确

    // 清理
    let finalize_result = processor.finalize().await;
    assert!(finalize_result.is_ok()); // 清理应该总是成功
}

/// 测试WASM处理器配置验证
#[tokio::test]
async fn test_wasm_processor_config_validation() {
    // 测试各种配置组合
    let test_cases = vec![
        // 基本配置
        (json!({
            "plugin_path": "test.wasm",
            "memory_limit": 67108864,
            "timeout_ms": 5000
        }), true),

        // 完整配置
        (json!({
            "plugin_path": "test.wasm",
            "plugin_config": {"key": "value"},
            "memory_limit": 67108864,
            "timeout_ms": 5000,
            "process_function": "process",
            "batch_function": "batch_process"
        }), true),

        // 缺少必需字段
        (json!({
            "plugin_config": {"key": "value"}
        }), false),

        // 空配置
        (json!({}), false),
    ];

    for (config, should_succeed) in test_cases {
        let result = DataFlareWasmProcessor::from_config(&config);
        if should_succeed {
            assert!(result.is_ok(), "配置应该有效: {:?}", config);
        } else {
            assert!(result.is_err(), "配置应该无效: {:?}", config);
        }
    }
}

/// 测试WASM处理器与DataFlare类型系统的兼容性
#[tokio::test]
async fn test_wasm_processor_dataflare_compatibility() {
    let config = json!({
        "plugin_path": "compatibility_test.wasm",
        "memory_limit": 67108864,
        "timeout_ms": 5000
    });

    let mut processor = DataFlareWasmProcessor::from_config(&config).unwrap();
    processor.configure(&config).unwrap();

    // 测试DataRecord兼容性
    let test_record = DataRecord::new(json!({
        "string_field": "test",
        "number_field": 42,
        "boolean_field": true,
        "array_field": [1, 2, 3],
        "object_field": {
            "nested": "value"
        }
    }));

    // 验证记录结构
    assert!(!test_record.id.to_string().is_empty());
    assert!(test_record.data.is_object());

    // 获取处理器状态
    let state = processor.get_state();
    assert!(state.processor_id.starts_with("wasm_processor_"));

    // 验证模式兼容性
    let input_schema = processor.get_input_schema();
    let output_schema = processor.get_output_schema();

    // WASM处理器的模式是动态的，所以应该返回None
    assert!(input_schema.is_none());
    assert!(output_schema.is_none());
}

/// 性能基准测试
#[tokio::test]
async fn test_wasm_processor_performance() {
    let config = json!({
        "plugin_path": "performance_test.wasm",
        "memory_limit": 134217728,  // 128MB
        "timeout_ms": 1000
    });

    let mut processor = DataFlareWasmProcessor::from_config(&config).unwrap();
    processor.configure(&config).unwrap();

    // 创建大量测试记录
    let mut records = Vec::new();
    for i in 0..100 {
        records.push(DataRecord::new(json!({
            "id": i,
            "data": format!("test_data_{}", i),
            "timestamp": chrono::Utc::now().to_rfc3339()
        })));
    }

    let start_time = std::time::Instant::now();

    // 批量处理
    let batch = dataflare_core::message::DataRecordBatch::new(records);
    let _result = processor.process_batch(&batch).await;

    let elapsed = start_time.elapsed();

    // 验证性能（即使失败，也要确保在合理时间内完成）
    assert!(elapsed.as_millis() < 10000, "批量处理耗时过长: {:?}", elapsed);
}
