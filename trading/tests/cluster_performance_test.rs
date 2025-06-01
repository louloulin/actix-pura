use std::sync::Arc;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio::task;
use futures::future::join_all;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::env;
use std::fs::File;
use std::io::Write;
use chrono::{DateTime, Utc, Local};
use serde::{Serialize, Deserialize};

use trading::{
    TradingClusterManager,
    OrderSide, OrderType, OrderRequest,
    cluster::ActorType
};

// 默认配置，如果没有命令行参数会使用这些值
const DEFAULT_NODE_COUNT: usize = 5;
const DEFAULT_TOTAL_ORDERS: usize = 100_000;
const DEFAULT_BATCH_SIZE_RATIO: usize = 100; // 每批处理为总订单数的1/100
const DEFAULT_SAMPLE_RATE: usize = 100; // 每100个订单采样1个延迟数据
const BASE_PORT: u16 = 8570; // 基础端口号，每个节点递增

/// 性能测试配置
struct TestConfig {
    /// 集群节点数量
    node_count: usize,
    /// 总订单数量
    total_orders: usize,
    /// 每批处理的订单数量
    batch_size: usize,
    /// 延迟采样率
    sample_rate: usize,
}

/// 解析命令行参数
fn parse_args() -> TestConfig {
    // 从环境变量获取参数
    let args: Vec<String> = env::args().collect();
    
    // 尝试读取节点数量参数
    let node_count = args.iter()
        .position(|arg| arg == "--nodes")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|val| val.parse::<usize>().ok())
        .unwrap_or(DEFAULT_NODE_COUNT);
    
    // 尝试读取订单数量参数
    let total_orders = args.iter()
        .position(|arg| arg == "--orders")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|val| val.parse::<usize>().ok())
        .unwrap_or(DEFAULT_TOTAL_ORDERS);
    
    // 计算批次大小
    let batch_size = total_orders / DEFAULT_BATCH_SIZE_RATIO;
    let batch_size = if batch_size < 1000 { 1000 } else { batch_size }; // 最小批次大小为1000
    
    // 采样率
    let sample_rate = args.iter()
        .position(|arg| arg == "--sample-rate")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|val| val.parse::<usize>().ok())
        .unwrap_or(DEFAULT_SAMPLE_RATE);
    
    TestConfig {
        node_count,
        total_orders,
        batch_size,
        sample_rate,
    }
}

/// 压测报告数据结构
#[derive(Serialize, Deserialize)]
struct TestReport {
    /// 测试开始时间
    start_time: DateTime<Utc>,
    /// 测试结束时间
    end_time: DateTime<Utc>,
    /// 测试持续时间（毫秒）
    duration_ms: u64,
    /// 节点数量
    node_count: usize,
    /// 总订单数
    total_orders: usize,
    /// 完成订单数
    completed_orders: usize,
    /// 平均吞吐量（订单/秒）
    throughput: f64,
    /// 采样订单数
    sampled_count: usize,
    /// 平均延迟（微秒）
    avg_latency_us: f64,
    /// 中位数延迟（微秒）
    p50_latency_us: u64,
    /// 95百分位延迟（微秒）
    p95_latency_us: u64,
    /// 99百分位延迟（微秒）
    p99_latency_us: u64,
    /// 批次详情
    batch_details: Vec<BatchResult>,
    /// 节点详情
    node_details: Vec<NodeResult>,
}

/// 批次测试结果
#[derive(Serialize, Deserialize)]
struct BatchResult {
    /// 批次编号
    batch_num: usize,
    /// 开始订单索引
    start_idx: usize,
    /// 结束订单索引
    end_idx: usize,
    /// 批次持续时间（毫秒）
    duration_ms: u64,
    /// 吞吐量（订单/秒）
    throughput: f64,
}

/// 节点测试结果
#[derive(Serialize, Deserialize)]
struct NodeResult {
    /// 节点ID
    node_id: String,
    /// 节点处理的订单数
    processed_orders: usize,
    /// 节点处理订单的平均延迟（微秒）
    avg_latency_us: f64,
}

