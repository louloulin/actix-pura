use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use std::sync::Arc;
use actix_rt::System;
use actix::prelude::*;
use tokio::sync::{Mutex, mpsc, RwLock};
use tokio::time::{sleep, timeout};
use std::collections::HashMap;
use std::fmt;
use std::collections::{VecDeque};
use env_logger;
use log::{self, info, debug, warn, error};

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole,
    SerializationFormat, NodeInfo, NodeId, ActorPath
};
use actix_cluster::cluster::ClusterSystemActor;

// 压测配置常量
const ACTOR_COUNT: usize = 10;    // 每个节点上的Actor数量
const CLUSTER_NODES: usize = 2;   // 集群节点数量

// 消息类型定义
#[derive(Message, Clone)]
#[rtype(result = "PingResponse")]
struct PingMessage {
    id: u64,
    timestamp: Instant,
    sender: NodeId,
    size: usize,        // 消息大小(字节)
    payload: Vec<u8>,   // 填充数据
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
struct PingResponse {
    id: u64,
    original_timestamp: Instant,
    responder: NodeId,
    round_trip: Duration,
}

// 节点状态监控消息
#[derive(Message)]
#[rtype(result = "NodeStatusReport")]
struct GetNodeStatus;

#[derive(Message, Clone)]
#[rtype(result = "()")]
struct NodeStatusReport {
    node_id: NodeId,
    timestamp: Instant,
    cpu_usage: f32,
    memory_usage: f32,
    message_processed: u64,
    is_healthy: bool,
}

// 记录结果的结构
#[derive(Clone)]
struct BenchmarkResults {
    node_id: NodeId,
    total_messages: u64,
    successful_messages: u64,
    failed_messages: u64,
    total_time: Duration,
    min_latency: Duration,
    max_latency: Duration,
    avg_latency: Duration,
    p95_latency: Duration,
    p99_latency: Duration,
    throughput: f64,
    node_failures: u32,
    errors: Vec<String>,
}

impl fmt::Display for BenchmarkResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "节点 {}: 总消息 = {}, 成功 = {}, 失败 = {}, 总时间 = {:?}, \n  延迟: 最小 = {:?}, 平均 = {:?}, 最大 = {:?}, P95 = {:?}, P99 = {:?}, \n  吞吐量 = {:.2} 消息/秒, 节点故障 = {}",
            self.node_id, self.total_messages, self.successful_messages, self.failed_messages,
            self.total_time, self.min_latency, self.avg_latency, self.max_latency,
            self.p95_latency, self.p99_latency, self.throughput, self.node_failures)
    }
}

// 压测配置
#[derive(Clone)]
struct BenchmarkConfig {
    message_count: u64,        // 每个节点发送的消息数
    message_size: usize,       // 消息大小(字节)
    batch_size: usize,         // 批处理大小
    wait_time: Duration,       // 节点启动后等待时间
    architecture: Architecture, // 集群架构
    serialization: SerializationFormat, // 序列化格式
}

// 压测Actor
struct BenchmarkActor {
    node_id: NodeId,
    name: String,
    received_count: u64,
    sent_count: u64,
    latencies: Vec<Duration>,
    started_at: Instant,
    errors: Vec<String>,
    node_failures: u32,
    status_reports: HashMap<NodeId, Vec<NodeStatusReport>>,
    cluster_system: Option<Arc<RwLock<ClusterSystem>>>,
    test_result_sender: Option<mpsc::Sender<BenchmarkResults>>,
}

impl Actor for BenchmarkActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("BenchmarkActor started: {} ({})", self.name, self.node_id);
        self.started_at = Instant::now();

        // 定期检查健康状态
        ctx.run_interval(Duration::from_secs(5), |actor, _| {
            actor.check_health_status();
        });
    }
}

impl Handler<PingMessage> for BenchmarkActor {
    type Result = ResponseFuture<PingResponse>;

    fn handle(&mut self, msg: PingMessage, _ctx: &mut Self::Context) -> Self::Result {
        // 处理接收到的消息
        self.received_count += 1;

        // 生成响应
        let now = Instant::now();
        let responder = self.node_id.clone();
        let round_trip = now.duration_since(msg.timestamp);

        if self.received_count % 10000 == 0 {
            log::info!("{} ({}) received {} messages", self.name, self.node_id, self.received_count);
        }

        let response = PingResponse {
            id: msg.id,
            original_timestamp: msg.timestamp,
            responder,
            round_trip,
        };

        Box::pin(async move { response })
    }
}

