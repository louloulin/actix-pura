//! 独立批处理基准测试
//! 
//! 这个基准测试不依赖dataflare运行时组件，只依赖于核心数据结构

use std::time::{Duration, Instant};
use std::sync::Arc;
use std::thread;

use serde_json::json;
use uuid::Uuid;
use chrono::Utc;

/// 数据记录结构
#[derive(Clone, Debug)]
struct DataRecord {
    id: Uuid,
    data: serde_json::Value,
    created_at: chrono::DateTime<Utc>,
}

impl DataRecord {
    fn new(data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            data,
            created_at: Utc::now(),
        }
    }
}

/// 数据记录批次结构
#[derive(Clone, Debug)]
struct DataRecordBatch {
    id: Uuid,
    records: Vec<DataRecord>,
    created_at: chrono::DateTime<Utc>,
}

impl DataRecordBatch {
    fn new(records: Vec<DataRecord>) -> Self {
        Self {
            id: Uuid::new_v4(),
            records,
            created_at: Utc::now(),
        }
    }
    
    fn len(&self) -> usize {
        self.records.len()
    }
}

/// 创建测试记录批次
fn create_test_batch(size: usize) -> DataRecordBatch {
    let mut records = Vec::with_capacity(size);
    
    for i in 0..size {
        let record = DataRecord::new(json!({
            "id": i,
            "name": format!("test{}", i),
            "value": i * 10,
            "timestamp": Utc::now().to_rfc3339(),
            "data": "这是一些示例数据，足够大以模拟真实场景".repeat(i % 5 + 1),
        }));
        
        records.push(record);
    }
    
    DataRecordBatch::new(records)
}

/// 测试批处理计算任务（模拟转换操作）
fn test_batch_process(batch: &DataRecordBatch) -> DataRecordBatch {
    // 创建转换后的批次
    let mut processed_records = Vec::with_capacity(batch.records.len());
    
    for record in &batch.records {
        // 模拟记录处理：读取值，进行计算，生成新记录
        let id = record.data.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let value = record.data.get("value").and_then(|v| v.as_i64()).unwrap_or(0);
        
        // 创建新记录（进行一些计算）
        let processed_record = DataRecord::new(json!({
            "id": id,
            "processed_value": value * 2,
            "status": "processed",
            "timestamp": Utc::now().to_rfc3339(),
        }));
        
        processed_records.push(processed_record);
    }
    
    DataRecordBatch::new(processed_records)
}

/// 测试无背压情况下的吞吐量
fn benchmark_throughput(batch_size: usize, total_records: usize) -> (f64, Duration) {
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

/// 测试多线程并行批处理吞吐量
fn benchmark_throughput_parallel(
    batch_size: usize, 
    total_records: usize,
    threads: usize
) -> (f64, Duration) {
    let start_time = Instant::now();
    let records_per_thread = total_records / threads;
    
    // 创建线程处理批次
    let mut handles = Vec::with_capacity(threads);
    
    for _ in 0..threads {
        let handle = thread::spawn(move || {
            let iterations = records_per_thread / batch_size;
            let mut thread_processed = 0;
            
            for _ in 0..iterations {
                let batch = create_test_batch(batch_size);
                let processed = test_batch_process(&batch);
                thread_processed += processed.len();
            }
            
            thread_processed
        });
        
        handles.push(handle);
    }
    
    // 等待所有线程完成并收集结果
    let mut total_processed = 0;
    for handle in handles {
        total_processed += handle.join().unwrap();
    }
    
    let elapsed = start_time.elapsed();
    let throughput = total_processed as f64 / elapsed.as_secs_f64();
    
    (throughput, elapsed)
}

fn main() {
    println!("========== 批处理性能基准测试 ==========");
    
    let batch_sizes = [100, 500, 1000, 5000, 10000];
    let total_records = 500000;
    
    // 1. 单线程吞吐量测试
    println!("\n1. 单线程批处理吞吐量测试");
    println!("{}", "-".repeat(50));
    println!("批大小\t\t吞吐量(记录/秒)\t处理时间(秒)");
    println!("{}", "-".repeat(50));
    
    let mut results = Vec::new();
    for batch_size in batch_sizes {
        let (throughput, elapsed) = benchmark_throughput(batch_size, total_records);
        println!("{}\t\t{:.2}\t\t{:.4}",
            batch_size, throughput, elapsed.as_secs_f64());
        results.push((batch_size, throughput, elapsed));
    }
    
    // 找出最佳批大小（基于吞吐量）
    let best_result = results.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();
    
    println!("{}", "-".repeat(50));
    println!("最佳批大小: {}，吞吐量: {:.2} 记录/秒",
        best_result.0, best_result.1);
    println!();
    
    // 2. 多线程吞吐量测试
    let thread_counts = [2, 4, 8];
    let best_batch_size = best_result.0;
    
    println!("\n2. 多线程批处理吞吐量测试 (使用最佳批大小: {})", best_batch_size);
    println!("{}", "-".repeat(50));
    println!("线程数\t\t吞吐量(记录/秒)\t处理时间(秒)\t线性加速比");
    println!("{}", "-".repeat(50));
    
    // 存储单线程最佳结果，用于计算多线程提升比例
    let single_thread_best = best_result.1;
    
    for &thread_count in &thread_counts {
        let (throughput, elapsed) = benchmark_throughput_parallel(best_batch_size, total_records, thread_count);
        let speedup = throughput / single_thread_best;
        let efficiency = speedup / thread_count as f64;
        
        println!("{}\t\t{:.2}\t\t{:.4}\t\t{:.2}x (效率: {:.1}%)",
            thread_count, throughput, elapsed.as_secs_f64(), 
            speedup, efficiency * 100.0);
    }
    
    println!("{}", "-".repeat(50));
    println!("\n测试完成！");
} 