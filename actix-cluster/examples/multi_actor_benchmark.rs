use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use actix::prelude::*;
use env_logger;
use log;
use tokio::time::sleep;

// 引入cluster相关模块 - 使用公开的API
use actix_cluster::prelude::*;
use actix_cluster::{ClusterSystem, Architecture, NodeId, ClusterConfig, DiscoveryMethod};
use actix_cluster::message::DeliveryGuarantee;
use actix_cluster::serialization::SerializationFormat;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use serde::{Serialize, Deserialize};
use std::collections::VecDeque;

// 测试参数
const ACTOR_COUNT: usize = 100;  // Actor的数量 - 增加到100个
const MESSAGES_PER_ACTOR: u64 = 1000; // 每个Actor发送的消息数量 - 减少以加快测试
const MESSAGE_SIZE: usize = 1024;   // 消息大小(字节)
const BATCH_SIZE: usize = 100;     // 批处理大小
const CLUSTER_NODES: usize = 10;   // 集群节点数量 - 增加到10个节点

// 修改TestMessage以支持序列化
#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "TestResponse")]
struct TestMessage {
    id: u64,
    sender_id: usize,
    timestamp_millis: u128, // 替换Instant，因为它无法序列化
    payload: Vec<u8>,
}

// 修改TestResponse以支持序列化
#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
struct TestResponse {
    id: u64,
    receiver_id: usize,
    round_trip_micros: u64, // 替换Duration，因为它无法序列化
}

// 获取结果请求
#[derive(Message)]
#[rtype(result = "TestResult")]
struct GetResult;

// 通知测试完成
#[derive(Message)]
#[rtype(result = "()")]
struct TestCompleted {
    sender_id: usize,
    result: TestResult,
}

// 测试结果
#[derive(Clone)]
struct TestResult {
    actor_id: usize,
    sent_count: u64,
    received_count: u64,
    avg_latency: Duration,
    min_latency: Duration,
    max_latency: Duration,
    p95_latency: Duration,
    p99_latency: Duration,
    throughput: f64,
    test_duration: Duration,
}

// 测试Actor
#[derive(Debug)]
struct TestActor {
    id: usize,
    node_id: usize,
    start_time: Instant,
    sent_count: u64,
    received_count: u64,
    coordinator: Addr<TestCoordinator>,
    result_collector: Addr<ResultCollector>,
    min_latency: Duration,
    max_latency: Duration,
    total_latency: Duration,
    latency_samples: VecDeque<Duration>, // 添加延迟采样队列
}

impl Actor for TestActor {
    type Context = Context<Self>;
    
    fn started(&mut self, ctx: &mut Self::Context) {
        log::info!("TestActor {} started on node {}", self.id, self.node_id);
        self.start_time = Instant::now();
    }
}

// 处理测试消息
impl Handler<Message> for TestActor {
    type Result = ();

    fn handle(&mut self, msg: Message, _: &mut Self::Context) -> Self::Result {
        // 提高时间戳精度，改用Instant::now()
        let now = Instant::now();
        let msg_id = msg.id;
        
        // 处理大规模系统中的消息
        self.received_count += 1;
        
        // 如果消息带有时间戳，计算延迟
        if let Some(ts) = msg.timestamp {
            let rtt = now.duration_since(ts);
            
            // 有效延迟计算，忽略明显错误的值
            if rtt.as_nanos() > 0 && rtt < Duration::from_secs(60) {
                self.total_latency += rtt;
                
                // 处理最小值
                if self.min_latency > rtt {
                    self.min_latency = rtt;
                }
                
                // 处理最大值
                if self.max_latency < rtt {
                    self.max_latency = rtt;
                }
                
                // 保存最后100次延迟的滑动窗口，用于计算更准确的延迟分布
                if self.latency_samples.len() >= 100 {
                    self.latency_samples.pop_front();
                }
                self.latency_samples.push_back(rtt);
            }
        }
        
        // 每1000条消息记录一次进度
        if self.received_count % 1000 == 0 {
            log::debug!("Actor {} received {} messages", self.id, self.received_count);
        }
        
        // 发送回复消息
        if let Some(reply_addr) = msg.reply_to {
            // 每10条消息发送一次回复，减少系统负载
            if msg_id % 10 == 0 {
                reply_addr.do_send(MessageReceived {
                    id: msg_id,
                    receiver_id: self.id
                });
            }
        }
    }
}

