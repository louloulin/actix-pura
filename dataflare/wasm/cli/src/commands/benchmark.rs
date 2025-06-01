//! Benchmark command implementation

use anyhow::Result;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use log::{info, warn};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use crate::commands::CommandUtils;
use crate::config::PluginConfig;

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub iterations: u32,
    pub warmup_iterations: u32,
    pub data_sizes: Vec<usize>,
    pub concurrent_workers: u32,
    pub timeout_ms: u64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 1000,
            warmup_iterations: 100,
            data_sizes: vec![100, 1000, 10000, 100000],
            concurrent_workers: 1,
            timeout_ms: 30000,
        }
    }
}

/// Benchmark result for a single test
#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub test_name: String,
    pub data_size: usize,
    pub iterations: u32,
    pub total_duration_ms: u64,
    pub avg_duration_ns: u64,
    pub min_duration_ns: u64,
    pub max_duration_ns: u64,
    pub median_duration_ns: u64,
    pub p95_duration_ns: u64,
    pub p99_duration_ns: u64,
    pub throughput_ops_per_sec: f64,
    pub memory_usage_mb: f64,
}

/// Complete benchmark report
#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub plugin_name: String,
    pub plugin_version: String,
    pub plugin_type: String,
    pub timestamp: String,
    pub config: BenchmarkConfigSummary,
    pub results: Vec<BenchmarkResult>,
    pub summary: BenchmarkSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkConfigSummary {
    pub iterations: u32,
    pub data_sizes: Vec<usize>,
    pub concurrent_workers: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub total_tests: usize,
    pub total_duration_ms: u64,
    pub fastest_test: String,
    pub slowest_test: String,
    pub avg_throughput_ops_per_sec: f64,
    pub peak_memory_usage_mb: f64,
}

/// Execute the benchmark command
pub async fn execute(
    iterations: u32,
    filter: Option<String>,
) -> Result<()> {
    info!("Running performance benchmarks...");

    // Ensure we're in a plugin project
    if !CommandUtils::is_plugin_project() {
        anyhow::bail!("Not in a plugin project directory. Run 'dataflare-plugin new' to create a new project.");
    }

    // Load plugin configuration
    let config_path = CommandUtils::find_plugin_config()?;
    let config = PluginConfig::load_from_file(&config_path)?;

    info!("Benchmarking plugin: {}", config.plugin.name.cyan().bold());

    // Create benchmark configuration
    let mut bench_config = BenchmarkConfig::default();
    bench_config.iterations = iterations;

    // Create progress bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());

    // Step 1: Build plugin for benchmarking
    pb.set_message("Building plugin for benchmarking...");
    build_plugin_for_benchmark(&config).await?;

    // Step 2: Run warmup
    pb.set_message("Running warmup iterations...");
    run_warmup(&config, &bench_config).await?;

    // Step 3: Run benchmarks
    pb.set_message("Running performance benchmarks...");
    let results = run_benchmarks(&config, &bench_config, filter.as_deref()).await?;

    // Step 4: Generate report
    pb.set_message("Generating benchmark report...");
    let report = generate_benchmark_report(&config, &bench_config, results)?;

    // Step 5: Save report
    pb.set_message("Saving benchmark report...");
    save_benchmark_report(&report)?;

    pb.finish_with_message("Benchmarks completed successfully!");

    // Print benchmark summary
    print_benchmark_summary(&report);

    Ok(())
}

/// Build plugin for benchmarking
async fn build_plugin_for_benchmark(config: &PluginConfig) -> Result<()> {
    // Use the build command to ensure we have an optimized build
    crate::commands::build::execute(
        true,  // release mode
        true,  // optimize
        "wasm32-wasip1".to_string(),
        None,
    ).await
}

/// Run warmup iterations
async fn run_warmup(config: &PluginConfig, bench_config: &BenchmarkConfig) -> Result<()> {
    info!("Running {} warmup iterations...", bench_config.warmup_iterations);

    // Generate test data
    let test_data = generate_test_data(1000); // Use medium size for warmup

    for _ in 0..bench_config.warmup_iterations {
        // Simulate plugin execution
        simulate_plugin_execution(&test_data)?;
    }

    Ok(())
}

/// Run all benchmarks
async fn run_benchmarks(
    config: &PluginConfig,
    bench_config: &BenchmarkConfig,
    filter: Option<&str>,
) -> Result<Vec<BenchmarkResult>> {
    let mut results = Vec::new();

    // Run benchmarks for different data sizes
    for &data_size in &bench_config.data_sizes {
        let test_name = format!("data_processing_{}", data_size);

        // Apply filter if specified
        if let Some(filter) = filter {
            if !test_name.contains(filter) {
                continue;
            }
        }

        info!("Running benchmark: {} (data size: {})", test_name.yellow(), data_size);

        let result = run_single_benchmark(&test_name, data_size, bench_config).await?;
        results.push(result);
    }

    // Run plugin-specific benchmarks
    // TODO: Add plugin_type field to PluginInfo to enable type-specific benchmarks
    results.extend(run_filter_benchmarks(bench_config, filter).await?);
    results.extend(run_map_benchmarks(bench_config, filter).await?);
    results.extend(run_aggregate_benchmarks(bench_config, filter).await?);

    Ok(results)
}

