//! 批处理系统性能基准测试
//! 
//! 测量批处理系统在不同配置下的吞吐量和延迟

use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use dataflare_core::{
    message::{DataRecord, DataRecordBatch},
    connector::{BatchSourceConnector, BatchDestinationConnector, Position},
};

use dataflare_runtime::batch::{
    SharedDataBatch, AdaptiveBatcher, AdaptiveBatchingConfig,
    BackpressureController, CreditConfig, CreditMode,
};

use serde_json::json;
use tokio::runtime::Runtime;

/// 创建测试记录批次
fn create_test_batch(size: usize) -> SharedDataBatch {
    let mut records = Vec::with_capacity(size);
    
    for i in 0..size {
        let record = DataRecord::new(json!({
            "id": i,
            "name": format!("test{}", i),
            "value": i * 10,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "data": "这是一些示例数据，足够大以模拟真实场景".repeat(i % 5 + 1),
        }));
        
        records.push(record);
    }
    
    SharedDataBatch::new(records)
}

/// 测试批处理计算任务（模拟转换操作）
fn test_batch_process(batch: &SharedDataBatch) -> SharedDataBatch {
    // 创建转换后的批次
    let processed = batch.map(|record| {
        // 模拟记录处理：读取值，进行计算，生成新记录
        let id = record.get::<i64>("id").unwrap_or(0);
        let value = record.get::<i64>("value").unwrap_or(0);
        
        // 创建新记录（进行一些计算）
        DataRecord::new(json!({
            "id": id,
            "processed_value": value * 2,
            "status": "processed",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    });
    
    processed
}

/// 测试无背压情况下的吞吐量
fn benchmark_throughput_no_backpressure(batch_size: usize, total_records: usize) -> (f64, Duration) {
    let start_time = Instant::now();
    let iterations = total_records / batch_size;
    
    let mut total_processed = 0;
    
    for _ in 0..iterations {
        let batch = create_test_batch(batch_size);
        let processed = test_batch_process(&batch);
        total_processed += processed.len();
    }
    
    let elapsed = start_time.elapsed();
    let throughput = total_processed as f64 / elapsed.as_secs_f64();
    
    (throughput, elapsed)
}

/// 测试使用自适应批大小的吞吐量
fn benchmark_throughput_adaptive(config: AdaptiveBatchingConfig, total_records: usize) -> (f64, Duration) {
    let mut batcher = AdaptiveBatcher::new(config);
    let start_time = Instant::now();
    
    let mut total_processed = 0;
    let mut remaining = total_records;
    
    while remaining > 0 {
        // 获取当前推荐的批大小
        let batch_size = batcher.get_batch_size().min(remaining);
        
        // 开始批处理
        batcher.start_batch();
        
        // 创建并处理批次
        let batch = create_test_batch(batch_size);
        let processed = test_batch_process(&batch);
        
        // 批处理完成
        batcher.complete_batch(processed.len());
        
        total_processed += processed.len();
        remaining -= batch_size;
    }
    
    let elapsed = start_time.elapsed();
    let throughput = total_processed as f64 / elapsed.as_secs_f64();
    
    (throughput, elapsed)
}

/// 测试使用背压控制的吞吐量
fn benchmark_throughput_with_backpressure(
    credit_config: CreditConfig, 
    batch_size: usize,
    total_records: usize
) -> (f64, Duration) {
    let mut controller = BackpressureController::new(credit_config);
    let start_time = Instant::now();
    
    let mut total_processed = 0;
    let mut remaining = total_records;
    
    while remaining > 0 {
        // 请求处理信用
        let credits = match controller.request_credits(batch_size) {
            Some(c) => c.min(remaining),
            None => {
                // 模拟等待一段时间（背压状态）
                thread::sleep(Duration::from_millis(1));
                continue;
            }
        };
        
        // 创建并处理批次
        let start_process = Instant::now();
        let batch = create_test_batch(credits);
        let processed = test_batch_process(&batch);
        let process_time = start_process.elapsed();
        
        // 更新队列状态（模拟实际队列大小）
        controller.update_stats(remaining, process_time);
        
        total_processed += processed.len();
        remaining -= credits;
    }
    
    let elapsed = start_time.elapsed();
    let throughput = total_processed as f64 / elapsed.as_secs_f64();
    
    (throughput, elapsed)
}

/// 测试多线程并行批处理吞吐量
fn benchmark_throughput_parallel(
    batch_size: usize, 
    total_records: usize,
    threads: usize
) -> (f64, Duration) {
    let start_time = Instant::now();
    
    // 创建共享计数器
    let remaining = Arc::new(AtomicUsize::new(total_records));
    let processed_count = Arc::new(AtomicUsize::new(0));
    
    // 创建线程处理批次
    let mut handles = Vec::with_capacity(threads);
    
    for _ in 0..threads {
        let remaining_clone = Arc::clone(&remaining);
        let processed_clone = Arc::clone(&processed_count);
        
        let handle = thread::spawn(move || {
            while remaining_clone.load(Ordering::Relaxed) > 0 {
                // 尝试获取工作单元
                let to_process = batch_size.min(
                    remaining_clone.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| {
                        if x >= batch_size {
                            Some(x - batch_size)
                        } else if x > 0 {
                            Some(0)
                        } else {
                            None
                        }
                    }).unwrap_or(0)
                );
                
                if to_process == 0 {
                    break;
                }
                
                // 创建并处理批次
                let batch = create_test_batch(to_process);
                let processed = test_batch_process(&batch);
                
                // 更新已处理计数
                processed_clone.fetch_add(processed.len(), Ordering::Relaxed);
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
    
    let elapsed = start_time.elapsed();
    let total_processed = processed_count.load(Ordering::Relaxed);
    let throughput = total_processed as f64 / elapsed.as_secs_f64();
    
    (throughput, elapsed)
}

/// 测试零拷贝批处理吞吐量（多阶段处理管道）
fn benchmark_zero_copy_pipeline(batch_size: usize, total_records: usize) -> (f64, Duration) {
    let start_time = Instant::now();
    
    let mut total_processed = 0;
    let iterations = total_records / batch_size;
    
    for _ in 0..iterations {
        // 第一阶段：创建批次
        let batch = create_test_batch(batch_size);
        
        // 第二阶段：过滤批次（零拷贝操作）
        let filtered = batch.filter(|record| {
            let id = record.get::<i64>("id").unwrap_or(0);
            id % 2 == 0 // 只保留偶数ID
        });
        
        // 第三阶段：转换批次（零拷贝操作）
        let transformed = filtered.map(|record| {
            let id = record.get::<i64>("id").unwrap_or(0);
            let value = record.get::<i64>("value").unwrap_or(0);
            
            DataRecord::new(json!({
                "id": id,
                "transformed_value": value * 3,
                "stage": "transformed",
            }))
        });
        
        // 第四阶段：再次过滤（零拷贝操作）
        let final_batch = transformed.filter(|record| {
            let value = record.get::<i64>("transformed_value").unwrap_or(0);
            value > 100 // 只保留大值
        });
        
        total_processed += final_batch.len();
    }
    
    let elapsed = start_time.elapsed();
    let throughput = total_processed as f64 / elapsed.as_secs_f64();
    
    (throughput, elapsed)
}

/// 运行所有基准测试
fn run_all_benchmarks() {
    println!("=== DataFlare批处理系统性能基准测试 ===");
    println!("目标吞吐量: 500,000 记录/秒");
    println!("");
    
    // 测试参数
    let total_records = 1_000_000; // 总测试记录数
    
    // 1. 测试不同批大小的吞吐量
    println!("1. 不同批大小的吞吐量测试（无背压控制）:");
    let batch_sizes = [100, 1000, 5000, 10000, 50000];
    
    for &size in &batch_sizes {
        let (throughput, elapsed) = benchmark_throughput_no_backpressure(size, total_records);
        
        println!("  批大小 {:6}: {:10.0} 记录/秒 ({:7.2}% 目标) - 耗时: {:?}",
            size, throughput, (throughput / 500000.0) * 100.0, elapsed);
    }
    println!("");
    
    // 2. 测试自适应批处理
    println!("2. 自适应批处理吞吐量测试:");
    let configs = [
        AdaptiveBatchingConfig {
            initial_size: 1000,
            min_size: 100,
            max_size: 10000,
            throughput_target: 100000,
            latency_target_ms: 50,
            adaptation_rate: 0.2,
            stability_threshold: 3,
        },
        AdaptiveBatchingConfig {
            initial_size: 5000,
            min_size: 1000,
            max_size: 50000,
            throughput_target: 300000,
            latency_target_ms: 100,
            adaptation_rate: 0.3,
            stability_threshold: 2,
        },
        AdaptiveBatchingConfig {
            initial_size: 10000,
            min_size: 5000,
            max_size: 100000,
            throughput_target: 500000,
            latency_target_ms: 200,
            adaptation_rate: 0.4,
            stability_threshold: 1,
        },
    ];
    
    for (i, config) in configs.iter().enumerate() {
        let (throughput, elapsed) = benchmark_throughput_adaptive(config.clone(), total_records);
        
        println!("  配置 {:1}: 初始={:5}, 最小={:5}, 最大={:6} - {:10.0} 记录/秒 ({:7.2}% 目标) - 耗时: {:?}",
            i+1, config.initial_size, config.min_size, config.max_size, 
            throughput, (throughput / 500000.0) * 100.0, elapsed);
    }
    println!("");
    
    // 3. 测试背压控制
    println!("3. 背压控制系统吞吐量测试:");
    let backpressure_configs = [
        (CreditConfig {
            base_credits: 1000,
            min_credits: 100,
            max_credits: 10000,
            credit_interval_ms: 10,
            mode: CreditMode::Fixed,
            target_queue_size: 5000,
            target_processing_time_ms: 50,
        }, 1000),
        (CreditConfig {
            base_credits: 5000,
            min_credits: 1000,
            max_credits: 50000,
            credit_interval_ms: 50,
            mode: CreditMode::Adaptive,
            target_queue_size: 10000,
            target_processing_time_ms: 100,
        }, 5000),
        (CreditConfig {
            base_credits: 10000,
            min_credits: 5000,
            max_credits: 100000,
            credit_interval_ms: 100,
            mode: CreditMode::TimeBased,
            target_queue_size: 20000,
            target_processing_time_ms: 200,
        }, 10000),
    ];
    
    for (i, (config, batch_size)) in backpressure_configs.iter().enumerate() {
        let (throughput, elapsed) = benchmark_throughput_with_backpressure(
            config.clone(), *batch_size, total_records);
        
        println!("  配置 {:1}: 模式={:?}, 批大小={:5} - {:10.0} 记录/秒 ({:7.2}% 目标) - 耗时: {:?}",
            i+1, config.mode, batch_size, 
            throughput, (throughput / 500000.0) * 100.0, elapsed);
    }
    println!("");
    
    // 4. 测试并行处理
    println!("4. 多线程并行处理吞吐量测试:");
    let thread_configs = [(1, 10000), (2, 5000), (4, 5000), (8, 2500)];
    
    for &(threads, batch_size) in &thread_configs {
        let (throughput, elapsed) = benchmark_throughput_parallel(
            batch_size, total_records, threads);
        
        println!("  线程 {:2}, 批大小 {:5}: {:10.0} 记录/秒 ({:7.2}% 目标) - 耗时: {:?}",
            threads, batch_size, throughput, (throughput / 500000.0) * 100.0, elapsed);
    }
    println!("");
    
    // 5. 测试零拷贝管道
    println!("5. 零拷贝管道吞吐量测试 (4阶段处理):");
    let zero_copy_batch_sizes = [1000, 5000, 10000, 50000];
    
    for &size in &zero_copy_batch_sizes {
        let (throughput, elapsed) = benchmark_zero_copy_pipeline(size, total_records);
        
        println!("  批大小 {:6}: {:10.0} 记录/秒 ({:7.2}% 目标) - 耗时: {:?}",
            size, throughput, (throughput / 500000.0) * 100.0, elapsed);
    }
    println!("");
    
    // 结论
    println!("=== 基准测试结论 ===");
    println!("1. 最佳批处理大小: 根据测试结果确定最佳批大小");
    println!("2. 多线程效果: 分析多线程提升百分比");
    println!("3. 自适应批处理: 评估自适应算法效果");
    println!("4. 背压控制: 分析不同背压模式的效果");
    println!("5. 零拷贝影响: 测量零拷贝带来的性能提升");
}

/// 执行基准测试的主函数
fn main() {
    run_all_benchmarks();
} 