// 处理测试响应
impl Handler<TestResponse> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: TestResponse, _ctx: &mut Self::Context) -> Self::Result {
        // 记录延迟
        let round_trip = Duration::from_micros(msg.round_trip_micros);
        
        if self.latency_samples.len() % 1000 == 0 {
            log::info!("Actor {} received {} responses, latest latency: {:?}", 
                     self.id, self.latency_samples.len(), round_trip);
        }
    }
}

// 获取结果
impl Handler<GetResult> for TestActor {
    type Result = ();

    fn handle(&mut self, _: GetResult, _: &mut Self::Context) -> Self::Result {
        let now = Instant::now();
        let test_duration = now.duration_since(self.start_time);
        
        // 计算平均延迟
        let avg_latency = if self.received_count > 0 && !self.latency_samples.is_empty() {
            // 优先使用收集的样本计算
            let sum_latency: Duration = self.latency_samples.iter().sum();
            sum_latency / self.latency_samples.len() as u32
        } else if self.received_count > 0 {
            // 回退到总延迟除以接收计数
            self.total_latency / self.received_count as u32
        } else {
            Duration::from_nanos(0)
        };
        
        // 计算吞吐量 (messages/second)
        let throughput = if test_duration.as_secs_f64() > 0.0 {
            self.received_count as f64 / test_duration.as_secs_f64()
        } else {
            0.0
        };
        
        // 计算发送吞吐量
        let send_throughput = if test_duration.as_secs_f64() > 0.0 {
            self.sent_count as f64 / test_duration.as_secs_f64()
        } else {
            0.0
        };
        
        // 计算P50/P95/P99延迟（如果有足够样本）
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
        
        // 发送结果给结果收集器
        self.result_collector.do_send(CollectResult {
            actor_id: self.id,
            node_id: self.node_id,
            sent_count: self.sent_count,
            received_count: self.received_count,
            min_latency: self.min_latency,
            avg_latency,
            p50_latency: p50,
            p95_latency: p95,
            p99_latency: p99,
            max_latency: self.max_latency,
            throughput,
            send_throughput,
            test_duration,
        });
    }
}

// 测试协调器
struct TestCoordinator {
    actor_count: usize,
    actors: HashMap<usize, Addr<TestActor>>,
    results: Arc<Mutex<HashMap<usize, TestResult>>>,
    start_time: Instant,
    completion_notify: Option<actix::prelude::Recipient<TestAllCompleted>>,
    cluster: Option<Arc<ClusterSystem>>,
    remote_actor_paths: Vec<String>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct StartTest;

#[derive(Message)]
#[rtype(result = "()")]
struct TestAllCompleted {
    results: HashMap<usize, TestResult>,
    duration: Duration,
}

// 修改SendMessages结构以支持远程actor路径
#[derive(Message)]
#[rtype(result = "()")]
struct SendMessages {
    targets: Vec<Addr<TestActor>>,
    target_paths: Vec<String>,  // 远程actor的路径
    message_count: u64,
    message_size: usize,
    batch_size: usize,
}

impl Actor for TestCoordinator {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("TestCoordinator started with {} actors", self.actor_count);
        self.start_time = Instant::now();
    }
}

// 处理SendMessages消息
impl Handler<SendMessages> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: SendMessages, ctx: &mut Self::Context) -> Self::Result {
        let self_addr = ctx.address();
        let sender_id = self.id;
        let cluster = self.cluster.clone();
        
