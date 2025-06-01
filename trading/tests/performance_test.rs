use std::sync::Arc;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio::sync::mpsc;
use tokio::task;
use uuid::Uuid;
use futures::future::join_all;

use trading::{
    TradingClusterManager,
    OrderSide, OrderType, OrderRequest, OrderResult,
    cluster::ActorType
};

/// 性能测试：测量系统吞吐量和延迟
#[tokio::test]
async fn test_order_processing_throughput() {
    println!("=== 订单处理吞吐量测试 ===");
    
    // 创建交易集群节点
    let node_id = format!("perf-test-{}", Uuid::new_v4());
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8560);
    let mut cluster_manager = TradingClusterManager::new(
        node_id.clone(),
        bind_addr,
        "perf-test-cluster".to_string()
    );
    
    // 初始化集群节点
    cluster_manager.initialize().expect("无法初始化性能测试集群节点");
    cluster_manager.register_actor_path("/user/order", ActorType::Order).expect("无法注册Order Actor路径");
    let cluster_manager = Arc::new(cluster_manager);
    
    // 测试参数
    let order_counts = [100, 500, 1000]; // 测试不同的订单数量
    
    for &count in &order_counts {
        println!("\n测试 {} 个订单的处理性能...", count);
        
        // 准备测试数据
        let mut orders = Vec::with_capacity(count);
        for i in 0..count {
            let order_req = OrderRequest {
                order_id: Some(format!("perf-test-{}-{}", count, i)),
                symbol: "AAPL".to_string(),
                side: if i % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
                price: Some(100.0 + (i as f64 % 10.0)),
                quantity: 10 + (i % 10) as u64,
                client_id: "perf-test-client".to_string(),
                order_type: OrderType::Limit,
            };
            orders.push(order_req);
        }
        
        // 测量总处理时间
        let start = Instant::now();
        
        // 创建多个任务并行发送订单
        let mut handles = Vec::with_capacity(count);
        for order in orders {
            let cm = cluster_manager.clone();
            let handle = task::spawn(async move {
                let send_time = Instant::now();
                let result = cm.send_typed_message("/user/order", "CREATE_ORDER", order);
                if result.is_err() {
                    println!("发送订单失败");
                    return None;
                }
                let latency = send_time.elapsed();
                Some(latency)
            });
            handles.push(handle);
        }
        
        // 等待所有订单处理完成并收集延迟数据
        let results = join_all(handles).await;
        let latencies: Vec<Duration> = results.into_iter()
            .filter_map(|r| r.ok().and_then(|opt| opt))
            .collect();
        
        // 计算总时间和吞吐量
        let total_duration = start.elapsed();
        let throughput = count as f64 / total_duration.as_secs_f64();
        
        // 计算平均延迟和百分位延迟
        let mut latency_nanos: Vec<u128> = latencies.iter().map(|d| d.as_nanos()).collect();
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
        
        // 输出性能测试结果
        println!("处理 {} 个订单:", count);
        println!("总时间: {:?}", total_duration);
        println!("吞吐量: {:.2} 订单/秒", throughput);
        println!("平均延迟: {:?}", avg_latency);
        println!("P50 延迟: {:?}", p50_latency);
        println!("P95 延迟: {:?}", p95_latency);
        println!("P99 延迟: {:?}", p99_latency);
        
        // 短暂休息，让系统恢复
        sleep(Duration::from_millis(500)).await;
    }
}

