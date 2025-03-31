use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use actix::prelude::*;
use env_logger;
use log;
use tokio::time::sleep;

// 引入cluster相关模块
use actix_cluster::prelude::*;
use actix_cluster::{ClusterSystem, Architecture, ClusterConfig, DiscoveryMethod};
use actix_cluster::serialization::SerializationFormat;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use serde::{Serialize, Deserialize};

// 配置参数
const ACTOR_COUNT: usize = 100;    // 每个节点上的Actor数量
const CLUSTER_NODES: usize = 10;   // 集群节点数量
const MESSAGES_PER_ACTOR: u64 = 1000; // 每个Actor发送的消息数量
const MESSAGE_SIZE: usize = 1024;     // 消息大小(字节)
const BATCH_SIZE: usize = 100;        // 批处理大小

// 简单消息 - 支持序列化
#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
struct BenchMessage {
    id: u64,
    sender_id: usize,
    timestamp: u128, // 使用毫秒时间戳，因为Instant不能序列化
    payload: Vec<u8>,
}

// 结果请求消息
#[derive(Message)]
#[rtype(result = "()")]
struct GetResult;

// 测试结果结构
#[derive(Debug, Clone)]
struct ActorMetrics {
    actor_id: usize,
    node_id: usize,
    sent_count: u64,
    received_count: u64,
    min_latency: Duration,
    max_latency: Duration,
    avg_latency: Duration,
    p50_latency: Duration,
    p95_latency: Duration,
    p99_latency: Duration,
    throughput: f64,
    test_duration: Duration,
}

// 测试Actor
struct BenchActor {
    id: usize,
    node_id: usize,
    start_time: Instant,
    sent_count: u64,
    received_count: u64,
    total_latency: Duration,
    min_latency: Duration,
    max_latency: Duration,
    latency_samples: VecDeque<Duration>,
    result_collector: Option<Addr<ResultCollector>>,
}

impl Actor for BenchActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        log::info!("BenchActor {} started on node {}", self.id, self.node_id);
        self.start_time = Instant::now();
    }
}

impl BenchActor {
    fn new(id: usize, node_id: usize, result_collector: Option<Addr<ResultCollector>>) -> Self {
        Self {
            id,
            node_id,
            start_time: Instant::now(),
            sent_count: 0,
            received_count: 0,
            total_latency: Duration::from_nanos(0),
            min_latency: Duration::from_secs(u64::MAX),
            max_latency: Duration::from_nanos(0),
            latency_samples: VecDeque::with_capacity(100),
            result_collector,
        }
    }
    
    // 计算延迟统计
    fn calculate_metrics(&self) -> ActorMetrics {
        let test_duration = self.start_time.elapsed();
        
        // 计算平均延迟
        let avg_latency = if !self.latency_samples.is_empty() {
            self.total_latency / self.latency_samples.len() as u32
        } else {
            Duration::from_nanos(0)
        };
        
        // 计算百分位延迟
        let (p50, p95, p99) = if self.latency_samples.len() >= 10 {
            let mut sorted_latencies: Vec<Duration> = self.latency_samples.iter().copied().collect();
            sorted_latencies.sort();
            
            let p50_idx = (sorted_latencies.len() as f64 * 0.5) as usize;
            let p95_idx = (sorted_latencies.len() as f64 * 0.95) as usize;
            let p99_idx = (sorted_latencies.len() as f64 * 0.99) as usize;
            
            (
                sorted_latencies.get(p50_idx).copied().unwrap_or_default(),
                sorted_latencies.get(p95_idx).copied().unwrap_or_default(),
                sorted_latencies.get(p99_idx).copied().unwrap_or_default()
            )
        } else {
            (Duration::from_nanos(0), Duration::from_nanos(0), Duration::from_nanos(0))
        };
        
        // 计算吞吐量 (消息/秒)
        let throughput = if test_duration.as_secs_f64() > 0.0 {
            self.received_count as f64 / test_duration.as_secs_f64()
        } else {
            0.0
        };
        
        // 构建结果
        ActorMetrics {
            actor_id: self.id,
            node_id: self.node_id,
            sent_count: self.sent_count,
            received_count: self.received_count,
            min_latency: if self.min_latency == Duration::from_secs(u64::MAX) {
                Duration::from_nanos(0)
            } else {
                self.min_latency
            },
            max_latency: self.max_latency,
            avg_latency,
            p50_latency: p50,
            p95_latency: p95,
            p99_latency: p99,
            throughput,
            test_duration,
        }
    }
}

// 接收基准测试消息
impl Handler<BenchMessage> for BenchActor {
    type Result = ();
    
