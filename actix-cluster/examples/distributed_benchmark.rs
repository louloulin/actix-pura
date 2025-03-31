use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use actix::prelude::*;
use env_logger;
use log::{info, warn, error, debug};
use serde::{Serialize, Deserialize};

// 配置参数
const ACTOR_COUNT: usize = 10;
const MESSAGES_PER_ACTOR: u64 = 10000;
const MESSAGE_SIZE: usize = 1024;
const BATCH_SIZE: usize = 100;

// 全局度量指标
struct Metrics {
    sent: AtomicU64,
    received: AtomicU64,
    acked: AtomicU64,
    failed: AtomicU64,
    min_latency: AtomicU64,
    max_latency: AtomicU64,
    total_latency: AtomicU64,
    start_time: Instant,
    end_time: Option<Instant>,
}

impl Metrics {
    fn new() -> Self {
        Self {
            sent: AtomicU64::new(0),
            received: AtomicU64::new(0),
            acked: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            min_latency: AtomicU64::new(u64::MAX),
            max_latency: AtomicU64::new(0),
            total_latency: AtomicU64::new(0),
            start_time: Instant::now(),
            end_time: None,
        }
    }

    fn inc_sent(&self, count: u64) {
        self.sent.fetch_add(count, Ordering::Relaxed);
    }

    fn inc_received(&self, count: u64) {
        self.received.fetch_add(count, Ordering::Relaxed);
    }

    fn inc_acked(&self, count: u64) {
        self.acked.fetch_add(count, Ordering::Relaxed);
    }

    fn inc_failed(&self, count: u64) {
        self.failed.fetch_add(count, Ordering::Relaxed);
    }

    fn record_latency(&self, latency: u64) {
        let mut min = self.min_latency.load(Ordering::Relaxed);
        while latency < min {
            match self.min_latency.compare_exchange(
                min,
                latency,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => min = actual,
            }
        }

        let mut max = self.max_latency.load(Ordering::Relaxed);
        while latency > max {
            match self.max_latency.compare_exchange(
                max,
                latency,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => max = actual,
            }
        }

        self.total_latency.fetch_add(latency, Ordering::Relaxed);
    }

    fn report(&self) {
        let sent = self.sent.load(Ordering::Relaxed);
        let received = self.received.load(Ordering::Relaxed);
        let acked = self.acked.load(Ordering::Relaxed);
        let failed = self.failed.load(Ordering::Relaxed);

        let end_time = self.end_time.unwrap_or_else(Instant::now);
        let duration = end_time.duration_since(self.start_time);
        let duration_secs = duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9;

        let throughput = if duration_secs > 0.0 {
            (sent as f64 / duration_secs) as u64
        } else {
            0
        };

        let min_latency = self.min_latency.load(Ordering::Relaxed);
        let max_latency = self.max_latency.load(Ordering::Relaxed);
        let total_latency = self.total_latency.load(Ordering::Relaxed);

        let avg_latency = if acked > 0 {
            total_latency / acked
        } else {
            0
        };

        info!("===================== 性能测试结果 =====================");
        info!("测试持续时间: {:?}", duration);
        info!("发送消息数量: {}", sent);
        info!("接收消息数量: {}", received);
        info!("确认消息数量: {}", acked);
        info!("失败消息数量: {}", failed);
        info!("吞吐量: {} 消息/秒", throughput);
        info!("最小延迟: {} 微秒", min_latency);
        info!("平均延迟: {} 微秒", avg_latency);
        info!("最大延迟: {} 微秒", max_latency);
        info!("=======================================================");
    }
}

// 基准测试消息
#[derive(Message, Serialize, Deserialize, Clone, Debug)]
#[rtype(result = "()")]
struct BenchMessage {
    id: u64,
    sender_id: String,
    timestamp: u64,
    payload: Vec<u8>,
}

// 确认消息
#[derive(Message, Serialize, Deserialize, Clone, Debug)]
#[rtype(result = "()")]
struct BenchAck {
    message_id: u64,
    original_timestamp: u64,
    receive_timestamp: u64,
    receiver_id: String,
}

// 启动测试命令
#[derive(Message, Clone)]
#[rtype(result = "()")]
struct StartBenchmark {
    target_messages: u64,
    message_size: usize,
    batch_size: usize,
}

// 获取结果请求
#[derive(Message)]
#[rtype(result = "BenchResult")]
struct GetResults;

