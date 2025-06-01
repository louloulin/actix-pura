//! WASM模块集成测试

use dataflare_wasm::*;
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn test_wasm_runtime_creation() {
    let config = runtime::WasmRuntimeConfig::default();
    let runtime = WasmRuntime::new(config);
    assert!(runtime.is_ok());
}

#[tokio::test]
async fn test_wasm_component_factory() {
    let factory = components::WasmComponentFactory::new();
    let types = factory.list_component_types();

    assert!(types.contains(&"source".to_string()));
    assert!(types.contains(&"destination".to_string()));
    assert!(types.contains(&"processor".to_string()));
    assert!(types.contains(&"transformer".to_string()));
    assert!(types.contains(&"filter".to_string()));
    assert!(types.contains(&"aggregator".to_string()));
}

#[tokio::test]
async fn test_wasm_component_manager() {
    let manager = components::WasmComponentManager::new();

    // 测试初始状态
    assert_eq!(manager.list_components().len(), 0);

    let stats = manager.get_stats();
    assert_eq!(stats.total_components, 0);
    assert_eq!(stats.source_components, 0);
    assert_eq!(stats.destination_components, 0);
}

#[tokio::test]
async fn test_wasm_plugin_config() {
    let config = WasmPluginConfig {
        name: "test_plugin".to_string(),
        module_path: "test.wasm".to_string(),
        config: HashMap::new(),
        security_policy: sandbox::SecurityPolicy::default(),
        memory_limit: 1024 * 1024, // 1MB
        timeout_ms: 5000,
    };

    assert_eq!(config.name, "test_plugin");
    assert_eq!(config.module_path, "test.wasm");
    assert_eq!(config.memory_limit, 1024 * 1024);
    assert_eq!(config.timeout_ms, 5000);
}

#[tokio::test]
async fn test_wasm_function_call() {
    let call = interface::WasmFunctionCall {
        function_name: "test_function".to_string(),
        parameters: {
            let mut params = HashMap::new();
            params.insert("param1".to_string(), json!("param1"));
            params.insert("param2".to_string(), json!(42));
            params
        },
        call_id: Some("test_call_123".to_string()),
        context: None,
    };

    assert_eq!(call.function_name, "test_function");
    assert_eq!(call.parameters.len(), 2);
    assert_eq!(call.call_id, Some("test_call_123".to_string()));
}

#[tokio::test]
async fn test_wasm_function_result() {
    let result = interface::WasmFunctionResult::success("test_result").unwrap();

    assert!(result.success);
    assert!(result.error.is_none());
    assert!(result.result.is_some());

    let error_result = interface::WasmFunctionResult::error("test_error");
    assert!(!error_result.success);
    assert!(error_result.error.is_some());
    assert!(error_result.result.is_none());
}

#[tokio::test]
async fn test_wasm_component_type_parsing() {
    use components::WasmComponentType;

    assert_eq!(WasmComponentType::from_str("source").unwrap(), WasmComponentType::Source);
    assert_eq!(WasmComponentType::from_str("destination").unwrap(), WasmComponentType::Destination);
    assert_eq!(WasmComponentType::from_str("processor").unwrap(), WasmComponentType::Processor);
    assert_eq!(WasmComponentType::from_str("transformer").unwrap(), WasmComponentType::Transformer);
    assert_eq!(WasmComponentType::from_str("filter").unwrap(), WasmComponentType::Filter);
    assert_eq!(WasmComponentType::from_str("aggregator").unwrap(), WasmComponentType::Aggregator);

    match WasmComponentType::from_str("custom_type").unwrap() {
        WasmComponentType::Custom(name) => assert_eq!(name, "custom_type"),
        _ => panic!("Expected custom type"),
    }
}

#[tokio::test]
async fn test_security_policy() {
    let policy = sandbox::SecurityPolicy::default();

    assert!(!policy.allow_network); // 默认禁用网络
    assert!(!policy.allow_filesystem); // 默认禁用文件系统
    assert!(!policy.allow_env_vars); // 默认禁用环境变量
    assert_eq!(policy.max_execution_time_ms, 5000); // 5s
}

#[tokio::test]
async fn test_host_functions() {
    let registry = host_functions::HostFunctionRegistry::new();

    // 测试默认函数注册
    let functions = registry.list_functions();
    assert!(functions.contains(&"log".to_string()));
    assert!(functions.contains(&"get_time".to_string()));
    assert!(functions.contains(&"random".to_string()));
}

#[tokio::test]
async fn test_wasm_error_types() {
    let config_error = WasmError::Configuration("test config error".to_string());
    assert!(matches!(config_error, WasmError::Configuration(_)));

    let runtime_error = WasmError::Runtime("test runtime error".to_string());
    assert!(matches!(runtime_error, WasmError::Runtime(_)));

    let plugin_error = WasmError::PluginLoad("test plugin error".to_string());
    assert!(matches!(plugin_error, WasmError::PluginLoad(_)));
}

#[tokio::test]
async fn test_wasm_plugin_metadata() {
    let metadata = WasmPluginMetadata::default();

    assert_eq!(metadata.name, "unknown");
    assert_eq!(metadata.version, "0.1.0");
    assert_eq!(metadata.author, "unknown");
    assert_eq!(metadata.dataflare_version, "0.1.0");
}

#[tokio::test]
async fn test_wasm_registry() {
    let registry = registry::WasmPluginRegistry::new();

    // 测试初始状态
    let plugins = registry.list_plugins();
    assert_eq!(plugins.len(), 0);
}

#[test]
fn test_wasm_component_config_serialization() {
    use components::{WasmComponentConfig, WasmComponentType};

    let config = WasmComponentConfig {
        component_type: WasmComponentType::Source,
        name: "test_source".to_string(),
        module_path: "test_source.wasm".to_string(),
        config: Some({
            let mut map = HashMap::new();
            map.insert("key1".to_string(), json!("value1"));
            map.insert("key2".to_string(), json!(42));
            map
        }),
        runtime_config: None,
        metadata: Some({
            let mut map = HashMap::new();
            map.insert("author".to_string(), "test_author".to_string());
            map
        }),
    };

    // 测试序列化
    let serialized = serde_json::to_string(&config).unwrap();
    assert!(serialized.contains("test_source"));
    assert!(serialized.contains("source"));

    // 测试反序列化
    let deserialized: WasmComponentConfig = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.name, config.name);
    assert_eq!(deserialized.component_type, config.component_type);
}

#[test]
fn test_wasm_plugin_capabilities() {
    let capabilities = interface::WasmPluginCapabilities::default();

    assert!(!capabilities.supports_async); // 默认不支持异步
    assert!(!capabilities.supports_streaming); // 默认不支持流式处理
    assert!(!capabilities.supports_batch); // 默认不支持批处理
}
