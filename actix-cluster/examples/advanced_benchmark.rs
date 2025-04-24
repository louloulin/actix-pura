use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::Arc;
use actix::prelude::*;
use tokio::sync::{Mutex, mpsc};
use tokio::time::sleep;
use rand::{Rng, thread_rng};
use std::io::Write;
use std::fs::File;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod,
    NodeRole, SerializationFormat, NodeInfo
};

// Message size options
const SMALL_MESSAGE: usize = 128;    // 128 bytes
const MEDIUM_MESSAGE: usize = 4096;  // 4 KB
const LARGE_MESSAGE: usize = 65536;  // 64 KB

// Load patterns
#[derive(Clone, Copy, Debug)]
enum LoadPattern {
    Constant,      // 稳定负载
    Burst,         // 突发流量
    Ramp,          // 逐步增加
    Wave           // 波动负载
}

// Message type for benchmark
#[derive(Message, Clone)]
#[rtype(result = "BenchmarkResponse")]
struct BenchmarkMessage {
    id: u64,
    sender_node: String,
    timestamp: Instant,
    payload: Vec<u8>,
}

// Response message
#[derive(Message, Clone)]
#[rtype(result = "()")]
struct BenchmarkResponse {
    id: u64,
    original_timestamp: Instant,
    response_timestamp: Instant,
    responder_node: String,
}

// Detailed metrics
struct Metrics {
    total_messages: u64,
    successful_messages: u64,
    failed_messages: u64,
    total_time: Duration,
    latencies: Vec<Duration>,
    throughput: f64,         // 消息/秒
    data_throughput: f64,    // MB/秒
    percentiles: HashMap<u8, Duration>, // 存储p50, p90, p95, p99延迟
    errors: HashMap<String, u64>, // 错误类型和数量
}

impl Metrics {
    fn new() -> Self {
        let mut percentiles = HashMap::new();
        percentiles.insert(50, Duration::from_secs(0));
        percentiles.insert(90, Duration::from_secs(0));
        percentiles.insert(95, Duration::from_secs(0));
        percentiles.insert(99, Duration::from_secs(0));

        Self {
            total_messages: 0,
            successful_messages: 0,
            failed_messages: 0,
            total_time: Duration::from_secs(0),
            latencies: Vec::new(),
            throughput: 0.0,
            data_throughput: 0.0,
            percentiles,
            errors: HashMap::new(),
        }
    }

    fn calculate_percentiles(&mut self) {
        if self.latencies.is_empty() {
            return;
        }

        // 排序延迟以计算百分位
        let mut sorted_latencies = self.latencies.clone();
        sorted_latencies.sort();

        let len = sorted_latencies.len();

        // 计算不同百分位的延迟
        self.percentiles.insert(50, sorted_latencies[len * 50 / 100]);
        self.percentiles.insert(90, sorted_latencies[len * 90 / 100]);
        self.percentiles.insert(95, sorted_latencies[len * 95 / 100]);
        self.percentiles.insert(99, sorted_latencies[len * 99 / 100]);
    }

    fn print_summary(&self) {
        println!("\n===== 性能测试结果 =====");
        println!("总消息数: {}", self.total_messages);
        println!("成功消息数: {}", self.successful_messages);
        println!("失败消息数: {}", self.failed_messages);
        println!("总耗时: {:?}", self.total_time);

        if !self.latencies.is_empty() {
            let min = self.latencies.iter().min().unwrap();
            let max = self.latencies.iter().max().unwrap();
            let avg: Duration = self.latencies.iter().sum::<Duration>() / self.latencies.len() as u32;

            println!("\n延迟统计:");
            println!("  最小延迟: {:?}", min);
            println!("  最大延迟: {:?}", max);
            println!("  平均延迟: {:?}", avg);
            println!("  P50 延迟: {:?}", self.percentiles.get(&50).unwrap_or(&Duration::from_secs(0)));
            println!("  P90 延迟: {:?}", self.percentiles.get(&90).unwrap_or(&Duration::from_secs(0)));
            println!("  P95 延迟: {:?}", self.percentiles.get(&95).unwrap_or(&Duration::from_secs(0)));
            println!("  P99 延迟: {:?}", self.percentiles.get(&99).unwrap_or(&Duration::from_secs(0)));
        }

        println!("\n吞吐量统计:");
        println!("  消息吞吐量: {:.2} 消息/秒", self.throughput);
        println!("  数据吞吐量: {:.2} MB/秒", self.data_throughput);

        if !self.errors.is_empty() {
            println!("\n错误统计:");
            for (error, count) in &self.errors {
                println!("  {}: {} 次", error, count);
            }
        }
    }