/// 多节点集群性能测试：测量分布式系统的吞吐量和延迟
#[tokio::test]
#[ignore = "这个测试需要手动执行，因为它会启动多个节点并运行较长时间"]
async fn test_cluster_performance() {
    // 从环境变量读取配置
    let node_count = std::env::var("CLUSTER_NODES")
        .unwrap_or_else(|_| DEFAULT_NODE_COUNT.to_string())
        .parse::<usize>()
        .unwrap_or(DEFAULT_NODE_COUNT);
    
    let total_orders = std::env::var("TOTAL_ORDERS")
        .unwrap_or_else(|_| DEFAULT_TOTAL_ORDERS.to_string())
        .parse::<usize>()
        .unwrap_or(DEFAULT_TOTAL_ORDERS);
    
    let sample_rate = std::env::var("SAMPLE_RATE")
        .unwrap_or_else(|_| DEFAULT_SAMPLE_RATE.to_string())
        .parse::<usize>()
        .unwrap_or(DEFAULT_SAMPLE_RATE);
    
    // 计算批次大小
    let batch_size = total_orders / DEFAULT_BATCH_SIZE_RATIO;
    let batch_size = if batch_size < 1000 { 1000 } else { batch_size }; // 最小批次大小为1000
    
    let config = TestConfig {
        node_count,
        total_orders,
        batch_size,
        sample_rate,
    };
    
    println!("=== 多节点集群性能测试 ({} 节点, {} 订单) ===", 
        config.node_count, config.total_orders);
    
    // 创建多个节点
    let mut cluster_managers = Vec::with_capacity(config.node_count);
    let mut handles = Vec::with_capacity(config.node_count);
    
    // 启动所有节点
    for i in 0..config.node_count {
        let node_id = format!("cluster-node-{}", i);
        let port = BASE_PORT + i as u16;
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
        
        println!("启动节点 {} 在端口 {}", node_id, port);
        
        let mut manager = TradingClusterManager::new(
            node_id.clone(),
            bind_addr,
            "perf-cluster".to_string()
        );
        
        // 初始化节点
        manager.initialize().expect(&format!("无法初始化节点 {}", i));
        manager.register_actor_path("/user/order", ActorType::Order)
            .expect(&format!("无法在节点 {} 上注册Order Actor路径", i));
        
        // 如果不是第一个节点，就连接到第一个节点形成集群
        if i > 0 {
            let seed_addr = format!("127.0.0.1:{}", BASE_PORT);
            println!("节点 {} 添加种子节点 {}", node_id, seed_addr);
            manager.add_seed_node(seed_addr);
        }
        
        let manager_arc = Arc::new(manager);
        cluster_managers.push(manager_arc.clone());
        
        // 创建一个后台任务来保持节点活跃
        let handle = task::spawn(async move {
            let _manager = manager_arc;
            // 保持节点运行
            loop {
                sleep(Duration::from_secs(1)).await;
            }
        });
        
        handles.push(handle);
        
        // 等待节点初始化
        sleep(Duration::from_millis(500)).await;
    }
    
    println!("所有 {} 个节点已启动并连接到集群", config.node_count);
    
    // 等待集群完全形成
    sleep(Duration::from_secs(2)).await;
    
    // 执行大规模吞吐量测试
    println!("\n执行集群性能测试 ({} 订单)...", config.total_orders);
    let report = run_large_scale_test(&cluster_managers, &config).await;
    
    // 保存测试报告
    save_test_report(&report).expect("无法保存测试报告");
}

/// 保存测试报告到文件
fn save_test_report(report: &TestReport) -> std::io::Result<()> {
    // 创建报告文件名，包含时间戳
    let now = Local::now();
    let filename = format!(
        "perf_test_report_{}nodes_{}orders_{}.json", 
        report.node_count, 
        report.total_orders,
        now.format("%Y%m%d_%H%M%S")
    );
    
    // 序列化报告为JSON
    let json = serde_json::to_string_pretty(report).expect("无法序列化报告");
    
    // 写入文件
    let mut file = File::create(&filename)?;
    file.write_all(json.as_bytes())?;
    
    println!("\n测试报告已保存到文件: {}", filename);
    
    // 同时生成一个人类可读的摘要
    generate_summary_report(report)?;
    
    Ok(())
}

