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
use log;

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole, 
    SerializationFormat, NodeInfo, NodeId, ActorPath
};
use actix_cluster::message::DeliveryGuarantee;
use serde::{Serialize, Deserialize};

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
struct BenchmarkConfig {
    message_count: u64,        // 每个节点发送的消息总数
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
    cluster: Option<Addr<ClusterSystem>>,
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

impl BenchmarkActor {
    fn new(
        node_id: NodeId, 
        name: String, 
        cluster: Addr<ClusterSystem>,
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
            cluster: Some(cluster),
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
        if let Some(cluster) = &self.cluster {
            // 真实场景可从集群获取节点状态
        }
    }
    
    // 批量发送消息的方法
    async fn send_messages_batch(
        &mut self, 
        target_nodes: &[NodeId], 
        batch_size: usize, 
        message_size: usize, 
        start_id: u64, 
        count: u64
    ) -> u64 {
        if target_nodes.is_empty() || count == 0 {
            return 0;
        }
        
        let mut successful_sends = 0;
        let node_count = target_nodes.len();
        let self_addr = Addr::recipient(self);
        
        // 生成批量消息并发送
        for batch_id in (0..count).step_by(batch_size) {
            let batch_end = std::cmp::min(batch_id + batch_size as u64, count);
            let mut futures = Vec::with_capacity(batch_size);
            
            for msg_id in batch_id..batch_end {
                // 选择目标节点 (轮询方式)
                let target_idx = (msg_id % node_count as u64) as usize;
                let target_node = &target_nodes[target_idx];
                
                // 创建消息
                let msg = PingMessage {
                    id: start_id + msg_id,
                    timestamp: Instant::now(),
                    sender: self.node_id.clone(),
                    size: message_size,
                    payload: vec![0u8; message_size],
                };
                
                // 只有有效的集群才能发送
                if let Some(cluster) = &self.cluster {
                    let msg_clone = msg.clone();
                    let self_addr_clone = self_addr.clone();
                    let target_clone = target_node.clone();
                    let cluster_clone = cluster.clone();
                    
                    // 异步发送消息
                    let future = async move {
                        // 真实场景下应通过集群发送
                        let actor_path = ActorPath::new_unchecked(&format!("benchmark-{}", target_clone));
                        let result = timeout(
                            Duration::from_secs(5),
                            cluster_clone.send(msg_clone)
                        ).await;
                        
                        match result {
                            Ok(Ok(response)) => {
                                // 发送响应回自己
                                let _ = self_addr_clone.do_send(response);
                                true
                            },
                            _ => false
                        }
                    };
                    
                    futures.push(future);
                }
            }
            
            // 等待批次完成
            let results = futures::future::join_all(futures).await;
            let batch_success = results.iter().filter(|&&success| success).count() as u64;
            successful_sends += batch_success;
            self.sent_count += (batch_end - batch_id);
            
            // 日志记录
            if self.sent_count % 10000 == 0 {
                log::info!("{} ({}) sent {} messages", self.name, self.node_id, self.sent_count);
            }
            
            // 限制发送速率
            if batch_id % (batch_size as u64 * 10) == 0 {
                sleep(Duration::from_millis(1)).await;
            }
        }
        
        successful_sends
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
    let mut system = ClusterSystem::new(&node_name, cluster_config);
    let local_node_id = system.local_node().id.clone();
    log::info!("Node {} 创建, NodeId: {}", node_id, local_node_id);
    
    // 启动集群系统
    let system_addr = match system.start().await {
        Ok(addr) => {
            log::info!("Node {} 启动成功", node_id);
            addr
        },
        Err(e) => {
            log::error!("Node {} 启动失败: {}", node_id, e);
            return;
        }
    };
    
    // 创建benchmark actor
    let actor_name = format!("benchmark-{}", node_id);
    let benchmark_actor = BenchmarkActor::new(
        local_node_id.clone(),
        actor_name.clone(),
        system_addr.clone(),
        result_sender.clone()
    ).start();
    
    // 注册actor到集群系统，让其他节点可以寻址
    if let Err(e) = system.register_actor(actor_name.clone(), benchmark_actor.clone().recipient()).await {
        log::error!("Node {} 注册actor失败: {}", node_id, e);
        return;
    }
    
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
            .filter(|&&ref nid| nid != local_node_id)
            .cloned()
            .collect::<Vec<_>>()
    };
    
    log::info!("Node {} 发现 {} 个其他节点", node_id, other_nodes.len());
    
