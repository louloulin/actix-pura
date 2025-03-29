use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use std::sync::Arc;
use actix_rt::System;
use actix::prelude::*;
use tokio::sync::{Mutex, mpsc};
use tokio::time::sleep;

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole, SerializationFormat
};

// 消息类型定义
#[derive(Message)]
#[rtype(result = "()")]
struct PingMessage {
    id: u64,
    timestamp: Instant,
    sender: String,
}

// 记录结果的结构
struct BenchmarkResults {
    total_messages: u64,
    successful_messages: u64,
    failed_messages: u64,
    total_time: Duration,
    min_latency: Duration,
    max_latency: Duration,
    avg_latency: Duration,
}

// 压测Actor
struct BenchmarkActor {
    name: String,
    received_count: u64,
    latencies: Vec<Duration>,
    test_result_sender: Option<mpsc::Sender<BenchmarkResults>>,
}

impl Actor for BenchmarkActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("BenchmarkActor started: {}", self.name);
    }
}

impl Handler<PingMessage> for BenchmarkActor {
    type Result = ();
    
    fn handle(&mut self, msg: PingMessage, _ctx: &mut Self::Context) -> Self::Result {
        // 计算延迟
        let now = Instant::now();
        let latency = now.duration_since(msg.timestamp);
        
        // 记录消息
        self.received_count += 1;
        self.latencies.push(latency);
        
        if self.received_count % 1000 == 0 {
            log::info!("{} received {} messages", self.name, self.received_count);
        }
    }
}

impl BenchmarkActor {
    fn new(name: String, test_result_sender: mpsc::Sender<BenchmarkResults>) -> Self {
        Self {
            name,
            received_count: 0,
            latencies: Vec::new(),
            test_result_sender: Some(test_result_sender),
        }
    }
    