    fn handle(&mut self, msg: BenchMessage, ctx: &mut Self::Context) -> Self::Result {
        self.received_count += 1;
        
        // 获取当前时间
        let now = Instant::now();
        
        // 计算消息延迟 (从发送方的时间戳)
        let timestamp_ms = msg.timestamp;
        let current_ms = now.elapsed().as_millis();
        
        // 计算往返延迟 (仅对有效时间戳)
        if timestamp_ms > 0 && current_ms >= timestamp_ms {
            let latency = Duration::from_millis((current_ms - timestamp_ms) as u64);
            
            // 仅记录合理的延迟值 (< 60秒)
            if latency < Duration::from_secs(60) {
                self.total_latency += latency;
                
                // 更新最小/最大延迟
                if latency < self.min_latency {
                    self.min_latency = latency;
                }
                
                if latency > self.max_latency {
                    self.max_latency = latency;
                }
                
                // 存储延迟样本 (最多100个)
                if self.latency_samples.len() >= 100 {
                    self.latency_samples.pop_front();
                }
                self.latency_samples.push_back(latency);
            }
        }
        
        // 每收到1000条消息记录一次
        if self.received_count % 1000 == 0 {
            log::info!("Actor {}/{} received {} messages", 
                     self.node_id, self.id, self.received_count);
        }
        
        // 如果发送者提供了ID，可以发送回复确认消息
        if msg.sender_id != self.id {
            // 可以在这里实现回复消息
            // 但默认情况下我们不需要，以减少网络开销
        }
    }
}

// 处理获取结果请求
impl Handler<GetResult> for BenchActor {
    type Result = ();
    
    fn handle(&mut self, _: GetResult, _: &mut Self::Context) -> Self::Result {
        let metrics = self.calculate_metrics();
        
        // 输出简短结果摘要
        log::info!("Actor {}/{}: sent={}, received={}, avg_latency={:?}, throughput={:.2} msg/sec",
                 self.node_id, self.id, metrics.sent_count, metrics.received_count,
                 metrics.avg_latency, metrics.throughput);
        
        // 如果有结果收集器，发送详细结果
        if let Some(collector) = &self.result_collector {
            collector.do_send(CollectResult(metrics));
        }
    }
}

// 发送消息任务
#[derive(Message)]
#[rtype(result = "()")]
struct SendMessages {
    targets: Vec<Addr<BenchActor>>,
    target_paths: Vec<String>,
    message_count: u64,
    message_size: usize,
}

// 处理发送消息请求
impl Handler<SendMessages> for BenchActor {
    type Result = ResponseFuture<()>;
    
    fn handle(&mut self, msg: SendMessages, ctx: &mut Self::Context) -> Self::Result {
        let actor_id = self.id;
        let node_id = self.node_id;
        let self_addr = ctx.address();
        
        // 创建异步任务
        Box::pin(async move {
            log::info!("Actor {}/{} starting to send {} messages to {} targets",
                     node_id, actor_id, msg.message_count, msg.targets.len());
            
            let start = Instant::now();
            let mut sent_count = 0;
            
            // 计算每个目标要发送的消息数
            let targets_len = msg.targets.len().max(1);
            let msgs_per_target = msg.message_count / targets_len as u64;
            
            // 给每个目标发送消息
            for target in msg.targets {
                for i in 0..msgs_per_target {
                    // 分批发送，避免系统过载
                    if i > 0 && i % BATCH_SIZE as u64 == 0 {
                        sleep(Duration::from_millis(10)).await;
                    }
                    
                    // 创建消息
                    let bench_msg = BenchMessage {
                        id: i,
                        sender_id: actor_id,
                        timestamp: Instant::now().elapsed().as_millis(),
                        payload: vec![0u8; msg.message_size],
                    };
                    
                    // 发送消息
                    target.do_send(bench_msg);
                    sent_count += 1;
                }
            }
            
            // 更新发送计数
            self_addr.do_send(UpdateSent(sent_count));
            
            let elapsed = start.elapsed();
            log::info!("Actor {}/{} finished sending {} messages in {:?}",
                     node_id, actor_id, sent_count, elapsed);
        })
    }
}

// 更新发送计数的消息
#[derive(Message)]
#[rtype(result = "()")]
struct UpdateSent(u64);

impl Handler<UpdateSent> for BenchActor {
    type Result = ();
    
    fn handle(&mut self, msg: UpdateSent, _: &mut Self::Context) -> Self::Result {
        self.sent_count = msg.0;
    }
}

// 结果收集器
#[derive(Message)]
#[rtype(result = "()")]
struct CollectResult(ActorMetrics);

struct ResultCollector {
    expected_actors: usize,
    results: HashMap<usize, ActorMetrics>,
    start_time: Instant,
}

impl Actor for ResultCollector {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        log::info!("ResultCollector started, expecting {} actors", self.expected_actors);
        self.start_time = Instant::now();
    }
}