    if node_id == 1 {  // 只使用第一个节点作为发送方，简化测试流程
        // 开始发送消息
        log::info!("Node {} 开始发送 {} 消息, 每批 {} 条", 
                 node_id, config.message_count, config.batch_size);
        
        let start_time = Instant::now();
        
        // 批量发送消息
        let sent = benchmark_actor.send(FinishBenchmark).await;
        
        let elapsed = start_time.elapsed();
        log::info!("Node {} 完成消息发送, 耗时: {:?}", node_id, elapsed);
    }
    
    // 等待消息处理完成
    sleep(Duration::from_secs(10)).await;
    
    // 完成测试
    benchmark_actor.do_send(FinishBenchmark);
    
    // 留出时间让结果发送出去
    sleep(Duration::from_secs(2)).await;
    
    // 停止集群
    if let Err(e) = system_addr.send(actix_cluster::ShutdownCluster).await {
        log::error!("Node {} 停止集群失败: {}", node_id, e);
    } else {
        log::info!("Node {} 停止集群成功", node_id);
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // 测试参数
    let node_count = 50;  // 节点数量
    let msgs_per_node = 10000;  // 每个节点发送的消息数
    let message_size = 1024;  // 消息大小(字节)
    let batch_size = 1000;   // 批处理大小
    let base_port = 8560;   // 起始端口号
    
    let config = BenchmarkConfig {
        message_count: msgs_per_node,
        message_size,
        batch_size,
        wait_time: Duration::from_secs(5), // 等待集群启动的时间
        architecture: Architecture::Decentralized, // 去中心化架构
        serialization: SerializationFormat::Bincode, // 二进制序列化格式
    };
    
    // 创建结果通道
    let (result_sender, mut result_receiver) = mpsc::channel::<BenchmarkResults>(node_count);
    
    // 创建共享的节点列表和任务句柄集合
    let all_nodes = Arc::new(RwLock::new(Vec::new()));
    let tasks = Arc::new(Mutex::new(Vec::new()));
    
    log::info!("开始测试: {} 节点, 每节点 {} 消息, 消息大小 {} 字节, 批大小 {}", 
             node_count, msgs_per_node, message_size, batch_size);
    
    // 创建并启动所有节点
    for i in 1..=node_count {
        let node_id = i;
        let port = base_port + i as u16;
        let sender = result_sender.clone();
        let config = config.clone();
        let all_nodes_clone = all_nodes.clone();
        
        // 延迟启动，避免端口冲突
        sleep(Duration::from_millis(50)).await;
        
        // 启动节点任务
        let task = tokio::spawn(async move {
            run_node_benchmark(node_id, port, node_count, config, sender, all_nodes_clone).await;
        });
        
        tasks.lock().await.push(task);
    }
    
    // 等待所有节点任务完成
    for task in Arc::try_unwrap(tasks)
        .unwrap_or_else(|_| panic!("无法解除任务Arc"))
        .into_inner() {
        let _ = task.await;
    }
    
    // 收集所有结果
    drop(result_sender); // 关闭发送端，让通道知道不会再有消息
    
    let mut all_results = Vec::new();
    while let Some(result) = result_receiver.recv().await {
        all_results.push(result);
    }
    
    // 分析结果
    analyze_results(all_results, node_count, msgs_per_node);
    
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
    
    // 计算平均延迟
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
    
    // 找到最大吞吐量和最低吞吐量
    let max_throughput = results.iter().map(|r| r.throughput).fold(0.0, f64::max);
    let min_throughput = results.iter()
        .filter(|r| r.throughput > 0.0)
        .map(|r| r.throughput)
        .fold(f64::MAX, f64::min);
    
    // 计算总吞吐量
    let total_throughput: f64 = results.iter().map(|r| r.throughput).sum();
    
    // 输出每个节点的结果
    for result in &results {
        log::info!("{}", result);
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
    log::info!("平均延迟: {:?}", avg_latency);
    log::info!("最高节点吞吐量: {:.2} 消息/秒", max_throughput);
    log::info!("最低节点吞吐量: {:.2} 消息/秒", min_throughput);
    log::info!("总吞吐量: {:.2} 消息/秒", total_throughput);
    
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
}

// 创建集群配置
fn create_config(
    name: String, 
    port: u16, 
    architecture: Architecture,
    serialization: SerializationFormat
) -> ClusterConfig {
    ClusterConfig::new()
        .name(name)
        .role(NodeRole::Worker)
        .architecture(architecture)
        .serialization_format(serialization)
        .bind_addr(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port
        ))
        .discovery(DiscoveryMethod::Static { 
            seed_nodes: (1..=50)
                .map(|i| {
                    SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        8560 + i as u16
                    )
                })
                .collect()
        })
}

// 配置参数
const ACTOR_COUNT: usize = 10;    // 每个节点上的Actor数量
const CLUSTER_NODES: usize = 10;   // 集群节点数量
const MESSAGES_PER_ACTOR: u64 = 1000; // 每个Actor发送的消息数量
const MESSAGE_SIZE: usize = 1024;     // 消息大小(字节)
const BATCH_SIZE: usize = 100;        // 批处理大小

// 定义可序列化的消息
#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
struct ClusterMessage {
    id: u64,
    sender_id: usize,
    sender_node: usize,
    timestamp: u128,
    payload: Vec<u8>,
}

// 消息接收确认
#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
struct MessageAck {
    message_id: u64,
    sender_id: usize,
    receiver_id: usize,
    receiver_node: usize,
    original_timestamp: u128,
    receive_timestamp: u128,
}

// 结果请求
#[derive(Message)]
#[rtype(result = "()")]
struct GetResults;

// 测试启动消息
#[derive(Message)]
#[rtype(result = "()")]
struct StartTest;

// 测试Actor注册
#[derive(Message)]
#[rtype(result = "()")]
struct RegisterTestActor {
    actor_id: usize,
    node_id: usize,
    path: String,
}

// Actor指标数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActorMetrics {
    actor_id: usize,
    node_id: usize,
    sent_count: u64,
    received_count: u64,
    ack_count: u64,
    min_latency: u64,
    max_latency: u64,
    avg_latency: u64,
    p50_latency: u64,
    p95_latency: u64,
    p99_latency: u64,
    throughput: f64,
    test_duration: u64,
}

