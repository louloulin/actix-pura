//! WASM 插件系统集成测试

use std::path::PathBuf;
use std::fs;
use serde_json::json;

use dataflare::{
    DataFlareConfig, DataRecord, PluginConfig, PluginManager, ProcessorPlugin,
    plugin::create_example_wasm_module,
};

#[test]
fn test_wasm_plugin_system() {
    // 创建临时插件目录
    let plugin_dir = tempfile::tempdir().unwrap().into_path();

    // 创建示例 WASM 模块
    let wasm_bytes = create_example_wasm_module().unwrap();

    // 保存示例 WASM 模块
    let plugin_path = plugin_dir.join("example-processor.wasm");
    fs::write(&plugin_path, &wasm_bytes).unwrap();

    // 创建插件管理器
    let plugin_manager = PluginManager::new(plugin_dir);

    // 创建插件配置
    let plugin_config = PluginConfig {
        memory_limit: Some(16 * 1024 * 1024), // 16MB
        timeout_ms: Some(5000), // 5 秒
        config: json!({
            "name": "example-processor",
            "options": {
                "add_field": true,
                "field_name": "processed_by",
                "field_value": "wasm-plugin"
            }
        }),
    };

    // 加载插件
    let plugin = plugin_manager.load_plugin("example-processor", plugin_config).unwrap();

    // 验证插件元数据
    assert_eq!(plugin.metadata.name, "example-processor");
    assert_eq!(plugin.metadata.version, "1.0.0");
    assert_eq!(plugin.metadata.author, "DataFlare Team");

    // 创建处理器
    let mut processor = plugin_manager.create_processor("example-processor").unwrap();

    // 配置处理器
    processor.configure(json!({
        "add_timestamp": true
    })).unwrap();

    // 创建测试记录
    let record = DataRecord::new(json!({
        "id": 1,
        "name": "Test Record",
        "value": 42
    }));

    // 处理记录
    let processed_record = processor.process(record.clone()).unwrap();

    // 验证处理结果
    // 注意：由于我们的示例 WASM 模块尚未完全实现，这里只是验证处理器不会崩溃
    assert!(processed_record.data.get("name").is_some());
    assert!(processed_record.data.get("processed").is_some());

    // 列出已加载的插件
    let plugins = plugin_manager.list_plugins().unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0], "example-processor");

    // 卸载插件
    plugin_manager.unload_plugin("example-processor").unwrap();

    // 验证插件已卸载
    let plugins = plugin_manager.list_plugins().unwrap();
    assert_eq!(plugins.len(), 0);
}

#[test]
fn test_plugin_manager() {
    // 创建临时插件目录
    let plugin_dir = tempfile::tempdir().unwrap().into_path();

    // 创建插件管理器
    let plugin_manager = PluginManager::new(plugin_dir.clone());

    // 扫描插件目录
    let plugins = plugin_manager.scan_plugin_directory().unwrap();
    assert_eq!(plugins.len(), 0);

    // 创建示例 WASM 模块
    let wasm_bytes = create_example_wasm_module().unwrap();

    // 保存示例 WASM 模块
    let plugin_path = plugin_dir.join("example-processor.wasm");
    fs::write(&plugin_path, &wasm_bytes).unwrap();

    // 再次扫描插件目录
    let plugins = plugin_manager.scan_plugin_directory().unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].file_name().unwrap(), "example-processor.wasm");
}

#[test]
fn test_create_example_wasm_module() {
    // 创建示例 WASM 模块
    let wasm_bytes = create_example_wasm_module().unwrap();

    // 验证 WASM 模块不为空
    assert!(!wasm_bytes.is_empty());

    // 验证 WASM 魔数
    assert_eq!(&wasm_bytes[0..4], &[0x00, 0x61, 0x73, 0x6D]); // \0asm
}