// 测试结果
#[derive(Debug, Clone)]
struct BenchResult {
    sent: u64,
    received: u64,
    acked: u64,
    failed: u64,
    min_latency: u64,
    max_latency: u64,
    total_latency: u64,
}

// 测试完成通知
#[derive(Message)]
#[rtype(result = "()")]
struct CompletionNotice;

// 基准测试Actor
struct BenchActor {
    id: String,
    targets: Vec<Addr<BenchActor>>,
    sent: u64,
    received: u64,
    acked: u64,
    failed: u64,
    latencies: Vec<u64>,
    metrics: Arc<Metrics>,
    target_messages: u64,
    message_size: usize,
    batch_size: usize,
    coordinator: Option<Addr<Coordinator>>,
    start_time: Option<Instant>,
    end_time: Option<Instant>,
}

impl BenchActor {
    fn new(id: String, metrics: Arc<Metrics>) -> Self {
        Self {
            id,
            targets: Vec::new(),
            sent: 0,
            received: 0,
            acked: 0,
            failed: 0,
            latencies: Vec::new(),
            metrics,
            target_messages: 0,
            message_size: 0,
            batch_size: 0,
            coordinator: None,
            start_time: None,
            end_time: None,
        }
    }

    fn calculate_results(&self) -> BenchResult {
        let mut min_latency = u64::MAX;
        let mut max_latency = 0;
        let mut total_latency = 0;

        for &latency in &self.latencies {
            min_latency = min_latency.min(latency);
            max_latency = max_latency.max(latency);
            total_latency += latency;
        }

        if self.latencies.is_empty() {
            min_latency = 0;
        }

        BenchResult {
            sent: self.sent,
            received: self.received,
            acked: self.acked,
            failed: self.failed,
            min_latency,
            max_latency,
            total_latency,
        }
    }
}

impl Actor for BenchActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("BenchActor {} started", self.id);
    }
}

// 处理基准测试消息
impl Handler<BenchMessage> for BenchActor {
    type Result = ();

    fn handle(&mut self, msg: BenchMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.received += 1;
        self.metrics.inc_received(1);

        // 获取当前时间戳(微秒)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        // 发送确认消息
        let ack = BenchAck {
            message_id: msg.id,
            original_timestamp: msg.timestamp,
            receive_timestamp: now,
            receiver_id: self.id.clone(),
        };

        // 查找发送者的地址
        for target in &self.targets {
            // 由于我们在本地测试，只要向所有targets发送确认即可
            // 在真实场景中需要查找特定发送者
            target.do_send(ack.clone());
        }

        // 如果已完成所有消息处理，发送完成通知
        if self.received >= self.target_messages && self.end_time.is_none() {
            self.end_time = Some(Instant::now());
            
            // 通知协调器此Actor已完成
            if let Some(ref coordinator) = self.coordinator {
                coordinator.do_send(ActorCompleted {
                    actor_id: self.id.clone(),
                });
            }
        }
    }
}

// 处理确认消息
impl Handler<BenchAck> for BenchActor {
    type Result = ();

    fn handle(&mut self, msg: BenchAck, _ctx: &mut Self::Context) -> Self::Result {
        self.acked += 1;
        self.metrics.inc_acked(1);

        // 计算延迟(微秒)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
            
        let latency = now - msg.original_timestamp;
        self.latencies.push(latency);
        self.metrics.record_latency(latency);

        // 如果已收到所有确认，标记完成时间
        if self.acked >= self.sent && self.end_time.is_none() {
            self.end_time = Some(Instant::now());
            
            // 通知协调器此Actor已完成
            if let Some(ref coordinator) = self.coordinator {
                coordinator.do_send(ActorCompleted {
                    actor_id: self.id.clone(),
                });
            }
        }
    }
}

// 设置目标Actors
#[derive(Message)]
#[rtype(result = "()")]
struct SetTargets {
    targets: Vec<Addr<BenchActor>>,
    coordinator: Addr<Coordinator>,
}

impl Handler<SetTargets> for BenchActor {
    type Result = ();

    fn handle(&mut self, msg: SetTargets, _ctx: &mut Self::Context) -> Self::Result {
        self.targets = msg.targets;
        self.coordinator = Some(msg.coordinator);
        info!("Actor {} received {} targets", self.id, self.targets.len());
    }
}

// 处理开始测试消息
impl Handler<StartBenchmark> for BenchActor {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: StartBenchmark, ctx: &mut Self::Context) -> Self::Result {
        self.target_messages = msg.target_messages;
        self.message_size = msg.message_size;
        self.batch_size = msg.batch_size;
        self.start_time = Some(Instant::now());

