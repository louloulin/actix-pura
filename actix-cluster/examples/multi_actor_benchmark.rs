use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use actix::prelude::*;
use env_logger;
use log;
use tokio::time::sleep;

// 测试参数
const ACTOR_COUNT: usize = 10;  // Actor的数量 - 减少到10个
const MESSAGES_PER_ACTOR: u64 = 1000; // 每个Actor发送的消息数量
const MESSAGE_SIZE: usize = 1024;   // 消息大小(字节)
const BATCH_SIZE: usize = 100;     // 批处理大小

// 测试消息
#[derive(Message, Clone)]
#[rtype(result = "TestResponse")]
struct TestMessage {
    id: u64,
    sender_id: usize,
    timestamp: Instant,
    payload: Vec<u8>,
}

// 测试响应
#[derive(Message, Clone)]
#[rtype(result = "()")]
struct TestResponse {
    id: u64,
    receiver_id: usize,
    round_trip: Duration,
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
struct TestActor {
    id: usize,
    sent_count: u64,
    received_count: u64,
    latencies: Vec<Duration>,
    start_time: Instant,
    coordinator: Option<Addr<TestCoordinator>>,
}

impl Actor for TestActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("TestActor {} started", self.id);
        self.start_time = Instant::now();
    }
}

// 处理测试消息
impl Handler<TestMessage> for TestActor {
    type Result = ResponseFuture<TestResponse>;
    
    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.received_count += 1;
        
        if self.received_count % 10000 == 0 {
            log::info!("Actor {} received {} messages", self.id, self.received_count);
        }
        
        // 计算延迟并返回响应
        let now = Instant::now();
        let round_trip = now.duration_since(msg.timestamp);
        let receiver_id = self.id;
        
        let response = TestResponse {
            id: msg.id,
            receiver_id,
            round_trip,
        };
        
        Box::pin(async move { response })
    }
}

// 处理测试响应
impl Handler<TestResponse> for TestActor {
    type Result = ();
    
    fn handle(&mut self, msg: TestResponse, _ctx: &mut Self::Context) -> Self::Result {
        // 记录延迟
        self.latencies.push(msg.round_trip);
        
        if self.latencies.len() % 10000 == 0 {
            log::info!("Actor {} received {} responses, latest latency: {:?}", 
                     self.id, self.latencies.len(), msg.round_trip);
        }
    }
}

// 获取结果
impl Handler<GetResult> for TestActor {
    type Result = MessageResult<GetResult>;
    
    fn handle(&mut self, _: GetResult, _ctx: &mut Self::Context) -> Self::Result {
        let result = self.calculate_results();
        
        // 发送结果到协调器
        if let Some(coordinator) = &self.coordinator {
            coordinator.do_send(TestCompleted {
                sender_id: self.id,
                result: result.clone(),
            });
        }
        
        MessageResult(result)
    }
}

impl TestActor {
    fn new(id: usize, coordinator: Addr<TestCoordinator>) -> Self {
        Self {
            id,
            sent_count: 0,
            received_count: 0,
            latencies: Vec::new(),
            start_time: Instant::now(),
            coordinator: Some(coordinator),
        }
    }
    