    fn save_to_csv(&self, filename: &str) -> std::io::Result<()> {
        let mut file = File::create(filename)?;

        // 写入CSV头
        writeln!(file, "消息ID,延迟(ms)")?;

        // 写入所有延迟数据
        for (i, latency) in self.latencies.iter().enumerate() {
            writeln!(file, "{},{}", i, latency.as_millis())?;
        }

        Ok(())
    }
}

// Test actor
struct BenchmarkActor {
    name: String,
    received_count: u64,
    sent_count: u64,
    latencies: Vec<Duration>,
    message_size: usize,
    result_sender: Option<mpsc::Sender<Metrics>>,
}

impl Actor for BenchmarkActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("BenchmarkActor started: {}", self.name);
    }
}

impl Handler<BenchmarkMessage> for BenchmarkActor {
    type Result = MessageResult<BenchmarkMessage>;

    fn handle(&mut self, msg: BenchmarkMessage, _ctx: &mut Self::Context) -> Self::Result {
        // 记录消息接收
        self.received_count += 1;

        if self.received_count % 1000 == 0 {
            log::info!("{} received {} messages", self.name, self.received_count);
        }

        // 创建响应
        let response = BenchmarkResponse {
            id: msg.id,
            original_timestamp: msg.timestamp,
            response_timestamp: Instant::now(),
            responder_node: self.name.clone(),
        };

        MessageResult(response)
    }
}

impl Handler<BenchmarkResponse> for BenchmarkActor {
    type Result = ();

    fn handle(&mut self, msg: BenchmarkResponse, _ctx: &mut Self::Context) -> Self::Result {
        // 计算延迟
        let latency = msg.response_timestamp.duration_since(msg.original_timestamp);

        // 记录延迟
        self.latencies.push(latency);

        if self.latencies.len() % 1000 == 0 {
            log::info!("{} received {} responses, latest latency: {:?}",
                      self.name, self.latencies.len(), latency);
        }
    }
}

struct FinishBenchmark;

impl Message for FinishBenchmark {
    type Result = ();
}

impl Handler<FinishBenchmark> for BenchmarkActor {
    type Result = ();

    fn handle(&mut self, _msg: FinishBenchmark, _ctx: &mut Self::Context) -> Self::Result {
        log::info!("{} finishing benchmark", self.name);

        // 创建指标结果
        let mut metrics = Metrics::new();
        metrics.total_messages = self.sent_count;
        metrics.successful_messages = self.latencies.len() as u64;
        metrics.failed_messages = self.sent_count - self.latencies.len() as u64;
        metrics.latencies = self.latencies.clone();

        // 计算百分位数
        metrics.calculate_percentiles();

        // 发送结果
        if let Some(sender) = self.result_sender.take() {
            let _ = sender.try_send(metrics);
        }
    }
}

impl BenchmarkActor {
    fn new(name: String, message_size: usize, result_sender: mpsc::Sender<Metrics>) -> Self {
        Self {
            name,
            received_count: 0,
            sent_count: 0,
            latencies: Vec::new(),
            message_size,
            result_sender: Some(result_sender),
        }
    }
}

// 添加一个设置sent_count的消息
#[derive(Message)]
#[rtype(result = "()")]
struct SetSentCount(u64);

impl Handler<SetSentCount> for BenchmarkActor {
    type Result = ();

    fn handle(&mut self, msg: SetSentCount, _ctx: &mut Self::Context) -> Self::Result {
        self.sent_count = msg.0;
    }
}