// 结果收集消息
#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
struct ReportMetrics(ActorMetrics);

// 测试完成消息
#[derive(Message)]
#[rtype(result = "()")]
struct TestCompleted;

// 集群测试Actor
struct ClusterTestActor {
    id: usize,
    node_id: usize,
    cluster: Arc<ClusterSystem>,
    start_time: Instant,
    sent_count: u64,
    received_count: u64,
    ack_count: u64,
    total_latency: u128,
    latencies: VecDeque<u64>,
    coordinator_path: Option<String>,
    target_actors: Vec<String>,
}

impl ClusterTestActor {
    fn new(id: usize, node_id: usize, cluster: Arc<ClusterSystem>) -> Self {
        Self {
            id,
            node_id,
            cluster,
            start_time: Instant::now(),
            sent_count: 0,
            received_count: 0,
            ack_count: 0,
            total_latency: 0,
            latencies: VecDeque::with_capacity(100),
            coordinator_path: None,
            target_actors: Vec::new(),
        }
    }
    
    // 计算性能指标
    fn calculate_metrics(&self) -> ActorMetrics {
        let test_duration = self.start_time.elapsed().as_millis();
        
        // 计算延迟统计
        let avg_latency = if !self.latencies.is_empty() {
            self.total_latency as u64 / self.latencies.len() as u64
    } else {
            0
        };
        
        // 计算百分位数
        let (p50, p95, p99) = if self.latencies.len() >= 10 {
            let mut latencies: Vec<u64> = self.latencies.iter().copied().collect();
            latencies.sort();
            
            let p50_idx = (latencies.len() as f64 * 0.5) as usize;
            let p95_idx = (latencies.len() as f64 * 0.95) as usize;
            let p99_idx = (latencies.len() as f64 * 0.99) as usize;
            
            (
                *latencies.get(p50_idx).unwrap_or(&0),
                *latencies.get(p95_idx).unwrap_or(&0),
                *latencies.get(p99_idx).unwrap_or(&0)
            )
        } else {
            (0, 0, 0)
        };
        
        // 计算最小/最大延迟
        let min_latency = self.latencies.iter().min().copied().unwrap_or(0);
        let max_latency = self.latencies.iter().max().copied().unwrap_or(0);
        
        // 计算吞吐量 (消息/秒)
        let throughput = if test_duration > 0 {
            self.received_count as f64 / (test_duration as f64 / 1000.0)
    } else {
            0.0
        };
        
        // 构建指标
        ActorMetrics {
            actor_id: self.id,
            node_id: self.node_id,
            sent_count: self.sent_count,
            received_count: self.received_count,
            ack_count: self.ack_count,
            min_latency,
            max_latency,
            avg_latency,
            p50_latency: p50,
            p95_latency: p95,
            p99_latency: p99,
            throughput,
            test_duration: test_duration as u64,
        }
    }
    
