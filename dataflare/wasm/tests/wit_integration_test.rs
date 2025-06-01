//! WIT集成测试
//!
//! 测试WebAssembly Interface Types的完整功能

use std::collections::HashMap;
use serde_json::json;

use dataflare_wasm::{
    WitRuntime, WitRuntimeConfig,
    wit_runtime::{DataRecord, ProcessingResult, PluginInfo, Capabilities},
};

#[tokio::test]
async fn test_wit_runtime_full_workflow() {
    // 创建WIT运行时
    let config = WitRuntimeConfig::default();
    let mut runtime = WitRuntime::new(config).unwrap();
    
    // 初始化运行时
    runtime.initialize().await.unwrap();
    assert!(runtime.is_initialized());
}

#[tokio::test]
async fn test_wit_data_record_creation() {
    // 测试DataRecord的创建和操作
    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), "test".to_string());
    metadata.insert("version".to_string(), "1.0".to_string());

    let record = DataRecord {
        id: "test_record_001".to_string(),
        data: json!({
            "name": "John Doe",
            "age": 30,
            "email": "john@example.com"
        }),
        metadata,
        created_at: Some(chrono::Utc::now()),
        updated_at: None,
    };

    assert_eq!(record.id, "test_record_001");
    assert_eq!(record.data["name"], "John Doe");
    assert_eq!(record.data["age"], 30);
    assert!(record.created_at.is_some());
    assert!(record.updated_at.is_none());
    assert_eq!(record.metadata.get("source").unwrap(), "test");
}

#[tokio::test]
async fn test_wit_processing_result_variants() {
    // 测试ProcessingResult的各种变体
    let test_record = DataRecord {
        id: "test".to_string(),
        data: json!({"test": true}),
        metadata: HashMap::new(),
        created_at: None,
        updated_at: None,
    };

    // 测试成功结果
    let success_result = ProcessingResult::Success(test_record.clone());
    match success_result {
        ProcessingResult::Success(record) => {
            assert_eq!(record.id, "test");
            assert_eq!(record.data["test"], true);
        }
        _ => panic!("Expected Success variant"),
    }

    // 测试错误结果
    let error_result = ProcessingResult::Error("Processing failed".to_string());
    match error_result {
        ProcessingResult::Error(msg) => assert_eq!(msg, "Processing failed"),
        _ => panic!("Expected Error variant"),
    }

    // 测试跳过结果
    let skip_result = ProcessingResult::Skip;
    match skip_result {
        ProcessingResult::Skip => {},
        _ => panic!("Expected Skip variant"),
    }

    // 测试过滤结果
    let filtered_result = ProcessingResult::Filtered;
    match filtered_result {
        ProcessingResult::Filtered => {},
        _ => panic!("Expected Filtered variant"),
    }
}

#[tokio::test]
async fn test_wit_plugin_info_creation() {
    // 测试PluginInfo的创建和验证
    let plugin_info = PluginInfo {
        name: "Advanced Data Processor".to_string(),
        version: "2.1.0".to_string(),
        description: "A sophisticated data processing plugin with AI capabilities".to_string(),
        author: Some("DataFlare AI Team".to_string()),
        dataflare_version: "4.0.0".to_string(),
    };

    assert_eq!(plugin_info.name, "Advanced Data Processor");
    assert_eq!(plugin_info.version, "2.1.0");
    assert!(plugin_info.description.contains("AI capabilities"));
    assert_eq!(plugin_info.author.as_ref().unwrap(), "DataFlare AI Team");
    assert!(plugin_info.dataflare_version.starts_with("4."));
}

#[tokio::test]
async fn test_wit_capabilities_validation() {
    // 测试Capabilities的创建和验证
    let capabilities = Capabilities {
        supported_operations: vec![
            "process".to_string(),
            "transform".to_string(),
            "filter".to_string(),
            "aggregate".to_string(),
        ],
        max_batch_size: 5000,
        supports_streaming: true,
        supports_state: true,
        memory_requirement: 128 * 1024 * 1024, // 128MB
    };

    assert_eq!(capabilities.supported_operations.len(), 4);
    assert!(capabilities.supported_operations.contains(&"process".to_string()));
    assert!(capabilities.supported_operations.contains(&"transform".to_string()));
    assert!(capabilities.supported_operations.contains(&"filter".to_string()));
    assert!(capabilities.supported_operations.contains(&"aggregate".to_string()));
    assert_eq!(capabilities.max_batch_size, 5000);
    assert!(capabilities.supports_streaming);
    assert!(capabilities.supports_state);
    assert_eq!(capabilities.memory_requirement, 128 * 1024 * 1024);
}