/// Run a single benchmark test
async fn run_single_benchmark(
    test_name: &str,
    data_size: usize,
    bench_config: &BenchmarkConfig,
) -> Result<BenchmarkResult> {
    let test_data = generate_test_data(data_size);
    let mut durations = Vec::new();

    // Run iterations
    for _ in 0..bench_config.iterations {
        let start = Instant::now();
        simulate_plugin_execution(&test_data)?;
        let duration = start.elapsed();
        durations.push(duration.as_nanos() as u64);
    }

    // Calculate statistics
    durations.sort_unstable();
    let total_duration_ms = durations.iter().sum::<u64>() / 1_000_000;
    let avg_duration_ns = durations.iter().sum::<u64>() / durations.len() as u64;
    let min_duration_ns = *durations.first().unwrap();
    let max_duration_ns = *durations.last().unwrap();
    let median_duration_ns = durations[durations.len() / 2];
    let p95_duration_ns = durations[(durations.len() as f64 * 0.95) as usize];
    let p99_duration_ns = durations[(durations.len() as f64 * 0.99) as usize];

    // Calculate throughput (operations per second)
    let avg_duration_sec = avg_duration_ns as f64 / 1_000_000_000.0;
    let throughput_ops_per_sec = 1.0 / avg_duration_sec;

    // Estimate memory usage (placeholder)
    let memory_usage_mb = estimate_memory_usage(data_size);

    Ok(BenchmarkResult {
        test_name: test_name.to_string(),
        data_size,
        iterations: bench_config.iterations,
        total_duration_ms,
        avg_duration_ns,
        min_duration_ns,
        max_duration_ns,
        median_duration_ns,
        p95_duration_ns,
        p99_duration_ns,
        throughput_ops_per_sec,
        memory_usage_mb,
    })
}

/// Run filter-specific benchmarks
async fn run_filter_benchmarks(
    bench_config: &BenchmarkConfig,
    filter: Option<&str>,
) -> Result<Vec<BenchmarkResult>> {
    let mut results = Vec::new();

    let tests = vec![
        ("filter_string_contains", 1000),
        ("filter_regex_match", 1000),
        ("filter_json_field", 1000),
    ];

    for (test_name, data_size) in tests {
        if let Some(filter) = filter {
            if !test_name.contains(filter) {
                continue;
            }
        }

        let result = run_single_benchmark(test_name, data_size, bench_config).await?;
        results.push(result);
    }

    Ok(results)
}

/// Run map-specific benchmarks
async fn run_map_benchmarks(
    bench_config: &BenchmarkConfig,
    filter: Option<&str>,
) -> Result<Vec<BenchmarkResult>> {
    let mut results = Vec::new();

    let tests = vec![
        ("map_string_transform", 1000),
        ("map_json_transform", 1000),
        ("map_data_enrichment", 1000),
    ];

    for (test_name, data_size) in tests {
        if let Some(filter) = filter {
            if !test_name.contains(filter) {
                continue;
            }
        }

        let result = run_single_benchmark(test_name, data_size, bench_config).await?;
        results.push(result);
    }

    Ok(results)
}

/// Run aggregate-specific benchmarks
async fn run_aggregate_benchmarks(
    bench_config: &BenchmarkConfig,
    filter: Option<&str>,
) -> Result<Vec<BenchmarkResult>> {
    let mut results = Vec::new();

    let tests = vec![
        ("aggregate_count", 10000),
        ("aggregate_sum", 10000),
        ("aggregate_group_by", 10000),
    ];

    for (test_name, data_size) in tests {
        if let Some(filter) = filter {
            if !test_name.contains(filter) {
                continue;
            }
        }

        let result = run_single_benchmark(test_name, data_size, bench_config).await?;
        results.push(result);
    }

    Ok(results)
}

/// Generate test data of specified size
fn generate_test_data(size: usize) -> Vec<u8> {
    // Generate JSON test data
    let data = serde_json::json!({
        "id": fastrand::u64(..),
        "name": format!("test_item_{}", fastrand::u32(..)),
        "value": fastrand::f64(),
        "active": fastrand::bool(),
        "tags": (0..5).map(|i| format!("tag_{}", i)).collect::<Vec<_>>(),
        "metadata": {
            "created_at": chrono::Utc::now().to_rfc3339(),
            "source": "benchmark",
            "size": size
        },
        "payload": "x".repeat(size.saturating_sub(200)) // Adjust for JSON overhead
    });

    serde_json::to_vec(&data).unwrap_or_else(|_| vec![0; size])
}