/// 负载测试：测试系统在持续负载下的表现
#[tokio::test]
async fn test_order_processing_load() {
    println!("=== 订单处理负载测试 ===");
    
    // 创建交易集群节点
    let node_id = format!("load-test-{}", Uuid::new_v4());
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8561);
    let mut cluster_manager = TradingClusterManager::new(
        node_id.clone(),
        bind_addr,
        "load-test-cluster".to_string()
    );
    
    // 初始化集群节点
    cluster_manager.initialize().expect("无法初始化负载测试集群节点");
    cluster_manager.register_actor_path("/user/order", ActorType::Order).expect("无法注册Order Actor路径");
    let cluster_manager = Arc::new(cluster_manager);
    
    // 测试参数
    let test_duration = Duration::from_secs(10); // 测试持续10秒
    let orders_per_second = 100; // 每秒发送100个订单
    
    println!("\n测试持续负载 {} 订单/秒，持续 {:?}...", orders_per_second, test_duration);
    
    // 创建通道用于发送和接收统计信息
    let (tx, mut rx) = mpsc::channel(1000);
    
    // 启动生成器任务
    let cm = cluster_manager.clone();
    let generator_handle = task::spawn(async move {
        let interval = Duration::from_micros(1_000_000 / orders_per_second as u64);
        let start = Instant::now();
        let mut order_count = 0;
        
        while start.elapsed() < test_duration {
            let order_id = format!("load-test-{}", order_count);
            let order_req = OrderRequest {
                order_id: Some(order_id),
                symbol: "LOAD".to_string(),
                side: if order_count % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
                price: Some(100.0 + (order_count % 10) as f64),
                quantity: 10 + (order_count % 5) as u64,
                client_id: "load-test-client".to_string(),
                order_type: OrderType::Limit,
            };
            
            let send_time = Instant::now();
            if cm.send_typed_message("/user/order", "CREATE_ORDER", order_req).is_ok() {
                let latency = send_time.elapsed();
                if tx.send(latency).await.is_err() {
                    break;
                }
            } else {
                println!("发送订单失败");
                break;
            }
            
            order_count += 1;
            
            // 计算下一个订单应该在什么时间发送
            let elapsed = send_time.elapsed();
            if elapsed < interval {
                sleep(interval - elapsed).await;
            }
        }
        
        order_count
    });
    
    // 收集所有延迟数据
    let collector_handle = task::spawn(async move {
        let mut latencies = Vec::new();
        while let Some(latency) = rx.recv().await {
            latencies.push(latency);
        }
        latencies
    });
    
    // 等待生成器完成
    let total_orders = generator_handle.await.unwrap_or(0);
    
    // 等待收集器完成并获取延迟数据
    let latencies = collector_handle.await.unwrap_or_default();
    
    // 计算总时间和吞吐量
    let actual_throughput = total_orders as f64 / test_duration.as_secs_f64();
    
    // 计算平均延迟和百分位延迟
    let mut latency_nanos: Vec<u128> = latencies.iter().map(|d| d.as_nanos()).collect();
    latency_nanos.sort();
    
    let avg_latency = if !latency_nanos.is_empty() {
        Duration::from_nanos((latency_nanos.iter().sum::<u128>() / latency_nanos.len() as u128) as u64)
    } else {
        Duration::from_nanos(0)
    };
    
    let p50_latency = if !latency_nanos.is_empty() && !latency_nanos.is_empty() {
        Duration::from_nanos(latency_nanos[latency_nanos.len() / 2] as u64)
    } else {
        Duration::from_nanos(0)
    };
    
    let p95_latency = if !latency_nanos.is_empty() && latency_nanos.len() > 20 {
        Duration::from_nanos(latency_nanos[(latency_nanos.len() * 95) / 100] as u64)
    } else {
        Duration::from_nanos(0)
    };
    
    let p99_latency = if !latency_nanos.is_empty() && latency_nanos.len() > 100 {
        Duration::from_nanos(latency_nanos[(latency_nanos.len() * 99) / 100] as u64)
    } else {
        Duration::from_nanos(0)
    };
    
    // 输出负载测试结果
    println!("负载测试结果:");
    println!("总订单数: {}", total_orders);
    println!("目标吞吐量: {} 订单/秒", orders_per_second);
    println!("实际吞吐量: {:.2} 订单/秒", actual_throughput);
    println!("平均延迟: {:?}", avg_latency);
    println!("P50 延迟: {:?}", p50_latency);
    println!("P95 延迟: {:?}", p95_latency);
    println!("P99 延迟: {:?}", p99_latency);
}