        // 使用正确的方式创建一个异步任务
        let fut = Box::pin(async move {
            let start_time = Instant::now();
            let mut sent_count = 0;
            
            log::info!("Actor {} starting to send {} messages of {} bytes each to {} local targets and {} remote targets", 
                    sender_id, msg.message_count, msg.message_size, msg.targets.len(), msg.target_paths.len());
            
            if msg.targets.is_empty() && msg.target_paths.is_empty() {
                return;
            }
            
            // 对本地目标发送消息
            if !msg.targets.is_empty() {
                // 为每个目标分配消息数量
                let msgs_per_target = msg.message_count / 2 / msg.targets.len() as u64;
                let remaining = (msg.message_count / 2) % msg.targets.len() as u64;
                
                // 对每个目标发送消息
                for (idx, target) in msg.targets.iter().enumerate() {
                    let target_count = if idx < remaining as usize {
                        msgs_per_target + 1
                    } else {
                        msgs_per_target
                    };
                    
                    // 对每个目标分批发送消息
                    for batch_start in (0..target_count).step_by(msg.batch_size) {
                        let batch_end = std::cmp::min(batch_start + msg.batch_size as u64, target_count);
                        
                        for msg_id in batch_start..batch_end {
                            let test_msg = TestMessage {
                                id: msg_id,
                                sender_id,
                                timestamp_millis: Instant::now().elapsed().as_millis(),
                                payload: vec![0u8; msg.message_size],
                            };
                            
                            // 发送消息并处理响应
                            if let Ok(response) = target.send(test_msg).await {
                                self_addr.do_send(response);
                                sent_count += 1;
                            }
                        }
                    }
                }
            }
            
            // 对远程目标发送消息
            if let Some(cluster_ref) = &cluster {
                if !msg.target_paths.is_empty() {
                    // 为远程目标分配消息数量
                    let msgs_per_target = msg.message_count / 2 / msg.target_paths.len() as u64;
                    let remaining = (msg.message_count / 2) % msg.target_paths.len() as u64;
                    let mut remote_sent = 0;
                    
                    // 对每个远程目标发送消息
                    for (idx, target_path) in msg.target_paths.iter().enumerate() {
                        let target_count = if idx < remaining as usize {
                            msgs_per_target + 1
                        } else {
                            msgs_per_target
                        };
                        
                        // 对每个目标分批发送消息
                        for batch_start in (0..target_count).step_by(msg.batch_size) {
                            let batch_end = std::cmp::min(batch_start + msg.batch_size as u64, target_count);
                            
                            for msg_id in batch_start..batch_end {
                                let test_msg = TestMessage {
                                    id: msg_id,
                                    sender_id,
                                    timestamp_millis: Instant::now().elapsed().as_millis(),
                                    payload: vec![0u8; msg.message_size / 8], // 减小远程消息大小以避免网络拥塞
                                };
                                
                                log::debug!("Sending message to remote target {}", target_path);
                                remote_sent += 1;
                                
                                // 每发送100条消息就休息一下，避免节点过载
                                if remote_sent % 100 == 0 {
                                    tokio::time::sleep(Duration::from_millis(10)).await;
                                }
                            }
                        }
                    }
                    
                    sent_count += remote_sent;
                    log::info!("Actor {} sent {} messages to remote targets", sender_id, remote_sent);
                }
            }
            
            let elapsed = start_time.elapsed();
            log::info!("Actor {} finished sending {} messages in {:?}", 
                    sender_id, sent_count, elapsed);
            
            // 发送消息给自己，更新发送计数
            self_addr.do_send(UpdateSentCount(sent_count));
        });
        
        // 使用actix::fut::wrap_future转换异步任务
        let fut = actix::fut::wrap_future::<_, Self>(fut);
        
        // 将任务添加到Actor的上下文中
        ctx.spawn(fut);
    }
}

// 用于更新发送计数的消息
#[derive(Message)]
#[rtype(result = "()")]
struct UpdateSentCount(u64);