async fn run_benchmark_node(
    node_id: usize,
    port: u16,
    target_node: Option<usize>,
    message_count: u64,
    message_size: usize,
    load_pattern: LoadPattern,
    result_sender: mpsc::Sender<Metrics>
) {
    // 创建节点配置
    let config = create_config(format!("node{}", node_id), port);

    // 创建集群系统
    let mut system = ClusterSystem::new(config);
    log::info!("Node {} created: {}", node_id, system.local_node().id);

    // 创建benchmark actor并跟踪sent_count
    let mut sent_count: u64 = 0;
    let benchmark_actor = BenchmarkActor::new(
        format!("benchmark-{}", node_id),
        message_size,
        result_sender
    ).start();

    // 启动集群系统
    match system.start().await {
        Ok(_) => {
            log::info!("Node {} started", node_id);
        },
        Err(e) => {
            log::error!("Node {} failed to start: {}", node_id, e);
            return;
        }
    };

    // 如果是发送者节点
    if let Some(target) = target_node {
        // 让系统稳定下来
        sleep(Duration::from_secs(5)).await;

        log::info!("Node {} starting to send {} messages to node {}",
                  node_id, message_count, target);

        let start_time = Instant::now();

        // 根据负载模式发送消息
        match load_pattern {
            LoadPattern::Constant => {
                // 恒定速率发送
                for i in 0..message_count {
                    let payload = vec![0u8; message_size];
                    benchmark_actor.do_send(BenchmarkMessage {
                        id: i,
                        sender_node: format!("node{}", node_id),
                        timestamp: Instant::now(),
                        payload,
                    });

                    sent_count += 1;

                    if i % 1000 == 0 {
                        sleep(Duration::from_millis(10)).await;
                        log::info!("Sent {} messages", i);
                    }
                }
            },
            LoadPattern::Burst => {
                // 突发流量模式
                let burst_size = 1000;
                let bursts = message_count / burst_size;

                for b in 0..bursts {
                    log::info!("Sending burst {} of {}", b+1, bursts);

                    // 快速发送一批消息
                    for i in 0..burst_size {
                        let msg_id = b * burst_size + i;
                        let payload = vec![0u8; message_size];
                        benchmark_actor.do_send(BenchmarkMessage {
                            id: msg_id,
                            sender_node: format!("node{}", node_id),
                            timestamp: Instant::now(),
                            payload,
                        });

                        sent_count += 1;
                    }

                    // 休息一段时间
                    sleep(Duration::from_secs(2)).await;
                }
            },
            LoadPattern::Ramp => {
                // 逐步增加负载
                let stages = 10;
                let msgs_per_stage = message_count / stages;

                for stage in 1..=stages {
                    let delay = Duration::from_millis(100 - (stage * 10) as u64);
                    log::info!("Stage {}/{}: delay between messages: {:?}",
                              stage, stages, delay);

                    for i in 0..msgs_per_stage {
                        let msg_id = (stage-1) * msgs_per_stage + i;
                        let payload = vec![0u8; message_size];
                        benchmark_actor.do_send(BenchmarkMessage {
                            id: msg_id,
                            sender_node: format!("node{}", node_id),
                            timestamp: Instant::now(),
                            payload,
                        });

                        sent_count += 1;

                        if i % 100 == 0 {
                            sleep(delay).await;
                        }
                    }
                }
            },
            LoadPattern::Wave => {
                // 波动负载模式
                let cycle_length = 1000; // 每个周期的消息数
                let cycles = message_count / cycle_length;

                for c in 0..cycles {
                    for i in 0..cycle_length {
                        let msg_id = c * cycle_length + i;
                        let payload = vec![0u8; message_size];
                        benchmark_actor.do_send(BenchmarkMessage {
                            id: msg_id,
                            sender_node: format!("node{}", node_id),
                            timestamp: Instant::now(),
                            payload,
                        });

                        sent_count += 1;

                        // 周期性调整发送间隔
                        let phase = (i as f64 / cycle_length as f64) * 2.0 * std::f64::consts::PI;
                        let delay_ms = ((phase.sin() + 1.0) * 10.0) as u64;

                        if i % 100 == 0 {
                            sleep(Duration::from_millis(delay_ms)).await;
                        }
                    }
                }
            }
        }

        let elapsed = start_time.elapsed();
        log::info!("Node {} finished sending {} messages in {:?}",
                  node_id, message_count, elapsed);

        // 等待所有响应
        sleep(Duration::from_secs(10)).await;

        // 直接在Actor中设置sent_count
        benchmark_actor.do_send(SetSentCount(sent_count));

        // 完成测试并发送结果
        benchmark_actor.do_send(FinishBenchmark);
    } else {
        // 接收者节点运行更长时间
        sleep(Duration::from_secs(120)).await;
        benchmark_actor.do_send(FinishBenchmark);
    }
}