impl ResultCollector {
    fn new(expected_actors: usize) -> Self {
        Self {
            expected_actors,
            results: HashMap::new(),
            start_time: Instant::now(),
        }
    }
    
    // 分析和输出结果
    fn analyze_results(&self) {
        if self.results.is_empty() {
            log::error!("No results collected!");
            return;
        }
        
        let total_actors = self.results.len();
        let total_sent: u64 = self.results.values().map(|r| r.sent_count).sum();
        let total_received: u64 = self.results.values().map(|r| r.received_count).sum();
        
        // 计算消息成功率
        let success_rate = if total_sent > 0 {
            (total_received as f64 / total_sent as f64) * 100.0
        } else {
            0.0
        };
        
        // 聚合延迟统计
        let mut all_latencies = Vec::new();
        let mut min_latency = Duration::from_secs(u64::MAX);
        let mut max_latency = Duration::from_nanos(0);
        
        for metrics in self.results.values() {
            if metrics.min_latency.as_nanos() > 0 && metrics.min_latency < min_latency {
                min_latency = metrics.min_latency;
            }
            
            if metrics.max_latency > max_latency {
                max_latency = metrics.max_latency;
            }
            
            if metrics.avg_latency.as_nanos() > 0 {
                all_latencies.push(metrics.avg_latency);
            }
        }
        
        // 确保最小延迟有效
        if min_latency == Duration::from_secs(u64::MAX) {
            min_latency = Duration::from_nanos(0);
        }
        
        // 计算平均延迟
        let avg_latency = if !all_latencies.is_empty() {
            all_latencies.iter().sum::<Duration>() / all_latencies.len() as u32
        } else {
            Duration::from_nanos(0)
        };
        
        // 计算总吞吐量
        let total_throughput: f64 = self.results.values().map(|r| r.throughput).sum();
        
        // 按节点分组并计算每个节点的统计信息
        let mut node_stats: HashMap<usize, Vec<&ActorMetrics>> = HashMap::new();
        
        for metrics in self.results.values() {
            node_stats.entry(metrics.node_id)
                .or_insert_with(Vec::new)
                .push(metrics);
        }
        
        // 输出结果
        log::info!("\n==== BENCHMARK RESULTS ====");
        log::info!("Total duration: {:?}", self.start_time.elapsed());
        log::info!("Total actors: {}/{}", total_actors, self.expected_actors);
        log::info!("Messages sent: {}", total_sent);
        log::info!("Messages received: {}", total_received);
        log::info!("Success rate: {:.2}%", success_rate);
        log::info!("Min latency: {:?}", min_latency);
        log::info!("Avg latency: {:?}", avg_latency);
        log::info!("Max latency: {:?}", max_latency);
        log::info!("Total throughput: {:.2} msg/sec", total_throughput);
        
        // 输出每个节点的统计信息
        log::info!("\n==== NODE STATISTICS ====");
        let mut node_ids: Vec<usize> = node_stats.keys().cloned().collect();
        node_ids.sort();
        
        for &node_id in &node_ids {
            if let Some(metrics) = node_stats.get(&node_id) {
                let node_received: u64 = metrics.iter().map(|m| m.received_count).sum();
                let node_sent: u64 = metrics.iter().map(|m| m.sent_count).sum();
                let node_throughput: f64 = metrics.iter().map(|m| m.throughput).sum();
                
                log::info!("Node {}: actors={}, sent={}, received={}, throughput={:.2} msg/sec",
                         node_id, metrics.len(), node_sent, node_received, node_throughput);
            }
        }
    }
}

// 处理结果收集
impl Handler<CollectResult> for ResultCollector {
    type Result = ();
    
    fn handle(&mut self, msg: CollectResult, ctx: &mut Self::Context) -> Self::Result {
        let actor_id = msg.0.actor_id;
        self.results.insert(actor_id, msg.0);
        
        // 记录进度
        log::info!("Collected result from actor {}, total: {}/{}",
                 actor_id, self.results.len(), self.expected_actors);
        
        // 如果所有结果都已收集，分析并退出
        if self.results.len() >= self.expected_actors {
            log::info!("All results collected, analyzing...");
            self.analyze_results();
            
            // 延迟退出系统
            ctx.run_later(Duration::from_secs(1), |_, _| {
                System::current().stop();
            });
        }
    }
}

// 集群测试协调器
struct BenchCoordinator {
    actors: HashMap<usize, Addr<BenchActor>>,
    collector: Addr<ResultCollector>,
    expected_actors: usize,
}

impl Actor for BenchCoordinator {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        log::info!("BenchCoordinator started");
    }
}