/// 生成人类可读的摘要报告
fn generate_summary_report(report: &TestReport) -> std::io::Result<()> {
    // 创建摘要文件名
    let now = Local::now();
    let filename = format!(
        "perf_test_summary_{}nodes_{}orders_{}.txt", 
        report.node_count, 
        report.total_orders,
        now.format("%Y%m%d_%H%M%S")
    );
    
    let mut file = File::create(&filename)?;
    
    // 写入摘要信息
    writeln!(file, "===== 交易系统集群性能测试摘要 =====\n")?;
    writeln!(file, "测试开始时间: {}", report.start_time.with_timezone(&Local))?;
    writeln!(file, "测试结束时间: {}", report.end_time.with_timezone(&Local))?;
    writeln!(file, "测试持续时间: {:.2} 秒", report.duration_ms as f64 / 1000.0)?;
    writeln!(file, "\n集群配置:")?;
    writeln!(file, "节点数量: {}", report.node_count)?;
    writeln!(file, "总订单数: {}", report.total_orders)?;
    writeln!(file, "完成订单数: {}", report.completed_orders)?;
    
    writeln!(file, "\n整体性能:")?;
    writeln!(file, "平均吞吐量: {:.2} 订单/秒", report.throughput)?;
    writeln!(file, "平均延迟: {:.2} 微秒", report.avg_latency_us)?;
    writeln!(file, "P50 延迟: {} 微秒", report.p50_latency_us)?;
    writeln!(file, "P95 延迟: {} 微秒", report.p95_latency_us)?;
    writeln!(file, "P99 延迟: {} 微秒", report.p99_latency_us)?;
    
    writeln!(file, "\n各节点性能:")?;
    for node in &report.node_details {
        writeln!(file, "节点 {}: 处理 {} 订单, 平均延迟 {:.2} 微秒", 
            node.node_id, node.processed_orders, node.avg_latency_us)?;
    }
    
    writeln!(file, "\n各批次性能:")?;
    for batch in &report.batch_details {
        writeln!(file, "批次 {}: 订单 {}-{}, 耗时 {:.2} 秒, 吞吐量 {:.2} 订单/秒", 
            batch.batch_num, batch.start_idx, batch.end_idx,
            batch.duration_ms as f64 / 1000.0, batch.throughput)?;
    }
    
    println!("测试摘要已保存到文件: {}", filename);
    
    Ok(())
}