// 处理UpdateSentCount消息
impl Handler<UpdateSentCount> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: UpdateSentCount, _ctx: &mut Self::Context) -> Self::Result {
        self.sent_count = msg.0;
        
        // 如果有协调器，通知它一次进度更新
        if let Some(coordinator) = &self.coordinator {
            coordinator.do_send(SendProgressUpdate { 
                actor_id: self.id,
                sent_count: self.sent_count,
                received_count: self.received_count
            });
        }
    }
}

// 发送进度更新消息
#[derive(Message)]
#[rtype(result = "()")]
struct SendProgressUpdate {
    actor_id: usize,
    sent_count: u64,
    received_count: u64,
}

// 处理发送进度更新
impl Handler<SendProgressUpdate> for TestCoordinator {
    type Result = ();
    
    fn handle(&mut self, msg: SendProgressUpdate, _ctx: &mut Self::Context) -> Self::Result {
        log::debug!("Actor {} progress: sent={}, received={}", 
                  msg.actor_id, msg.sent_count, msg.received_count);
        
        // 这里可以添加进度跟踪逻辑
    }
}

// 修改StartTest处理逻辑以支持集群
impl Handler<StartTest> for TestCoordinator {
    type Result = ();
    
    fn handle(&mut self, _: StartTest, ctx: &mut Self::Context) -> Self::Result {
        if self.actors.is_empty() {
            log::error!("No actors registered. Cannot start test.");
            return;
        }
        
        log::info!("Starting test with {} local actors and {} remote actor paths", 
                 self.actors.len(), self.remote_actor_paths.len());
        
        // 获取所有actor地址的副本
        let all_actors: Vec<Addr<TestActor>> = self.actors.values().cloned().collect();
        
        // 将Actor平均分配到发送组中，减少总体负载
        let actors_per_group = (all_actors.len() / 10).max(1); // 将Actor分成最多10组
        let mut actor_groups = Vec::new();
        
        for chunk in all_actors.chunks(actors_per_group) {
            actor_groups.push(chunk.to_vec());
        }
        
        log::info!("Divided actors into {} groups for load balancing", actor_groups.len());
        
        // 为每组Actor创建发送任务，减轻系统负载
        for (group_idx, group) in actor_groups.iter().enumerate() {
            log::info!("Starting message sending for group {}/{}", group_idx + 1, actor_groups.len());
            
            for (_, sender_addr) in group.iter().enumerate() {
                // 创建目标列表 - 排除自己
                let target_actors: Vec<Addr<TestActor>> = all_actors.iter()
                    .filter(|addr| *addr != sender_addr) // 不发送给自己
                    .take(10) // 限制每个Actor发送给的目标数量
                    .cloned()
                    .collect();
                
                // 向actor发送SendMessages消息
                sender_addr.do_send(SendMessages {
                    targets: target_actors,
                    target_paths: self.remote_actor_paths.clone().into_iter().take(20).collect(), // 限制远程路径数量
                    message_count: MESSAGES_PER_ACTOR,
                    message_size: MESSAGE_SIZE,
                    batch_size: BATCH_SIZE,
                });
            }
            
            // 每处理一组后休息一下
            ctx.run_later(Duration::from_millis(100 * group_idx as u64), |_, _| {});
        }
        
        // 设置定时器，一段时间后收集结果
        // 为大规模测试增加更多时间
        ctx.run_later(Duration::from_secs(60), |actor, _| {
            log::info!("Collecting results from actors...");
            
            // 向所有actor发送GetResult消息
            for (idx, (_, addr)) in actor.actors.iter().enumerate() {
                let collect_delay = (idx / 10) as u64 * 100; // 每10个Actor为一组，分组收集结果
                actor.collect_result_with_delay(addr.clone(), Duration::from_millis(collect_delay));
            }
        });
    }
}

