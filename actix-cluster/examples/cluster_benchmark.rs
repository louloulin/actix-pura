use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use std::sync::Arc;
use actix_rt::System;
use actix::prelude::*;
use tokio::sync::{Mutex, mpsc, RwLock};
use tokio::time::{sleep, timeout};
use std::collections::HashMap;
use std::fmt;

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole, 
    SerializationFormat, NodeInfo, NodeId, ActorPath
};

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