    // 计算测试结果
    fn calculate_results(&self) -> TestResult {
        let received_count = self.latencies.len() as u64;
        let test_duration = self.start_time.elapsed();
        
        if self.latencies.is_empty() {
            return TestResult {
                actor_id: self.id,
                sent_count: self.sent_count,
                received_count,
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
        let avg_latency = sum / received_count as u32;
        
        let p95_idx = ((received_count as f64 * 0.95) as usize).min(sorted.len() - 1);
        let p99_idx = ((received_count as f64 * 0.99) as usize).min(sorted.len() - 1);
        
        let p95_latency = sorted[p95_idx];
        let p99_latency = sorted[p99_idx];
        
        // 计算吞吐量 (消息/秒)
        let throughput = if test_duration.as_secs_f64() > 0.0 {
            received_count as f64 / test_duration.as_secs_f64()
        } else {
            0.0
        };
        
        TestResult {
            actor_id: self.id,
            sent_count: self.sent_count,
            received_count,
            avg_latency,
            min_latency,
            max_latency,
            p95_latency,
            p99_latency,
            throughput,
            test_duration,
        }
    }
}

// 测试协调器
struct TestCoordinator {
    actor_count: usize,
    actors: HashMap<usize, Addr<TestActor>>,
    results: Arc<Mutex<HashMap<usize, TestResult>>>,
    start_time: Instant,
    completion_notify: Option<actix::prelude::Recipient<TestAllCompleted>>,
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

// 添加新的消息定义
#[derive(Message)]
#[rtype(result = "()")]
struct SendMessages {
    targets: Vec<Addr<TestActor>>,
    message_count: u64,
    message_size: usize,
    batch_size: usize,
}

// 获取Actor自身引用
//[derive(Message)]
//[rtype(result = "Box<TestActor>")]
//struct GetSelf;

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
        
        // 使用正确的方式创建一个异步任务
        let fut = Box::pin(async move {
            let start_time = Instant::now();
            let mut sent_count = 0;
            
            log::info!("Actor {} starting to send {} messages of {} bytes each to {} targets", 
                     sender_id, msg.message_count, msg.message_size, msg.targets.len());
            
            if msg.targets.is_empty() {
                return;
            }
            
            // 为每个目标分配消息数量
            let msgs_per_target = msg.message_count / msg.targets.len() as u64;
            let remaining = msg.message_count % msg.targets.len() as u64;
            
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
                            timestamp: Instant::now(),
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
            
            let elapsed = start_time.elapsed();
            log::info!("Actor {} finished sending {} messages in {:?}", 
                     sender_id, sent_count, elapsed);
            
            // 发送消息给自己，更新发送计数
            self_addr.do_send(UpdateSentCount(sent_count));
        });
        
        // 使用AtomicResponse转换异步任务
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

// 修改StartTest处理逻辑以修复执行流程
impl Handler<StartTest> for TestCoordinator {
    type Result = ();
    
    fn handle(&mut self, _: StartTest, ctx: &mut Self::Context) -> Self::Result {
        if self.actors.is_empty() {
            log::error!("No actors registered. Cannot start test.");
            return;
        }
        
        log::info!("Starting test with {} actors", self.actors.len());
        
        // 获取所有actor地址的副本
        let all_actors: Vec<Addr<TestActor>> = self.actors.values().cloned().collect();
        
        // 为每个actor创建发送任务
        for (&sender_id, sender_addr) in &self.actors {
            // 创建目标列表 - 排除自己
            let target_actors: Vec<Addr<TestActor>> = all_actors.iter()
                .filter(|addr| *addr != sender_addr) // 不发送给自己
                .cloned()
                .collect();
            
            // 向actor发送SendMessages消息
            sender_addr.do_send(SendMessages {
                targets: target_actors,
                message_count: MESSAGES_PER_ACTOR,
                message_size: MESSAGE_SIZE,
                batch_size: BATCH_SIZE,
            });
        }
        
        // 设置定时器，一段时间后收集结果
        ctx.run_later(Duration::from_secs(10), |actor, _| {
            log::info!("Collecting results from actors...");
            
            // 向所有actor发送GetResult消息
            for (_, addr) in &actor.actors {
                addr.do_send(GetResult);
            }
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
    fn new(actor_count: usize, completion_notify: Option<actix::prelude::Recipient<TestAllCompleted>>) -> Self {
        Self {
            actor_count,
            actors: HashMap::new(),
            results: Arc::new(Mutex::new(HashMap::new())),
            start_time: Instant::now(),
            completion_notify,
        }
    }
    
    fn register_actor(&mut self, id: usize, addr: Addr<TestActor>) {
        self.actors.insert(id, addr);
    }
}

// 结果收集器
struct ResultCollector {
    results: HashMap<usize, TestResult>,
    test_completed: bool,
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
    fn new() -> Self {
        Self {
            results: HashMap::new(),
            test_completed: false,
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
        
        // 计算延迟统计
        let mut all_latencies = Vec::new();
        let mut min_latency = Duration::from_secs(u64::MAX);
        let mut max_latency = Duration::from_secs(0);
        
        for result in self.results.values() {
            if result.min_latency > Duration::from_secs(0) && result.min_latency < min_latency {
                min_latency = result.min_latency;
            }
            
            if result.max_latency > max_latency {
                max_latency = result.max_latency;
            }
            
            if result.received_count > 0 {
                all_latencies.push(result.avg_latency);
            }
        }
        
        let avg_latency = if !all_latencies.is_empty() {
            all_latencies.iter().sum::<Duration>() / all_latencies.len() as u32
        } else {
            Duration::from_secs(0)
        };
        
        // 计算吞吐量
        let total_throughput: f64 = self.results.values().map(|r| r.throughput).sum();
        
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
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志 - 改为debug级别
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));
    
    // 在LocalSet上运行测试
    let local = tokio::task::LocalSet::new();
    
    local.run_until(async {
        log::info!("Starting multi-actor benchmark with {} actors", ACTOR_COUNT);
        log::info!("Each actor will send {} messages of {} bytes to all other actors", 
                 MESSAGES_PER_ACTOR, MESSAGE_SIZE);
        
        // 创建结果收集器
        let collector = ResultCollector::new().start();
        let collector_recipient = collector.recipient();
        
        // 创建协调器
        let coordinator = TestCoordinator::new(ACTOR_COUNT, Some(collector_recipient)).start();
        
        // 创建多个测试actor
        for i in 0..ACTOR_COUNT {
            // 创建actor
            let test_actor = TestActor::new(i, coordinator.clone()).start();
            
            // 注册actor到协调器
            coordinator.send(RegisterActor { id: i, addr: test_actor.clone() }).await.unwrap();
            
            // 等待一小段时间，避免同时创建所有actor
            if i % 5 == 0 {
                sleep(Duration::from_millis(50)).await;
                log::debug!("Created {} actors so far", i+1);
            }
        }
        
        // 等待所有actor注册完成
        sleep(Duration::from_secs(1)).await;
        
        // 开始测试
        coordinator.do_send(StartTest);
        
        // 等待测试完成
        log::info!("Waiting for test to complete...");
        
        // 添加超时检测
        let start_time = Instant::now();
        let timeout = Duration::from_secs(60); // 60秒超时
        
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