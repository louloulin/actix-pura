//! 测试新的3层插件架构
//!
//! 验证新的DataFlarePlugin接口、PluginRuntime和适配器的功能

use std::collections::HashMap;
use dataflare_plugin::{
    DataFlarePlugin, PluginRecord, PluginResult, PluginType, PluginError, Result,
    PluginRuntime, SmartPluginAdapter,
};
use dataflare_core::message::DataRecord;
use serde_json::json;

/// 测试过滤器插件
struct TestFilterPlugin {
    name: String,
    keyword: String,
}

impl TestFilterPlugin {
    fn new(name: &str, keyword: &str) -> Self {
        Self {
            name: name.to_string(),
            keyword: keyword.to_string(),
        }
    }
}

impl DataFlarePlugin for TestFilterPlugin {
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
        let data = record.value_as_str().map_err(|e| {
            PluginError::Execution(format!("UTF-8解析失败: {}", e))
        })?;

        Ok(PluginResult::Filtered(data.contains(&self.keyword)))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Filter
    }
}

/// 测试转换器插件
struct TestTransformPlugin {
    name: String,
}

impl TestTransformPlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl DataFlarePlugin for TestTransformPlugin {
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
        let data = record.value_as_str().map_err(|e| {
            PluginError::Execution(format!("UTF-8解析失败: {}", e))
        })?;

        let transformed = data.to_uppercase();
        Ok(PluginResult::Transformed(transformed.into_bytes()))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Transform
    }
}

/// 测试批处理插件
struct TestBatchPlugin {
    name: String,
}

impl TestBatchPlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl DataFlarePlugin for TestBatchPlugin {
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
        let data = record.value_as_str().map_err(|e| {
            PluginError::Execution(format!("UTF-8解析失败: {}", e))
        })?;

        Ok(PluginResult::Transformed(format!("batch:{}", data).into_bytes()))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn plugin_type(&self) -> PluginType {
        PluginType::Processor
    }

    fn supports_batch(&self) -> bool {
        true
    }

    fn process_batch(&self, records: &[PluginRecord]) -> Vec<Result<PluginResult>> {
        // 批处理实现：将所有记录合并
        let combined_data: Vec<String> = records
            .iter()
            .filter_map(|r| r.value_as_str().ok())
            .map(|s| s.to_string())
            .collect();

        let combined = combined_data.join(",");
        let result = Ok(PluginResult::Transformed(format!("batch:[{}]", combined).into_bytes()));

        // 返回单个结果，其他记录标记为跳过
        let mut results = vec![result];
        for _ in 1..records.len() {
            results.push(Ok(PluginResult::Skip));
        }

        results
    }
}

#[tokio::test]
async fn test_plugin_runtime_basic() {
    println!("🧪 测试插件运行时基础功能");

    let mut runtime = PluginRuntime::default();

    // 注册过滤器插件
    let filter_plugin = Box::new(TestFilterPlugin::new("test_filter", "keep"));
    runtime.register_native_plugin("test_filter".to_string(), filter_plugin).unwrap();

    // 注册转换器插件
    let transform_plugin = Box::new(TestTransformPlugin::new("test_transform"));
    runtime.register_native_plugin("test_transform".to_string(), transform_plugin).unwrap();

    // 验证插件注册
    let plugins = runtime.list_plugins();
    assert_eq!(plugins.len(), 2);

    // 测试过滤器
    let value = b"keep this record";
    let metadata = HashMap::new();
    let record = PluginRecord::new(value, &metadata, 1234567890, 0, 100);

    let result = runtime.process_record("test_filter", &record).unwrap();
    match result {
        PluginResult::Filtered(keep) => assert!(keep),
        _ => panic!("Expected filtered result"),
    }

    // 测试转换器
    let value = b"hello world";
    let record = PluginRecord::new(value, &metadata, 1234567890, 0, 101);

    let result = runtime.process_record("test_transform", &record).unwrap();
    match result {
        PluginResult::Transformed(data) => {
            let transformed = String::from_utf8(data).unwrap();
            assert_eq!(transformed, "HELLO WORLD");
        }
        _ => panic!("Expected transformed result"),
    }

    println!("✅ 插件运行时基础功能测试通过");
}

#[tokio::test]
async fn test_plugin_metrics() {
    println!("🧪 测试插件性能指标");

    let mut runtime = PluginRuntime::default();
    let plugin = Box::new(TestFilterPlugin::new("metrics_test", "test"));
    runtime.register_native_plugin("metrics_test".to_string(), plugin).unwrap();

    let metadata = HashMap::new();

    // 处理多条记录
    for i in 0..10 {
        let value = format!("test record {}", i);
        let record = PluginRecord::new(value.as_bytes(), &metadata, 1234567890, 0, i);
        let _ = runtime.process_record("metrics_test", &record);
    }

    // 检查指标
    let metrics = runtime.get_plugin_metrics("metrics_test").unwrap();
    assert_eq!(metrics.total_calls, 10);
    assert_eq!(metrics.successful_calls, 10);
    assert_eq!(metrics.records_processed, 10);
    assert_eq!(metrics.success_rate(), 100.0);
    assert!(metrics.throughput() > 0.0);

    println!("✅ 插件性能指标测试通过");
}

