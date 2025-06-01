//! DataFlare Plugin 性能基准测试
//!
//! 验证阶段2性能优化的实际效果

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use dataflare_plugin::{
    PluginManager, PluginManagerConfig, MemoryPool, MemoryPoolConfig, BufferSize,
    BatchPlugin, BatchConfig, BatchResult, BatchAdapter,
    core::{PluginRecord, PluginResult, PluginInfo, DataFlarePlugin},
    interface::{PluginType, PluginState},
    test_utils::{create_test_plugin_record, MockPlugin},
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 简单的测试插件
struct BenchmarkPlugin {
    name: String,
}

impl BenchmarkPlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl DataFlarePlugin for BenchmarkPlugin {
    fn process(&self, record: &PluginRecord) -> std::result::Result<PluginResult, dataflare_plugin::interface::PluginError> {
        // 简单的数据转换：转换为大写
        let data = std::str::from_utf8(record.value)
            .map_err(|e| dataflare_plugin::interface::PluginError::processing(format!("UTF-8 error: {}", e)))?;
        
        let result = data.to_uppercase();
        Ok(PluginResult::success(result.into_bytes()))
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            plugin_type: PluginType::Transformer,
            description: "Benchmark plugin".to_string(),
            author: "DataFlare Team".to_string(),
            api_version: "1.0".to_string(),
        }
    }

    fn get_state(&self) -> PluginState {
        PluginState::Ready
    }
}

impl BatchPlugin for BenchmarkPlugin {
    fn process_batch(&self, records: &[PluginRecord]) -> dataflare_plugin::core::Result<BatchResult> {
        let start_time = Instant::now();
        let mut results = Vec::new();
        let mut errors = Vec::new();

        for (idx, record) in records.iter().enumerate() {
            match self.process(record) {
                Ok(result) => results.push(result),
                Err(error) => errors.push((idx, dataflare_plugin::core::PluginError::Processing(error.to_string()))),
            }
        }

        let processing_time = start_time.elapsed();
        
        Ok(BatchResult {
            results,
            errors,
            processed_count: records.len(),
            success_count: results.len(),
            error_count: errors.len(),
            processing_time,
            throughput_per_second: if processing_time.as_secs_f64() > 0.0 {
                records.len() as f64 / processing_time.as_secs_f64()
            } else {
                0.0
            },
        })
    }

    fn optimize_batch_size(&self, current_size: usize, _latency: Duration, _throughput: f64) -> usize {
        // 简单的优化策略
        if current_size < 100 {
            current_size * 2
        } else if current_size > 1000 {
            current_size / 2
        } else {
            current_size
        }
    }

    fn info(&self) -> PluginInfo {
        DataFlarePlugin::info(self)
    }

    fn get_state(&self) -> PluginState {
        DataFlarePlugin::get_state(self)
    }
}

/// 内存池性能测试
fn bench_memory_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pool");
    
    // 测试不同大小的缓冲区分配
    let sizes = vec![
        (BufferSize::Tiny, 256),
        (BufferSize::Small, 4096),
        (BufferSize::Medium, 65536),
        (BufferSize::Large, 1048576),
    ];

    for (buffer_size, size_bytes) in sizes {
        let config = MemoryPoolConfig {
            max_tiny_buffers: 1000,
            max_small_buffers: 500,
            max_medium_buffers: 100,
            max_large_buffers: 50,
            max_huge_buffers: 10,
            cleanup_interval: Duration::from_secs(60),
            max_idle_time: Duration::from_secs(300),
            enable_metrics: true,
        };
        
        let pool = MemoryPool::new(config);
        pool.warmup();

        group.bench_with_input(
            BenchmarkId::new("allocation", format!("{:?}", buffer_size)),
            &size_bytes,
            |b, &size| {
                b.iter(|| {
                    let buffer = pool.get_buffer(size);
                    black_box(&buffer);
                    pool.return_buffer(buffer);
                });
            },
        );
    }

    group.finish();
}