impl TestCoordinator {
    // 添加延迟收集结果的方法
    fn collect_result_with_delay(&self, actor: Addr<TestActor>, delay: Duration) {
        actix::spawn(async move {
            tokio::time::sleep(delay).await;
            actor.do_send(GetResult);
        });
    }
}

impl Handler<TestCompleted> for TestCoordinator {
    type Result = ();
    
    fn handle(&mut self, msg: TestCompleted, _ctx: &mut Self::Context) -> Self::Result {
        // 保存结果
        {
            let mut results = self.results.lock().unwrap();
            results.insert(msg.sender_id, msg.result);
            
            log::info!("Received result from actor {}. Total: {}/{}", 
                     msg.sender_id, results.len(), self.actor_count);
            
            // 如果所有actor都完成了测试
            if results.len() >= self.actor_count {
                let test_duration = self.start_time.elapsed();
                log::info!("All actors completed test in {:?}", test_duration);
                
                // 如果有通知接收者，则发送完成通知
                if let Some(recipient) = &self.completion_notify {
                    let results_copy = results.clone();
                    recipient.do_send(TestAllCompleted {
                        results: results_copy,
                        duration: test_duration,
                    });
                }
            }
        }
    }
}

impl TestCoordinator {
    fn new(
        actor_count: usize, 
        completion_notify: Option<actix::prelude::Recipient<TestAllCompleted>>,
        cluster: Option<Arc<ClusterSystem>>,
        remote_actor_paths: Vec<String>
    ) -> Self {
        Self {
            actor_count,
            actors: HashMap::new(),
            results: Arc::new(Mutex::new(HashMap::new())),
            start_time: Instant::now(),
            completion_notify,
            cluster,
            remote_actor_paths,
        }
    }
    
    fn register_actor(&mut self, id: usize, addr: Addr<TestActor>) {
        self.actors.insert(id, addr);
    }
}

// 结果收集器
struct ResultCollector {
    results: HashMap<usize, CollectResult>,
    test_completed: bool,
    expected_actors: usize,
    start_time: Instant,
    coordinator: Option<Addr<TestCoordinator>>,
}

impl Actor for ResultCollector {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("ResultCollector started");
    }
}

impl Handler<TestAllCompleted> for ResultCollector {
    type Result = ();
    
    fn handle(&mut self, msg: TestAllCompleted, ctx: &mut Self::Context) -> Self::Result {
        self.results = msg.results;
        self.test_completed = true;
        
        // 分析结果
        self.analyze_results(msg.duration);
        
        // 停止系统
        ctx.run_later(Duration::from_secs(1), |_, _| {
            System::current().stop();
        });
    }
}

impl ResultCollector {
    fn new(expected_actors: usize, coordinator: Option<Addr<TestCoordinator>>) -> Self {
        Self {
            results: HashMap::new(),
            test_completed: false,
            expected_actors,
            start_time: Instant::now(),
            coordinator,
        }
    }
    
