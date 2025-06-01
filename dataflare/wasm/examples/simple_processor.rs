/// 简单的WASM处理器示例
///
/// 这个示例展示了如何创建一个基本的DataFlare WASM插件
/// 该插件实现了数据记录的简单转换功能

use dataflare_wasm::{
    WasmSystem, WasmResult,
    components::{WasmComponentConfig, WasmComponentType},
    sandbox::SecurityPolicy,
};
use serde_json::{json, Value, Map};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> WasmResult<()> {
    // 初始化日志
    env_logger::init();

    println!("🚀 DataFlare WASM 插件系统示例");

    // 1. 创建WASM系统
    let mut wasm_system = WasmSystem::new().await.map_err(|e| dataflare_wasm::WasmError::runtime(format!("创建WASM系统失败: {}", e)))?;

    // 2. 模拟插件加载（实际应用中需要真实的WASM文件）
    println!("📦 模拟WASM插件加载...");
    let plugin_id = "simple_processor_plugin".to_string();
    println!("✅ 插件加载成功，ID: {}", plugin_id);

    // 3. 创建组件配置
    let component_config = WasmComponentConfig {
        name: "simple_processor".to_string(),
        component_type: WasmComponentType::Processor,
        module_path: "examples/simple_processor.wasm".to_string(),
        config: Some(create_component_config()),
        runtime_config: Some(create_runtime_config()),
        metadata: Some(create_string_metadata()),
    };

    // 4. 模拟处理器组件创建
    println!("🔧 创建处理器组件...");
    // let processor = wasm_system.create_component(component_config).await?;
    println!("✅ 处理器组件创建成功");

    // 5. 模拟数据处理
    println!("📊 开始数据处理...");
    let test_data = create_test_data();

    for (i, record) in test_data.iter().enumerate() {
        println!("处理记录 {}: {:?}", i + 1, record);
        // 这里可以调用处理器的process方法
        // let result = processor.process(record).await?;
        // println!("处理结果: {:?}", result);
    }

    // 6. 显示系统统计信息
    println!("\n📈 系统统计信息:");
    let stats = wasm_system.get_stats();
    println!("- 已加载插件数: {}", stats.loaded_plugins);
    println!("- 活跃组件数: {}", stats.active_components);
    println!("- 热重载启用: {}", stats.hot_reload_enabled);
    println!("- 指标收集启用: {}", stats.metrics_enabled);

    println!("\n🎉 示例执行完成！");
    Ok(())
}

/// 创建字符串元数据
fn create_string_metadata() -> HashMap<String, String> {
    let mut metadata = HashMap::new();

    // 插件元数据
    metadata.insert("name".to_string(), "simple_processor".to_string());
    metadata.insert("version".to_string(), "1.0.0".to_string());
    metadata.insert("author".to_string(), "DataFlare Team".to_string());
    metadata.insert("description".to_string(), "简单的数据处理器示例".to_string());
    metadata.insert("dataflare_version".to_string(), "4.0.0".to_string());

    metadata
}

/// 创建运行时配置
fn create_runtime_config() -> HashMap<String, Value> {
    let mut config = HashMap::new();

    // 运行时配置
    config.insert("memory_limit".to_string(), json!(64 * 1024 * 1024)); // 64MB
    config.insert("timeout_ms".to_string(), json!(5000));
    config.insert("enable_logging".to_string(), json!(true));

    config
}

/// 创建组件配置
fn create_component_config() -> HashMap<String, Value> {
    let mut config = HashMap::new();

    // 转换规则
    config.insert("transform_rules".to_string(), json!([
        {
            "field": "name",
            "operation": "uppercase"
        },
        {
            "field": "age",
            "operation": "multiply",
            "value": 1.1
        }
    ]));

    // 输出配置
    config.insert("output".to_string(), json!({
        "format": "json",
        "include_metadata": true
    }));

    config
}

/// 创建测试数据
fn create_test_data() -> Vec<HashMap<String, Value>> {
    vec![
        convert_json_to_hashmap(json!({
            "id": 1,
            "name": "Alice",
            "age": 25,
            "city": "Beijing"
        })),

        convert_json_to_hashmap(json!({
            "id": 2,
            "name": "Bob",
            "age": 30,
            "city": "Shanghai"
        })),

        convert_json_to_hashmap(json!({
            "id": 3,
            "name": "Charlie",
            "age": 35,
            "city": "Guangzhou"
        })),
    ]
}

/// 将JSON Value转换为HashMap
fn convert_json_to_hashmap(value: Value) -> HashMap<String, Value> {
    if let Value::Object(map) = value {
        map.into_iter().collect()
    } else {
        HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_creation() {
        let metadata = create_string_metadata();
        assert!(metadata.contains_key("name"));
        assert!(metadata.contains_key("version"));
    }

    #[test]
    fn test_runtime_config_creation() {
        let config = create_runtime_config();
        assert!(config.contains_key("memory_limit"));
        assert!(config.contains_key("timeout_ms"));
    }

    #[test]
    fn test_component_config_creation() {
        let config = create_component_config();
        assert!(config.contains_key("transform_rules"));
        assert!(config.contains_key("output"));
    }

    #[test]
    fn test_test_data_creation() {
        let data = create_test_data();
        assert_eq!(data.len(), 3);
        assert!(data[0].contains_key("name"));
        assert!(data[0].contains_key("age"));
    }
}