/// 大规模集群吞吐量测试
async fn run_large_scale_test(
    cluster_managers: &[Arc<TradingClusterManager>], 
    config: &TestConfig
) -> TestReport {
    println!("开始测试集群处理 {} 个订单的性能...", config.total_orders);
    
    let start_time = Utc::now();
    let start_instant = Instant::now();
    
    // 追踪完成的订单数
    let completed_orders = Arc::new(AtomicUsize::new(0));
    
    // 追踪延迟数据 (采样)
    let mut sampled_latencies = Vec::new();
    
    // 节点处理统计
    let mut node_processed = vec![0; config.node_count];
    let mut node_latencies = vec![Vec::new(); config.node_count];
    
    // 批次结果
    let mut batch_results = Vec::new();
    
    // 分批处理订单
    let total_batches = (config.total_orders + config.batch_size - 1) / config.batch_size;
    println!("将分 {} 批执行，每批 {} 个订单", total_batches, config.batch_size);
    
    for batch_num in 0..total_batches {
        let batch_start_idx = batch_num * config.batch_size;
        let batch_end_idx = (batch_start_idx + config.batch_size).min(config.total_orders);
        let batch_size = batch_end_idx - batch_start_idx;
        
        println!("处理批次 {}/{}: 订单 {} 到 {}", 
            batch_num + 1, total_batches, batch_start_idx, batch_end_idx - 1);
        
        let batch_start_time = Instant::now();
        
        // 创建多个任务并行发送订单，均匀分布到各个节点
        let mut handles = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let global_idx = batch_start_idx + i;
            
            // 轮询选择节点
            let node_index = global_idx % config.node_count;
            let cm = cluster_managers[node_index].clone();
            let completed_orders_clone = completed_orders.clone();
            
            // 创建订单对象
            let order_req = OrderRequest {
                order_id: Some(format!("test-{}", global_idx)),
                symbol: "AAPL".to_string(),
                side: if global_idx % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
                price: Some(100.0 + (global_idx as f64 % 10.0)),
                quantity: 10 + (global_idx % 10) as u64,
                client_id: format!("client-{}", global_idx / 1000),
                order_type: OrderType::Limit,
            };
            
            let node_idx = node_index; // 捕获节点索引
            
            // 复制采样率到本地，避免在闭包中使用config引用
            let sample_rate = config.sample_rate;
            
            let handle = task::spawn(async move {
                let send_time = Instant::now();
                let result = cm.send_typed_message("/user/order", "CREATE_ORDER", order_req);
                
                // 更新计数器
                completed_orders_clone.fetch_add(1, Ordering::Relaxed);
                
                // 只对一部分订单采样延迟
                if global_idx % sample_rate == 0 {
                    if result.is_ok() {
                        let latency = send_time.elapsed();
                        return Some((node_idx, latency));
                    }
                }
                None
            });
            
            handles.push(handle);
            
            // 每50000个请求打印进度
            if global_idx > 0 && global_idx % 50000 == 0 {
                println!("  已发送 {} 个订单...", global_idx);
            }
        }
        
        // 等待批次完成并收集延迟数据
        let results = join_all(handles).await;
        
        // 从结果中收集采样的延迟数据
        for result in results {
            if let Ok(Some((node_idx, latency))) = result {
                node_processed[node_idx] += 1;
                node_latencies[node_idx].push(latency);
                sampled_latencies.push(latency);
            }
        }
        
        let batch_duration = batch_start_time.elapsed();
        let batch_throughput = batch_size as f64 / batch_duration.as_secs_f64();
        
        // 记录批次结果
        batch_results.push(BatchResult {
            batch_num: batch_num + 1,
            start_idx: batch_start_idx,
            end_idx: batch_end_idx - 1,
            duration_ms: batch_duration.as_millis() as u64,
            throughput: batch_throughput,
        });
        
        println!("批次 {}/{} 完成，用时: {:?}, 吞吐量: {:.2} 订单/秒", 
            batch_num + 1, total_batches, batch_duration, batch_throughput);
        
        // 打印当前统计信息
        let current_total_duration = start_instant.elapsed();
        let current_total_throughput = completed_orders.load(Ordering::Relaxed) as f64 / current_total_duration.as_secs_f64();
        
        println!("当前累计统计 - 已处理: {} 订单, 总用时: {:?}, 平均吞吐量: {:.2} 订单/秒",
            completed_orders.load(Ordering::Relaxed), 
            current_total_duration,
            current_total_throughput);
            
        // 计算并显示采样延迟
        if !sampled_latencies.is_empty() {
            // 对延迟数据排序以计算百分位数
            let mut latency_nanos: Vec<u128> = sampled_latencies.iter().map(|d| d.as_nanos()).collect();
            latency_nanos.sort();
            
            let avg_latency = if !latency_nanos.is_empty() {
                Duration::from_nanos((latency_nanos.iter().sum::<u128>() / latency_nanos.len() as u128) as u64)
            } else {
                Duration::from_nanos(0)
            };
            
            let p50_latency = if !latency_nanos.is_empty() {
                Duration::from_nanos(latency_nanos[latency_nanos.len() / 2] as u64)
            } else {
                Duration::from_nanos(0)
            };
            
            let p95_latency = if !latency_nanos.is_empty() {
                Duration::from_nanos(latency_nanos[(latency_nanos.len() * 95) / 100] as u64)
            } else {
                Duration::from_nanos(0)
            };
            
            let p99_latency = if !latency_nanos.is_empty() {
                Duration::from_nanos(latency_nanos[(latency_nanos.len() * 99) / 100] as u64)
            } else {
                Duration::from_nanos(0)
            };
            
            println!("当前延迟统计 (采样) - 平均: {:?}, P50: {:?}, P95: {:?}, P99: {:?}",
                avg_latency, p50_latency, p95_latency, p99_latency);
        }
    }
    
    // 计算最终结果
    let total_duration = start_instant.elapsed();
    let end_time = Utc::now();
    let final_completed_orders = completed_orders.load(Ordering::Relaxed);
    let total_throughput = final_completed_orders as f64 / total_duration.as_secs_f64();
    
    // 计算节点统计信息
    let mut node_results = Vec::with_capacity(config.node_count);
    for i in 0..config.node_count {
        let avg_latency_us = if !node_latencies[i].is_empty() {
            let total_ns: u128 = node_latencies[i].iter().map(|d| d.as_nanos()).sum();
            total_ns as f64 / node_latencies[i].len() as f64 / 1000.0 // 转换为微秒
        } else {
            0.0
        };
        
        node_results.push(NodeResult {
            node_id: format!("cluster-node-{}", i),
            processed_orders: node_processed[i],
            avg_latency_us,
        });
    }
    
    // 计算最终延迟统计
    let mut report = TestReport {
        start_time,
        end_time,
        duration_ms: total_duration.as_millis() as u64,
        node_count: config.node_count,
        total_orders: config.total_orders,
        completed_orders: final_completed_orders,
        throughput: total_throughput,
        sampled_count: sampled_latencies.len(),
        avg_latency_us: 0.0,
        p50_latency_us: 0,
        p95_latency_us: 0,
        p99_latency_us: 0,
        batch_details: batch_results,
        node_details: node_results,
    };
    
    if !sampled_latencies.is_empty() {
        let mut latency_nanos: Vec<u128> = sampled_latencies.iter().map(|d| d.as_nanos()).collect();
        latency_nanos.sort();
        
        let avg_latency = (latency_nanos.iter().sum::<u128>() as f64 / latency_nanos.len() as f64) / 1000.0; // 转换为微秒
        let p50_latency = latency_nanos[latency_nanos.len() / 2] / 1000;
        let p95_latency = latency_nanos[(latency_nanos.len() * 95) / 100] / 1000;
        let p99_latency = latency_nanos[(latency_nanos.len() * 99) / 100] / 1000;
        
        report.avg_latency_us = avg_latency;
        report.p50_latency_us = p50_latency as u64;
        report.p95_latency_us = p95_latency as u64;
        report.p99_latency_us = p99_latency as u64;
        
        // 输出最终性能测试结果
        println!("\n===== 性能测试最终结果 =====");
        println!("集群节点数: {}", config.node_count);
        println!("总订单数: {}", config.total_orders);
        println!("完成订单数: {}", final_completed_orders);
        println!("总耗时: {:?}", total_duration);
        println!("平均吞吐量: {:.2} 订单/秒", total_throughput);
        println!("采样订单数: {}", sampled_latencies.len());
        println!("平均延迟: {:.2} 微秒", avg_latency);
        println!("P50 延迟: {} 微秒", p50_latency as u64);
        println!("P95 延迟: {} 微秒", p95_latency as u64);
        println!("P99 延迟: {} 微秒", p99_latency as u64);
        
        // 打印各节点统计，使用报告中的节点详情而不是已移动的node_results
        println!("\n各节点统计:");
        for (i, node) in report.node_details.iter().enumerate() {
            println!("节点 {}: 处理 {} 订单 ({:.1}%), 平均延迟 {:.2} 微秒", 
                i, node.processed_orders, 
                node.processed_orders as f64 / final_completed_orders as f64 * 100.0,
                node.avg_latency_us);
        }
    } else {
        println!("\n===== 性能测试最终结果 =====");
        println!("集群节点数: {}", config.node_count);
        println!("总订单数: {}", config.total_orders);
        println!("完成订单数: {}", final_completed_orders);
        println!("总耗时: {:?}", total_duration);
        println!("平均吞吐量: {:.2} 订单/秒", total_throughput);
        println!("未收集到延迟数据");
    }
    
    report
} 