    // 发送测试结果到协调器
    fn send_results_to_coordinator(&self, ctx: &mut <Self as Actor>::Context) {
        if let Some(coord_path) = &self.coordinator_path {
            let metrics = self.calculate_metrics();
            
            log::info!("Actor {}/{} sending results to coordinator: sent={}, received={}, acks={}",
                     self.node_id, self.id, metrics.sent_count, metrics.received_count, metrics.ack_count);
            
            // 构造ReportMetrics消息
            let report = ReportMetrics(metrics);
            
            // 通过集群发送到协调器
            if let Err(e) = self.cluster.send_remote(
                coord_path,
                report,
                DeliveryGuarantee::AtLeastOnce
            ) {
                log::error!("Failed to send results to coordinator: {:?}", e);
            }
    } else {
            log::error!("No coordinator path set, can't send results");
        }
    }
}

impl Actor for ClusterTestActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        log::info!("ClusterTestActor {}/{} started", self.node_id, self.id);
        self.start_time = Instant::now();
    }
}

// 处理集群消息
impl Handler<ClusterMessage> for ClusterTestActor {
    type Result = ();
    
    fn handle(&mut self, msg: ClusterMessage, _: &mut Self::Context) -> Self::Result {
        self.received_count += 1;
        
        // 收集延迟统计
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        
        if msg.timestamp > 0 && now >= msg.timestamp {
            let latency = now - msg.timestamp;
            
            // 只记录合理的延迟 (< 60秒)
            if latency < 60000 {
                self.total_latency += latency;
                
                // 存储延迟样本 (保留最近100个)
                if self.latencies.len() >= 100 {
                    self.latencies.pop_front();
                }
                self.latencies.push_back(latency as u64);
            }
        }
        
        // 记录日志，每100条消息记录一次
        if self.received_count % 100 == 0 {
            log::info!("Actor {}/{} received {} messages", 
                     self.node_id, self.id, self.received_count);
        }
        
        // 发送确认消息回发送者
        self.send_ack(msg);
    }
}

// 处理确认消息
impl Handler<MessageAck> for ClusterTestActor {
    type Result = ();
    
    fn handle(&mut self, msg: MessageAck, _: &mut Self::Context) -> Self::Result {
        self.ack_count += 1;
        
        // 计算往返延迟
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        
        if msg.original_timestamp > 0 && now >= msg.original_timestamp {
            let rtt = now - msg.original_timestamp;
            
            // 只记录合理的延迟 (< 2分钟)
            if rtt < 120000 {
                // 可以在这里单独记录往返延迟统计
            }
        }
        
        // 每100个确认记录一次
        if self.ack_count % 100 == 0 {
            log::debug!("Actor {}/{} received {} acks", 
                      self.node_id, self.id, self.ack_count);
        }
    }
}

impl ClusterTestActor {
    // 发送确认消息
    fn send_ack(&self, msg: ClusterMessage) {
        // 构造源Actor的路径
        let source_path = format!("test-actor-{}-{}", msg.sender_node, msg.sender_id);
        
        // 获取当前时间戳
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        
        // 创建确认消息
        let ack = MessageAck {
            message_id: msg.id,
            sender_id: msg.sender_id,
            receiver_id: self.id,
            receiver_node: self.node_id,
            original_timestamp: msg.timestamp,
            receive_timestamp: now,
        };
        
        // 通过集群发送确认
        if let Err(e) = self.cluster.send_remote(
            &source_path,
            ack,
            DeliveryGuarantee::AtMostOnce // 确认消息可以用较低的保证级别
        ) {
            log::error!("Failed to send ack to {}: {:?}", source_path, e);
        }
    }
}

// 处理发送测试消息请求
impl Handler<StartTest> for ClusterTestActor {
    type Result = ResponseFuture<()>;
    
