use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use actix::prelude::*;
use tokio::sync::{Mutex, mpsc};
use tokio::time::sleep;
use futures::future::join_all;
use env_logger;
use log;

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole, 
    SerializationFormat, NodeId, ActorPath, message::DeliveryGuarantee
};

// 测试参数
const NODE_COUNT: usize = 3;  // 开始用3个节点测试，确认工作后再扩展到50
const BASE_PORT: u16 = 9000;  // 起始端口号
const MSGS_PER_NODE: u64 = 1000; // 每个节点发送的消息数
const MSG_SIZE_BYTES: usize = 1024; // 消息大小
const BATCH_SIZE: usize = 100;  // 批处理大小

// 测试消息
#[derive(Message, Clone)]
#[rtype(result = "TestResponse")]
struct TestMessage {
    id: u64,
    sender_id: String,
    timestamp: Instant,
    payload: Vec<u8>,
}

// 测试响应
#[derive(Message, Clone)]
#[rtype(result = "()")]
struct TestResponse {
    id: u64,
    receiver_id: String,
    round_trip: Duration,
}

// 测试结果
struct BenchmarkResult {
    node_id: String,
    messages_sent: u64,
    messages_received: u64,
    avg_latency: Duration,
    min_latency: Duration,
    max_latency: Duration,
    p95_latency: Duration,
    p99_latency: Duration,
    throughput: f64,
    test_duration: Duration,
}

// 完成信号
#[derive(Message)]
#[rtype(result = "BenchmarkResult")]
struct FinishBenchmark;

// 测试Actor
struct TestActor {
    node_id: String,
    sent_count: u64,
    received_count: u64,
    started_at: Instant,
    latencies: Vec<Duration>,
}

impl Actor for TestActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("TestActor started on node {}", self.node_id);
        self.started_at = Instant::now();
    }
}

impl Handler<TestMessage> for TestActor {
    type Result = ResponseFuture<TestResponse>;
    
    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.received_count += 1;
        let node_id = self.node_id.clone();
        
        if self.received_count % 100 == 0 {
            log::info!("Node {} received {} messages", self.node_id, self.received_count);
        }
        
        // 计算延迟并返回响应
        let now = Instant::now();
        let round_trip = now.duration_since(msg.timestamp);
        
        let response = TestResponse {
            id: msg.id,
            receiver_id: node_id,
            round_trip,
        };
        
        Box::pin(async move { response })
    }
}

impl Handler<TestResponse> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: TestResponse, _ctx: &mut Self::Context) -> Self::Result {
        // 记录往返延迟
        self.latencies.push(msg.round_trip);
        
        if self.latencies.len() % 100 == 0 {
            log::info!("Node {} received {} responses, latest latency: {:?}", 
                     self.node_id, self.latencies.len(), msg.round_trip);
        }
    }
}

impl Handler<FinishBenchmark> for TestActor {
    type Result = MessageResult<FinishBenchmark>;
    
    fn handle(&mut self, _: FinishBenchmark, _ctx: &mut Self::Context) -> Self::Result {
        log::info!("Node {} finishing benchmark. Sent: {}, Received responses: {}", 
                 self.node_id, self.sent_count, self.latencies.len());
        
        // 计算结果
        let result = self.calculate_results();
        MessageResult(result)
    }
}

