//! WASM插件系统性能基准测试

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use dataflare_wasm::*;
use serde_json::json;
use std::collections::HashMap;
use tokio::runtime::Runtime;

/// 基准测试：WASM运行时创建
fn bench_runtime_creation(c: &mut Criterion) {
    c.bench_function("wasm_runtime_creation", |b| {
        b.iter(|| {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                let config = runtime::WasmRuntimeConfig::default();
                let runtime = WasmRuntime::new(config);
                black_box(runtime)
            })
        })
    });
}

/// 基准测试：组件管理器操作
fn bench_component_manager(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("component_manager_creation", |b| {
        b.iter(|| {
            let manager = components::WasmComponentManager::new();
            black_box(manager)
        })
    });
    
    c.bench_function("component_registration", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut manager = components::WasmComponentManager::new();
                let config = components::WasmComponentConfig {
                    component_type: components::WasmComponentType::Transformer,
                    name: format!("test_component_{}", fastrand::u32(..)),
                    module_path: "test.wasm".to_string(),
                    config: None,
                    runtime_config: None,
                    metadata: None,
                };
                
                let result = manager.register_component(config).await;
                black_box(result)
            })
        })
    });
}

/// 基准测试：插件配置序列化
fn bench_plugin_config_serialization(c: &mut Criterion) {
    let config = WasmPluginConfig {
        name: "benchmark_plugin".to_string(),
        module_path: "./plugins/benchmark.wasm".to_string(),
        config: {
            let mut map = HashMap::new();
            map.insert("param1".to_string(), json!("value1"));
            map.insert("param2".to_string(), json!(42));
            map.insert("param3".to_string(), json!(true));
            map
        },
        security_policy: sandbox::SecurityPolicy::default(),
        memory_limit: 16 * 1024 * 1024,
        timeout_ms: 10000,
    };
    
    c.bench_function("plugin_config_serialize", |b| {
        b.iter(|| {
            let serialized = serde_json::to_string(&config);
            black_box(serialized)
        })
    });
    
    let serialized = serde_json::to_string(&config).unwrap();
    c.bench_function("plugin_config_deserialize", |b| {
        b.iter(|| {
            let deserialized: Result<WasmPluginConfig, _> = serde_json::from_str(&serialized);
            black_box(deserialized)
        })
    });
}

/// 基准测试：不同大小的数据处理
fn bench_data_processing_by_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("data_processing");
    
    for size in [100, 1000, 10000, 100000].iter() {
        let data = generate_test_data(*size);
        
        group.bench_with_input(BenchmarkId::new("json_parsing", size), size, |b, _| {
            b.iter(|| {
                let parsed: serde_json::Value = serde_json::from_str(&data).unwrap();
                black_box(parsed)
            })
        });
        
        group.bench_with_input(BenchmarkId::new("json_serialization", size), size, |b, _| {
            let parsed: serde_json::Value = serde_json::from_str(&data).unwrap();
            b.iter(|| {
                let serialized = serde_json::to_string(&parsed).unwrap();
                black_box(serialized)
            })
        });
    }
    
    group.finish();
}

/// 基准测试：主机函数调用
fn bench_host_functions(c: &mut Criterion) {
    let registry = host_functions::HostFunctionRegistry::new();
    
    c.bench_function("host_function_lookup", |b| {
        b.iter(|| {
            let functions = registry.list_functions();
            black_box(functions)
        })
    });
    
    c.bench_function("host_function_registration", |b| {
        b.iter(|| {
            let mut registry = host_functions::HostFunctionRegistry::new();
            registry.register_function(
                "test_function".to_string(),
                host_functions::HostFunction {
                    name: "test_function".to_string(),
                    description: "Test function".to_string(),
                    parameters: vec![],
                    return_type: "void".to_string(),
                    handler: Box::new(|_| Ok(json!(null))),
                }
            );
            black_box(registry)
        })
    });
}

/// 基准测试：安全策略验证
fn bench_security_policy(c: &mut Criterion) {
    let policy = sandbox::SecurityPolicy::default();
    
    c.bench_function("security_policy_creation", |b| {
        b.iter(|| {
            let policy = sandbox::SecurityPolicy::default();
            black_box(policy)
        })
    });
    
    c.bench_function("security_policy_validation", |b| {
        b.iter(|| {
            let result = policy.validate_file_access("/tmp/test.txt");
            black_box(result)
        })
    });
    
    c.bench_function("security_policy_network_check", |b| {
        b.iter(|| {
            let result = policy.validate_network_access("example.com", 80);
            black_box(result)
        })
    });
}

/// 基准测试：错误处理性能
fn bench_error_handling(c: &mut Criterion) {
    c.bench_function("error_creation", |b| {
        b.iter(|| {
            let error = WasmError::Configuration("test error".to_string());
            black_box(error)
        })
    });
    
    c.bench_function("error_display", |b| {
        let error = WasmError::Runtime("test runtime error".to_string());
        b.iter(|| {
            let display = error.to_string();
            black_box(display)
        })
    });
    
    c.bench_function("result_handling", |b| {
        b.iter(|| {
            let result: WasmResult<String> = Ok("success".to_string());
            let handled = result.map(|s| s.len());
            black_box(handled)
        })
    });
}

/// 基准测试：并发操作
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("concurrent_component_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let tasks: Vec<_> = (0..10).map(|i| {
                    tokio::spawn(async move {
                        let manager = components::WasmComponentManager::new();
                        let config = components::WasmComponentConfig {
                            component_type: components::WasmComponentType::Processor,
                            name: format!("concurrent_component_{}", i),
                            module_path: "test.wasm".to_string(),
                            config: None,
                            runtime_config: None,
                            metadata: None,
                        };
                        manager
                    })
                }).collect();
                
                let results = futures::future::join_all(tasks).await;
                black_box(results)
            })
        })
    });
}

/// 生成测试数据
fn generate_test_data(size: usize) -> String {
    let mut data = Vec::new();
    for i in 0..size {
        data.push(json!({
            "id": i,
            "name": format!("item_{}", i),
            "value": fastrand::f64(),
            "active": fastrand::bool(),
            "tags": vec![
                format!("tag_{}", i % 10),
                format!("category_{}", i % 5)
            ]
        }));
    }
    serde_json::to_string(&data).unwrap()
}

criterion_group!(
    benches,
    bench_runtime_creation,
    bench_component_manager,
    bench_plugin_config_serialization,
    bench_data_processing_by_size,
    bench_host_functions,
    bench_security_policy,
    bench_error_handling,
    bench_concurrent_operations
);

criterion_main!(benches);
