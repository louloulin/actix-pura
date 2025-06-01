//! 简单过滤器插件示例
//!
//! 这个示例展示了如何使用DataFlare插件系统创建一个简单的过滤器插件。
//! 该插件会过滤包含"error"关键字的日志记录。

use dataflare_plugin::{SmartPlugin, PluginRecord, PluginResult, PluginType, Result};
use dataflare_plugin::test_utils::create_test_plugin_record;

/// 错误日志过滤器插件
///
/// 这个插件实现了SmartPlugin trait，用于过滤包含错误信息的日志记录。
pub struct ErrorLogFilter {
    keywords: Vec<String>,
}

impl ErrorLogFilter {
    /// 创建新的错误日志过滤器
    pub fn new(keywords: Vec<String>) -> Self {
        Self { keywords }
    }

    /// 使用默认关键字创建过滤器
    pub fn default() -> Self {
        Self::new(vec![
            "error".to_string(),
            "ERROR".to_string(),
            "fail".to_string(),
            "FAIL".to_string(),
        ])
    }
}

impl SmartPlugin for ErrorLogFilter {
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
        // 将字节数据转换为字符串
        let data = record.value_as_str()?;

        // 检查是否包含任何错误关键字
        let contains_error = self.keywords.iter().any(|keyword| data.contains(keyword));

        // 返回过滤结果：true表示保留记录，false表示过滤掉
        Ok(PluginResult::Filtered(contains_error))
    }

    fn name(&self) -> &str {
        "error-log-filter"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Filter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dataflare_plugin::test_utils::create_test_string_record;

    #[test]
    fn test_error_filter_matches() {
        let filter = ErrorLogFilter::default();
        let record = create_test_string_record("This is an error message");
        let plugin_record = record.as_plugin_record();

        let result = filter.process(&plugin_record).unwrap();

        match result {
            PluginResult::Filtered(keep) => assert!(keep, "Should keep error messages"),
            _ => panic!("Expected filtered result"),
        }
    }

    #[test]
    fn test_error_filter_no_match() {
        let filter = ErrorLogFilter::default();
        let record = create_test_string_record("This is a normal info message");
        let plugin_record = record.as_plugin_record();

        let result = filter.process(&plugin_record).unwrap();

        match result {
            PluginResult::Filtered(keep) => assert!(!keep, "Should filter out normal messages"),
            _ => panic!("Expected filtered result"),
        }
    }

    #[test]
    fn test_custom_keywords() {
        let filter = ErrorLogFilter::new(vec!["warning".to_string(), "WARN".to_string()]);
        let record = create_test_string_record("This is a warning message");
        let plugin_record = record.as_plugin_record();

        let result = filter.process(&plugin_record).unwrap();

        match result {
            PluginResult::Filtered(keep) => assert!(keep, "Should keep warning messages"),
            _ => panic!("Expected filtered result"),
        }
    }
}

/// 演示如何在DataFlare工作流中使用插件的示例函数
#[cfg(test)]
mod integration_example {
    use super::*;
    use dataflare_plugin::{SmartPluginAdapter, test_utils::create_test_data_record};
    use dataflare_core::processor::Processor;

    #[tokio::test]
    async fn test_plugin_in_workflow() {
        // 1. 创建插件实例
        let plugin = Box::new(ErrorLogFilter::default());

        // 2. 创建插件适配器
        let mut adapter = SmartPluginAdapter::new(plugin);

        // 3. 模拟DataFlare工作流中的数据记录
        let error_record = create_test_data_record("ERROR: Database connection failed");
        let info_record = create_test_data_record("INFO: Application started successfully");

        // 4. 处理错误记录（应该被保留）
        let result = adapter.process_record(&error_record).await;
        assert!(result.is_ok(), "Error record should be processed successfully");

        // 5. 处理信息记录（应该被过滤掉）
        let result = adapter.process_record(&info_record).await;
        assert!(result.is_err(), "Info record should be filtered out");

        // 6. 检查插件信息
        let info = adapter.plugin_info();
        assert_eq!(info.name, "error-log-filter");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.plugin_type, PluginType::Filter);
    }
}

fn main() {
    println!("Running simple filter example...");

    let plugin = ErrorLogFilter {
        keywords: vec!["ERROR".to_string(), "FATAL".to_string()],
    };
    println!("Plugin: {} v{}", plugin.name(), plugin.version());

    // Test with error log
    let error_record = create_test_plugin_record(b"ERROR: Database connection failed");
    let error_plugin_record = error_record.as_plugin_record();

    match plugin.process(&error_plugin_record) {
        Ok(PluginResult::Filtered(keep)) => {
            println!("Error log result: keep = {}", keep);
        },
        Ok(_) => println!("Unexpected result type"),
        Err(e) => eprintln!("Plugin error: {}", e),
    }

    // Test with info log
    let info_record = create_test_plugin_record(b"INFO: Application started");
    let info_plugin_record = info_record.as_plugin_record();

    match plugin.process(&info_plugin_record) {
        Ok(PluginResult::Filtered(keep)) => {
            println!("Info log result: keep = {}", keep);
        },
        Ok(_) => println!("Unexpected result type"),
        Err(e) => eprintln!("Plugin error: {}", e),
    }
}