    fn handle(&mut self, _: StartTest, ctx: &mut Self::Context) -> Self::Result {
        let actor_id = self.id;
        let node_id = self.node_id;
        let targets = self.target_actors.clone();
        let cluster = self.cluster.clone();
        let self_addr = ctx.address();
        
        Box::pin(async move {
            if targets.is_empty() {
                log::warn!("Actor {}/{} has no targets to send to", node_id, actor_id);
                return;
            }
            
            log::info!("Actor {}/{} starting test, sending to {} targets", 
                     node_id, actor_id, targets.len());
            
            let start = Instant::now();
            let mut sent_count = 0;
            
            // 计算每个目标要发送的消息数
            let msgs_per_target = MESSAGES_PER_ACTOR / targets.len() as u64;
            let msgs_per_batch = BATCH_SIZE.min(msgs_per_target as usize) as u64;
            
            // 对每个目标发送消息
            for target_path in &targets {
                let mut target_sent = 0;
                
                while target_sent < msgs_per_target {
                    // 每批次消息数量
                    let batch_size = msgs_per_batch.min(msgs_per_target - target_sent);
                    
                    for i in 0..batch_size {
                        // 生成当前消息ID
                        let msg_id = target_sent + i;
                        
                        // 获取当前时间戳
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();
                        
                        // 创建测试消息
                        let msg = ClusterMessage {
                            id: msg_id,
                            sender_id: actor_id,
                            sender_node: node_id,
                            timestamp,
                            payload: vec![0u8; MESSAGE_SIZE],
                        };
                        
                        // 发送到远程Actor
                        if let Err(e) = cluster.send_remote(
                            target_path,
                            msg,
                            DeliveryGuarantee::AtLeastOnce
                        ) {
                            log::error!("Failed to send message to {}: {:?}", target_path, e);
                        } else {
                            sent_count += 1;
                        }
                    }
                    
                    target_sent += batch_size;
                    
                    // 每批次之间暂停，避免系统过载
                    if target_sent < msgs_per_target {
                        sleep(Duration::from_millis(50)).await;
                    }
                }
                
                // 每个目标之间暂停，避免系统过载
                sleep(Duration::from_millis(100)).await;
            }
            
            // 更新发送计数
            self_addr.do_send(UpdateSendCount(sent_count));
            
            let elapsed = start.elapsed();
            log::info!("Actor {}/{} finished sending {} messages in {:?}", 
                     node_id, actor_id, sent_count, elapsed);
        })
    }
}

// 更新发送计数消息
#[derive(Message)]
#[rtype(result = "()")]
struct UpdateSendCount(u64);

impl Handler<UpdateSendCount> for ClusterTestActor {
    type Result = ();
    
    fn handle(&mut self, msg: UpdateSendCount, _: &mut Self::Context) -> Self::Result {
        self.sent_count = msg.0;
    }
}

// 更新目标Actor列表
#[derive(Message)]
#[rtype(result = "()")]
struct UpdateTargets {
    targets: Vec<String>,
    coordinator: String,
}

impl Handler<UpdateTargets> for ClusterTestActor {
    type Result = ();
    
    fn handle(&mut self, msg: UpdateTargets, _: &mut Self::Context) -> Self::Result {
        self.target_actors = msg.targets;
        self.coordinator_path = Some(msg.coordinator);
        
        log::info!("Actor {}/{} updated with {} targets and coordinator {}",
                 self.node_id, self.id, self.target_actors.len(), msg.coordinator);
    }
}

// 处理获取结果请求
impl Handler<GetResults> for ClusterTestActor {
    type Result = ();
    
    fn handle(&mut self, _: GetResults, ctx: &mut Self::Context) -> Self::Result {
        // 计算并发送结果到协调器
        self.send_results_to_coordinator(ctx);
    }
}

// 测试协调器
struct TestCoordinator {
    node_id: usize,
    cluster: Arc<ClusterSystem>,
    expected_actors: usize,
    registered_actors: HashMap<String, (usize, usize)>, // path -> (node_id, actor_id)
    results: HashMap<(usize, usize), ActorMetrics>, // (node_id, actor_id) -> metrics
    start_time: Instant,
    test_started: bool,
}

impl TestCoordinator {
    fn new(node_id: usize, cluster: Arc<ClusterSystem>, expected_actors: usize) -> Self {
        Self {
            node_id,
            cluster,
            expected_actors,
            registered_actors: HashMap::new(),
            results: HashMap::new(),
            start_time: Instant::now(),
            test_started: false,
        }
    }
    
    // 分发目标Actor给所有测试Actor
    fn distribute_targets(&self) {
        if self.registered_actors.len() < self.expected_actors {
            log::warn!("Not all actors registered yet: {}/{}", 
                     self.registered_actors.len(), self.expected_actors);
        }
        
        log::info!("Distributing targets to {} registered actors", self.registered_actors.len());
        
        let coordinator_path = format!("test-coordinator-{}", self.node_id);
        let all_paths: Vec<String> = self.registered_actors.keys().cloned().collect();
        
        // 为每个Actor分配目标
        for (actor_path, (node_id, actor_id)) in &self.registered_actors {
            // 创建目标列表 (排除自己)
            let targets: Vec<String> = all_paths.iter()
                .filter(|&path| path != actor_path)
                .take(10) // 每个Actor最多10个目标
                .cloned()
                .collect();
            
            // 更新目标消息
            let update = UpdateTargets {
                targets,
                coordinator: coordinator_path.clone(),
            };
            
            // 发送更新
            if let Err(e) = self.cluster.send_remote(
                actor_path,
                update,
                DeliveryGuarantee::AtLeastOnce
            ) {
                log::error!("Failed to update targets for {}/{}: {:?}", 
                          node_id, actor_id, e);
            }
        }
    }
    