/// 延迟测试：测量不同操作的延迟
#[tokio::test]
async fn test_operation_latency() {
    println!("=== 操作延迟测试 ===");
    
    // 创建交易集群节点
    let node_id = format!("latency-test-{}", Uuid::new_v4());
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8562);
    let mut cluster_manager = TradingClusterManager::new(
        node_id.clone(),
        bind_addr,
        "latency-test-cluster".to_string()
    );
    
    // 初始化集群节点
    cluster_manager.initialize().expect("无法初始化延迟测试集群节点");
    cluster_manager.register_actor_path("/user/order", ActorType::Order).expect("无法注册Order Actor路径");
    let cluster_manager = Arc::new(cluster_manager);
    
    // 测试参数
    let operation_count = 100; // 每种操作测试100次
    
    // 1. 测试创建订单延迟
    println!("\n测试创建订单延迟...");
    let mut create_latencies = Vec::with_capacity(operation_count);
    
    for i in 0..operation_count {
        let order_req = OrderRequest {
            order_id: Some(format!("latency-test-create-{}", i)),
            symbol: "AAPL".to_string(),
            side: if i % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
            price: Some(150.0),
            quantity: 10,
            client_id: "latency-test-client".to_string(),
            order_type: OrderType::Limit,
        };
        
        let start = Instant::now();
        cluster_manager.send_typed_message("/user/order", "CREATE_ORDER", order_req).expect("发送订单失败");
        create_latencies.push(start.elapsed());
        
        // 短暂休息，避免压力过大
        if i % 10 == 9 {
            sleep(Duration::from_millis(10)).await;
        }
    }
    
    // 2. 测试查询订单延迟
    println!("测试查询订单延迟...");
    let mut query_latencies = Vec::with_capacity(operation_count);
    
    for i in 0..operation_count {
        let query = trading::models::order::OrderQuery::ById(format!("latency-test-create-{}", i % operation_count));
        
        let start = Instant::now();
        cluster_manager.send_typed_message("/user/order", "QUERY_ORDERS", query).expect("发送查询失败");
        query_latencies.push(start.elapsed());
        
        // 短暂休息，避免压力过大
        if i % 10 == 9 {
            sleep(Duration::from_millis(10)).await;
        }
    }
    
    // 3. 测试取消订单延迟
    println!("测试取消订单延迟...");
    let mut cancel_latencies = Vec::with_capacity(operation_count);
    
    for i in 0..operation_count {
        let cancel_req = trading::models::order::CancelOrderRequest {
            order_id: format!("latency-test-create-{}", i % operation_count),
            client_id: "latency-test-client".to_string(),
        };
        
        let start = Instant::now();
        cluster_manager.send_typed_message("/user/order", "CANCEL_ORDER", cancel_req).expect("发送取消请求失败");
        cancel_latencies.push(start.elapsed());
        
        // 短暂休息，避免压力过大
        if i % 10 == 9 {
            sleep(Duration::from_millis(10)).await;
        }
    }
    
    // 计算并输出结果
    let calculate_stats = |latencies: &[Duration], operation: &str| {
        let mut nanos: Vec<u128> = latencies.iter().map(|d| d.as_nanos()).collect();
        nanos.sort();
        
        let avg = Duration::from_nanos((nanos.iter().sum::<u128>() / nanos.len() as u128) as u64);
        let p50 = Duration::from_nanos(nanos[nanos.len() / 2] as u64);
        let p95 = Duration::from_nanos(nanos[(nanos.len() * 95) / 100] as u64);
        let p99 = Duration::from_nanos(nanos[(nanos.len() * 99) / 100] as u64);
        
        println!("{} 操作延迟:", operation);
        println!("  平均: {:?}", avg);
        println!("  P50: {:?}", p50);
        println!("  P95: {:?}", p95);
        println!("  P99: {:?}", p99);
    };
    
    println!("\n延迟测试结果:");
    calculate_stats(&create_latencies, "创建订单");
    calculate_stats(&query_latencies, "查询订单");
    calculate_stats(&cancel_latencies, "取消订单");
} 