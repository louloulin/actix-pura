use std::sync::Arc;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio::sync::mpsc;
use tokio::task;
use uuid::Uuid;
use futures::future::join_all;
use std::sync::atomic::{AtomicUsize, Ordering};

use trading::{
    TradingClusterManager,
    OrderSide, OrderType, OrderRequest,
    cluster::ActorType
};

const NODE_COUNT: usize = 5;
const BASE_PORT: u16 = 8570; // 基础端口号，每个节点递增
const TOTAL_ORDERS: usize = 10_000_000; // 1000万订单
const BATCH_SIZE: usize = 100_000; // 每批处理10万订单
const SAMPLE_RATE: usize = 200; // 每200个订单采样1个延迟数据

/// 多节点集群性能测试：测量分布式系统的吞吐量和延迟
#[tokio::test]
async fn test_cluster_performance() {
    println!("=== 多节点集群性能测试 ({} 节点) ===", NODE_COUNT);
    
    // 创建多个节点
    let mut cluster_managers = Vec::with_capacity(NODE_COUNT);
    let mut handles = Vec::with_capacity(NODE_COUNT);
    
    // 启动所有节点
    for i in 0..NODE_COUNT {
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
    
    println!("所有 {} 个节点已启动并连接到集群", NODE_COUNT);
    
    // 等待集群完全形成
    sleep(Duration::from_secs(2)).await;
    
    // 执行大规模吞吐量测试 - 1000万订单
    println!("\n执行集群大规模吞吐量测试 (1000万订单)...");
    run_large_scale_test(&cluster_managers, TOTAL_ORDERS).await;
}

/// 大规模集群吞吐量测试 (10万订单)
async fn run_large_scale_test(cluster_managers: &[Arc<TradingClusterManager>], total_orders: usize) {
    println!("开始测试集群处理 {} 个订单的性能...", total_orders);
    
    // 追踪完成的订单数
    let completed_orders = Arc::new(AtomicUsize::new(0));
    
    // 追踪延迟数据 (采样)
    let mut sampled_latencies = Vec::new();
    
    // 测量总处理时间
    let start_time = Instant::now();
    
    // 分批处理订单
    let total_batches = (total_orders + BATCH_SIZE - 1) / BATCH_SIZE;
    println!("将分 {} 批执行，每批 {} 个订单", total_batches, BATCH_SIZE);
    
    for batch_num in 0..total_batches {
        let batch_start_idx = batch_num * BATCH_SIZE;
        let batch_end_idx = (batch_start_idx + BATCH_SIZE).min(total_orders);
        let batch_size = batch_end_idx - batch_start_idx;
        
        println!("处理批次 {}/{}: 订单 {} 到 {}", 
            batch_num + 1, total_batches, batch_start_idx, batch_end_idx - 1);
        
        let batch_start_time = Instant::now();
        
        // 创建多个任务并行发送订单，均匀分布到各个节点
        let mut handles = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let global_idx = batch_start_idx + i;
            
            // 轮询选择节点
            let node_index = global_idx % NODE_COUNT;
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
            
            let handle = task::spawn(async move {
                let send_time = Instant::now();
                let result = cm.send_typed_message("/user/order", "CREATE_ORDER", order_req);
                
                // 更新计数器
                completed_orders_clone.fetch_add(1, Ordering::Relaxed);
                
                // 只对一部分订单采样延迟
                if global_idx % SAMPLE_RATE == 0 {
                    if result.is_ok() {
                        Some(send_time.elapsed())
                    } else {
                        None
                    }
                } else {
                    None
                }
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
            if let Ok(Some(latency)) = result {
                sampled_latencies.push(latency);
            }
        }
        
        let batch_duration = batch_start_time.elapsed();
        let batch_throughput = batch_size as f64 / batch_duration.as_secs_f64();
        
        println!("批次 {}/{} 完成，用时: {:?}, 吞吐量: {:.2} 订单/秒", 
            batch_num + 1, total_batches, batch_duration, batch_throughput);
        
        // 打印当前统计信息
        let current_total_duration = start_time.elapsed();
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
    let total_duration = start_time.elapsed();
    let total_throughput = total_orders as f64 / total_duration.as_secs_f64();
    
    // 计算最终延迟统计
    if !sampled_latencies.is_empty() {
        let mut latency_nanos: Vec<u128> = sampled_latencies.iter().map(|d| d.as_nanos()).collect();
        latency_nanos.sort();
        
        let avg_latency = Duration::from_nanos((latency_nanos.iter().sum::<u128>() / latency_nanos.len() as u128) as u64);
        let p50_latency = Duration::from_nanos(latency_nanos[latency_nanos.len() / 2] as u64);
        let p95_latency = Duration::from_nanos(latency_nanos[(latency_nanos.len() * 95) / 100] as u64);
        let p99_latency = Duration::from_nanos(latency_nanos[(latency_nanos.len() * 99) / 100] as u64);
        
        // 输出最终性能测试结果
        println!("\n===== 1000万订单压测最终结果 =====");
        println!("总订单数: {}", total_orders);
        println!("完成订单数: {}", completed_orders.load(Ordering::Relaxed));
        println!("总耗时: {:?}", total_duration);
        println!("平均吞吐量: {:.2} 订单/秒", total_throughput);
        println!("采样订单数: {}", sampled_latencies.len());
        println!("平均延迟: {:?}", avg_latency);
        println!("P50 延迟: {:?}", p50_latency);
        println!("P95 延迟: {:?}", p95_latency);
        println!("P99 延迟: {:?}", p99_latency);
    } else {
        println!("\n===== 1000万订单压测最终结果 =====");
        println!("总订单数: {}", total_orders);
        println!("完成订单数: {}", completed_orders.load(Ordering::Relaxed));
        println!("总耗时: {:?}", total_duration);
        println!("平均吞吐量: {:.2} 订单/秒", total_throughput);
        println!("未收集到延迟数据");
    }
} 