// 创建集群配置
fn create_config(name: String, port: u16) -> ClusterConfig {
    let mut config = ClusterConfig::new();
    config = config.cluster_name(name);
    config = config.bind_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port));
    config = config.node_role(NodeRole::Worker);
    config = config.architecture(Architecture::Decentralized);
    config = config.discovery(DiscoveryMethod::Multicast);
    config = config.serialization_format(SerializationFormat::Bincode);
    config
}

#[actix_rt::main]
async fn main() {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // 测试参数
    let node_count = 3;                           // 节点数量
    let message_count = 10_000;                   // 消息总数
    let message_size = MEDIUM_MESSAGE;            // 消息大小
    let load_pattern = LoadPattern::Constant;     // 负载模式
    let base_port = 8560;                         // 起始端口

    println!("==== 开始 actix-cluster 性能测试 ====");
    println!("节点数量: {}", node_count);
    println!("消息数量: {}", message_count);
    println!("消息大小: {} bytes", message_size);
    println!("负载模式: {:?}", load_pattern);

    // 创建结果通道
    let (result_sender, mut result_receiver) = mpsc::channel::<Metrics>(node_count);

    // 创建共享的任务集合
    let tasks = Arc::new(Mutex::new(Vec::new()));

    // 创建LocalSet
    let local = tokio::task::LocalSet::new();

    // 在LocalSet内运行测试
    local.run_until(async move {
        // 创建并启动所有节点
        for i in 1..=node_count {
            let node_id = i;
            let port = base_port + i as u16;
            let sender = result_sender.clone();

            // 只让第一个节点发送消息
            let target = if i == 1 { Some(2) } else { None };

            // 启动节点任务 - 使用spawn_local
            let task = tokio::task::spawn_local(async move {
                run_benchmark_node(
                    node_id,
                    port,
                    target,
                    message_count,
                    message_size,
                    load_pattern,
                    sender
                ).await;
            });

            tasks.lock().await.push(task);
        }

        // 收集所有结果
        let mut all_metrics = Vec::new();
        for _ in 0..node_count {
            if let Some(metrics) = result_receiver.recv().await {
                all_metrics.push(metrics);
            }
        }

        // 等待所有任务完成
        for task in Arc::try_unwrap(tasks).unwrap().into_inner() {
            let _ = task.await;
        }

        // 分析结果
        if !all_metrics.is_empty() {
            println!("\n==== 集群性能分析 ====");

            for (i, metrics) in all_metrics.iter().enumerate() {
                println!("\n节点 {} 结果:", i+1);
                metrics.print_summary();

                // 保存详细结果到CSV
                let filename = format!("node{}_results.csv", i+1);
                if let Err(e) = metrics.save_to_csv(&filename) {
                    println!("Warning: Failed to save results to {}: {}", filename, e);
                } else {
                    println!("Detailed metrics saved to {}", filename);
                }
            }

            // 汇总集群整体性能
            if let Some(sender_metrics) = all_metrics.first() {
                println!("\n==== 集群整体性能 ====");
                println!("总消息量: {}", sender_metrics.total_messages);
                println!("总耗时: {:?}", sender_metrics.total_time);
                println!("集群吞吐量: {:.2} msgs/sec", sender_metrics.throughput);
                println!("数据吞吐量: {:.2} MB/sec", sender_metrics.data_throughput);

                // 性能建议
                println!("\n==== 性能分析和建议 ====");

                if let Some(p99) = sender_metrics.percentiles.get(&99) {
                    if p99 > &Duration::from_millis(100) {
                        println!("⚠️ P99延迟较高 (>100ms)，可能需要优化消息处理逻辑");
                    }
                }

                if sender_metrics.throughput < 1000.0 {
                    println!("⚠️ 集群吞吐量较低，建议检查序列化方式或网络配置");
                }

                if sender_metrics.failed_messages > 0 {
                    let failure_rate = sender_metrics.failed_messages as f64 /
                                      sender_metrics.total_messages as f64 * 100.0;

                    println!("⚠️ 消息失败率: {:.2}%，建议检查错误日志和集群配置", failure_rate);
                }
            }
        } else {
            println!("No benchmark results received!");
        }
    }).await;
}