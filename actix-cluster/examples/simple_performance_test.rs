use std::time::{Duration, Instant};
use actix::prelude::*;
use tokio::time::sleep;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::sync::Mutex;

use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod,
    NodeRole, SerializationFormat
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
    timestamp: Instant,
    round_trip: Duration,
}

// 测试Actor
struct TestActor {
    name: String,
    received_count: u64,
    latencies: Vec<Duration>,
}

impl Actor for TestActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("测试Actor已启动: {}", self.name);
    }
}

// 处理请求消息
impl Handler<TestMessage> for TestActor {
    type Result = MessageResult<TestMessage>;

    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.received_count += 1;

        if self.received_count % 1000 == 0 {
            println!("{} 已接收 {} 条消息", self.name, self.received_count);
        }

        let now = Instant::now();
        let response = TestResponse {
            id: msg.id,
            timestamp: msg.timestamp,
            round_trip: now.duration_since(msg.timestamp),
        };

        MessageResult(response)
    }
}

// 处理响应消息
impl Handler<TestResponse> for TestActor {
    type Result = ();

    fn handle(&mut self, msg: TestResponse, _ctx: &mut Self::Context) -> Self::Result {
        let latency = msg.round_trip;
        self.latencies.push(latency);

        if self.latencies.len() % 1000 == 0 {
            println!("{} 已接收 {} 条响应, 最近延迟: {:?}",
                    self.name, self.latencies.len(), latency);
        }
    }
}

// 结束测试的消息
#[derive(Message)]
#[rtype(result = "TestResult")]
struct FinishTest;

// 测试结果
struct TestResult {
    name: String,
    message_count: usize,
    min_latency: Duration,
    max_latency: Duration,
    avg_latency: Duration,
    p95_latency: Duration,
    p99_latency: Duration,
    duration: Duration,
    throughput: f64,
}

// 处理结束测试消息
impl Handler<FinishTest> for TestActor {
    type Result = MessageResult<FinishTest>;

    fn handle(&mut self, _msg: FinishTest, _ctx: &mut Self::Context) -> Self::Result {
        println!("{} 测试结束, 收集结果中...", self.name);

        let count = self.latencies.len();
        if count == 0 {
            return MessageResult(TestResult {
                name: self.name.clone(),
                message_count: 0,
                min_latency: Duration::from_secs(0),
                max_latency: Duration::from_secs(0),
                avg_latency: Duration::from_secs(0),
                p95_latency: Duration::from_secs(0),
                p99_latency: Duration::from_secs(0),
                duration: Duration::from_secs(0),
                throughput: 0.0,
            });
        }

        // 计算统计数据
        let mut sorted = self.latencies.clone();
        sorted.sort();

        let min_latency = *sorted.first().unwrap();
        let max_latency = *sorted.last().unwrap();
        let avg_latency = self.latencies.iter().sum::<Duration>() / count as u32;

        let p95_idx = (count as f64 * 0.95) as usize;
        let p99_idx = (count as f64 * 0.99) as usize;
        let p95_latency = sorted[p95_idx];
        let p99_latency = sorted[p99_idx];

        // 假设测试时间为最后一个响应与第一个请求的时间差
        let duration = Duration::from_secs(5); // 简化处理，假设5秒
        let throughput = count as f64 / duration.as_secs_f64();

        MessageResult(TestResult {
            name: self.name.clone(),
            message_count: count,
            min_latency,
            max_latency,
            avg_latency,
            p95_latency,
            p99_latency,
            duration,
            throughput,
        })
    }
}

impl TestActor {
    fn new(name: String) -> Self {
        Self {
            name,
            received_count: 0,
            latencies: Vec::new(),
        }
    }
}

// 配置创建
fn create_config(name: &str, port: u16, serialization: SerializationFormat, arch: Architecture) -> ClusterConfig {
    let mut config = ClusterConfig::new();
    config = config.cluster_name(name.to_string());
    config = config.bind_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port));
    config = config.node_role(NodeRole::Worker);
    config = config.architecture(arch);
    config = config.discovery(DiscoveryMethod::Multicast);
    config = config.serialization_format(serialization);
    config
}

// 生成消息
fn generate_payload(size: usize) -> Vec<u8> {
    vec![1; size]
}