        let self_addr = ctx.address();
        let id = self.id.clone();
        let targets = self.targets.clone();
        let target_messages = self.target_messages;
        let message_size = self.message_size;
        let batch_size = self.batch_size;
        let metrics = self.metrics.clone();

        Box::pin(async move {
            info!("Actor {} starting benchmark", id);
            
            if targets.is_empty() {
                warn!("Actor {} found no targets, skipping test", id);
                return;
            }

            info!("Actor {} has {} targets", id, targets.len());
            
            // 计算每个目标的消息数量
            let targets_count = targets.len() as u64;
            let msgs_per_target = target_messages / targets_count;
            let mut remaining = target_messages % targets_count;
            
            let mut sent_count = 0;
            let start_time = Instant::now();
            
            // 为每个目标并行发送消息
            let mut futures = Vec::new();
            
            for (idx, target_addr) in targets.iter().enumerate() {
                // 计算要发送的消息数
                let mut to_send = msgs_per_target;
                if remaining > 0 {
                    to_send += 1;
                    remaining -= 1;
                }
                
                if to_send == 0 {
                    continue;
                }
                
                let sender_id = id.clone();
                let target = target_addr.clone();
                let metrics = metrics.clone();
                let self_addr2 = self_addr.clone();
                
                let send_future = async move {
                    let mut local_sent = 0;
                    
                    // 分批发送
                    for batch_start in (0..to_send).step_by(batch_size) {
                        let batch_end = std::cmp::min(batch_start + batch_size as u64, to_send);
                        let batch_size = (batch_end - batch_start) as usize;
                        
                        // 准备这批消息
                        let mut batch_messages = Vec::with_capacity(batch_size);
                        
                        for i in 0..batch_size {
                            let msg_id = batch_start + i as u64;
                            
                            // 获取当前时间戳(微秒)
                            let timestamp = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_micros() as u64;
                                
                            let msg = BenchMessage {
                                id: msg_id + (idx as u64 * msgs_per_target),
                                sender_id: sender_id.clone(),
                                timestamp,
                                payload: vec![0; message_size],
                            };
                            
                            batch_messages.push(msg);
                        }
                        
                        // 发送这批消息
                        for msg in batch_messages {
                            // 使用actix标准发送机制
                            target.do_send(msg);
                            local_sent += 1;
                        }
                        
                        // 简短暂停，避免过载
                        tokio::time::sleep(Duration::from_micros(10)).await;
                    }
                    
                    // 更新发送计数
                    if local_sent > 0 {
                        metrics.inc_sent(local_sent);
                        self_addr2.do_send(UpdateSentCount(local_sent));
                    }
                    
                    local_sent
                };
                
                futures.push(send_future);
            }
            
            // 等待所有发送任务完成
            let results = futures::future::join_all(futures).await;
            sent_count = results.iter().sum();
            
            info!("Actor {} sent {} messages in {:?}", 
                 id, sent_count, start_time.elapsed());
        })
    }
}

// Actor完成处理消息
#[derive(Message)]
#[rtype(result = "()")]
struct ActorCompleted {
    actor_id: String,
}

// 更新发送计数消息
#[derive(Message)]
#[rtype(result = "()")]
struct UpdateSentCount(u64);

impl Handler<UpdateSentCount> for BenchActor {
    type Result = ();

    fn handle(&mut self, msg: UpdateSentCount, _ctx: &mut Self::Context) -> Self::Result {
        self.sent += msg.0;
    }
}

// 处理获取结果消息
impl Handler<GetResults> for BenchActor {
    type Result = MessageResult<GetResults>;

    fn handle(&mut self, _: GetResults, _ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.calculate_results())
    }
}

// 协调器Actor负责管理测试
struct Coordinator {
    metrics: Arc<Metrics>,
    actors: Vec<Addr<BenchActor>>,
    completed_actors: usize,
}

impl Coordinator {
    fn new(metrics: Arc<Metrics>) -> Self {
        Self {
            metrics,
            actors: Vec::new(),
            completed_actors: 0,
        }
    }
    
    fn create_actors(&mut self, count: usize) -> Vec<Addr<BenchActor>> {
        info!("Creating {} actors", count);
        
        let mut actors = Vec::with_capacity(count);
        
        for i in 0..count {
            let actor_id = format!("actor-{}", i);
            let actor = BenchActor::new(
                actor_id.clone(),
                self.metrics.clone(),
            );
            
            let addr = actor.start();
            actors.push(addr);
            
            info!("Actor {} created", actor_id);
        }
        
        self.actors = actors.clone();
        actors
    }
    
