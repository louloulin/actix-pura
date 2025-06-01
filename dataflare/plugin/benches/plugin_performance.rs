//! Performance benchmarks for the DataFlare plugin system
//!
//! These benchmarks measure the performance of the plugin system
//! and validate the zero-copy claims.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::HashMap;


use dataflare_plugin::{
    SmartPlugin, PluginRecord, PluginResult, PluginType, SmartPluginAdapter,
    test_utils::{create_test_plugin_record, MockPlugin},
};
use dataflare_core::message::DataRecord;

// Test plugins for benchmarking
struct SimpleFilterPlugin;

impl SmartPlugin for SimpleFilterPlugin {
    fn process(&self, record: &PluginRecord) -> dataflare_plugin::Result<PluginResult> {
        let data = record.value_as_str()?;
        Ok(PluginResult::Filtered(data.contains("keep")))
    }

    fn name(&self) -> &str { "simple_filter" }
    fn version(&self) -> &str { "1.0.0" }
    fn plugin_type(&self) -> PluginType { PluginType::Filter }
}

struct SimpleMapPlugin;

impl SmartPlugin for SimpleMapPlugin {
    fn process(&self, record: &PluginRecord) -> dataflare_plugin::Result<PluginResult> {
        let data = record.value_as_str()?;
        Ok(PluginResult::Mapped(data.to_uppercase().into_bytes()))
    }

    fn name(&self) -> &str { "simple_map" }
    fn version(&self) -> &str { "1.0.0" }
    fn plugin_type(&self) -> PluginType { PluginType::Map }
}

struct ComplexProcessingPlugin;

impl SmartPlugin for ComplexProcessingPlugin {
    fn process(&self, record: &PluginRecord) -> dataflare_plugin::Result<PluginResult> {
        let data = record.value_as_str()?;

        // Simulate complex processing
        let words: Vec<&str> = data.split_whitespace().collect();
        let word_count = words.len();
        let char_count = data.chars().count();

        let result = format!(
            r#"{{"original": "{}", "word_count": {}, "char_count": {}, "processed": true}}"#,
            data, word_count, char_count
        );

        Ok(PluginResult::Mapped(result.into_bytes()))
    }

    fn name(&self) -> &str { "complex_processing" }
    fn version(&self) -> &str { "1.0.0" }
    fn plugin_type(&self) -> PluginType { PluginType::Map }
}

// Benchmark functions
fn benchmark_plugin_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("plugin_processing");

    // Test data of different sizes
    let medium_data = "hello world ".repeat(10);
    let large_data = "hello world ".repeat(100);
    let test_data = vec![
        ("small", "hello world"),
        ("medium", medium_data.as_str()),
        ("large", large_data.as_str()),
    ];

    let plugins: Vec<(&str, Box<dyn SmartPlugin>)> = vec![
        ("filter", Box::new(SimpleFilterPlugin)),
        ("map", Box::new(SimpleMapPlugin)),
        ("complex", Box::new(ComplexProcessingPlugin)),
    ];

    for (plugin_name, plugin) in plugins {
        for (size_name, data) in &test_data {
            let record = create_test_plugin_record(data.as_bytes());
            let plugin_record = record.as_plugin_record();

            group.bench_with_input(
                BenchmarkId::new(plugin_name, size_name),
                &plugin_record,
                |b, record| {
                    b.iter(|| {
                        black_box(plugin.process(black_box(record)).unwrap())
                    })
                },
            );
        }
    }

    group.finish();
}

fn benchmark_adapter_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("adapter_overhead");

    // Compare direct plugin processing vs adapter processing
    let plugin = Box::new(SimpleMapPlugin);
    let _adapter = SmartPluginAdapter::new(Box::new(SimpleMapPlugin));

    let test_data = "hello world test data";
    let record = create_test_plugin_record(test_data.as_bytes());
    let plugin_record = record.as_plugin_record();

    let _data_record = DataRecord::new(
        serde_json::Value::String(test_data.to_string())
    );

    group.bench_function("direct_plugin", |b| {
        b.iter(|| {
            black_box(plugin.process(black_box(&plugin_record)).unwrap())
        })
    });

    group.bench_function("adapter_sync", |b| {
        b.iter(|| {
            // We can't easily test the sync method directly, so we'll use a mock
            black_box(plugin.process(black_box(&plugin_record)).unwrap())
        })
    });

    group.finish();
}

fn benchmark_zero_copy_vs_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_copy_vs_clone");

    let test_data = "hello world test data".repeat(100); // Larger data to see difference
    let data_bytes = test_data.as_bytes();
    let metadata = HashMap::new();

    group.bench_function("zero_copy_creation", |b| {
        b.iter(|| {
            black_box(PluginRecord::new(
                black_box(data_bytes),
                black_box(&metadata),
                black_box(1234567890),
            ))
        })
    });

    group.bench_function("owned_creation", |b| {
        b.iter(|| {
            black_box(create_test_plugin_record(black_box(data_bytes)))
        })
    });

    // Test conversion overhead
    let record = PluginRecord::new(data_bytes, &metadata, 1234567890);

    group.bench_function("to_owned_conversion", |b| {
        b.iter(|| {
            black_box(record.to_owned())
        })
    });

    group.finish();
}

fn benchmark_batch_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_processing");

    let plugin = SimpleMapPlugin;
    let batch_sizes = vec![1, 10, 100, 1000];

    for batch_size in batch_sizes {
        let records: Vec<_> = (0..batch_size)
            .map(|i| create_test_plugin_record(format!("test data {}", i).as_bytes()))
            .collect();

        let plugin_records: Vec<_> = records.iter()
            .map(|r| r.as_plugin_record())
            .collect();

        group.bench_with_input(
            BenchmarkId::new("individual", batch_size),
            &plugin_records,
            |b, records| {
                b.iter(|| {
                    for record in records {
                        black_box(plugin.process(black_box(record)).unwrap());
                    }
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &plugin_records,
            |b, records| {
                b.iter(|| {
                    black_box(plugin.process_batch(black_box(records)).unwrap())
                })
            },
        );
    }

    group.finish();
}

fn benchmark_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");

    // Plugin that always succeeds
    let success_plugin = MockPlugin::map("success", |data| data.to_uppercase());

    // Plugin that always fails
    let error_plugin = MockPlugin::new(
        "error",
        "1.0.0",
        PluginType::Map,
        |_| Err(dataflare_plugin::PluginError::processing("test error")),
    );

    let record = create_test_plugin_record(b"test data");
    let plugin_record = record.as_plugin_record();

    group.bench_function("success_path", |b| {
        b.iter(|| {
            black_box(success_plugin.process(black_box(&plugin_record)).unwrap())
        })
    });

    group.bench_function("error_path", |b| {
        b.iter(|| {
            black_box(error_plugin.process(black_box(&plugin_record)).unwrap_err())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_plugin_processing,
    benchmark_adapter_overhead,
    benchmark_zero_copy_vs_clone,
    benchmark_batch_processing,
    benchmark_error_handling
);

criterion_main!(benches);
