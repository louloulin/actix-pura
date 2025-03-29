use std::time::{Duration, Instant};
use actix::prelude::*;
use tokio::time::sleep;

// 测试消息
#[derive(Message, Clone)]
#[rtype(result = "BenchmarkResponse")]
struct BenchmarkMessage {
    id: u64,
    sender_node: String,
    timestamp: Instant,
    payload: Vec<u8>, // 模拟不同大小的消息载荷
}

// 响应消息
#[derive(Message, Clone)]
#[rtype(result = "()")]
struct BenchmarkResponse {
    id: u64,
    original_timestamp: Instant,
    response_timestamp: Instant,
    responder_node: String,
}

// 测试结果
struct BenchmarkResult {
    messages_sent: u64,
    messages_received: u64,
    min_latency: Duration,
    max_latency: Duration,
    avg_latency: Duration,
}

// 测试Actor
struct BenchmarkActor {
    name: String,
    received_count: u64,
    sent_count: u64,
    latencies: Vec<Duration>,
}

impl Actor for BenchmarkActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("BenchmarkActor started: {}", self.name);
    }
}

impl Handler<BenchmarkMessage> for BenchmarkActor {
    type Result = MessageResult<BenchmarkMessage>;
    
    fn handle(&mut self, msg: BenchmarkMessage, _ctx: &mut Self::Context) -> Self::Result {
        // 记录消息接收
        self.received_count += 1;
        
        if self.received_count % 1000 == 0 {
            log::info!("{} received {} messages", self.name, self.received_count);
        }
        
        // 创建响应
        let response = BenchmarkResponse {
            id: msg.id,
            original_timestamp: msg.timestamp,
            response_timestamp: Instant::now(),
            responder_node: self.name.clone(),
        };
        
        MessageResult(response)
    }
}

impl Handler<BenchmarkResponse> for BenchmarkActor {
    type Result = ();
    
    fn handle(&mut self, msg: BenchmarkResponse, _ctx: &mut Self::Context) -> Self::Result {
        // 计算延迟
        let latency = msg.response_timestamp.duration_since(msg.original_timestamp);
        
        // 记录延迟
        self.latencies.push(latency);
        
        if self.latencies.len() % 1000 == 0 {
            log::info!("{} received {} responses, latest latency: {:?}", 
                      self.name, self.latencies.len(), latency);
        }
    }
}

impl BenchmarkActor {
    fn new(name: String) -> Self {
        Self {
            name,
            received_count: 0,
            sent_count: 0,
            latencies: Vec::new(),
        }
    }
    
    fn calculate_results(&self) -> BenchmarkResult {
        if self.latencies.is_empty() {
            return BenchmarkResult {
                messages_sent: self.sent_count,
                messages_received: self.received_count,
                min_latency: Duration::from_secs(0),
                max_latency: Duration::from_secs(0),
                avg_latency: Duration::from_secs(0),
            };
        }
        
        let min_latency = self.latencies.iter().min().cloned().unwrap_or(Duration::from_secs(0));
        let max_latency = self.latencies.iter().max().cloned().unwrap_or(Duration::from_secs(0));
        
        let total_latency: Duration = self.latencies.iter().sum();
        let avg_latency = total_latency / self.latencies.len() as u32;
        
        BenchmarkResult {
            messages_sent: self.sent_count,
            messages_received: self.received_count,
            min_latency,
            max_latency,
            avg_latency,
        }
    }
}

#[derive(Message)]
#[rtype(result = "BenchmarkResult")]
struct GetResults;

impl Handler<GetResults> for BenchmarkActor {
    type Result = MessageResult<GetResults>;
    
    fn handle(&mut self, _msg: GetResults, _ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.calculate_results())
    }
}

#[actix_rt::main]
async fn main() {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // 测试参数
    let msgs_count = 10000;    // 消息数量
    let message_size = 1024;   // 消息大小 (1KB)
    
    println!("Starting simplified distributed benchmark");
    println!("Messages: {}, Size: {} bytes", msgs_count, message_size);
    
    // 创建两个测试actor
    let actor1 = BenchmarkActor::new("Node1".to_string()).start();
    let actor2 = BenchmarkActor::new("Node2".to_string()).start();
    
    println!("Sending {} messages between actors...", msgs_count);
    let start = Instant::now();
    
    // 发送消息
    for i in 0..msgs_count {
        let payload = vec![0u8; message_size];
        
        // 从actor1发送到actor2
        let msg = BenchmarkMessage {
            id: i,
            sender_node: "Node1".to_string(),
            timestamp: Instant::now(),
            payload: payload.clone(),
        };
        
        // 发送消息并处理响应
        match actor2.send(msg).await {
            Ok(response) => {
                // 将响应发送回actor1处理
                actor1.do_send(response);
            },
            Err(e) => {
                log::error!("Failed to send message: {}", e);
            }
        }
        
        // 限制发送速率，避免过载
        if i % 1000 == 0 {
            sleep(Duration::from_millis(1)).await;
            println!("Sent {} messages...", i);
        }
    }
    
    // 等待所有消息处理完成
    sleep(Duration::from_secs(2)).await;
    
    let elapsed = start.elapsed();
    println!("All messages sent in {:?}", elapsed);
    
    // 收集结果
    let results1 = actor1.send(GetResults).await.unwrap();
    let results2 = actor2.send(GetResults).await.unwrap();
    
    // 打印结果
    println!("\n===== BENCHMARK RESULTS =====");
    println!("Messages sent: {}", msgs_count);
    println!("Messages size: {} bytes", message_size);
    
    println!("\nNode1 (Sender):");
    println!("  Responses received: {}", results1.messages_received);
    println!("  Minimum latency: {:?}", results1.min_latency);
    println!("  Maximum latency: {:?}", results1.max_latency);
    println!("  Average latency: {:?}", results1.avg_latency);
    
    println!("\nNode2 (Receiver):");
    println!("  Messages received: {}", results2.messages_received);
    
    // 计算吞吐量
    let throughput = msgs_count as f64 / elapsed.as_secs_f64();
    println!("\nPerformance:");
    println!("  Duration: {:?}", elapsed);
    println!("  Messages per second: {:.2}", throughput);
    println!("  Data throughput: {:.2} MB/s", 
        throughput * message_size as f64 / (1024.0 * 1024.0));
    
    // 系统分析
    println!("\nSystem Analysis:");
    if results1.avg_latency > Duration::from_millis(100) {
        println!("  ⚠️ High average latency detected (>100ms)");
        println!("  Consider: Optimizing message handling or reducing payload size");
    } else if results1.avg_latency < Duration::from_millis(10) {
        println!("  ✅ Excellent latency performance (<10ms)");
    } else {
        println!("  ✅ Acceptable latency performance");
    }
    
    if results1.max_latency - results1.min_latency > Duration::from_millis(200) {
        println!("  ⚠️ High latency variance detected");
        println!("  Consider: Investigating processing spikes or GC pauses");
    }
    
    if throughput < 5000.0 {
        println!("  ⚠️ Low throughput detected (<5000 msgs/sec)");
        println!("  Consider: Optimizing serialization, reducing payload size");
    } else if throughput > 50000.0 {
        println!("  ✅ Excellent throughput performance (>50K msgs/sec)");
    } else {
        println!("  ✅ Good throughput performance");
    }
    
    println!("\nNote: This is a simplified benchmark using local messaging.");
    println!("Actual distributed performance would be impacted by network");
    println!("conditions and hardware resources.");
} 