    fn calculate_results(&self, total_time: Duration) -> BenchmarkResults {
        let total_messages = self.latencies.len() as u64;
        
        if total_messages == 0 {
            return BenchmarkResults {
                total_messages,
                successful_messages: 0,
                failed_messages: 0,
                total_time,
                min_latency: Duration::from_secs(0),
                max_latency: Duration::from_secs(0),
                avg_latency: Duration::from_secs(0),
            };
        }
        
        let min_latency = self.latencies.iter().min().cloned().unwrap_or(Duration::from_secs(0));
        let max_latency = self.latencies.iter().max().cloned().unwrap_or(Duration::from_secs(0));
        
        let total_latency: Duration = self.latencies.iter().sum();
        let avg_latency = total_latency / total_messages as u32;
        
        BenchmarkResults {
            total_messages,
            successful_messages: total_messages,
            failed_messages: 0,
            total_time,
            min_latency,
            max_latency,
            avg_latency,
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct FinishBenchmark;

impl Handler<FinishBenchmark> for BenchmarkActor {
    type Result = ();
    
    fn handle(&mut self, _msg: FinishBenchmark, _ctx: &mut Self::Context) -> Self::Result {
        log::info!("{} finishing benchmark with {} messages", self.name, self.received_count);
        
        // 计算结果
        let results = self.calculate_results(Duration::from_secs(0));
        
        // 发送结果
        if let Some(sender) = self.test_result_sender.take() {
            let _ = sender.try_send(results);
        }
    }
}

// 运行单个节点的压测
async fn run_node_benchmark(
    node_id: usize,
    port: u16, 
    node_count: usize,
    msgs_per_node: u64,
    result_sender: mpsc::Sender<BenchmarkResults>
) {
    // 创建节点配置
    let config = create_config(format!("node{}", node_id), port, Architecture::Decentralized);
    
    // 创建集群系统
    let system = ClusterSystem::new(&format!("node{}", node_id), config);
    log::info!("Node {} created: {}", node_id, system.local_node().id);
    
    // 创建benchmark actor
    let benchmark_actor = BenchmarkActor::new(
        format!("benchmark-{}", node_id),
        result_sender
    ).start();
    
    // 启动集群系统
    let system_addr = match system.start().await {
        Ok(addr) => {
            log::info!("Node {} started", node_id);
            addr
        },
        Err(e) => {
            log::error!("Node {} failed to start: {}", node_id, e);
            return;
        }
    };
    
    // 等待所有节点启动
    sleep(Duration::from_secs(2)).await;
    
    // 发送消息
    if node_id == 1 {  // 只让第一个节点发送消息以简化示例
        log::info!("Node {} sending {} messages to each node", node_id, msgs_per_node);
        
        let start_time = Instant::now();
        
        // 发送消息到每个节点
        for target_id in 1..=node_count {
            if target_id == node_id {
                continue;  // 跳过自己
            }
            
            for i in 0..msgs_per_node {
                let msg = PingMessage {
                    id: i,
                    timestamp: Instant::now(),
                    sender: format!("node{}", node_id),
                };
                
                // 发送消息到目标节点的benchmark actor
                // 注意：在实际的集群中，这里应该使用集群内的消息路由机制
                if let Err(e) = benchmark_actor.try_send(msg) {
                    log::error!("Failed to send message {}: {}", i, e);
                }
                
                // 短暂睡眠避免发送过快
                if i % 1000 == 0 {
                    sleep(Duration::from_millis(1)).await;
                }
            }
        }
        
        let elapsed = start_time.elapsed();
        log::info!("Node {} finished sending messages in {:?}", node_id, elapsed);
    }
    
    // 等待所有消息处理完成
    sleep(Duration::from_secs(5)).await;
    
    // 通知actor完成测试
    benchmark_actor.do_send(FinishBenchmark);
    
    // 给时间让结果发送出去
    sleep(Duration::from_secs(1)).await;
}

#[actix_rt::main]
async fn main() {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // 测试参数
    let node_count = 3;  // 节点数量
    let msgs_per_node = 10000;  // 每个节点发送的消息数
    let base_port = 8560;  // 起始端口号
    
    // 创建结果通道
    let (result_sender, mut result_receiver) = mpsc::channel::<BenchmarkResults>(node_count);
    
    // 创建共享的任务句柄集合
    let tasks = Arc::new(Mutex::new(Vec::new()));
    
    log::info!("Starting benchmark with {} nodes, {} messages per node", node_count, msgs_per_node);
    
    // 创建并启动所有节点
    for i in 1..=node_count {
        let node_id = i;
        let port = base_port + i as u16;
        let sender = result_sender.clone();
        let node_count = node_count;
        let msgs_per_node = msgs_per_node;
        
        // 启动节点任务
        let task = tokio::spawn(async move {
            run_node_benchmark(node_id, port, node_count, msgs_per_node, sender).await;
        });
        
        tasks.lock().await.push(task);
    }
    
    // 等待所有任务完成
    for task in Arc::try_unwrap(tasks).unwrap().into_inner() {
        let _ = task.await;
    }
    
    // 收集所有结果
    let mut all_results = Vec::new();
    while let Ok(result) = result_receiver.try_recv() {
        all_results.push(result);
    }
    
    // 分析结果
    analyze_results(all_results, node_count, msgs_per_node);
}

// 分析测试结果
fn analyze_results(results: Vec<BenchmarkResults>, node_count: usize, msgs_per_node: u64) {
    println!("\n===== BENCHMARK RESULTS =====");
    println!("Number of nodes: {}", node_count);
    println!("Messages per node: {}", msgs_per_node);
    
    if results.is_empty() {
        println!("No results collected!");
        return;
    }
    
    let total_messages: u64 = results.iter().map(|r| r.total_messages).sum();
    let successful_messages: u64 = results.iter().map(|r| r.successful_messages).sum();
    let failed_messages: u64 = results.iter().map(|r| r.failed_messages).sum();
    
    // 计算平均延迟
    let mut all_min = Duration::from_secs(u64::MAX);
    let mut all_max = Duration::from_secs(0);
    let mut total_latency = Duration::from_secs(0);
    
    for result in &results {
        if result.min_latency < all_min {
            all_min = result.min_latency;
        }
        if result.max_latency > all_max {
            all_max = result.max_latency;
        }
        total_latency += result.avg_latency * result.total_messages as u32;
    }
    
    let avg_latency = if total_messages > 0 {
        total_latency / total_messages as u32
    } else {
        Duration::from_secs(0)
    };
    
    println!("\nMessage Statistics:");
    println!("  Total messages: {}", total_messages);
    println!("  Successful messages: {}", successful_messages);
    println!("  Failed messages: {}", failed_messages);
    println!("  Success rate: {:.2}%", (successful_messages as f64 / total_messages as f64) * 100.0);
    
    println!("\nLatency Statistics:");
    println!("  Minimum latency: {:?}", all_min);
    println!("  Maximum latency: {:?}", all_max);
    println!("  Average latency: {:?}", avg_latency);
    
    println!("\nThroughput:");
    let total_expected = (node_count * (node_count - 1)) as u64 * msgs_per_node;
    println!("  Expected messages: {}", total_expected);
    println!("  Actual messages: {}", total_messages);
    println!("  Delivery ratio: {:.2}%", (total_messages as f64 / total_expected as f64) * 100.0);
    
    // 结论和建议
    println!("\nAnalysis:");
    if avg_latency > Duration::from_millis(100) {
        println!("  High latency detected. Consider optimizing network or message serialization.");
    } else if avg_latency < Duration::from_millis(10) {
        println!("  Excellent latency performance.");
    } else {
        println!("  Acceptable latency performance.");
    }
    
    if (successful_messages as f64 / total_expected as f64) < 0.95 {
        println!("  Message delivery ratio is below 95%. Check for bottlenecks or node failures.");
    } else {
        println!("  Good message delivery ratio. System is reliable under load.");
    }
    
    println!("\nNote: This is a simplified benchmark focusing on message throughput and latency.");
    println!("For real-world applications, consider additional metrics like CPU usage, memory consumption,");
    println!("and network bandwidth usage.");
}

// 创建节点配置
fn create_config(name: String, port: u16, architecture: Architecture) -> ClusterConfig {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
    
    let seed_nodes = vec![
        "127.0.0.1:8561".to_string(),  // 第一个节点作为种子节点
    ];
    
    ClusterConfig::new()
        .architecture(architecture)
        .node_role(NodeRole::Peer)
        .bind_addr(addr)
        .cluster_name(format!("{}-cluster", name))
        .heartbeat_interval(Duration::from_millis(500))
        .node_timeout(Duration::from_secs(5))
        .discovery(DiscoveryMethod::Static {
            seed_nodes: if name == "node1" { vec![] } else { seed_nodes }
        })
        .serialization_format(SerializationFormat::Bincode)
        .build()
        .expect("创建集群配置失败")
} 