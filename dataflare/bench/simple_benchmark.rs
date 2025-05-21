//! 简单批处理性能基准测试
//! 
//! 独立测试批处理操作的性能，不依赖Actix运行时

use std::time::{Duration, Instant};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use dataflare_core::message::{DataRecord, DataRecordBatch};

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
fn benchmark_throughput_no_backpressure(batch_size: usize, total_records: usize) -> (f64, Duration) {
    let start_time = Instant::now();
    let iterations = total_records / batch_size;
    
    let mut total_processed = 0;
    
    for _ in 0..iterations {
        let batch = create_test_batch(batch_size);
        let processed = test_batch_process(&batch);
        total_processed += processed.records.len();
    }
    
    let elapsed = start_time.elapsed();
    let throughput = total_processed as f64 / elapsed.as_secs_f64();
    
    (throughput, elapsed)
}

/// 主函数
fn main() {
    println!("Running simple batch processing benchmark...");
    
    // 创建测试批次并处理
    let batch_sizes = [100, 1000, 10000];
    let total_records = 100000;
    
    println!("\n批处理性能基准测试结果：");
    println!("----------------------------------------");
    println!("批大小\t\t吞吐量(记录/秒)\t处理时间");
    println!("----------------------------------------");
    
    for &batch_size in &batch_sizes {
        let (throughput, duration) = benchmark_throughput_no_backpressure(batch_size, total_records);
        println!("{}\t\t{:.2}\t\t{:?}", batch_size, throughput, duration);
    }
    
    println!("----------------------------------------");
    println!("测试完成！");
} 