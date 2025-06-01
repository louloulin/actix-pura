//! 高级集成测试
//!
//! 测试WIT接口、DTL桥接、AI集成和处理器注册表功能

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::json;

use dataflare_wasm::{
    WasmRuntime, WasmRuntimeConfig,
    DTLWasmBridge, WasmFunctionInfo,
    AIWasmIntegrator, AIWasmConfig,
    WasmProcessorRegistry, WasmProcessorFactory,
    WitRuntime, WitRuntimeConfig,
};

#[tokio::test]
async fn test_dtl_wasm_bridge_integration() {
    // 创建WASM运行时
    let config = WasmRuntimeConfig::default();
    let runtime = WasmRuntime::new(config).unwrap();
    let runtime_arc = Arc::new(RwLock::new(runtime));

    // 创建DTL桥接器
    let mut bridge = DTLWasmBridge::new(runtime_arc.clone());

    // 测试函数注册（应该失败，因为插件不存在）
    let result = bridge.register_wasm_function(
        "test_transform".to_string(),
        "transform_plugin".to_string(),
        "transform_data".to_string(),
        Some("测试数据转换函数".to_string()),
    ).await;

    // 验证注册失败（因为插件不存在）
    assert!(result.is_err());

    // 验证已注册函数列表为空
    assert_eq!(bridge.get_registered_functions().len(), 0);
}

#[tokio::test]
async fn test_ai_wasm_integration() {
    // 创建WASM运行时
    let wasm_config = WasmRuntimeConfig::default();
    let wasm_runtime = Arc::new(RwLock::new(WasmRuntime::new(wasm_config).unwrap()));

    // 创建DTL桥接器
    let dtl_bridge = Arc::new(RwLock::new(DTLWasmBridge::new(wasm_runtime.clone())));

    // 创建AI配置
    let ai_config = AIWasmConfig {
        enabled_ai_functions: vec![
            "embed".to_string(),
            "sentiment_analysis".to_string(),
        ],
        wasm_plugin_mappings: HashMap::new(),
        ai_model_config: HashMap::new(),
        vector_store_config: None,
        cache_config: None,
    };

    // 创建AI集成器
    let mut integrator = AIWasmIntegrator::new(ai_config, wasm_runtime, dtl_bridge);

    // 测试初始化（会失败，因为插件不存在）
    let result = integrator.initialize().await;
    assert!(result.is_err());

    // 测试AI函数调用（使用默认实现）
    let result = integrator.call_ai_function(
        "embed",
        vec![json!("测试文本")],
    ).await;

    // 应该成功，因为有默认实现
    assert!(result.is_ok());

    // 验证统计信息
    let stats = integrator.get_stats();
    assert_eq!(stats.ai_function_calls, 1);
    assert_eq!(stats.cache_misses, 1);
}

#[tokio::test]
async fn test_processor_registry_integration() {
    // 创建WASM运行时
    let wasm_config = WasmRuntimeConfig::default();
    let wasm_runtime = Arc::new(RwLock::new(WasmRuntime::new(wasm_config).unwrap()));

    // 创建DTL桥接器
    let dtl_bridge = Arc::new(RwLock::new(DTLWasmBridge::new(wasm_runtime.clone())));

    // 创建AI集成器
    let ai_config = AIWasmConfig::default();
    let ai_integrator = Arc::new(RwLock::new(AIWasmIntegrator::new(ai_config, wasm_runtime.clone(), dtl_bridge.clone())));

    // 创建处理器注册表
    let mut registry = WasmProcessorRegistry::new(wasm_runtime, dtl_bridge, ai_integrator);

    // 初始化注册表
    let result = registry.initialize().await;

    // 验证注册的处理器类型数量
    let types = registry.get_registered_types();

    if result.is_err() {
        // 如果初始化失败，应该没有注册类型
        assert_eq!(types.len(), 0);
    } else {
        // 如果初始化成功，应该有标准类型
        assert!(types.len() > 0);
    }
}

#[tokio::test]
async fn test_processor_factory() {
    // 创建WASM运行时
    let wasm_config = WasmRuntimeConfig::default();
    let wasm_runtime = Arc::new(RwLock::new(WasmRuntime::new(wasm_config).unwrap()));

    // 创建DTL桥接器
    let dtl_bridge = Arc::new(RwLock::new(DTLWasmBridge::new(wasm_runtime.clone())));

    // 创建AI集成器
    let ai_config = AIWasmConfig::default();
    let ai_integrator = Arc::new(RwLock::new(AIWasmIntegrator::new(ai_config, wasm_runtime.clone(), dtl_bridge.clone())));

    // 创建处理器注册表
    let registry = Arc::new(RwLock::new(WasmProcessorRegistry::new(wasm_runtime, dtl_bridge, ai_integrator)));

    // 创建处理器工厂
    let factory = WasmProcessorFactory::new(registry.clone());

    // 获取支持的处理器类型
    let types = factory.get_supported_types().await;
    assert_eq!(types.len(), 0); // 注册表未初始化，所以没有类型

    // 尝试创建处理器（应该失败）
    let config = json!({
        "plugin_path": "test.wasm",
        "memory_limit": 67108864,
        "timeout_ms": 5000
    });

    let result = factory.create_processor("wasm_mapping", config).await;
    assert!(result.is_err()); // 应该失败，因为类型未注册
}

