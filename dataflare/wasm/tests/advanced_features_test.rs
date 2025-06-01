//! 高级功能集成测试
//!
//! 测试性能分析、监控指标、开发工具等高级功能

use std::time::Duration;
use tempfile::TempDir;

use dataflare_wasm::{
    WasmProfiler, MetricsCollector, PluginDevTools,
    ExecutionPhase, PluginMetrics, TemplateType
};

#[tokio::test]
async fn test_performance_profiler_workflow() {
    // 创建性能分析器
    let mut profiler = WasmProfiler::new();

    // 启用分析
    profiler.enable();
    assert!(profiler.get_stats().get("enabled").unwrap().as_bool().unwrap());

    // 开始分析会话
    profiler.start_profiling();

    // 模拟一些函数调用
    profiler.record_function_call("test_function_1", Duration::from_millis(50), true);
    profiler.record_function_call("test_function_2", Duration::from_millis(30), true);
    profiler.record_function_call("test_function_1", Duration::from_millis(70), false);

    // 记录内存使用
    profiler.record_memory_usage(1024 * 1024); // 1MB
    profiler.record_memory_usage(2048 * 1024); // 2MB

    // 记录执行阶段
    profiler.record_execution_phase(ExecutionPhase::Load, Duration::from_millis(100));
    profiler.record_execution_phase(ExecutionPhase::Processing, Duration::from_millis(200));

    // 停止分析并获取报告
    let report = profiler.stop_profiling().unwrap();

    // 验证报告内容
    assert!(report.duration > Duration::ZERO);
    assert_eq!(report.function_stats.len(), 2);

    // 验证函数统计
    let func1_stats = report.function_stats.get("test_function_1").unwrap();
    assert_eq!(func1_stats.call_count, 2);
    assert_eq!(func1_stats.error_count, 1);

    let func2_stats = report.function_stats.get("test_function_2").unwrap();
    assert_eq!(func2_stats.call_count, 1);
    assert_eq!(func2_stats.error_count, 0);

    // 验证内存统计
    assert_eq!(report.memory_stats.peak_memory, 2048 * 1024);
    assert_eq!(report.memory_stats.allocation_count, 2);

    // 验证执行统计
    assert_eq!(report.execution_stats.load_time, Duration::from_millis(100));
    assert_eq!(report.execution_stats.processing_time, Duration::from_millis(200));
}

#[tokio::test]
async fn test_metrics_collector_comprehensive() {
    let collector = MetricsCollector::new();

    // 测试计数器
    collector.increment_counter("requests_total", 100);
    collector.increment_counter("requests_success", 95);
    collector.increment_counter("requests_error", 5);

    assert_eq!(collector.get_counter("requests_total"), Some(100));
    assert_eq!(collector.get_counter("requests_success"), Some(95));
    assert_eq!(collector.get_counter("requests_error"), Some(5));

    // 测试仪表盘
    collector.set_gauge("cpu_usage", 75.5);
    collector.set_gauge("memory_usage", 1024.0 * 1024.0 * 512.0); // 512MB
    collector.set_gauge("memory_usage_percent", 60.0);

    assert_eq!(collector.get_gauge("cpu_usage"), Some(75.5));
    assert_eq!(collector.get_gauge("memory_usage_percent"), Some(60.0));

    // 测试直方图
    collector.record_histogram("response_time", 0.1);
    collector.record_histogram("response_time", 0.5);
    collector.record_histogram("response_time", 1.2);
    collector.record_histogram("response_time", 2.8);

    // 测试执行时间记录
    collector.record_execution_time("plugin_load", Duration::from_millis(150));
    collector.record_execution_time("data_process", Duration::from_millis(50));

    // 获取系统指标 - 使用不同的计数器名称以避免冲突
    collector.increment_counter("total_requests", 100);
    collector.increment_counter("successful_requests", 95);
    collector.increment_counter("failed_requests", 5);

    let system_metrics = collector.get_system_metrics();
    assert_eq!(system_metrics.total_requests, 100);
    assert_eq!(system_metrics.successful_requests, 95);
    assert_eq!(system_metrics.failed_requests, 5);
    assert_eq!(system_metrics.cpu_usage, 75.5);

    // 测试插件指标记录
    let plugin_metrics = PluginMetrics {
        plugin_id: "test_plugin".to_string(),
        load_time: Duration::from_millis(200),
        execution_count: 50,
        success_count: 48,
        error_count: 2,
        avg_execution_time: Duration::from_millis(25),
        memory_usage: 1024 * 1024, // 1MB
        last_execution: Some(std::time::SystemTime::now()),
    };

    collector.record_plugin_metrics("test_plugin", &plugin_metrics);

    // 验证插件指标已记录 - 注意record_plugin_metrics使用increment_counter，所以会累加
    assert!(collector.get_counter("plugin.test_plugin.execution_count").unwrap_or(0) >= 50);
    assert!(collector.get_counter("plugin.test_plugin.success_count").unwrap_or(0) >= 48);
    assert!(collector.get_counter("plugin.test_plugin.error_count").unwrap_or(0) >= 2);

    // 测试Prometheus导出
    let prometheus_output = collector.export_prometheus();
    assert!(prometheus_output.contains("requests_total 100"));
    assert!(prometheus_output.contains("cpu_usage 75.5"));
    assert!(prometheus_output.contains("response_time_bucket"));

    // 获取所有指标
    let all_metrics = collector.get_all_metrics();
    assert!(!all_metrics.is_empty());
    assert!(all_metrics.contains_key("requests_total"));
    assert!(all_metrics.contains_key("cpu_usage"));
}