impl BenchCoordinator {
    fn new(expected_actors: usize, collector: Addr<ResultCollector>) -> Self {
        Self {
            actors: HashMap::new(),
            collector,
            expected_actors,
        }
    }
}

// 注册Actor消息
#[derive(Message)]
#[rtype(result = "()")]
struct RegisterActor {
    id: usize,
    addr: Addr<BenchActor>,
}

// 处理注册Actor请求
impl Handler<RegisterActor> for BenchCoordinator {
    type Result = ();
    
    fn handle(&mut self, msg: RegisterActor, _: &mut Self::Context) -> Self::Result {
        self.actors.insert(msg.id, msg.addr);
        
        // 当所有Actor都注册完成时开始测试
        if self.actors.len() >= self.expected_actors {
            log::info!("All {} actors registered, starting benchmark...", self.actors.len());
            self.start_benchmark();
        }
    }
}

// 开始测试消息
#[derive(Message)]
#[rtype(result = "()")]
struct StartBenchmark;

// 处理开始测试请求
impl Handler<StartBenchmark> for BenchCoordinator {
    type Result = ();
    
    fn handle(&mut self, _: StartBenchmark, ctx: &mut Self::Context) -> Self::Result {
        self.start_benchmark();
        
        // 设置定时器收集结果
        let actors = self.actors.clone(); // 克隆actors以避免借用问题
        
        ctx.run_later(Duration::from_secs(60), move |_, ctx| {
            log::info!("Collecting results...");
            
            // 分批收集结果，避免系统过载
            for (i, (_, actor)) in actors.iter().enumerate() {
                // 每10个Actor一批收集
                let delay = (i / 10) as u64 * 100;
                let actor_addr = actor.clone();
                
                ctx.run_later(Duration::from_millis(delay), move |_, _| {
                    actor_addr.do_send(GetResult);
                });
            }
        });
    }
}

impl BenchCoordinator {
    fn start_benchmark(&self) {
        log::info!("Starting benchmark with {} actors", self.actors.len());
        
        // 获取所有Actor的地址
        let all_actors: Vec<_> = self.actors.values().cloned().collect();
        
        // 将Actor分成组，每组最多10个Actor
        for (i, chunk) in all_actors.chunks(10).enumerate() {
            let chunk_actors = chunk.to_vec();
            
            // 每组Actor分别处理
            for actor in &chunk_actors {
                // 创建目标列表 (不包括自己)
                let targets: Vec<_> = all_actors.iter()
                    .filter(|a| *a != actor)
                    .take(10) // 每个Actor最多发给10个目标
                    .cloned()
                    .collect();
                
                // 发送消息请求
                actor.do_send(SendMessages {
                    targets,
                    target_paths: Vec::new(), // 简化模型，不使用远程路径
                    message_count: MESSAGES_PER_ACTOR,
                    message_size: MESSAGE_SIZE,
                });
            }
            
            // 每处理一组后记录日志
            log::info!("Started sending messages for actor group {}", i+1);
        }
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // 计算集群中的总Actor数量
    let total_actors = ACTOR_COUNT * CLUSTER_NODES;
    
    log::info!("Starting benchmark with {} actors across {} nodes", 
             total_actors, CLUSTER_NODES);
    
    // 在本地运行
    let local = tokio::task::LocalSet::new();
    
    // 运行测试
    local.run_until(async {
        // 创建结果收集器
        let collector = ResultCollector::new(total_actors).start();
        
        // 创建测试协调器
        let coordinator = BenchCoordinator::new(total_actors, collector.clone()).start();
        
        // 模拟集群节点
        let mut actor_counter = 0;
        
        for node_id in 0..CLUSTER_NODES {
            log::info!("Creating actors for node {}", node_id);
            
            // 在每个节点上创建Actor
            for _ in 0..ACTOR_COUNT {
                let actor_id = actor_counter;
                actor_counter += 1;
                
                // 创建Actor
                let actor = BenchActor::new(actor_id, node_id, Some(collector.clone())).start();
                
                // 注册到协调器
                coordinator.do_send(RegisterActor {
                    id: actor_id,
                    addr: actor,
                });
                
                // 每创建10个Actor后暂停一下，避免资源耗尽
                if actor_id % 10 == 9 {
                    sleep(Duration::from_millis(10)).await;
                }
            }
            
            log::info!("Created {} actors for node {}", ACTOR_COUNT, node_id);
        }
        
        // 设置超时检测
        let start_time = Instant::now();
        let timeout = Duration::from_secs(300); // 5分钟超时
        
        // 主循环
        while start_time.elapsed() < timeout {
            sleep(Duration::from_secs(1)).await;
        }
        
        log::warn!("Test timeout reached after {:?}", timeout);
        System::current().stop();
    }).await;
    
    Ok(())
} 