    // 启动测试
    fn start_test(&mut self, ctx: &mut <Self as Actor>::Context) {
        if self.test_started {
            return;
        }
        
        self.test_started = true;
        self.start_time = Instant::now();
        
        log::info!("Starting test with {} actors", self.registered_actors.len());
        
        // 向所有Actor发送开始测试消息
        for (actor_path, (node_id, actor_id)) in &self.registered_actors {
            if let Err(e) = self.cluster.send_remote(
                actor_path,
                StartTest {},
                DeliveryGuarantee::AtLeastOnce
            ) {
                log::error!("Failed to start test for {}/{}: {:?}", 
                          node_id, actor_id, e);
            }
        }
        
        // 设置定时器收集结果
        ctx.run_later(Duration::from_secs(120), |_, ctx| {
            ctx.address().do_send(CollectAllResults);
        });
    }
    
    // 分析并输出结果
    fn analyze_results(&self) {
        if self.results.is_empty() {
            log::error!("No results received!");
            return;
        }
        
        let total_duration = self.start_time.elapsed();
        let total_actors = self.results.len();
        let total_sent: u64 = self.results.values().map(|m| m.sent_count).sum();
        let total_received: u64 = self.results.values().map(|m| m.received_count).sum();
        let total_acks: u64 = self.results.values().map(|m| m.ack_count).sum();
        
        // 计算消息成功率
        let delivery_rate = if total_sent > 0 {
            (total_received as f64 / total_sent as f64) * 100.0
        } else {
            0.0
        };
        
        let ack_rate = if total_sent > 0 {
            (total_acks as f64 / total_sent as f64) * 100.0
        } else {
            0.0
        };
        
        // 聚合延迟统计
        let mut all_latencies = Vec::new();
        let mut min_latency = u64::MAX;
        let mut max_latency = 0;
        
        for metrics in self.results.values() {
            if metrics.min_latency > 0 && metrics.min_latency < min_latency {
                min_latency = metrics.min_latency;
            }
            
            if metrics.max_latency > max_latency {
                max_latency = metrics.max_latency;
            }
            
            if metrics.avg_latency > 0 {
                all_latencies.push(metrics.avg_latency);
            }
        }
        
        // 确保最小延迟有效
        if min_latency == u64::MAX {
            min_latency = 0;
        }
        
        // 计算平均延迟
        let avg_latency = if !all_latencies.is_empty() {
            all_latencies.iter().sum::<u64>() / all_latencies.len() as u64
        } else {
            0
        };
        
        // 计算吞吐量
        let total_throughput: f64 = self.results.values().map(|m| m.throughput).sum();
        
        // 按节点分组计算统计
        let mut node_stats: HashMap<usize, Vec<&ActorMetrics>> = HashMap::new();
        
        for metrics in self.results.values() {
            node_stats.entry(metrics.node_id)
                .or_insert_with(Vec::new)
                .push(metrics);
        }
        
        // 输出整体结果
        log::info!("\n==== CLUSTER BENCHMARK RESULTS ====");
        log::info!("Test duration: {:?}", total_duration);
        log::info!("Total actors: {}/{}", total_actors, self.expected_actors);
        log::info!("Messages sent: {}", total_sent);
        log::info!("Messages received: {}", total_received);
        log::info!("Acknowledgements received: {}", total_acks);
        log::info!("Message delivery rate: {:.2}%", delivery_rate);
        log::info!("Acknowledgement rate: {:.2}%", ack_rate);
        log::info!("Min latency: {} ms", min_latency);
        log::info!("Avg latency: {} ms", avg_latency);
        log::info!("Max latency: {} ms", max_latency);
        log::info!("Total throughput: {:.2} msg/sec", total_throughput);
        log::info!("Avg throughput per actor: {:.2} msg/sec", 
                 if total_actors > 0 { total_throughput / total_actors as f64 } else { 0.0 });
        
        // 输出每个节点的结果
        log::info!("\n==== NODE STATISTICS ====");
        
        let mut node_ids: Vec<usize> = node_stats.keys().cloned().collect();
        node_ids.sort();
        
        for &node_id in &node_ids {
            if let Some(metrics) = node_stats.get(&node_id) {
                let node_sent: u64 = metrics.iter().map(|m| m.sent_count).sum();
                let node_received: u64 = metrics.iter().map(|m| m.received_count).sum();
                let node_acks: u64 = metrics.iter().map(|m| m.ack_count).sum();
                let node_throughput: f64 = metrics.iter().map(|m| m.throughput).sum();
                
                log::info!("Node {}: actors={}, sent={}, received={}, acks={}, throughput={:.2} msg/sec",
                         node_id, metrics.len(), node_sent, node_received, node_acks, node_throughput);
            }
        }
    }
}

