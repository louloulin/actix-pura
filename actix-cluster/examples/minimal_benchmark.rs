use std::time::{Duration, Instant};
use std::collections::HashMap;
use actix::prelude::*;
use tokio::time::sleep;
use futures::future::join_all;

// 消息大小选项
const SMALL_MESSAGE: usize = 128;    // 128 bytes
const MEDIUM_MESSAGE: usize = 4096;  // 4 KB
const LARGE_MESSAGE: usize = 65536;  // 64 KB

// 测试消息
#[derive(Message, Clone)]
#[rtype(result = "BenchmarkResponse")]
struct BenchmarkMessage {
    id: u64,
    timestamp: Instant,
    payload: Vec<u8>,
}

// 测试响应
#[derive(Message, Clone)]
#[rtype(result = "()")]
struct BenchmarkResponse {
    id: u64,
    original_timestamp: Instant,
    round_trip: Duration,
}

// 测试结果请求
#[derive(Message)]
#[rtype(result = "BenchmarkResult")]
struct GetResults;

// 测试结果
struct BenchmarkResult {
    message_count: usize,
    min_latency: Duration,
    max_latency: Duration,
    avg_latency: Duration,
    p95_latency: Duration,
    p99_latency: Duration,
    throughput: f64,
    test_duration: Duration,
}

// 测试Actor
struct BenchmarkActor {
    name: String,
    received_count: u64,
    latencies: Vec<Duration>,
}

impl Actor for BenchmarkActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("Benchmark actor started: {}", self.name);
    }
}

// 处理请求
impl Handler<BenchmarkMessage> for BenchmarkActor {
    type Result = MessageResult<BenchmarkMessage>;
    
    fn handle(&mut self, msg: BenchmarkMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.received_count += 1;
        
        if self.received_count % 10000 == 0 {
            println!("{} received {} messages", self.name, self.received_count);
        }
        
        let now = Instant::now();
        let response = BenchmarkResponse {
            id: msg.id,
            original_timestamp: msg.timestamp,
            round_trip: now.duration_since(msg.timestamp),
        };
        
        MessageResult(response)
    }
}

// 处理响应
impl Handler<BenchmarkResponse> for BenchmarkActor {
    type Result = ();
    
    fn handle(&mut self, msg: BenchmarkResponse, _ctx: &mut Self::Context) -> Self::Result {
        let latency = msg.round_trip;
        self.latencies.push(latency);
        
        if self.latencies.len() % 10000 == 0 {
            println!("{} received {} responses, latest latency: {:?}", 
                     self.name, self.latencies.len(), latency);
        }
    }
}

// 处理结果请求
impl Handler<GetResults> for BenchmarkActor {
    type Result = MessageResult<GetResults>;
    
    fn handle(&mut self, _: GetResults, _ctx: &mut Self::Context) -> Self::Result {
        // 计算结果
        if self.latencies.is_empty() {
            return MessageResult(BenchmarkResult {
                message_count: 0,
                min_latency: Duration::from_secs(0),
                max_latency: Duration::from_secs(0),
                avg_latency: Duration::from_secs(0),
                p95_latency: Duration::from_secs(0),
                p99_latency: Duration::from_secs(0),
                throughput: 0.0,
                test_duration: Duration::from_secs(0),
            });
        }
        
        // 计算延迟统计
        let mut sorted = self.latencies.clone();
        sorted.sort();
        
        let min_latency = *sorted.first().unwrap();
        let max_latency = *sorted.last().unwrap();
        let avg_latency = self.latencies.iter().sum::<Duration>() / self.latencies.len() as u32;
        
        let p95_idx = (self.latencies.len() as f64 * 0.95) as usize;
        let p99_idx = (self.latencies.len() as f64 * 0.99) as usize;
        let p95_latency = sorted[p95_idx];
        let p99_latency = sorted[p99_idx];
        
        // 使用测试持续时间计算吞吐量
        let test_duration = Duration::from_secs(5); // 这个值会在测试函数中更新
        let throughput = self.latencies.len() as f64 / test_duration.as_secs_f64();
        
        MessageResult(BenchmarkResult {
            message_count: self.latencies.len(),
            min_latency,
            max_latency,
            avg_latency,
            p95_latency,
            p99_latency,
            throughput,
            test_duration,
        })
    }
}