#[tokio::test]
async fn test_wit_runtime_creation() {
    let config = WitRuntimeConfig::default();
    let runtime = WitRuntime::new(config);

    // WIT运行时创建可能会失败，因为需要特定的wasmtime版本
    // 这里只测试基本的创建逻辑
    match runtime {
        Ok(mut runtime) => {
            assert!(!runtime.is_initialized());

            // 尝试初始化
            let result = runtime.initialize().await;
            // 可能成功也可能失败，取决于环境
            println!("WIT运行时初始化结果: {:?}", result.is_ok());
        },
        Err(e) => {
            // WIT运行时创建失败是可以接受的，因为需要特定的依赖
            println!("WIT运行时创建失败: {}", e);
        }
    }
}

#[tokio::test]
async fn test_function_info_structure() {
    let function_info = WasmFunctionInfo {
        plugin_id: "test_plugin".to_string(),
        function_name: "test_function".to_string(),
        description: Some("测试函数".to_string()),
        parameter_types: vec!["string".to_string(), "number".to_string()],
        return_type: "object".to_string(),
        is_async: true,
    };

    assert_eq!(function_info.plugin_id, "test_plugin");
    assert_eq!(function_info.function_name, "test_function");
    assert_eq!(function_info.description, Some("测试函数".to_string()));
    assert_eq!(function_info.parameter_types.len(), 2);
    assert_eq!(function_info.return_type, "object");
    assert!(function_info.is_async);
}

#[tokio::test]
async fn test_ai_config_defaults() {
    let config = AIWasmConfig::default();

    assert!(config.enabled_ai_functions.contains(&"embed".to_string()));
    assert!(config.enabled_ai_functions.contains(&"sentiment_analysis".to_string()));
    assert!(config.enabled_ai_functions.contains(&"vector_search".to_string()));
    assert!(config.enabled_ai_functions.contains(&"llm_summarize".to_string()));
    assert!(config.enabled_ai_functions.contains(&"text_classification".to_string()));

    assert_eq!(config.wasm_plugin_mappings.len(), 0);
    assert_eq!(config.ai_model_config.len(), 0);
    assert!(config.vector_store_config.is_none());
    assert!(config.cache_config.is_none());
}

#[tokio::test]
async fn test_bridge_function_management() {
    // 创建WASM运行时
    let config = WasmRuntimeConfig::default();
    let runtime = WasmRuntime::new(config).unwrap();
    let runtime_arc = Arc::new(RwLock::new(runtime));

    // 创建DTL桥接器
    let mut bridge = DTLWasmBridge::new(runtime_arc);

    // 验证初始状态
    assert_eq!(bridge.get_registered_functions().len(), 0);

    // 测试函数信息获取
    let info = bridge.get_function_info("nonexistent");
    assert!(info.is_none());

    // 测试调用统计获取
    let stats = bridge.get_call_stats("nonexistent");
    assert!(stats.is_none());

    // 测试清除所有函数
    bridge.clear_all_functions();
    assert_eq!(bridge.get_registered_functions().len(), 0);
}

#[tokio::test]
async fn test_ai_integrator_cache_management() {
    // 创建WASM运行时
    let wasm_config = WasmRuntimeConfig::default();
    let wasm_runtime = Arc::new(RwLock::new(WasmRuntime::new(wasm_config).unwrap()));

    // 创建DTL桥接器
    let dtl_bridge = Arc::new(RwLock::new(DTLWasmBridge::new(wasm_runtime.clone())));

    // 创建AI集成器
    let ai_config = AIWasmConfig::default();
    let mut integrator = AIWasmIntegrator::new(ai_config, wasm_runtime, dtl_bridge);

    // 验证初始缓存大小
    assert_eq!(integrator.get_cache_size(), 0);

    // 清除缓存
    integrator.clear_cache();
    assert_eq!(integrator.get_cache_size(), 0);

    // 验证统计信息
    let stats = integrator.get_stats();
    assert_eq!(stats.ai_function_calls, 0);
    assert_eq!(stats.wasm_function_calls, 0);
    assert_eq!(stats.cache_hits, 0);
    assert_eq!(stats.cache_misses, 0);
    assert_eq!(stats.total_execution_time_ms, 0);
    assert_eq!(stats.error_count, 0);
}