    fn start_benchmark(&self, messages: u64, message_size: usize, batch_size: usize, ctx: &mut <Self as Actor>::Context) {
        info!("Starting benchmark with {} messages per actor, size: {} bytes", 
             messages, message_size);
        
        // 设置目标（所有Actors互相连接）
        for actor in &self.actors {
            let targets: Vec<_> = self.actors.iter()
                .filter(|&a| a != actor)
                .cloned()
                .collect();
                
            actor.do_send(SetTargets {
                targets,
                coordinator: ctx.address(),
            });
        }
        
        // 发送开始消息给所有Actor
        let start_cmd = StartBenchmark {
            target_messages: messages,
            message_size,
            batch_size,
        };
        
        for actor in &self.actors {
            actor.do_send(start_cmd.clone());
        }
    }
    
    fn collect_results(&self, ctx: &mut <Self as Actor>::Context) {
        info!("Collecting results from all actors");
        
        let mut futures = Vec::new();
        
        for actor in &self.actors {
            let fut = actor.send(GetResults);
            futures.push(fut);
        }
        
        // 等待所有结果并处理
        let addr = ctx.address().clone();  // 克隆地址以便在闭包中使用
        ctx.spawn(
            futures::future::join_all(futures)
                .into_actor(self)
                .map(move |results, _actor, _ctx| {  // 使用move移动addr所有权
                    // 处理所有成功收到的结果
                    let valid_results: Vec<_> = results
                        .into_iter()
                        .filter_map(Result::ok)
                        .collect();
                    
                    info!("Received {} valid results from actors", valid_results.len());
                    
                    // 所有测试完成，生成报告
                    addr.do_send(CompletionNotice);
                })
        );
    }
}

impl Actor for Coordinator {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("Coordinator started");
    }
}

// 处理Actor完成通知
impl Handler<ActorCompleted> for Coordinator {
    type Result = ();

    fn handle(&mut self, msg: ActorCompleted, ctx: &mut Self::Context) -> Self::Result {
        self.completed_actors += 1;
        
        info!("Actor {} completed benchmark. {}/{} actors completed",
             msg.actor_id, self.completed_actors, self.actors.len());
        
        // 如果所有Actor都已完成，收集结果
        if self.completed_actors >= self.actors.len() {
            info!("All actors completed, collecting results");
            
            // 设置结束时间
            let now = Instant::now();
            if let Some(metrics) = Arc::get_mut(&mut self.metrics) {
                metrics.end_time = Some(now);
            }
            
            // 等待一段时间确保所有消息都已处理完毕
            ctx.run_later(Duration::from_secs(3), |coord, ctx| {
                coord.collect_results(ctx);
            });
        }
    }
}

// 处理测试完成消息
impl Handler<CompletionNotice> for Coordinator {
    type Result = ();

    fn handle(&mut self, _: CompletionNotice, _ctx: &mut Self::Context) -> Self::Result {
        info!("Benchmark test completed");
        
        // 报告结果
        self.metrics.report();
        
        // 停止系统
        System::current().stop();
    }
}

// 实现StartBenchmark处理器
impl Handler<StartBenchmark> for Coordinator {
    type Result = ();

    fn handle(&mut self, msg: StartBenchmark, ctx: &mut Self::Context) -> Self::Result {
        self.start_benchmark(msg.target_messages, msg.message_size, msg.batch_size, ctx);
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // 配置系统参数 - 设置actix线程池大小
    std::env::set_var("ACTIX_THREADPOOL", "8");
    
    info!("Starting benchmark with {} actors", ACTOR_COUNT);
    
    // 创建全局指标
    let metrics = Arc::new(Metrics::new());
    
    // 创建协调器
    let mut coordinator = Coordinator::new(metrics.clone());
    
    // 创建actors
    coordinator.create_actors(ACTOR_COUNT);
    
    // 启动协调器
    let coordinator_addr = coordinator.start();
    
    // 启动测试
    coordinator_addr.do_send(StartBenchmark {
        target_messages: MESSAGES_PER_ACTOR,
        message_size: MESSAGE_SIZE,
        batch_size: BATCH_SIZE,
    });
    
    // 等待信号或测试完成
    tokio::signal::ctrl_c().await.ok();
    
    Ok(())
} 