/// 单记录处理性能测试
fn bench_single_record_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_record");
    
    let plugin = BenchmarkPlugin::new("benchmark");
    let test_data = b"hello world this is a test message for benchmarking";
    let record = create_test_plugin_record(test_data);
    let plugin_record = record.as_plugin_record();

    group.bench_function("direct_processing", |b| {
        b.iter(|| {
            let result = plugin.process(black_box(&plugin_record));
            black_box(result);
        });
    });

    group.finish();
}

/// 批处理性能测试
fn bench_batch_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_processing");
    
    let plugin = BenchmarkPlugin::new("benchmark");
    
    // 测试不同批次大小
    let batch_sizes = vec![10, 50, 100, 500, 1000];
    
    for batch_size in batch_sizes {
        let records: Vec<_> = (0..batch_size)
            .map(|i| {
                let data = format!("test message number {}", i);
                create_test_plugin_record(data.as_bytes())
            })
            .collect();
        
        let plugin_records: Vec<_> = records.iter()
            .map(|r| r.as_plugin_record())
            .collect();

        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            &plugin_records,
            |b, records| {
                b.iter(|| {
                    let result = plugin.process_batch(black_box(records));
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// 插件管理器性能测试
fn bench_plugin_manager(c: &mut Criterion) {
    let mut group = c.benchmark_group("plugin_manager");
    
    let config = PluginManagerConfig::default();
    let manager = PluginManager::new(config).expect("Failed to create plugin manager");
    
    let plugin = BenchmarkPlugin::new("benchmark");
    manager.register_plugin("benchmark".to_string(), plugin)
        .expect("Failed to register plugin");

    let test_data = b"hello world this is a test message";
    let record = create_test_plugin_record(test_data);
    let plugin_record = record.as_plugin_record();

    group.bench_function("manager_single_record", |b| {
        b.iter(|| {
            let result = manager.process_record("benchmark", black_box(&plugin_record));
            black_box(result);
        });
    });

    // 批处理测试
    let batch_size = 100;
    let records: Vec<_> = (0..batch_size)
        .map(|i| {
            let data = format!("test message {}", i);
            create_test_plugin_record(data.as_bytes())
        })
        .collect();
    
    let plugin_records: Vec<_> = records.iter()
        .map(|r| r.as_plugin_record())
        .collect();

    group.bench_function("manager_batch_processing", |b| {
        b.iter(|| {
            let result = manager.process_batch("benchmark", black_box(&plugin_records));
            black_box(result);
        });
    });

    group.finish();
}

/// 内存使用效率测试
fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");
    
    // 比较有无内存池的性能差异
    let test_data = b"test data for memory efficiency benchmark";
    
    // 无内存池版本（直接分配）
    group.bench_function("without_pool", |b| {
        b.iter(|| {
            let mut buffer = Vec::with_capacity(test_data.len());
            buffer.extend_from_slice(black_box(test_data));
            black_box(buffer);
        });
    });

    // 有内存池版本
    let config = MemoryPoolConfig::default();
    let pool = MemoryPool::new(config);
    pool.warmup();

    group.bench_function("with_pool", |b| {
        b.iter(|| {
            let buffer = pool.get_buffer(black_box(test_data.len()));
            black_box(&buffer);
            pool.return_buffer(buffer);
        });
    });

    group.finish();
}

/// 并发性能测试
fn bench_concurrent_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent");
    
    let config = PluginManagerConfig::default();
    let manager = std::sync::Arc::new(PluginManager::new(config).expect("Failed to create manager"));
    
    let plugin = BenchmarkPlugin::new("concurrent_benchmark");
    manager.register_plugin("concurrent_benchmark".to_string(), plugin)
        .expect("Failed to register plugin");

    let test_data = b"concurrent test message";
    let record = create_test_plugin_record(test_data);
    let plugin_record = record.as_plugin_record();

    group.bench_function("concurrent_single_thread", |b| {
        let manager = manager.clone();
        b.iter(|| {
            let result = manager.process_record("concurrent_benchmark", black_box(&plugin_record));
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_pool,
    bench_single_record_processing,
    bench_batch_processing,
    bench_plugin_manager,
    bench_memory_efficiency,
    bench_concurrent_processing
);

criterion_main!(benches);