#[tokio::test]
async fn test_batch_processing() {
    println!("🧪 测试批处理功能");

    let mut runtime = PluginRuntime::default();
    let plugin = Box::new(TestBatchPlugin::new("batch_test"));
    runtime.register_native_plugin("batch_test".to_string(), plugin).unwrap();

    let metadata = HashMap::new();
    let records = vec![
        PluginRecord::new(b"record1", &metadata, 1234567890, 0, 1),
        PluginRecord::new(b"record2", &metadata, 1234567890, 0, 2),
        PluginRecord::new(b"record3", &metadata, 1234567890, 0, 3),
    ];

    let results = runtime.process_batch("batch_test", &records);
    assert_eq!(results.len(), 3);

    // 第一个结果应该是合并的数据
    match &results[0] {
        Ok(PluginResult::Transformed(data)) => {
            let result_str = String::from_utf8(data.clone()).unwrap();
            assert_eq!(result_str, "batch:[record1,record2,record3]");
        }
        _ => panic!("Expected transformed result"),
    }

    // 其他结果应该是跳过
    for result in &results[1..] {
        match result {
            Ok(PluginResult::Skip) => {},
            _ => panic!("Expected skip result"),
        }
    }

    println!("✅ 批处理功能测试通过");
}

#[tokio::test]
async fn test_adapter_integration() {
    println!("🧪 测试适配器集成");

    let plugin = Box::new(TestTransformPlugin::new("adapter_test"));
    let adapter = SmartPluginAdapter::new(plugin);

    // 创建测试记录
    let record = DataRecord::new(json!("hello world"));

    // 检查适配器信息
    let info = adapter.plugin_info();
    assert_eq!(info.name, "adapter_test");
    assert_eq!(info.plugin_type, PluginType::Transform);

    // 检查适配器指标
    let metrics = adapter.metrics();
    assert_eq!(metrics.records_processed, 0); // 还没有处理任何记录

    println!("✅ 适配器集成测试通过");
}

#[tokio::test]
async fn test_plugin_lifecycle() {
    println!("🧪 测试插件生命周期");

    let mut runtime = PluginRuntime::default();

    // 注册插件
    let plugin = Box::new(TestFilterPlugin::new("lifecycle_test", "test"));
    runtime.register_native_plugin("lifecycle_test".to_string(), plugin).unwrap();

    // 验证插件存在
    let info = runtime.get_plugin_info("lifecycle_test").unwrap();
    assert_eq!(info.name, "lifecycle_test");
    assert_eq!(info.plugin_type, PluginType::Filter);

    // 禁用插件
    runtime.set_plugin_enabled("lifecycle_test", false).unwrap();

    // 尝试处理记录（应该失败）
    let metadata = HashMap::new();
    let record = PluginRecord::new(b"test", &metadata, 1234567890, 0, 1);
    let result = runtime.process_record("lifecycle_test", &record);
    assert!(result.is_err());

    // 重新启用插件
    runtime.set_plugin_enabled("lifecycle_test", true).unwrap();

    // 现在应该可以处理
    let result = runtime.process_record("lifecycle_test", &record);
    assert!(result.is_ok());

    // 卸载插件
    runtime.unregister_plugin("lifecycle_test").unwrap();

    // 验证插件已被移除
    let plugins = runtime.list_plugins();
    assert_eq!(plugins.len(), 0);

    println!("✅ 插件生命周期测试通过");
}

#[tokio::test]
async fn test_zero_copy_performance() {
    println!("🧪 测试零拷贝性能");

    let mut runtime = PluginRuntime::default();
    let plugin = Box::new(TestFilterPlugin::new("perf_test", "test"));
    runtime.register_native_plugin("perf_test".to_string(), plugin).unwrap();

    let metadata = HashMap::new();
    let large_data = "test data ".repeat(1000); // 9KB数据

    let start = std::time::Instant::now();

    // 处理大量记录
    for i in 0..1000 {
        let record = PluginRecord::new(large_data.as_bytes(), &metadata, 1234567890, 0, i);
        let _ = runtime.process_record("perf_test", &record);
    }

    let elapsed = start.elapsed();
    let metrics = runtime.get_plugin_metrics("perf_test").unwrap();

    println!("处理1000条记录耗时: {:?}", elapsed);
    println!("平均处理时间: {:?}", metrics.average_execution_time);
    println!("吞吐量: {:.2} records/sec", metrics.throughput());

    // 性能断言
    assert!(elapsed.as_millis() < 1000, "处理时间应该少于1秒");
    assert!(metrics.throughput() > 100.0, "吞吐量应该超过100 records/sec");

    println!("✅ 零拷贝性能测试通过");
}