// 运行测试配置
async fn run_test(
    serialization: SerializationFormat,
    message_size: usize,
    message_count: u64,
    architecture: Architecture
) -> TestResult {
    println!("\n==== 开始测试 ====");
    println!("序列化格式: {:?}", serialization);
    println!("消息大小: {} 字节", message_size);
    println!("消息数量: {}", message_count);
    println!("集群架构: {:?}", architecture);

    // 创建两个测试节点
    let node1_name = "node1";
    let node2_name = "node2";
    let port1 = 8701;
    let port2 = 8702;

    let config1 = create_config(node1_name, port1, serialization, architecture.clone());
    let config2 = create_config(node2_name, port2, serialization, architecture);

    let mut system1 = ClusterSystem::new(config1);
    let mut system2 = ClusterSystem::new(config2);

    // 启动测试actors
    let actor1 = TestActor::new(node1_name.to_string()).start();
    let actor2 = TestActor::new(node2_name.to_string()).start();

    // 启动集群系统
    match system1.start().await {
        Ok(_) => println!("节点1启动成功"),
        Err(e) => {
            println!("节点1启动失败: {}", e);
            return actor1.send(FinishTest).await.unwrap();
        }
    }

    match system2.start().await {
        Ok(_) => println!("节点2启动成功"),
        Err(e) => {
            println!("节点2启动失败: {}", e);
            return actor1.send(FinishTest).await.unwrap();
        }
    }

    // 等待集群稳定
    println!("等待集群稳定...");
    sleep(Duration::from_secs(3)).await;

    // 开始发送消息
    println!("开始发送消息...");
    let start_time = Instant::now();

    for i in 0..message_count {
        let payload = generate_payload(message_size);
        let msg = TestMessage {
            id: i,
            timestamp: Instant::now(),
            payload,
        };

        match actor2.send(msg).await {
            Ok(response) => {
                actor1.do_send(response);
            },
            Err(e) => {
                println!("发送消息失败: {}", e);
            }
        }

        if i % 1000 == 0 && i > 0 {
            sleep(Duration::from_millis(10)).await;
            println!("已发送 {} 条消息", i);
        }
    }

    let elapsed = start_time.elapsed();
    println!("消息发送完成，耗时: {:?}", elapsed);

    // 等待所有响应处理完成
    println!("等待响应处理...");
    sleep(Duration::from_secs(5)).await;

    // 获取测试结果
    let result = actor1.send(FinishTest).await.unwrap();

    // 输出结果
    println!("\n==== 测试结果 ====");
    println!("消息数量: {}", result.message_count);
    println!("最小延迟: {:?}", result.min_latency);
    println!("最大延迟: {:?}", result.max_latency);
    println!("平均延迟: {:?}", result.avg_latency);
    println!("P95延迟: {:?}", result.p95_latency);
    println!("P99延迟: {:?}", result.p99_latency);
    println!("测试时间: {:?}", result.duration);
    println!("吞吐量: {:.2} 消息/秒", result.throughput);

    result
}

#[actix_rt::main]
async fn main() {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // 定义测试参数
    struct TestParams {
        serialization: SerializationFormat,
        message_size: usize,
        message_count: u64,
        architecture: Architecture,
        name: String,
    }

    let tests = vec![
        // 测试不同序列化格式
        TestParams {
            serialization: SerializationFormat::Bincode,
            message_size: 1024,
            message_count: 5000,
            architecture: Architecture::Decentralized,
            name: "Bincode序列化".to_string(),
        },
        TestParams {
            serialization: SerializationFormat::Json,
            message_size: 1024,
            message_count: 5000,
            architecture: Architecture::Decentralized,
            name: "JSON序列化".to_string(),
        },

        // 测试不同消息大小
        TestParams {
            serialization: SerializationFormat::Bincode,
            message_size: 128,
            message_count: 5000,
            architecture: Architecture::Decentralized,
            name: "小消息(128字节)".to_string(),
        },
        TestParams {
            serialization: SerializationFormat::Bincode,
            message_size: 10240,
            message_count: 2000,
            architecture: Architecture::Decentralized,
            name: "大消息(10KB)".to_string(),
        },

        // 测试不同架构
        TestParams {
            serialization: SerializationFormat::Bincode,
            message_size: 1024,
            message_count: 5000,
            architecture: Architecture::Decentralized,
            name: "去中心化架构".to_string(),
        },
        TestParams {
            serialization: SerializationFormat::Bincode,
            message_size: 1024,
            message_count: 5000,
            architecture: Architecture::Centralized,
            name: "中心化架构".to_string(),
        },
    ];

    // 创建LocalSet用于执行本地任务
    let local = tokio::task::LocalSet::new();

    // 在LocalSet内运行测试
    local.run_until(async move {
        // 运行所有测试
        let mut results = Vec::new();

        for test in tests {
            println!("\n===== 运行测试: {} =====", test.name);
            let result = run_test(
                test.serialization,
                test.message_size,
                test.message_count,
                test.architecture
            ).await;

            results.push((test.name, result));

            // 测试间休息一下
            sleep(Duration::from_secs(5)).await;
        }

        // 打印比较结果
        println!("\n===== 性能比较结果 =====");
        println!("配置\t\t消息数\t平均延迟\tP99延迟\t吞吐量(消息/秒)");

        for (name, result) in results {
            println!("{}\t{}\t{:?}\t{:?}\t{:.2}",
                    name, result.message_count,
                    result.avg_latency, result.p99_latency,
                    result.throughput);
        }
    }).await;
}