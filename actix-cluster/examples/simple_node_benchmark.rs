use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use actix::prelude::*;
use env_logger;
use log;
use tokio::time::sleep;

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole,
    SerializationFormat
};

// 测试消息
#[derive(Message, Clone)]
#[rtype(result = "TestResponse")]
struct TestMessage {
    id: u64,
    timestamp: Instant,
    payload: Vec<u8>,
}

// 测试响应
#[derive(Message, Clone)]
#[rtype(result = "()")]
struct TestResponse {
    id: u64,
    round_trip: Duration,
}

// 测试结果请求
#[derive(Message)]
#[rtype(result = "TestResult")]
struct GetResults;

// 测试结果
struct TestResult {
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

// 发送者Actor
struct SenderActor {
    name: String,
    sent_count: u64,
    latencies: Vec<Duration>,
    started_at: Instant,
    batch_size: usize,
}

// 接收者Actor
struct ReceiverActor {
    name: String,
    received_count: u64,
}

impl Actor for SenderActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("SenderActor started: {}", self.name);
        self.started_at = Instant::now();
    }
}

impl Actor for ReceiverActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("ReceiverActor started: {}", self.name);
    }
}

// 处理测试消息
impl Handler<TestMessage> for ReceiverActor {
    type Result = ResponseFuture<TestResponse>;

    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.received_count += 1;

        if self.received_count % 10000 == 0 {
            log::info!("{} received {} messages", self.name, self.received_count);
        }

        // 计算延迟并返回响应
        let now = Instant::now();
        let round_trip = now.duration_since(msg.timestamp);

        let response = TestResponse {
            id: msg.id,
            round_trip,
        };

        Box::pin(async move { response })
    }
}

// 处理测试响应
impl Handler<TestResponse> for SenderActor {
    type Result = ();

    fn handle(&mut self, msg: TestResponse, _ctx: &mut Self::Context) -> Self::Result {
        // 记录延迟
        self.latencies.push(msg.round_trip);

        if self.latencies.len() % 10000 == 0 {
            log::info!("{} received {} responses, latest latency: {:?}",
                     self.name, self.latencies.len(), msg.round_trip);
        }
    }
}

// 获取结果
impl Handler<GetResults> for SenderActor {
    type Result = MessageResult<GetResults>;

    fn handle(&mut self, _: GetResults, _ctx: &mut Self::Context) -> Self::Result {
        let result = self.calculate_results();
        MessageResult(result)
    }
}

impl SenderActor {
    fn new(name: String, batch_size: usize) -> Self {
        Self {
            name,
            sent_count: 0,
            latencies: Vec::new(),
            started_at: Instant::now(),
            batch_size,
        }
    }

    // 计算测试结果
    fn calculate_results(&self) -> TestResult {
        let received_count = self.latencies.len() as u64;
        let test_duration = self.started_at.elapsed();

        if self.latencies.is_empty() {
            return TestResult {
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

    // 批量发送消息
    async fn send_messages(&mut self, receiver: &Addr<ReceiverActor>, message_count: u64, message_size: usize) {
        log::info!("Starting to send {} messages of {} bytes each", message_count, message_size);
        let start_time = Instant::now();

        let self_addr = Actor::create(|_| self.clone());
        let batch_size = self.batch_size;

        for batch_start in (0..message_count).step_by(batch_size) {
            let batch_end = std::cmp::min(batch_start + batch_size as u64, message_count);

            for msg_id in batch_start..batch_end {
                let msg = TestMessage {
                    id: msg_id,
                    timestamp: Instant::now(),
                    payload: vec![0u8; message_size],
                };

                self.sent_count += 1;

                // 发送并等待响应
                if let Ok(response) = receiver.send(msg).await {
                    self_addr.do_send(response);
                }
            }

            if batch_start % (batch_size as u64 * 10) == 0 && batch_start > 0 {
                log::info!("Sent {} messages, elapsed: {:?}", batch_start, start_time.elapsed());
            }
        }

        let elapsed = start_time.elapsed();
        log::info!("Finished sending {} messages in {:?}", message_count, elapsed);
    }
}

// 添加Clone实现
impl Clone for SenderActor {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            sent_count: self.sent_count,
            latencies: self.latencies.clone(),
            started_at: self.started_at,
            batch_size: self.batch_size,
        }
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // 创建LocalSet以支持本地任务
    let local = tokio::task::LocalSet::new();

    // 在LocalSet上运行主测试逻辑
    let result = local.run_until(async {
        // 测试参数
        let message_count = 100000;  // 消息数量
        let message_size = 1024;     // 消息大小(字节)
        let batch_size = 1000;       // 批处理大小

        log::info!("Starting benchmark with {} messages of {} bytes",
                 message_count, message_size);

        // 创建发送者和接收者Actor
        let sender_actor = SenderActor::new("Sender".to_string(), batch_size).start();
        let receiver_actor = ReceiverActor {
            name: "Receiver".to_string(),
            received_count: 0
        }.start();

        // 创建集群配置
        let config = ClusterConfig::new()
            .bind_addr(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                8000
            ))
            .node_role(NodeRole::Worker)
            .architecture(Architecture::Decentralized)
            .serialization_format(SerializationFormat::Bincode)
            .discovery(DiscoveryMethod::Static {
                seed_nodes: Vec::new()
            });

        // 创建集群系统
        let mut cluster = ClusterSystem::new(config);

        // 启动集群系统
        match cluster.start().await {
            Ok(_addr) => {
                log::info!("Cluster system started");
                // 等待集群启动
                sleep(Duration::from_secs(1)).await;

                // 发送测试消息并收集结果
                let mut sender_instance = SenderActor::new("SenderInstance".to_string(), batch_size);
                sender_instance.send_messages(&receiver_actor, message_count, message_size).await;

                // 等待所有处理完成
                sleep(Duration::from_secs(2)).await;

                // 计算结果
                let result = sender_instance.calculate_results();

                // 关闭集群系统
                log::info!("Shutting down cluster system");
                System::current().stop();

                Ok(result)
            },
            Err(e) => {
                log::error!("Failed to start cluster: {}", e);
                Err(e)
            }
        }
    }).await;

    // 输出结果
    match result {
        Ok(test_result) => {
            log::info!("\n===== BENCHMARK RESULTS =====");
            log::info!("Messages sent: {}", test_result.sent_count);
            log::info!("Messages received: {}", test_result.received_count);
            log::info!("Success rate: {:.2}%",
                    (test_result.received_count as f64 / test_result.sent_count as f64) * 100.0);
            log::info!("Test duration: {:?}", test_result.test_duration);
            log::info!("Average latency: {:?}", test_result.avg_latency);
            log::info!("Minimum latency: {:?}", test_result.min_latency);
            log::info!("Maximum latency: {:?}", test_result.max_latency);
            log::info!("P95 latency: {:?}", test_result.p95_latency);
            log::info!("P99 latency: {:?}", test_result.p99_latency);
            log::info!("Throughput: {:.2} messages/second", test_result.throughput);
        },
        Err(e) => {
            log::error!("Benchmark failed: {}", e);
        }
    }

    Ok(())
}