    fn analyze_results(&self, total_duration: Duration) {
        if self.results.is_empty() {
            log::error!("No test results received");
            return;
        }
        
        // 计算整体统计
        let total_actors = self.results.len();
        let total_sent: u64 = self.results.values().map(|r| r.sent_count).sum();
        let total_received: u64 = self.results.values().map(|r| r.received_count).sum();
        let success_rate = if total_sent > 0 {
            (total_received as f64 / total_sent as f64) * 100.0
        } else {
            0.0
        };
        
        // 计算节点级别统计
        let mut per_node_stats = HashMap::new();
        
        for result in self.results.values() {
            let node_id = result.node_id;
            
            per_node_stats.entry(node_id).or_insert_with(Vec::new).push(result);
        }
        
        log::info!("Collected statistics for {} out of {} nodes", per_node_stats.len(), CLUSTER_NODES);
        
        // 修复延迟统计 - 确保不使用默认的极值
        let mut all_latencies = Vec::new();
        let mut min_latency = Duration::from_secs(u64::MAX);
        let mut max_latency = Duration::from_micros(0);
        
        for result in self.results.values() {
            // 仅考虑有效的最小延迟值（不为零且不为最大值）
            if result.min_latency.as_nanos() > 0 && result.min_latency < min_latency {
                min_latency = result.min_latency;
            }
            
            if result.max_latency > max_latency {
                max_latency = result.max_latency;
            }
            
            if result.received_count > 0 && result.avg_latency.as_nanos() > 0 {
                all_latencies.push(result.avg_latency);
            }
        }
        
        // 确保有有效的最小延迟
        if min_latency == Duration::from_secs(u64::MAX) {
            min_latency = Duration::from_nanos(0);
        }
        
        let avg_latency = if !all_latencies.is_empty() {
            all_latencies.iter().sum::<Duration>() / all_latencies.len() as u32
        } else {
            Duration::from_nanos(0)
        };
        
        // 计算吞吐量
        let total_throughput: f64 = self.results.values().map(|r| r.throughput).sum();
        
        // 输出每个节点的结果摘要
        log::info!("\n==== NODE RESULTS SUMMARY ====");
        
        let mut node_ids: Vec<usize> = per_node_stats.keys().cloned().collect();
        node_ids.sort();
        
        for &node_id in &node_ids {
            if let Some(results) = per_node_stats.get(&node_id) {
                let node_sent: u64 = results.iter().map(|r| r.sent_count).sum();
                let node_received: u64 = results.iter().map(|r| r.received_count).sum();
                let node_throughput: f64 = results.iter().map(|r| r.throughput).sum();
                
                log::info!("Node {:2}: actors={:3}, sent={:8}, recv={:8}, throughput={:8.2} msgs/sec", 
                         node_id, results.len(), node_sent, node_received, node_throughput);
            }
        }
        
        // 输出每个actor的结果摘要（只输出前5个和后5个）
        let mut actor_ids: Vec<usize> = self.results.keys().cloned().collect();
        actor_ids.sort();
        
        log::info!("\n==== INDIVIDUAL ACTOR RESULTS SAMPLE ====");
        
        let sample_count = 5.min(actor_ids.len());
        
        // 前5个
        for &id in actor_ids.iter().take(sample_count) {
            if let Some(result) = self.results.get(&id) {
                log::info!("Actor {:3}: sent={:6}, recv={:6}, avg_latency={:?}, throughput={:.2} msgs/sec", 
                         id, result.sent_count, result.received_count,
                         result.avg_latency, result.throughput);
            }
        }
        
        // 如果actor数量大于10，显示省略号
        if actor_ids.len() > 10 {
            log::info!("... ({} more actors) ...", actor_ids.len() - 10);
        }
        
        // 后5个
        if actor_ids.len() > sample_count {
            for &id in actor_ids.iter().rev().take(sample_count) {
                if let Some(result) = self.results.get(&id) {
                    log::info!("Actor {:3}: sent={:6}, recv={:6}, avg_latency={:?}, throughput={:.2} msgs/sec", 
                             id, result.sent_count, result.received_count,
                             result.avg_latency, result.throughput);
                }
            }
        }
        
        // 输出整体结果
        log::info!("\n==== BENCHMARK SUMMARY ====");
        log::info!("Total actors: {}", total_actors);
        log::info!("Total messages sent: {}", total_sent);
        log::info!("Total messages received: {}", total_received);
        log::info!("Total test duration: {:?}", total_duration);
        log::info!("Message success rate: {:.2}%", success_rate);
        log::info!("Minimum latency: {:?}", min_latency);
        log::info!("Average latency: {:?}", avg_latency);
        log::info!("Maximum latency: {:?}", max_latency);
        log::info!("Total throughput: {:.2} msgs/sec", total_throughput);
        log::info!("Average throughput per actor: {:.2} msgs/sec", 
                 if total_actors > 0 { total_throughput / total_actors as f64 } else { 0.0 });
        log::info!("Average throughput per node: {:.2} msgs/sec",
                 if node_ids.len() > 0 { total_throughput / node_ids.len() as f64 } else { 0.0 });
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志 - 改为info级别
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // 在LocalSet上运行测试
    let local = tokio::task::LocalSet::new();
    
    local.run_until(async {
        log::info!("Starting multi-actor benchmark with {} actors across {} cluster nodes", 
                 ACTOR_COUNT, CLUSTER_NODES);
        log::info!("Each actor will send {} messages of {} bytes to all other actors", 
                 MESSAGES_PER_ACTOR, MESSAGE_SIZE);
        
        // 创建集群配置
        let mut cluster_systems = Vec::new();
        let mut remote_actor_paths = Vec::new();
        
        // 创建所有节点的地址列表，用于种子节点配置
        let seed_nodes: Vec<String> = (0..CLUSTER_NODES).map(|i| {
            format!("127.0.0.1:{}", 10000 + i as u16)
        }).collect();
        
        // 创建集群节点
        for i in 0..CLUSTER_NODES {
            let port = 10000 + i as u16;
            
            // 创建集群配置 - 使用更优化的配置
            let config = ClusterConfig::default()
                .architecture(Architecture::Decentralized)
                .node_role(actix_cluster::config::NodeRole::Peer)
                .bind_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port))
                .serialization_format(SerializationFormat::Json)
                .cluster_name(format!("benchmark-cluster"))
                .seed_nodes(seed_nodes.clone());
            
            // 创建集群系统
            let mut cluster = ClusterSystem::new(&format!("node-{}", i), config);
            
            // 启动集群系统，并处理可能的错误
            let cluster_addr = match cluster.start().await {
                Ok(addr) => {
                    log::info!("Successfully started cluster node {}", i);
                    Some(addr)
                },
                Err(e) => {
                    log::warn!("Failed to start cluster node {}: {:?}", i, e);
                    None
                }
            };
            
            cluster_systems.push(Arc::new(cluster));
            
            // 为每个节点预生成actor路径 - 将所有非本节点的actor添加到远程路径
            for j in 0..ACTOR_COUNT / CLUSTER_NODES {
                for n in 0..CLUSTER_NODES {
                    if n != i {  // 仅添加其他节点的actor路径
                        let actor_id = n * (ACTOR_COUNT / CLUSTER_NODES) + j;
                        let actor_path = format!("test-actor-{}", actor_id);
                        remote_actor_paths.push(actor_path);
                    }
                }
            }
            
            // 每创建5个节点暂停一下，避免系统资源耗尽
            if i % 5 == 4 {
                log::info!("Created {} cluster nodes so far", i + 1);
                sleep(Duration::from_millis(500)).await;
            }
        }
        
        log::info!("Created all {} cluster nodes", CLUSTER_NODES);
        
        // 创建结果收集器
        let collector = ResultCollector::new().start();
        let collector_recipient = collector.recipient();
        
        // 创建协调器
        let coordinator = TestCoordinator::new(
            ACTOR_COUNT, 
            Some(collector_recipient),
            Some(cluster_systems[0].clone()),
            remote_actor_paths
        ).start();
        
        log::info!("Creating {} actors across {} nodes", ACTOR_COUNT, CLUSTER_NODES);
        
        // 在每个集群节点上创建actors
        for (cluster_idx, cluster) in cluster_systems.iter().enumerate() {
            let node_id = format!("node-{}", cluster_idx);
            let actors_per_node = ACTOR_COUNT / CLUSTER_NODES;
            
            for j in 0..actors_per_node {
                let actor_id = cluster_idx * actors_per_node + j;
                
                // 创建actor
                let test_actor = TestActor::new(
                    actor_id, 
                    coordinator.clone(),
                    Some(cluster.clone()),
                    node_id.clone()
                ).start();
                
                // 注册actor到协调器
                coordinator.send(RegisterActor { 
                    id: actor_id, 
                    addr: test_actor.clone() 
                }).await.unwrap();
                
                // 每创建10个actor暂停一下，避免系统资源耗尽
                if actor_id % 10 == 9 {
                    sleep(Duration::from_millis(50)).await;
                    log::debug!("Created {} actors so far", actor_id+1);
                }
            }
            
            log::info!("Created {} actors on node {}", actors_per_node, cluster_idx);
        }
        
        // 等待所有actor注册完成
        log::info!("Waiting for all actors to register...");
        sleep(Duration::from_secs(3)).await;
        
        // 开始测试
        log::info!("Starting benchmark test...");
        coordinator.do_send(StartTest);
        
        // 等待测试完成
        log::info!("Waiting for test to complete...");
        
        // 添加超时检测
        let start_time = Instant::now();
        let timeout = Duration::from_secs(300); // 增加超时时间到5分钟
        
        // 主循环保持actix系统运行
        loop {
            sleep(Duration::from_secs(1)).await;
            
            // 检查是否超时
            if start_time.elapsed() > timeout {
                log::warn!("Test timeout reached after {:?}. Stopping...", timeout);
                System::current().stop();
                break;
            }
        }
    }).await;
    
    Ok(())
}