impl Actor for TestCoordinator {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        log::info!("TestCoordinator started on node {}, expecting {} actors", 
                 self.node_id, self.expected_actors);
    }
}

// 处理Actor注册
impl Handler<RegisterTestActor> for TestCoordinator {
    type Result = ();
    
    fn handle(&mut self, msg: RegisterTestActor, ctx: &mut Self::Context) -> Self::Result {
        // 注册Actor
        self.registered_actors.insert(
            msg.path.clone(),
            (msg.node_id, msg.actor_id)
        );
        
        log::info!("Registered actor {}/{} (path: {}), total: {}/{}",
                 msg.node_id, msg.actor_id, msg.path,
                 self.registered_actors.len(), self.expected_actors);
        
        // 当所有Actor都注册完成后开始测试
        if self.registered_actors.len() >= self.expected_actors && !self.test_started {
            log::info!("All actors registered, distributing targets and starting test");
            self.distribute_targets();
            
            // 稍微延迟测试开始，确保所有Actor都收到目标更新
            ctx.run_later(Duration::from_secs(5), |coord, ctx| {
                coord.start_test(ctx);
            });
        }
    }
}

// 处理指标报告
impl Handler<ReportMetrics> for TestCoordinator {
    type Result = ();
    
    fn handle(&mut self, msg: ReportMetrics, _: &mut Self::Context) -> Self::Result {
        let actor_id = msg.0.actor_id;
        let node_id = msg.0.node_id;
        
        self.results.insert((node_id, actor_id), msg.0.clone());
        
        log::info!("Received metrics from {}/{}, total: {}/{}",
                 node_id, actor_id, self.results.len(), self.expected_actors);
        
        // 如果所有结果都已收集，分析结果
        if self.results.len() >= self.expected_actors {
            log::info!("All results collected, analyzing...");
            self.analyze_results();
        }
    }
}

// 收集所有结果消息
#[derive(Message)]
#[rtype(result = "()")]
struct CollectAllResults;

impl Handler<CollectAllResults> for TestCoordinator {
    type Result = ();
    
    fn handle(&mut self, _: CollectAllResults, ctx: &mut Self::Context) -> Self::Result {
        log::info!("Collecting results from all actors...");
        
        // 向所有Actor发送获取结果请求
        for (actor_path, (node_id, actor_id)) in &self.registered_actors {
            if let Err(e) = self.cluster.send_remote(
                actor_path,
                GetResults {},
                DeliveryGuarantee::AtLeastOnce
            ) {
                log::error!("Failed to request results from {}/{}: {:?}", 
                          node_id, actor_id, e);
            }
        }
        
        // 设置超时，确保测试最终会完成
        ctx.run_later(Duration::from_secs(30), |coord, ctx| {
            // 即使没有收到所有结果，也分析已有数据
            log::warn!("Results collection timeout, analyzing available data: {}/{}",
                     coord.results.len(), coord.expected_actors);
            coord.analyze_results();
            
            // 发送测试完成消息，通知系统可以关闭
            ctx.address().do_send(TestCompleted);
        });
    }
}

// 处理测试完成
impl Handler<TestCompleted> for TestCoordinator {
    type Result = ();
    