#[tokio::test]
async fn test_dev_tools_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let dev_tools = PluginDevTools::new(temp_dir.path()).unwrap();

    // 测试创建插件项目
    let result = dev_tools.create_plugin_project("test_plugin", TemplateType::Basic);
    assert!(result.is_ok());

    // 验证项目目录已创建
    let plugin_dir = temp_dir.path().join("test_plugin");
    assert!(plugin_dir.exists());

    // 验证基本文件已创建
    assert!(plugin_dir.join("src").join("lib.rs").exists());
    assert!(plugin_dir.join("Cargo.toml").exists());

    // 测试构建插件
    let build_result = dev_tools.build_plugin("test_plugin");
    assert!(build_result.is_ok());

    // 测试运行测试
    let test_results = dev_tools.run_tests("test_plugin");
    assert!(test_results.is_ok());

    let results = test_results.unwrap();
    assert!(results.passed_count() > 0);
    assert_eq!(results.failed_count(), 0);

    // 测试运行基准测试
    let benchmark_results = dev_tools.run_benchmarks("test_plugin");
    assert!(benchmark_results.is_ok());

    let benchmarks = benchmark_results.unwrap();
    assert!(!benchmarks.is_empty());

    for benchmark in &benchmarks {
        assert!(benchmark.avg_time > Duration::ZERO);
        assert!(benchmark.throughput > 0.0);
        assert!(benchmark.memory_usage > 0);
    }

    // 测试生成文档
    let docs_result = dev_tools.generate_docs("test_plugin");
    assert!(docs_result.is_ok());
}

#[tokio::test]
async fn test_dev_tools_debugging() {
    let temp_dir = TempDir::new().unwrap();
    let mut dev_tools = PluginDevTools::new(temp_dir.path()).unwrap();

    // 启动调试会话
    let session_id = dev_tools.start_debug_session("debug_plugin").unwrap();
    assert!(!session_id.is_empty());
    assert!(session_id.contains("debug_plugin"));

    // 设置断点
    let breakpoint_result = dev_tools.set_breakpoint("src/lib.rs", 42);
    assert!(breakpoint_result.is_ok());

    let breakpoint_result2 = dev_tools.set_breakpoint("src/main.rs", 10);
    assert!(breakpoint_result2.is_ok());
}

#[tokio::test]
async fn test_template_types() {
    let temp_dir = TempDir::new().unwrap();
    let dev_tools = PluginDevTools::new(temp_dir.path()).unwrap();

    // 测试不同类型的模板
    let template_types = vec![
        TemplateType::Basic,
        TemplateType::Processor,
    ];

    for (i, template_type) in template_types.into_iter().enumerate() {
        let plugin_name = format!("test_plugin_{}", i);
        let result = dev_tools.create_plugin_project(&plugin_name, template_type.clone());
        assert!(result.is_ok(), "Failed to create plugin with template type: {:?}", template_type);

        // 验证项目目录存在
        let plugin_dir = temp_dir.path().join(&plugin_name);
        assert!(plugin_dir.exists());
    }
}