impl Handler<PingResponse> for BenchmarkActor {
    type Result = ();

    fn handle(&mut self, msg: PingResponse, _ctx: &mut Self::Context) -> Self::Result {
        // 记录往返延迟
        let latency = msg.round_trip;
        self.latencies.push(latency);

        if self.latencies.len() % 10000 == 0 {
            log::info!("{} ({}) received {} responses, latest latency: {:?}",
                     self.name, self.node_id, self.latencies.len(), latency);
        }
    }
}

impl Handler<GetNodeStatus> for BenchmarkActor {
    type Result = MessageResult<GetNodeStatus>;

    fn handle(&mut self, _: GetNodeStatus, _ctx: &mut Self::Context) -> Self::Result {
        // 模拟获取节点状态，真实场景可用系统命令获取CPU/内存使用情况
        let cpu_usage = 50.0 + (self.received_count as f32 % 40.0); // 模拟CPU使用率
        let memory_usage = 30.0 + (self.received_count as f32 % 50.0); // 模拟内存使用率

        let report = NodeStatusReport {
            node_id: self.node_id.clone(),
            timestamp: Instant::now(),
            cpu_usage,
            memory_usage,
            message_processed: self.received_count,
            is_healthy: true,
        };

        MessageResult(report)
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct FinishBenchmark;

impl Handler<FinishBenchmark> for BenchmarkActor {
    type Result = ();

    fn handle(&mut self, _msg: FinishBenchmark, _ctx: &mut Self::Context) -> Self::Result {
        log::info!("{} ({}) finishing benchmark with {} messages sent, {} received",
                 self.name, self.node_id, self.sent_count, self.received_count);

        // 计算结果
        let results = self.calculate_results();

        // 发送结果
        if let Some(sender) = self.test_result_sender.take() {
            let _ = sender.try_send(results);
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct SimulateProcessing {
    count: u64,
}

impl Handler<SimulateProcessing> for BenchmarkActor {
    type Result = ();

    fn handle(&mut self, msg: SimulateProcessing, _ctx: &mut Self::Context) -> Self::Result {
        self.simulate_message_processing(msg.count);
    }
}

impl BenchmarkActor {
    fn new(
        node_id: NodeId,
        name: String,
        cluster_system: Arc<RwLock<ClusterSystem>>,
        test_result_sender: mpsc::Sender<BenchmarkResults>
    ) -> Self {
        Self {
            node_id,
            name,
            received_count: 0,
            sent_count: 0,
            latencies: Vec::new(),
            started_at: Instant::now(),
            errors: Vec::new(),
            node_failures: 0,
            status_reports: HashMap::new(),
            cluster_system: Some(cluster_system),
            test_result_sender: Some(test_result_sender),
        }
    }

    fn calculate_results(&self) -> BenchmarkResults {
        let total_messages = self.sent_count;
        let successful_messages = self.latencies.len() as u64;
        let failed_messages = total_messages.saturating_sub(successful_messages);
        let total_time = self.started_at.elapsed();

        if self.latencies.is_empty() {
            return BenchmarkResults {
                node_id: self.node_id.clone(),
                total_messages,
                successful_messages,
                failed_messages,
                total_time,
                min_latency: Duration::from_secs(0),
                max_latency: Duration::from_secs(0),
                avg_latency: Duration::from_secs(0),
                p95_latency: Duration::from_secs(0),
                p99_latency: Duration::from_secs(0),
                throughput: 0.0,
                node_failures: self.node_failures,
                errors: self.errors.clone(),
            };
        }

        // 计算延迟统计
        let mut sorted = self.latencies.clone();
        sorted.sort();

        let min_latency = *sorted.first().unwrap();
        let max_latency = *sorted.last().unwrap();

        let total_latency: Duration = self.latencies.iter().sum();
        let avg_latency = total_latency / successful_messages as u32;

        // 计算百分位数
        let p95_idx = ((successful_messages as f64 * 0.95) as usize).min(sorted.len() - 1);
        let p99_idx = ((successful_messages as f64 * 0.99) as usize).min(sorted.len() - 1);

        let p95_latency = sorted[p95_idx];
        let p99_latency = sorted[p99_idx];

        // 计算吞吐量 (消息/秒)
        let throughput = if total_time.as_secs_f64() > 0.0 {
            successful_messages as f64 / total_time.as_secs_f64()
        } else {
            0.0
        };

        BenchmarkResults {
            node_id: self.node_id.clone(),
            total_messages,
            successful_messages,
            failed_messages,
            total_time,
            min_latency,
            max_latency,
            avg_latency,
            p95_latency,
            p99_latency,
            throughput,
            node_failures: self.node_failures,
            errors: self.errors.clone(),
        }
    }

    fn check_health_status(&mut self) {
        // 检查节点健康状态，真实场景下可以检查连接状态等
    }

    // 模拟消息处理（简化版本）
    fn simulate_message_processing(&mut self, count: u64) {
        // 简单模拟消息处理，直接增加计数和记录延迟
        self.sent_count += count;

        let now = Instant::now();

        // 生成各种不同的延迟模式，模拟真实网络和处理环境
        let base_delays = vec![
            Duration::from_micros(50),    // 极快消息 - 本地处理
            Duration::from_micros(150),   // 快速消息 - 同数据中心
            Duration::from_micros(500),   // 标准消息 - 正常网络
            Duration::from_millis(1),     // 慢消息 - 轻微网络延迟
            Duration::from_millis(5),     // 较慢消息 - 网络拥塞
        ];

        // 概率分布 - 大多数消息应该有中等延迟
        let distribution = vec![5, 15, 60, 15, 5]; // 百分比分布
        let mut delay_types = Vec::with_capacity(100);

        // 根据分布构建延迟类型数组
        let mut idx = 0;
        for (i, &pct) in distribution.iter().enumerate() {
            for _ in 0..pct {
                delay_types.push(i);
                idx += 1;
                if idx >= 100 {
                    break;
                }
            }
        }

        // 随机数生成种子
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let seed = seed + self.received_count; // 添加接收计数使不同节点有不同种子

        // 添加虚拟延迟 - 使用更复杂的方法模拟网络延迟波动
        for i in 0..count {
            // 简单的伪随机数生成
            let rnd = (seed.wrapping_mul(i + 1).wrapping_add(0x12345678)) % 100;
            let delay_type = delay_types[rnd as usize];
            let mut delay = base_delays[delay_type];

            // 对于一小部分消息，添加额外延迟，模拟网络抖动和异常
            if rnd < 2 { // 2% 的消息有严重延迟
                delay += Duration::from_millis(50 + (rnd * 5) as u64); // 50-100ms 额外延迟
            } else if rnd < 10 { // 8% 的消息有轻微延迟
                delay += Duration::from_millis(1 + rnd as u64); // 1-10ms 额外延迟
            }

            self.latencies.push(delay);
        }

        // 计算总延迟和平均延迟
        let total_latency: Duration = self.latencies.iter().sum();
        let avg_latency = if !self.latencies.is_empty() {
            total_latency / self.latencies.len() as u32
        } else {
            Duration::from_secs(0)
        };

        log::info!("模拟处理了 {} 条消息，耗时: {:?}，平均延迟: {:?}",
                  count, now.elapsed(), avg_latency);
    }
}

// 运行单个节点的压测
async fn run_node_benchmark(
    node_id: usize,
    port: u16,
    total_nodes: usize,
    config: BenchmarkConfig,
    result_sender: mpsc::Sender<BenchmarkResults>,
    all_nodes: Arc<RwLock<Vec<NodeId>>>,
) {
    // 创建节点配置
    let node_name = format!("node{}", node_id);
    let cluster_config = create_config(
        node_name.clone(),
        port,
        config.architecture,
        config.serialization
    );

    // 创建集群系统
    let mut system = ClusterSystem::new(cluster_config);
    let local_node_id = system.local_node().id.clone();
    log::info!("Node {} 创建, NodeId: {}", node_id, local_node_id);

    // 将系统包装在Arc<RwLock>中以便共享
    let system_arc = Arc::new(RwLock::new(system));

    // 创建benchmark actor
    let actor_name = format!("benchmark-{}", node_id);
    let benchmark_actor = BenchmarkActor::new(
        local_node_id.clone(),
        actor_name.clone(),
        system_arc.clone(),
        result_sender.clone()
    ).start();

    // 将自己添加到节点列表中
    {
        let mut nodes = all_nodes.write().await;
        nodes.push(local_node_id.clone());
    }

    // 等待集群启动和发现
    log::info!("Node {} 等待集群启动 ({:?})...", node_id, config.wait_time);
    sleep(config.wait_time).await;

    // 获取所有其他节点
    let other_nodes = {
        let nodes = all_nodes.read().await;
        nodes.iter()
            .filter(|&nid| *nid != local_node_id)
            .cloned()
            .collect::<Vec<_>>()
    };

    log::info!("Node {} 发现 {} 个其他节点", node_id, other_nodes.len());

    // 所有节点都参与消息处理，模拟更真实的场景
    let msgs_per_node = config.message_count / total_nodes as u64;
    log::info!("Node {} 开始模拟 {} 消息处理", node_id, msgs_per_node);

    let start_time = Instant::now();

    // 模拟处理入站消息
    benchmark_actor.do_send(SimulateProcessing {
        count: msgs_per_node
    });

    // 模拟处理出站消息并向其他节点发送响应
    // 此处简化为随机延迟，模拟网络延迟
    let msg_delays = vec![
        Duration::from_micros(50),  // 非常快的消息
        Duration::from_micros(100), // 快速消息
        Duration::from_micros(500), // 正常消息
        Duration::from_millis(1),   // 稍慢消息
        Duration::from_millis(5),   // 慢消息
        Duration::from_millis(20),  // 很慢的消息（网络拥塞）
    ];

    // 模拟节点间通信
    if node_id == 1 && !other_nodes.is_empty() {
        // 节点1作为协调者，额外生成节点间消息
        let extra_msgs = config.message_count / 5; // 额外20%的跨节点消息
        log::info!("Node {} 额外模拟 {} 条跨节点消息", node_id, extra_msgs);

        for i in 0..extra_msgs {
            // 随机选择延迟模式
            let delay_idx = (i % msg_delays.len() as u64) as usize;
            let delay = msg_delays[delay_idx];

            // 创建具有不同延迟的响应
            let response = PingResponse {
                id: i,
                original_timestamp: Instant::now() - delay, // 模拟发送时间
                responder: other_nodes[0].clone(), // 使用第一个节点作为响应方
                round_trip: delay,
            };

            // 发送模拟响应
            benchmark_actor.do_send(response);
        }
    }

    let elapsed = start_time.elapsed();
    log::info!("Node {} 完成消息处理, 耗时: {:?}", node_id, elapsed);

    // 等待消息处理完成
    sleep(Duration::from_secs(3)).await;

    // 完成测试
    benchmark_actor.do_send(FinishBenchmark);

    // 留出时间让结果发送出去
    sleep(Duration::from_secs(1)).await;

    log::info!("节点 {} 测试完成", node_id);
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // 测试参数
    let node_count = 3;  // 减少节点数量使测试更快
    let msgs_per_node = 100;  // 减少消息数量
    let message_size = 1024;  // 消息大小(字节)
    let batch_size = 10;   // 批处理大小
    let base_port = 8560;   // 起始端口号

    let config = BenchmarkConfig {
        message_count: msgs_per_node,
        message_size,
        batch_size,
        wait_time: Duration::from_secs(2), // 等待集群启动的时间
        architecture: Architecture::Decentralized, // 去中心化架构
        serialization: SerializationFormat::Bincode, // 二进制序列化格式
    };

    // 创建结果通道
    let (result_sender, mut result_receiver) = mpsc::channel::<BenchmarkResults>(node_count);

    // 创建共享的节点列表
    let all_nodes = Arc::new(RwLock::new(Vec::new()));

    log::info!("开始测试: {} 节点, 每节点 {} 消息, 消息大小 {} 字节, 批大小 {}",
             node_count, msgs_per_node, message_size, batch_size);

    // 使用 LocalSet 来运行所有测试
    let local_set = tokio::task::LocalSet::new();

    // 运行单个节点测试
    local_set.spawn_local(async move {
        // 创建并顺序启动所有节点
        for i in 1..=node_count {
            let node_id = i;
            let port = base_port + i as u16;
            let sender = result_sender.clone();
            let config = config.clone();
            let all_nodes_clone = all_nodes.clone();

            // 延迟启动，避免端口冲突
            sleep(Duration::from_millis(50)).await;

            // 直接运行节点任务
            run_node_benchmark(node_id, port, node_count, config, sender, all_nodes_clone).await;
        }

        // 收集所有结果
        drop(result_sender); // 关闭发送端，让通道知道不会再有消息

        let mut all_results = Vec::new();
        while let Some(result) = result_receiver.recv().await {
            all_results.push(result);
        }

        // 分析结果
        analyze_results(all_results, node_count, msgs_per_node);
    });

    // 等待所有任务完成
    local_set.await;

    Ok(())
}

// 分析和输出结果
fn analyze_results(results: Vec<BenchmarkResults>, node_count: usize, msgs_per_node: u64) {
    if results.is_empty() {
        log::error!("未收到任何测试结果!");
        return;
    }

    log::info!("收到 {} 个节点的测试结果 (总共 {} 个节点)", results.len(), node_count);

    // 计算整体性能指标
    let total_messages: u64 = results.iter().map(|r| r.total_messages).sum();
    let successful_messages: u64 = results.iter().map(|r| r.successful_messages).sum();
    let failed_messages: u64 = results.iter().map(|r| r.failed_messages).sum();
    let node_failures: u32 = results.iter().map(|r| r.node_failures).sum();

    // 收集所有延迟数据
    let mut all_latencies: Vec<Duration> = Vec::new();
    for result in &results {
        if result.successful_messages > 0 {
            all_latencies.push(result.avg_latency);
        }
    }

    let avg_latency = if !all_latencies.is_empty() {
        all_latencies.iter().sum::<Duration>() / all_latencies.len() as u32
    } else {
        Duration::from_secs(0)
    };

    // 收集所有节点的各种延迟指标
    let mut min_latencies = Vec::new();
    let mut max_latencies = Vec::new();
    let mut p95_latencies = Vec::new();
    let mut p99_latencies = Vec::new();

    for result in &results {
        if result.successful_messages > 0 {
            min_latencies.push(result.min_latency);
            max_latencies.push(result.max_latency);
            p95_latencies.push(result.p95_latency);
            p99_latencies.push(result.p99_latency);
        }
    }

    // 计算整体延迟指标
    let overall_min_latency = min_latencies.iter().min().copied().unwrap_or(Duration::from_secs(0));
    let overall_max_latency = max_latencies.iter().max().copied().unwrap_or(Duration::from_secs(0));
    let overall_p95_latency = if !p95_latencies.is_empty() {
        p95_latencies.iter().sum::<Duration>() / p95_latencies.len() as u32
    } else {
        Duration::from_secs(0)
    };
    let overall_p99_latency = if !p99_latencies.is_empty() {
        p99_latencies.iter().sum::<Duration>() / p99_latencies.len() as u32
    } else {
        Duration::from_secs(0)
    };

    // 找到最大吞吐量和最低吞吐量
    let max_throughput = results.iter().map(|r| r.throughput).fold(0.0, f64::max);
    let min_throughput = results.iter()
        .filter(|r| r.throughput > 0.0)
        .map(|r| r.throughput)
        .fold(f64::MAX, f64::min);

    // 计算总吞吐量
    let total_throughput: f64 = results.iter().map(|r| r.throughput).sum();

    // 输出每个节点的详细结果
    log::info!("\n==== 节点详细测试结果 ====");
    for (i, result) in results.iter().enumerate() {
        log::info!("节点 {} ({})", i + 1, result.node_id);
        log::info!("  消息总数: {}, 成功: {} ({:.2}%), 失败: {} ({:.2}%)",
                 result.total_messages,
                 result.successful_messages,
                 if result.total_messages > 0 { (result.successful_messages as f64 / result.total_messages as f64) * 100.0 } else { 0.0 },
                 result.failed_messages,
                 if result.total_messages > 0 { (result.failed_messages as f64 / result.total_messages as f64) * 100.0 } else { 0.0 });
        log::info!("  延迟: 最小 = {:?}, 平均 = {:?}, 最大 = {:?}, P95 = {:?}, P99 = {:?}",
                 result.min_latency, result.avg_latency, result.max_latency,
                 result.p95_latency, result.p99_latency);
        log::info!("  吞吐量: {:.2} 消息/秒, 测试时间: {:?}\n",
                 result.throughput, result.total_time);
    }

    // 输出整体结果
    log::info!("\n==== 整体测试结果 ====");
    log::info!("节点数量: {}", node_count);
    log::info!("期望总消息: {}", node_count as u64 * msgs_per_node);
    log::info!("实际总消息: {}", total_messages);
    log::info!("成功消息: {} ({:.2}%)",
             successful_messages,
             if total_messages > 0 { (successful_messages as f64 / total_messages as f64) * 100.0 } else { 0.0 });
    log::info!("失败消息: {} ({:.2}%)",
             failed_messages,
             if total_messages > 0 { (failed_messages as f64 / total_messages as f64) * 100.0 } else { 0.0 });
    log::info!("节点故障: {}", node_failures);

    // 输出延迟指标
    log::info!("\n==== 延迟统计 ====");
    log::info!("最小延迟: {:?}", overall_min_latency);
    log::info!("平均延迟: {:?}", avg_latency);
    log::info!("最大延迟: {:?}", overall_max_latency);
    log::info!("P95延迟: {:?}", overall_p95_latency);
    log::info!("P99延迟: {:?}", overall_p99_latency);

    // 输出吞吐量指标
    log::info!("\n==== 吞吐量统计 ====");
    log::info!("最高节点吞吐量: {:.2} 消息/秒", max_throughput);
    log::info!("最低节点吞吐量: {:.2} 消息/秒", min_throughput);
    log::info!("总吞吐量: {:.2} 消息/秒", total_throughput);
    log::info!("平均节点吞吐量: {:.2} 消息/秒",
             if results.len() > 0 { total_throughput / results.len() as f64 } else { 0.0 });

    // 输出稳定性总结
    let success_rate = if total_messages > 0 {
        (successful_messages as f64 / total_messages as f64) * 100.0
    } else {
        0.0
    };

    log::info!("\n==== 稳定性评估 ====");
    log::info!("消息成功率: {:.2}%", success_rate);
    log::info!("节点可用率: {:.2}%",
             (results.len() as f64 / node_count as f64) * 100.0);

    if success_rate >= 99.0 && node_failures == 0 {
        log::info!("稳定性评级: 优秀");
    } else if success_rate >= 95.0 && node_failures <= 2 {
        log::info!("稳定性评级: 良好");
    } else if success_rate >= 90.0 && node_failures <= 5 {
        log::info!("稳定性评级: 一般");
    } else {
        log::info!("稳定性评级: 需要改进");
    }

    log::info!("\n==== 测试总结 ====");
    if success_rate >= 99.0 && max_throughput > 0.0 {
        log::info!("集群性能表现: 良好");
        log::info!("推荐最大负载: 每秒约 {} 消息", (max_throughput * 0.8) as u32);
    } else if success_rate >= 95.0 && max_throughput > 0.0 {
        log::info!("集群性能表现: 一般");
        log::info!("推荐最大负载: 每秒约 {} 消息", (max_throughput * 0.6) as u32);
    } else {
        log::info!("集群性能表现: 需要改进");
        log::info!("推荐进行更多测试以确定合适的负载");
    }
}

// 创建集群配置
fn create_config(
    name: String,
    port: u16,
    architecture: Architecture,
    serialization: SerializationFormat
) -> ClusterConfig {
    let mut config = ClusterConfig::new();
    config.node_role = NodeRole::Worker;
    config.architecture = architecture;
    config.serialization_format = serialization;
    config.bind_addr = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        port
    );

    // 设置发现方法
    config.discovery = DiscoveryMethod::Static {
        seed_nodes: (1..=50)
            .map(|i| {
                format!("127.0.0.1:{}", 8560 + i as u16)
            })
            .collect()
    };

    config
}