//! PostgreSQL批处理连接器性能基准测试
//!
//! 测量PostgreSQL批处理连接器在不同配置下的性能

use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use futures::StreamExt;
use serde_json::{json, Value};
use log::{info, error};
use std::env;

use dataflare_core::{
    connector::{BatchSourceConnector, Position, ExtractionMode},
    message::DataRecordBatch,
};

use dataflare_connector::postgres::batch::PostgresBatchSourceConnector;
use dataflare_runtime::batch::{AdaptiveBatcher, AdaptiveBatchingConfig};

/// 创建测试配置
fn create_test_config() -> Value {
    // 尝试从环境变量读取数据库配置
    let host = env::var("PG_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("PG_PORT").unwrap_or_else(|_| "5432".to_string());
    let database = env::var("PG_DATABASE").unwrap_or_else(|_| "test_db".to_string());
    let username = env::var("PG_USERNAME").unwrap_or_else(|_| "postgres".to_string());
    let password = env::var("PG_PASSWORD").unwrap_or_else(|_| "postgres".to_string());
    let table = env::var("PG_TABLE").unwrap_or_else(|_| "test_data".to_string());
    
    json!({
        "host": host,
        "port": port.parse::<u16>().unwrap_or(5432),
        "database": database,
        "username": username,
        "password": password,
        "table": table,
        "mode": "full",
        "batch_size": 1000,
        "extraction_mode": "batch"
    })
}

/// 测试固定批大小读取性能
async fn benchmark_fixed_batch_size(
    config: Value,
    batch_size: usize,
    total_records: Option<usize>,
) -> (f64, usize, Duration) {
    let mut connector_config = config.clone();
    connector_config["batch_size"] = json!(batch_size);
    
    let mut connector = PostgresBatchSourceConnector::new(connector_config);
    let start_time = Instant::now();
    
    // 获取总记录数（如果未指定）
    let total_to_read = match total_records {
        Some(count) => count,
        None => {
            let count = connector.estimate_record_count().await.unwrap_or(0) as usize;
            count.min(1_000_000) // 限制最大读取记录数
        }
    };
    
    let mut total_read = 0;
    let mut batches_count = 0;
    
    while total_read < total_to_read {
        // 读取批次
        match connector.read_batch(batch_size).await {
            Ok(batch) => {
                let records_count = batch.records.len();
                if records_count == 0 {
                    // 没有更多数据
                    break;
                }
                
                total_read += records_count;
                batches_count += 1;
                
                // 如果读取足够的记录，停止
                if total_read >= total_to_read {
                    break;
                }
            },
            Err(e) => {
                error!("读取批次时出错: {:?}", e);
                break;
            }
        }
    }
    
    let elapsed = start_time.elapsed();
    let throughput = if elapsed.as_secs_f64() > 0.0 {
        total_read as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };
    
    (throughput, total_read, elapsed)
}

/// 测试自适应批大小读取性能
async fn benchmark_adaptive_batch_size(
    config: Value,
    adaptive_config: AdaptiveBatchingConfig,
    total_records: Option<usize>,
) -> (f64, usize, Duration) {
    let mut connector_config = config.clone();
    connector_config["batch_size"] = json!(adaptive_config.initial_size);
    
    let mut connector = PostgresBatchSourceConnector::new(connector_config);
    let mut batcher = AdaptiveBatcher::new(adaptive_config);
    let start_time = Instant::now();
    
    // 获取总记录数（如果未指定）
    let total_to_read = match total_records {
        Some(count) => count,
        None => {
            let count = connector.estimate_record_count().await.unwrap_or(0) as usize;
            count.min(1_000_000) // 限制最大读取记录数
        }
    };
    
    let mut total_read = 0;
    let mut batches_count = 0;
    
    while total_read < total_to_read {
        // 获取当前推荐的批大小
        let current_batch_size = batcher.get_batch_size();
        
        // 开始批处理
        batcher.start_batch();
        
        // 读取批次
        match connector.read_batch(current_batch_size).await {
            Ok(batch) => {
                let records_count = batch.records.len();
                if records_count == 0 {
                    // 没有更多数据
                    break;
                }
                
                // 批处理完成
                batcher.complete_batch(records_count);
                
                total_read += records_count;
                batches_count += 1;
                
                // 如果读取足够的记录，停止
                if total_read >= total_to_read {
                    break;
                }
            },
            Err(e) => {
                error!("读取批次时出错: {:?}", e);
                break;
            }
        }
    }
    
    let elapsed = start_time.elapsed();
    let throughput = if elapsed.as_secs_f64() > 0.0 {
        total_read as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };
    
    // 输出自适应批处理指标
    let metrics = batcher.get_metrics();
    info!("自适应批处理指标:");
    info!("- 平均批大小: {:?}", metrics.average_batch_size());
    info!("- 平均延迟: {:?}", metrics.average_latency());
    info!("- 吞吐量: {:?} 记录/秒", metrics.throughput());
    info!("- 系统是否稳定: {}", metrics.is_system_stable());
    
    (throughput, total_read, elapsed)
}

/// 测试CDC连接器性能
async fn benchmark_cdc_connector(
    config: Value,
    batch_size: usize,
    duration_secs: u64,
) -> (f64, usize, Duration) {
    // 创建CDC配置
    let mut cdc_config = config.clone();
    cdc_config["mode"] = json!("cdc");
    cdc_config["batch_size"] = json!(batch_size);
    cdc_config["cdc"] = json!({
        "slot_name": "dataflare_bench_slot",
        "publication_name": "dataflare_bench_pub",
        "plugin": "pgoutput"
    });
    
    let mut connector = dataflare_connector::postgres::cdc::PostgresCDCConnector::new(cdc_config);
    let start_time = Instant::now();
    
    let mut total_read = 0;
    let mut batches_count = 0;
    
    // 运行指定时间
    let end_time = start_time + Duration::from_secs(duration_secs);
    
    while Instant::now() < end_time {
        // 读取批次
        match connector.read_batch(batch_size).await {
            Ok(batch) => {
                let records_count = batch.records.len();
                total_read += records_count;
                batches_count += 1;
                
                if records_count > 0 {
                    // 获取并提交位置
                    if let Ok(position) = connector.get_position() {
                        let _ = connector.commit(position).await;
                    }
                } else {
                    // 没有数据，稍等片刻
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            },
            Err(e) => {
                error!("读取CDC批次时出错: {:?}", e);
                // 短暂等待后重试
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
    
    let elapsed = end_time.saturating_duration_since(start_time);
    let throughput = if elapsed.as_secs_f64() > 0.0 {
        total_read as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };
    
    (throughput, total_read, elapsed)
}

/// 运行所有PostgreSQL基准测试
fn run_postgres_benchmarks() {
    println!("=== PostgreSQL批处理连接器性能基准测试 ===");
    println!("目标吞吐量: 500,000 记录/秒");
    println!("");
    
    // 创建配置
    let config = create_test_config();
    
    // 创建异步运行时
    let rt = Runtime::new().unwrap();
    
    // 1. 测试不同批大小的读取性能
    println!("1. 不同批大小的PostgreSQL读取性能:");
    let batch_sizes = [100, 1000, 5000, 10000, 50000];
    
    for &size in &batch_sizes {
        let (throughput, total_read, elapsed) = rt.block_on(
            benchmark_fixed_batch_size(config.clone(), size, Some(100_000))
        );
        
        println!("  批大小 {:6}: {:10.0} 记录/秒 ({:7.2}% 目标), 读取 {:6} 条记录, 耗时: {:?}",
            size, throughput, (throughput / 500000.0) * 100.0, total_read, elapsed);
    }
    println!("");
    
    // 2. 测试自适应批大小读取性能
    println!("2. 自适应批大小PostgreSQL读取性能:");
    let adaptive_configs = [
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
    
    for (i, config) in adaptive_configs.iter().enumerate() {
        let (throughput, total_read, elapsed) = rt.block_on(
            benchmark_adaptive_batch_size(config.clone(), config.clone(), Some(100_000))
        );
        
        println!("  配置 {:1}: 初始={:5}, 最小={:5}, 最大={:6} - {:10.0} 记录/秒 ({:7.2}% 目标), 读取 {:6} 条记录, 耗时: {:?}",
            i+1, config.initial_size, config.min_size, config.max_size, 
            throughput, (throughput / 500000.0) * 100.0, total_read, elapsed);
    }
    println!("");
    
    // 3. 测试CDC连接器性能（可选，需要设置逻辑复制）
    println!("3. PostgreSQL CDC连接器性能测试 (运行30秒):");
    println!("   注: 需要配置PostgreSQL逻辑复制，否则此测试将失败");
    let cdc_batch_sizes = [100, 500, 1000];
    
    for &size in &cdc_batch_sizes {
        let (throughput, total_read, elapsed) = rt.block_on(
            benchmark_cdc_connector(config.clone(), size, 30)
        );
        
        println!("  批大小 {:6}: {:10.0} 记录/秒 ({:7.2}% 目标), 读取 {:6} 条变更, 耗时: {:?}",
            size, throughput, (throughput / 500000.0) * 100.0, total_read, elapsed);
    }
    println!("");
    
    // 结论
    println!("=== PostgreSQL连接器基准测试结论 ===");
    println!("1. 最佳批处理大小: 根据测试结果确定最佳批大小");
    println!("2. 自适应批处理表现: 评估自适应算法在实际数据库操作中的效果");
    println!("3. CDC性能: 测量CDC模式下的性能表现");
    println!("4. 建议: 根据测试结果优化PostgreSQL连接器配置以达到目标吞吐量");
}

/// 基准测试主函数
fn main() {
    env_logger::init();
    run_postgres_benchmarks();
} 