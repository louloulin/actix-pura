use std::time::{Duration, Instant};
use actix::prelude::*;
use env_logger;
use log;

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

// 接收者Actor
struct ReceiverActor {
    name: String,
    received_count: u64,
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

// 发送者Actor
struct SenderActor {
    name: String,
    sent_count: u64,
    received_count: u64,
    latencies: Vec<Duration>,
    started_at: Instant,
    batch_size: usize,
}

impl Actor for SenderActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("SenderActor started: {}", self.name);
        self.started_at = Instant::now();
    }
}

// 处理测试响应
impl Handler<TestResponse> for SenderActor {
    type Result = ();
    
    fn handle(&mut self, msg: TestResponse, _ctx: &mut Self::Context) -> Self::Result {
        self.received_count += 1;
        // 记录延迟
        self.latencies.push(msg.round_trip);
        
        if self.received_count % 10000 == 0 {
            log::info!("{} received {} responses, latest latency: {:?}", 
                     self.name, self.received_count, msg.round_trip);
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
            received_count: 0,
            latencies: Vec::new(),
            started_at: Instant::now(),
            batch_size,
        }
    }
    
    // 计算测试结果
    fn calculate_results(&self) -> TestResult {
        let received_count = self.received_count;
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
    async fn send_messages(&mut self, receiver: Addr<ReceiverActor>, message_count: u64, message_size: usize) {
        log::info!("Starting to send {} messages of {} bytes each", message_count, message_size);
        let start_time = Instant::now();
        
        for batch_start in (0..message_count).step_by(self.batch_size) {
            let batch_end = std::cmp::min(batch_start + self.batch_size as u64, message_count);
            
            for msg_id in batch_start..batch_end {
                let msg = TestMessage {
                    id: msg_id,
                    timestamp: Instant::now(),
                    payload: vec![0u8; message_size],
                };
                
                self.sent_count += 1;
                
                // 发送消息并处理响应
                if let Ok(response) = receiver.send(msg).await {
                    self.handle(response, &mut Context::new());
                }
            }
            
            if batch_start % (self.batch_size as u64 * 10) == 0 && batch_start > 0 {
                log::info!("Sent {} messages, elapsed: {:?}", batch_start, start_time.elapsed());
            }
        }
        
        let elapsed = start_time.elapsed();
        log::info!("Finished sending {} messages in {:?}", message_count, elapsed);
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // 创建接收者Actor
    let receiver = ReceiverActor {
        name: "Receiver".to_string(),
        received_count: 0
    }.start();
    
    // 测试参数
    let message_count = 100000;  // 消息数量
    let message_size = 1024;     // 消息大小(字节)
    let batch_size = 1000;       // 批处理大小
    
    log::info!("Starting benchmark with {} messages of {} bytes", 
             message_count, message_size);
    
    // 创建发送者Actor
    let mut sender = SenderActor::new("Sender".to_string(), batch_size);
    
    // 发送消息
    sender.send_messages(receiver, message_count, message_size).await;
    
    // 等待所有消息都被处理
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // 计算测试结果
    let result = sender.calculate_results();
    
    // 输出结果
    log::info!("\n===== BENCHMARK RESULTS =====");
    log::info!("Messages sent: {}", result.sent_count);
    log::info!("Messages received: {}", result.received_count);
    log::info!("Success rate: {:.2}%", 
            (result.received_count as f64 / result.sent_count as f64) * 100.0);
    log::info!("Test duration: {:?}", result.test_duration);
    log::info!("Average latency: {:?}", result.avg_latency);
    log::info!("Minimum latency: {:?}", result.min_latency);
    log::info!("Maximum latency: {:?}", result.max_latency);
    log::info!("P95 latency: {:?}", result.p95_latency);
    log::info!("P99 latency: {:?}", result.p99_latency);
    log::info!("Throughput: {:.2} messages/second", result.throughput);
    
    // 正常退出
    System::current().stop();
    Ok(())
} 