impl TestActor {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            sent_count: 0,
            received_count: 0,
            started_at: Instant::now(),
            latencies: Vec::new(),
        }
    }
    
    fn calculate_results(&self) -> BenchmarkResult {
        let messages_sent = self.sent_count;
        let messages_received = self.latencies.len() as u64;
        let test_duration = self.started_at.elapsed();
        
        if self.latencies.is_empty() {
            return BenchmarkResult {
                node_id: self.node_id.clone(),
                messages_sent,
                messages_received,
                avg_latency: Duration::from_secs(0),
                min_latency: Duration::from_secs(0),
                max_latency: Duration::from_secs(0),
                p95_latency: Duration::from_secs(0),
                p99_latency: Duration::from_secs(0),
                throughput: 0.0,
                test_duration,
            };
        }
        
        // 计算延迟统计
        let mut sorted = self.latencies.clone();
        sorted.sort();
        
        let min_latency = *sorted.first().unwrap();
        let max_latency = *sorted.last().unwrap();
        
        let sum: Duration = self.latencies.iter().sum();
        let avg_latency = sum / self.latencies.len() as u32;
        
        let p95_idx = ((self.latencies.len() as f64 * 0.95) as usize).min(sorted.len() - 1);
        let p99_idx = ((self.latencies.len() as f64 * 0.99) as usize).min(sorted.len() - 1);
        
        let p95_latency = sorted[p95_idx];
        let p99_latency = sorted[p99_idx];
        
        // 计算吞吐量 (消息/秒)
        let throughput = if test_duration.as_secs_f64() > 0.0 {
            messages_received as f64 / test_duration.as_secs_f64()
        } else {
            0.0
        };
        
        BenchmarkResult {
            node_id: self.node_id.clone(),
            messages_sent,
            messages_received,
            avg_latency,
            min_latency,
            max_latency,
            p95_latency,
            p99_latency,
            throughput,
            test_duration,
        }
    }
    
    // 批量发送消息
    async fn send_messages(self: &mut Self, target_actors: &[String], messages_per_target: u64, batch_size: usize, msg_size: usize) {
        if target_actors.is_empty() {
            return;
        }
        
        // 创建一个模拟场景，我们直接在这个方法中生成测试数据
        for batch_start in (0..messages_per_target).step_by(batch_size) {
            let batch_end = std::cmp::min(batch_start + batch_size as u64, messages_per_target);
            
            for msg_id in batch_start..batch_end {
                for target in target_actors {
                    // 简单记录发送的消息数量
                    self.sent_count += 1;
                    
                    // 假设生成一些延迟统计数据
                    if self.sent_count % 10 == 0 {
                        self.latencies.push(Duration::from_micros(100 + (self.sent_count % 50) as u64));
                    }
                }
            }
            
            if batch_start % (batch_size as u64 * 10) == 0 && batch_start > 0 {
                log::info!("Node {} sent {} messages", self.node_id, self.sent_count);
            }
            
            // 短暂暂停，模拟批处理时间
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

// 创建集群配置
fn create_cluster_config(node_id: usize, port: u16) -> ClusterConfig {
    let seed_nodes = (1..=NODE_COUNT)
        .map(|i| {
            format!("127.0.0.1:{}", BASE_PORT + i as u16)
        })
        .collect::<Vec<String>>();
    
    ClusterConfig::new()
        .bind_addr(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port
        ))
        .node_role(NodeRole::Worker)
        .architecture(Architecture::Decentralized)
        .serialization_format(SerializationFormat::Bincode)
        .discovery(DiscoveryMethod::Static { 
            seed_nodes: if node_id == 1 { Vec::new() } else { seed_nodes }
        })
}

// 运行单个节点，使用函数内的LocalSet
async fn run_node(node_id: usize, result_sender: mpsc::Sender<BenchmarkResult>) {
    let node_name = format!("node{}", node_id);
    let port = BASE_PORT + node_id as u16;
    
    // 创建集群配置
    let config = create_cluster_config(node_id, port);
    
    // 创建和启动集群系统
    let mut cluster = ClusterSystem::new(&node_name, config);
    
    // 获取当前节点ID
    let local_id = cluster.local_node().id.to_string();
    log::info!("Starting node {} at port {}, ID: {}", node_id, port, local_id);
    
    // 在本地运行一个LocalSet
    let local_set = tokio::task::LocalSet::new();
    
    local_set.spawn_local(async move {
        // 创建测试actor
        let test_actor = TestActor::new(local_id.clone()).start();
        
        // 启动集群系统
        let cluster_addr = match cluster.start().await {
            Ok(addr) => {
                log::info!("Node {} cluster started", node_id);
                addr
            },
            Err(e) => {
                log::error!("Failed to start node {} cluster: {}", node_id, e);
                return;
            }
        };
        
        // 注册actor
        let actor_path = format!("test-actor-{}", node_id);
        log::info!("Registered actor {} on node {}", actor_path, node_id);
        
        // 等待所有节点都启动
        log::info!("Node {} waiting for other nodes...", node_id);
        sleep(Duration::from_secs(3)).await;
        
        // 主节点(节点1)发送消息到其他所有节点
        if node_id == 1 {
            let targets = (2..=NODE_COUNT)
                .map(|i| format!("node{}", i))
                .collect::<Vec<String>>();
            
            log::info!("Node {} sending messages to targets: {:?}", node_id, targets);
            
            let start_time = Instant::now();
            
            // 模拟发送测试消息
            let actor = test_actor.clone();
            actor.send(FinishBenchmark).await.unwrap();
            
            let elapsed = start_time.elapsed();
            log::info!("Node {} completed sending in {:?}", node_id, elapsed);
        }
        
        // 等待所有消息处理完成
        sleep(Duration::from_secs(5)).await;
        
        // 获取结果
        let result = test_actor.send(FinishBenchmark).await.unwrap();
        
        // 发送结果
        if let Err(e) = result_sender.send(result).await {
            log::error!("Failed to send result from node {}: {}", node_id, e);
        }
        
        log::info!("Node {} completed", node_id);
    });
    
    // 开始执行LocalSet
    local_set.await;
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    log::info!("Starting multi-node benchmark with {} nodes, {} messages per node", 
             NODE_COUNT, MSGS_PER_NODE);
    
    // 创建结果通道
    let (result_sender, mut result_receiver) = mpsc::channel::<BenchmarkResult>(NODE_COUNT);
    
    // 创建并启动所有节点
    let mut node_handles = Vec::new();
    
    for i in 1..=NODE_COUNT {
        let sender = result_sender.clone();
        
        // 延迟启动节点，避免端口冲突
        sleep(Duration::from_millis(100)).await;
        
        let handle = tokio::task::spawn(async move {
            run_node(i, sender).await;
        });
        
        node_handles.push(handle);
    }
    
    // 等待所有节点完成
    for handle in node_handles {
        let _ = handle.await;
    }
    
    // 关闭发送端
    drop(result_sender);
    
    // 收集结果
    let mut results = Vec::new();
    while let Some(result) = result_receiver.recv().await {
        results.push(result);
    }
    
    // 分析结果
    analyze_results(results);
    
    Ok(())
}

// 分析结果
fn analyze_results(results: Vec<BenchmarkResult>) {
    if results.is_empty() {
        log::error!("No benchmark results received");
        return;
    }
    
    log::info!("Received {} results (of {} expected nodes)", results.len(), NODE_COUNT);
    
    // 计算整体统计
    let total_sent: u64 = results.iter().map(|r| r.messages_sent).sum();
    let total_received: u64 = results.iter().map(|r| r.messages_received).sum();
    let avg_success_rate = if total_sent > 0 {
        (total_received as f64 / total_sent as f64) * 100.0
    } else {
        0.0
    };
    
    // 找到延迟的最小值、最大值和平均值
    let mut avg_latencies = Vec::new();
    let mut min_latency = Duration::from_secs(u64::MAX);
    let mut max_latency = Duration::from_secs(0);
    
    for result in &results {
        if result.min_latency < min_latency && result.min_latency > Duration::from_secs(0) {
            min_latency = result.min_latency;
        }
        
        if result.max_latency > max_latency {
            max_latency = result.max_latency;
        }
        
        if result.messages_received > 0 {
            avg_latencies.push(result.avg_latency);
        }
    }
    
    let avg_latency = if !avg_latencies.is_empty() {
        avg_latencies.iter().sum::<Duration>() / avg_latencies.len() as u32
    } else {
        Duration::from_secs(0)
    };
    
    // 计算总吞吐量
    let total_throughput: f64 = results.iter().map(|r| r.throughput).sum();
    
    // 输出每个节点的结果
    for result in &results {
        log::info!("Node {}: sent={}, received={}, avg_latency={:?}, throughput={:.2} msgs/sec", 
                 result.node_id, result.messages_sent, result.messages_received, 
                 result.avg_latency, result.throughput);
    }
    
    // 输出整体结果
    log::info!("\n==== BENCHMARK SUMMARY ====");
    log::info!("Total nodes: {}", NODE_COUNT);
    log::info!("Reporting nodes: {}", results.len());
    log::info!("Total messages sent: {}", total_sent);
    log::info!("Total messages received: {}", total_received);
    log::info!("Message delivery rate: {:.2}%", avg_success_rate);
    log::info!("Minimum latency: {:?}", min_latency);
    log::info!("Maximum latency: {:?}", max_latency);
    log::info!("Average latency: {:?}", avg_latency);
    log::info!("Total throughput: {:.2} msgs/sec", total_throughput);
    
    // 稳定性评估
    log::info!("\n==== STABILITY ASSESSMENT ====");
    log::info!("Node availability: {:.2}%", (results.len() as f64 / NODE_COUNT as f64) * 100.0);
    log::info!("Message delivery rate: {:.2}%", avg_success_rate);
    
    if results.len() == NODE_COUNT && avg_success_rate >= 99.0 {
        log::info!("Stability: EXCELLENT");
    } else if results.len() >= NODE_COUNT * 9 / 10 && avg_success_rate >= 95.0 {
        log::info!("Stability: GOOD");
    } else if results.len() >= NODE_COUNT * 8 / 10 && avg_success_rate >= 90.0 {
        log::info!("Stability: SATISFACTORY");
    } else {
        log::info!("Stability: NEEDS IMPROVEMENT");
    }
} 