#[tokio::test]
async fn test_performance_benchmarking() {
    let temp_dir = TempDir::new().unwrap();
    let dev_tools = PluginDevTools::new(temp_dir.path()).unwrap();

    // 创建测试插件
    dev_tools.create_plugin_project("benchmark_plugin", TemplateType::Basic).unwrap();

    // 运行基准测试
    let benchmark_results = dev_tools.run_benchmarks("benchmark_plugin").unwrap();

    // 验证基准测试结果
    assert!(!benchmark_results.is_empty());

    for result in &benchmark_results {
        // 验证基本指标
        assert!(!result.name.is_empty());
        assert!(result.avg_time > Duration::ZERO);
        assert!(result.min_time <= result.avg_time);
        assert!(result.avg_time <= result.max_time);
        assert!(result.throughput > 0.0);
        assert!(result.memory_usage > 0);

        // 验证性能指标合理性
        assert!(result.avg_time < Duration::from_secs(1)); // 平均执行时间应该小于1秒
        assert!(result.throughput > 1.0); // 吞吐量应该大于1 ops/sec
        assert!(result.memory_usage < 1024 * 1024 * 1024); // 内存使用应该小于1GB
    }
}

#[tokio::test]
async fn test_metrics_integration_with_profiler() {
    let mut profiler = WasmProfiler::new();
    let collector = MetricsCollector::new();

    profiler.enable();
    profiler.start_profiling();

    // 模拟插件执行并同时收集指标
    for i in 0..10 {
        let execution_time = Duration::from_millis(50 + i * 10);

        // 记录到性能分析器
        profiler.record_function_call("plugin_execute", execution_time, true);
        profiler.record_memory_usage(1024 * 1024 * (i + 1));

        // 记录到指标收集器
        collector.increment_counter("plugin_executions", 1);
        collector.record_execution_time("plugin_execute", execution_time);
        collector.set_gauge("current_memory", (1024 * 1024 * (i + 1)) as f64);
    }

    // 获取性能报告
    let perf_report = profiler.stop_profiling().unwrap();

    // 获取系统指标
    collector.increment_counter("total_requests", 10);
    collector.increment_counter("successful_requests", 10);
    let system_metrics = collector.get_system_metrics();

    // 验证数据一致性
    assert_eq!(perf_report.function_stats.get("plugin_execute").unwrap().call_count, 10);
    assert_eq!(collector.get_counter("plugin_executions"), Some(10));
    assert_eq!(system_metrics.total_requests, 10);
    assert_eq!(system_metrics.successful_requests, 10);

    // 验证内存峰值
    assert_eq!(perf_report.memory_stats.peak_memory, 1024 * 1024 * 10);
    assert_eq!(collector.get_gauge("current_memory"), Some((1024 * 1024 * 10) as f64));
}

#[tokio::test]
async fn test_comprehensive_error_handling() {
    // 测试性能分析器错误处理
    let mut profiler = WasmProfiler::new();

    // 未启用时不应记录
    profiler.record_function_call("test", Duration::from_millis(100), true);
    assert!(profiler.stop_profiling().is_none());

    // 测试开发工具错误处理
    let result = PluginDevTools::new("/nonexistent/path");
    assert!(result.is_err());

    // 测试指标收集器的健壮性
    let collector = MetricsCollector::new();

    // 大量并发操作不应导致崩溃
    let collector = std::sync::Arc::new(collector);
    let handles: Vec<_> = (0..100).map(|i| {
        let collector = collector.clone();
        tokio::spawn(async move {
            collector.increment_counter("concurrent_test", 1);
            collector.set_gauge(&format!("gauge_{}", i), i as f64);
            collector.record_histogram("concurrent_histogram", i as f64);
        })
    }).collect();

    for handle in handles {
        handle.await.unwrap();
    }

    // 验证并发操作结果
    assert_eq!(collector.get_counter("concurrent_test"), Some(100));

    let all_metrics = collector.get_all_metrics();
    assert!(all_metrics.len() >= 102); // 至少有100个gauge + 1个counter + 1个histogram
}