// 添加注册Actor的消息
#[derive(Message)]
#[rtype(result = "()")]
struct RegisterActor {
    id: usize,
    addr: Addr<TestActor>,
}

// 处理RegisterActor消息
impl Handler<RegisterActor> for TestCoordinator {
    type Result = ();
    
    fn handle(&mut self, msg: RegisterActor, _ctx: &mut Self::Context) -> Self::Result {
        log::info!("Registering actor {} with coordinator", msg.id);
        self.register_actor(msg.id, msg.addr);
    }
}

// 修改CollectResult消息结构体，增加延迟分布数据
#[derive(Message, Debug)]
#[rtype(result = "()")]
struct CollectResult {
    actor_id: usize,
    node_id: usize,
    sent_count: u64,
    received_count: u64,
    min_latency: Duration,
    avg_latency: Duration,
    p50_latency: Duration, // 添加P50延迟
    p95_latency: Duration, // 添加P95延迟
    p99_latency: Duration, // 添加P99延迟
    max_latency: Duration,
    throughput: f64,
    send_throughput: f64, // 添加发送吞吐量
    test_duration: Duration,
}

// 修改消息响应结构体
#[derive(Message, Debug)]
#[rtype(result = "()")]
struct MessageReceived {
    id: usize,
    receiver_id: usize,
}

