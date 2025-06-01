//! 简单映射插件示例
//!
//! 这个示例展示了如何使用DataFlare插件系统创建一个数据转换插件。
//! 该插件会将输入的文本转换为大写，并添加时间戳。

use dataflare_plugin::{SmartPlugin, PluginRecord, PluginResult, PluginType, Result};
use dataflare_plugin::test_utils::create_test_plugin_record;
use serde_json::json;
use chrono::Utc;

/// 文本大写转换插件
///
/// 这个插件实现了SmartPlugin trait，用于将文本数据转换为大写并添加处理时间戳。
pub struct UppercaseTransformer {
    add_timestamp: bool,
    prefix: Option<String>,
}

impl UppercaseTransformer {
    /// 创建新的大写转换器
    pub fn new(add_timestamp: bool, prefix: Option<String>) -> Self {
        Self { add_timestamp, prefix }
    }

    /// 使用默认配置创建转换器
    pub fn default() -> Self {
        Self::new(true, Some("[PROCESSED]".to_string()))
    }
}

impl SmartPlugin for UppercaseTransformer {
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
        // 将字节数据转换为字符串
        let input_text = record.value_as_str()?;

        // 转换为大写
        let mut output_text = input_text.to_uppercase();

        // 添加前缀（如果配置了）
        if let Some(ref prefix) = self.prefix {
            output_text = format!("{} {}", prefix, output_text);
        }

        // 创建输出JSON对象
        let mut output = json!({
            "original": input_text,
            "transformed": output_text,
        });

        // 添加时间戳（如果配置了）
        if self.add_timestamp {
            output["processed_at"] = json!(Utc::now().to_rfc3339());
        }

        // 添加元数据信息
        if !record.metadata.is_empty() {
            output["metadata"] = json!(record.metadata);
        }

        // 序列化为字节数组
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|e| dataflare_plugin::PluginError::processing(format!("JSON serialization failed: {}", e)))?;

        Ok(PluginResult::Mapped(output_bytes))
    }

    fn name(&self) -> &str {
        "uppercase-transformer"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dataflare_plugin::test_utils::create_test_string_record;
    use serde_json::Value;

    #[test]
    fn test_uppercase_transformation() {
        let transformer = UppercaseTransformer::default();
        let record = create_test_string_record("hello world");
        let plugin_record = record.as_plugin_record();

        let result = transformer.process(&plugin_record).unwrap();

        match result {
            PluginResult::Mapped(data) => {
                let output: Value = serde_json::from_slice(&data).unwrap();
                assert_eq!(output["original"], "hello world");
                assert_eq!(output["transformed"], "[PROCESSED] HELLO WORLD");
                assert!(output["processed_at"].is_string());
            },
            _ => panic!("Expected mapped result"),
        }
    }

    #[test]
    fn test_no_timestamp() {
        let transformer = UppercaseTransformer::new(false, None);
        let record = create_test_string_record("test message");
        let plugin_record = record.as_plugin_record();

        let result = transformer.process(&plugin_record).unwrap();

        match result {
            PluginResult::Mapped(data) => {
                let output: Value = serde_json::from_slice(&data).unwrap();
                assert_eq!(output["original"], "test message");
                assert_eq!(output["transformed"], "TEST MESSAGE");
                assert!(output["processed_at"].is_null());
            },
            _ => panic!("Expected mapped result"),
        }
    }

    #[test]
    fn test_custom_prefix() {
        let transformer = UppercaseTransformer::new(false, Some("[CUSTOM]".to_string()));
        let record = create_test_string_record("data");
        let plugin_record = record.as_plugin_record();

        let result = transformer.process(&plugin_record).unwrap();

        match result {
            PluginResult::Mapped(data) => {
                let output: Value = serde_json::from_slice(&data).unwrap();
                assert_eq!(output["transformed"], "[CUSTOM] DATA");
            },
            _ => panic!("Expected mapped result"),
        }
    }
}

/// 演示如何在DataFlare工作流中使用映射插件的示例
#[cfg(test)]
mod integration_example {
    use super::*;
    use dataflare_plugin::{SmartPluginAdapter, test_utils::create_test_data_record};
    use dataflare_core::processor::Processor;
    use serde_json::Value;

    #[tokio::test]
    async fn test_map_plugin_in_workflow() {
        // 1. 创建插件实例
        let plugin = Box::new(UppercaseTransformer::default());

        // 2. 创建插件适配器
        let mut adapter = SmartPluginAdapter::new(plugin);

        // 3. 模拟DataFlare工作流中的数据记录
        let input_record = create_test_data_record("hello dataflare world");

        // 4. 处理数据记录
        let result = adapter.process_record(&input_record).await.unwrap();

        // 5. 验证输出结果
        if let Value::String(output_str) = &result.data {
            let output: Value = serde_json::from_str(output_str).unwrap();
            assert_eq!(output["original"], "hello dataflare world");
            assert_eq!(output["transformed"], "[PROCESSED] HELLO DATAFLARE WORLD");
            assert!(output["processed_at"].is_string());
        } else {
            panic!("Expected string output");
        }

        // 6. 检查插件信息
        let info = adapter.plugin_info();
        assert_eq!(info.name, "uppercase-transformer");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.plugin_type, PluginType::Map);
    }
}

fn main() {
    println!("Running simple map example...");

    let plugin = UppercaseTransformer {
        add_timestamp: true,
        prefix: Some("[PROCESSED]".to_string()),
    };
    println!("Plugin: {} v{}", plugin.name(), plugin.version());

    // Test with simple text
    let text_record = create_test_plugin_record(b"hello world");
    let text_plugin_record = text_record.as_plugin_record();

    match plugin.process(&text_plugin_record) {
        Ok(PluginResult::Mapped(data)) => {
            let output = String::from_utf8_lossy(&data);
            println!("Text transformation result: {}", output);
        },
        Ok(_) => println!("Unexpected result type"),
        Err(e) => eprintln!("Plugin error: {}", e),
    }

    // Test with JSON
    let json_record = create_test_plugin_record(br#"{"message": "hello world"}"#);
    let json_plugin_record = json_record.as_plugin_record();

    match plugin.process(&json_plugin_record) {
        Ok(PluginResult::Mapped(data)) => {
            let output = String::from_utf8_lossy(&data);
            println!("JSON transformation result: {}", output);
        },
        Ok(_) => println!("Unexpected result type"),
        Err(e) => eprintln!("Plugin error: {}", e),
    }
}