impl BenchmarkActor {
    fn new(name: String) -> Self {
        Self {
            name,
            received_count: 0,
            latencies: Vec::new(),
        }
    }
}

// 运行简单的基准测试，无需完整集群
async fn run_simple_benchmark(message_count: u64, message_size: usize, batch_size: usize) -> BenchmarkResult {
    println!("\n==== 开始简单基准测试 ====");
    println!("消息数量: {}", message_count);
    println!("消息大小: {} bytes", message_size);
    println!("批处理大小: {}", batch_size);
    
    // 创建两个Actor
    let sender = BenchmarkActor::new("Sender".to_string()).start();
    let receiver = BenchmarkActor::new("Receiver".to_string()).start();
    
    println!("开始发送消息...");
    let start_time = Instant::now();
    
    // 使用高效的发送方式
    for batch_start in (0..message_count).step_by(batch_size) {
        let batch_end = std::cmp::min(batch_start + batch_size as u64, message_count);
        let mut futures = Vec::with_capacity(batch_size);
        
        // 为每批次创建消息并发送
        for i in batch_start..batch_end {
            let payload = vec![0u8; message_size];
            let msg = BenchmarkMessage {
                id: i,
                timestamp: Instant::now(),
                payload,
            };
            
            // 使用clone避免所有权问题
            let receiver_clone = receiver.clone();
            let sender_clone = sender.clone();
            
            // 创建异步任务
            let future = async move {
                if let Ok(response) = receiver_clone.send(msg).await {
                    sender_clone.do_send(response);
                }
            };
            
            futures.push(future);
        }
        
        // 并行等待所有消息
        join_all(futures).await;
        
        if batch_start % (batch_size as u64 * 10) == 0 && batch_start > 0 {
            println!("已发送 {} 消息", batch_start);
        }
    }
    
    let elapsed = start_time.elapsed();
    println!("消息发送完成，耗时: {:?}", elapsed);
    
    // 等待所有响应处理完成
    sleep(Duration::from_secs(2)).await;
    
    // 获取结果
    let mut result = sender.send(GetResults).await.unwrap();
    
    // 更新实际测试持续时间
    result.test_duration = elapsed;
    
    // 打印结果
    println!("\n==== 测试结果 ====");
    println!("消息数量: {}", result.message_count);
    println!("最小延迟: {:?}", result.min_latency);
    println!("最大延迟: {:?}", result.max_latency);
    println!("平均延迟: {:?}", result.avg_latency);
    println!("P95延迟: {:?}", result.p95_latency);
    println!("P99延迟: {:?}", result.p99_latency);
    println!("测试持续时间: {:?}", result.test_duration);
    println!("吞吐量: {:.2} 消息/秒", result.throughput);
    
    result
}

#[actix_rt::main]
async fn main() {
    // 测试不同消息大小的性能
    let test_sizes = vec![
        ("小消息", SMALL_MESSAGE),
        ("中等消息", MEDIUM_MESSAGE),
        ("大消息", LARGE_MESSAGE),
    ];
    
    // 增加消息数量和批处理大小
    let message_count = 500000;   // 增加到50万条消息
    let batch_size = 5000;       // 更大的批处理大小
    
    let mut results = HashMap::new();
    
    for (name, size) in test_sizes {
        println!("\n===== 测试 {} ({} bytes) =====", name, size);
        let result = run_simple_benchmark(message_count, size, batch_size).await;
        results.insert(name, result);
        
        // 等待一下再测试下一个大小
        sleep(Duration::from_secs(2)).await;
    }
    
    // 比较结果
    println!("\n===== 性能比较 =====");
    println!("消息类型\t消息数\t平均延迟\tP99延迟\t吞吐量(消息/秒)");
    
    for (name, result) in &results {
        println!("{}\t{}\t{:?}\t{:?}\t{:.2}", 
                name, result.message_count, result.avg_latency, 
                result.p99_latency, result.throughput);
    }
} 