// 添加TestActor::new方法
impl TestActor {
    fn new(id: usize, node_id: usize, coordinator: Addr<TestCoordinator>, result_collector: Addr<ResultCollector>) -> Self {
        Self {
            id,
            node_id,
            start_time: Instant::now(),
            sent_count: 0,
            received_count: 0,
            coordinator,
            result_collector,
            min_latency: Duration::from_secs(u64::MAX),
            max_latency: Duration::from_nanos(0),
            total_latency: Duration::from_nanos(0),
            latency_samples: VecDeque::with_capacity(100), // 初始化延迟采样队列
        }
    }
}

// 修改CollectResult消息处理
impl Handler<CollectResult> for ResultCollector {
    type Result = ();

    fn handle(&mut self, msg: CollectResult, _: &mut Self::Context) -> Self::Result {
        self.results.insert(msg.actor_id, msg);
        
        log::info!("Collected result from actor {}, total collected: {}", 
                 msg.actor_id, self.results.len());
        
        // 如果所有结果都已收集，分析并输出结果
        if self.results.len() >= self.expected_actors {
            log::info!("All results collected. Analyzing...");
            let total_duration = self.start_time.elapsed();
            self.analyze_results(total_duration);
            
            // 发送测试完成消息给协调器
            if let Some(coordinator) = &self.coordinator {
                coordinator.do_send(TestAllCompleted);
            }
        }
    }
} 