//! WASM 插件示例
//!
//! 本示例演示如何使用 WASM 插件系统。

use std::path::PathBuf;
use std::fs;
use serde_json::json;

use dataflare::{
    DataFlareConfig, DataRecord, PluginConfig, PluginManager, ProcessorPlugin, WasmProcessor,
    plugin::create_example_wasm_module,
};

#[actix_rt::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();
    
    // 创建临时插件目录
    let plugin_dir = tempfile::tempdir()?.into_path();
    println!("插件目录: {:?}", plugin_dir);
    
    // 创建示例 WASM 模块
    let wasm_bytes = create_example_wasm_module()?;
    
    // 保存示例 WASM 模块
    let plugin_path = plugin_dir.join("example-processor.wasm");
    fs::write(&plugin_path, &wasm_bytes)?;
    println!("已保存示例 WASM 模块: {:?}", plugin_path);
    
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
    let plugin = plugin_manager.load_plugin("example-processor", plugin_config)?;
    println!("已加载插件: {}", plugin.id);
    println!("插件元数据: {:?}", plugin.metadata);
    
    // 创建处理器
    let mut processor = plugin_manager.create_processor("example-processor")?;
    
    // 配置处理器
    processor.configure(json!({
        "add_timestamp": true
    }))?;
    
    // 创建测试记录
    let record = DataRecord::new(json!({
        "id": 1,
        "name": "Test Record",
        "value": 42
    }));
    
    // 处理记录
    let processed_record = processor.process(record)?;
    println!("处理后的记录: {:?}", processed_record);
    
    // 列出已加载的插件
    let plugins = plugin_manager.list_plugins()?;
    println!("已加载的插件: {:?}", plugins);
    
    // 卸载插件
    plugin_manager.unload_plugin("example-processor")?;
    println!("已卸载插件: example-processor");
    
    Ok(())
}