    fn handle(&mut self, _: TestCompleted, ctx: &mut Self::Context) -> Self::Result {
        log::info!("Test completed, stopping system");
        
        // 延迟停止系统，确保日志能够输出
        actix_rt::spawn(async {
            sleep(Duration::from_secs(2)).await;
            System::current().stop();
        });
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    log::info!("Starting cluster benchmark with {} actors on {} nodes",
             ACTOR_COUNT, CLUSTER_NODES);
    
    // 创建集群节点
    let mut cluster_systems = Vec::new();
    let mut coordinator_addr = None;
    
    // 在本地模拟多个集群节点
    for node_id in 0..CLUSTER_NODES {
        let port = 10000 + node_id as u16;
        
        // 创建集群配置
        let config = ClusterConfig::default()
            .architecture(Architecture::Decentralized)
            .node_role(actix_cluster::config::NodeRole::Peer)
            .bind_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port))
            .serialization_format(SerializationFormat::Json)
            .cluster_name(format!("benchmark-cluster"))
            .discovery_method(DiscoveryMethod::Manual(
                (0..CLUSTER_NODES).map(|i| {
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 10000 + i as u16)
                }).collect()
            ));
        
        // 创建集群系统
        let cluster = ClusterSystem::new(&format!("node-{}", node_id), config);
        
        // 启动集群
        match cluster.start().await {
            Ok(_) => {
                log::info!("Started cluster node {}", node_id);
                
                // 在第一个节点上创建协调器
                if node_id == 0 {
                    let expected_actors = ACTOR_COUNT * CLUSTER_NODES;
                    let coordinator = TestCoordinator::new(node_id, Arc::new(cluster.clone()), expected_actors).start();
                    
                    // 将协调器注册到集群
                    if let Err(e) = cluster.register_recipient(
                        &format!("test-coordinator-{}", node_id),
                        coordinator.recipient::<RegisterTestActor>()
                    ).await {
                        log::error!("Failed to register coordinator for RegisterTestActor: {:?}", e);
                    }
                    
                    if let Err(e) = cluster.register_recipient(
                        &format!("test-coordinator-{}", node_id),
                        coordinator.recipient::<ReportMetrics>()
                    ).await {
                        log::error!("Failed to register coordinator for ReportMetrics: {:?}", e);
                    }
                    
                    coordinator_addr = Some(coordinator);
                }
                
                cluster_systems.push(Arc::new(cluster));
            },
            Err(e) => {
                log::error!("Failed to start cluster node {}: {:?}", node_id, e);
            }
        }
        
        // 每创建一个节点暂停一下，避免资源冲突
        sleep(Duration::from_millis(500)).await;
    }
    
    log::info!("Started {} cluster nodes", cluster_systems.len());
    
    // 遍历所有节点创建测试Actor
    for (node_idx, cluster) in cluster_systems.iter().enumerate() {
        log::info!("Creating {} actors on node {}", ACTOR_COUNT, node_idx);
        
        for actor_idx in 0..ACTOR_COUNT {
            // 创建测试Actor
            let test_actor = ClusterTestActor::new(actor_idx, node_idx, cluster.clone()).start();
            
            // 生成Actor路径
            let actor_path = format!("test-actor-{}-{}", node_idx, actor_idx);
            
            // 注册Actor到集群
            if let Err(e) = cluster.register_recipient(
                &actor_path,
                test_actor.recipient::<ClusterMessage>()
            ).await {
                log::error!("Failed to register actor for ClusterMessage: {:?}", e);
            }
            
            if let Err(e) = cluster.register_recipient(
                &actor_path,
                test_actor.recipient::<MessageAck>()
            ).await {
                log::error!("Failed to register actor for MessageAck: {:?}", e);
            }
            
            if let Err(e) = cluster.register_recipient(
                &actor_path,
                test_actor.recipient::<UpdateTargets>()
            ).await {
                log::error!("Failed to register actor for UpdateTargets: {:?}", e);
            }
            
            if let Err(e) = cluster.register_recipient(
                &actor_path,
                test_actor.recipient::<StartTest>()
            ).await {
                log::error!("Failed to register actor for StartTest: {:?}", e);
            }
            
            if let Err(e) = cluster.register_recipient(
                &actor_path,
                test_actor.recipient::<GetResults>()
            ).await {
                log::error!("Failed to register actor for GetResults: {:?}", e);
            }
            
            // 向协调器注册Actor
            if let Some(coordinator_path) = coordinator_addr.as_ref().map(|_| format!("test-coordinator-0")) {
                let register_msg = RegisterTestActor {
                    actor_id: actor_idx,
                    node_id: node_idx,
                    path: actor_path.clone(),
                };
                
                if let Err(e) = cluster.send_remote(
                    &coordinator_path,
                    register_msg,
                    DeliveryGuarantee::AtLeastOnce
                ) {
                    log::error!("Failed to register actor with coordinator: {:?}", e);
                }
            }
            
            // 每创建10个Actor暂停一下
            if actor_idx % 10 == 9 {
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
    
    // 等待测试完成
    log::info!("All actors created and registered, waiting for test to complete");
    
    // 设置全局超时
    let timeout = Duration::from_secs(600); // 10分钟超时
    let start_time = Instant::now();
    
    while start_time.elapsed() < timeout {
        sleep(Duration::from_secs(1)).await;
    }
    
    log::warn!("Global timeout reached after {:?}, stopping", timeout);
    System::current().stop();
    
    Ok(())
} 