/// Simulate plugin execution (placeholder)
fn simulate_plugin_execution(_data: &[u8]) -> Result<()> {
    // TODO: Actually load and execute the WASM plugin
    // For now, simulate some processing time
    std::thread::sleep(Duration::from_nanos(fastrand::u64(50..200)));
    Ok(())
}

/// Estimate memory usage for given data size
fn estimate_memory_usage(data_size: usize) -> f64 {
    // Rough estimation: data size + overhead
    (data_size as f64 * 1.5) / 1_048_576.0 // Convert to MB
}

/// Generate benchmark report
fn generate_benchmark_report(
    config: &PluginConfig,
    bench_config: &BenchmarkConfig,
    results: Vec<BenchmarkResult>,
) -> Result<BenchmarkReport> {
    let total_tests = results.len();
    let total_duration_ms = results.iter().map(|r| r.total_duration_ms).sum();
    let avg_throughput = results.iter().map(|r| r.throughput_ops_per_sec).sum::<f64>() / results.len() as f64;
    let peak_memory = results.iter().map(|r| r.memory_usage_mb).fold(0.0, f64::max);

    let fastest_test = results.iter()
        .min_by_key(|r| r.avg_duration_ns)
        .map(|r| r.test_name.clone())
        .unwrap_or_else(|| "N/A".to_string());

    let slowest_test = results.iter()
        .max_by_key(|r| r.avg_duration_ns)
        .map(|r| r.test_name.clone())
        .unwrap_or_else(|| "N/A".to_string());

    Ok(BenchmarkReport {
        plugin_name: config.plugin.name.clone(),
        plugin_version: config.plugin.version.clone(),
        plugin_type: "unknown".to_string(), // TODO: Add plugin_type to PluginInfo
        timestamp: chrono::Utc::now().to_rfc3339(),
        config: BenchmarkConfigSummary {
            iterations: bench_config.iterations,
            data_sizes: bench_config.data_sizes.clone(),
            concurrent_workers: bench_config.concurrent_workers,
        },
        results,
        summary: BenchmarkSummary {
            total_tests,
            total_duration_ms,
            fastest_test,
            slowest_test,
            avg_throughput_ops_per_sec: avg_throughput,
            peak_memory_usage_mb: peak_memory,
        },
    })
}

/// Save benchmark report to file
fn save_benchmark_report(report: &BenchmarkReport) -> Result<()> {
    let root_dir = CommandUtils::get_plugin_root()?;
    let report_file = root_dir.join(format!("benchmark_report_{}.json",
        chrono::Utc::now().format("%Y%m%d_%H%M%S")));

    let report_json = serde_json::to_string_pretty(report)?;
    std::fs::write(&report_file, report_json)?;

    info!("Benchmark report saved to: {}", report_file.display());
    Ok(())
}

/// Print benchmark summary
fn print_benchmark_summary(report: &BenchmarkReport) {
    println!("\n{}", "🚀 Benchmark Results".green().bold());
    println!("  Plugin: {} v{}", report.plugin_name.cyan(), report.plugin_version.yellow());
    println!("  Type: {}", report.plugin_type.blue());
    println!("  Tests: {}", report.summary.total_tests.to_string().cyan());
    println!("  Total Duration: {}ms", report.summary.total_duration_ms.to_string().yellow());
    println!("  Avg Throughput: {:.2} ops/sec", report.summary.avg_throughput_ops_per_sec.to_string().green());
    println!("  Peak Memory: {:.2} MB", report.summary.peak_memory_usage_mb.to_string().magenta());

    println!("\n{}", "📊 Performance Summary:".green().bold());
    println!("  Fastest Test: {}", report.summary.fastest_test.green());
    println!("  Slowest Test: {}", report.summary.slowest_test.red());

    println!("\n{}", "📋 Detailed Results:".green().bold());
    for result in &report.results {
        println!("  • {} ({})",
            result.test_name.cyan(),
            format_data_size(result.data_size).yellow()
        );
        println!("    Avg: {}ns | P95: {}ns | Throughput: {:.2} ops/sec",
            result.avg_duration_ns.to_string().green(),
            result.p95_duration_ns.to_string().yellow(),
            result.throughput_ops_per_sec.to_string().blue()
        );
    }

    println!("\n{}", "✅ Benchmark completed successfully!".green().bold());
}

/// Format data size in human readable format
fn format_data_size(size: usize) -> String {
    if size >= 1_000_000 {
        format!("{:.1}M", size as f64 / 1_000_000.0)
    } else if size >= 1_000 {
        format!("{:.1}K", size as f64 / 1_000.0)
    } else {
        format!("{}", size)
    }
}