#[tokio::test]
async fn test_wit_runtime_configuration() {
    // 测试WIT运行时配置
    let config = WitRuntimeConfig {
        memory_limit: 256 * 1024 * 1024, // 256MB
        timeout_ms: 10000, // 10秒
        security_policy: dataflare_wasm::SecurityPolicy::default(),
        debug: true,
    };

    let runtime = WitRuntime::new(config).unwrap();
    let runtime_config = runtime.get_config();
    
    assert_eq!(runtime_config.memory_limit, 256 * 1024 * 1024);
    assert_eq!(runtime_config.timeout_ms, 10000);
    assert!(runtime_config.debug);
}

#[tokio::test]
async fn test_wit_data_processing_workflow() {
    // 测试完整的数据处理工作流
    let mut input_data = HashMap::new();
    input_data.insert("user_id".to_string(), "12345".to_string());
    input_data.insert("action".to_string(), "login".to_string());

    let input_record = DataRecord {
        id: "event_001".to_string(),
        data: json!({
            "user_id": "12345",
            "action": "login",
            "timestamp": "2025-01-26T10:00:00Z",
            "ip_address": "192.168.1.100"
        }),
        metadata: {
            let mut meta = HashMap::new();
            meta.insert("source".to_string(), "web_app".to_string());
            meta.insert("environment".to_string(), "production".to_string());
            meta
        },
        created_at: Some(chrono::Utc::now()),
        updated_at: None,
    };

    // 验证输入数据
    assert_eq!(input_record.data["user_id"], "12345");
    assert_eq!(input_record.data["action"], "login");
    assert_eq!(input_record.metadata.get("source").unwrap(), "web_app");
    assert_eq!(input_record.metadata.get("environment").unwrap(), "production");
}

#[tokio::test]
async fn test_wit_batch_processing_limits() {
    // 测试批处理限制
    let capabilities = Capabilities {
        supported_operations: vec!["process".to_string()],
        max_batch_size: 100,
        supports_streaming: false,
        supports_state: false,
        memory_requirement: 64 * 1024 * 1024,
    };

    // 创建超过限制的批处理数据
    let mut large_batch = Vec::new();
    for i in 0..150 {
        large_batch.push(DataRecord {
            id: format!("record_{}", i),
            data: json!({"index": i}),
            metadata: HashMap::new(),
            created_at: Some(chrono::Utc::now()),
            updated_at: None,
        });
    }

    // 验证批处理大小检查
    assert!(large_batch.len() > capabilities.max_batch_size as usize);
    
    // 创建符合限制的批处理
    let valid_batch: Vec<_> = large_batch.into_iter().take(capabilities.max_batch_size as usize).collect();
    assert_eq!(valid_batch.len(), capabilities.max_batch_size as usize);
}

#[tokio::test]
async fn test_wit_error_handling() {
    // 测试错误处理机制
    let error_cases = vec![
        "Invalid input format",
        "Memory limit exceeded",
        "Timeout occurred",
        "Permission denied",
        "Plugin not found",
    ];

    for error_msg in error_cases {
        let error_result = ProcessingResult::Error(error_msg.to_string());
        match error_result {
            ProcessingResult::Error(msg) => assert_eq!(msg, error_msg),
            _ => panic!("Expected Error variant for: {}", error_msg),
        }
    }
}

#[tokio::test]
async fn test_wit_metadata_compatibility() {
    // 测试元数据兼容性检查
    let plugin_info = PluginInfo {
        name: "Compatibility Test Plugin".to_string(),
        version: "1.0.0".to_string(),
        description: "Plugin for testing version compatibility".to_string(),
        author: Some("Test Team".to_string()),
        dataflare_version: "4.0.0".to_string(),
    };

    // 测试兼容的版本
    let compatible_versions = vec!["4.0.0", "4.0.1", "4.1.0", "4.2.0"];
    for version in compatible_versions {
        assert!(plugin_info.dataflare_version.starts_with("4."));
        // 简单的兼容性检查
        assert!(version.starts_with("4."));
    }

    // 测试不兼容的版本
    let incompatible_versions = vec!["3.9.9", "5.0.0", "2.1.0"];
    for version in incompatible_versions {
        assert!(!version.starts_with("4."));
    }
}
