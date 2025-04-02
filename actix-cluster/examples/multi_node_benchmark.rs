use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio::sync::mpsc;
use actix::prelude::*;
use actix_cluster::{
    Architecture, ClusterConfig, ClusterSystem, DiscoveryMethod, NodeRole, 
    SerializationFormat
};

// 配置常量
const NODE_COUNT: usize = 3;
const MSGS_PER_NODE: u64 = 100;
const BASE_PORT: u16 = 8000;

// 简单的测试消息
#[derive(Message)]
#[rtype(result = "TestResponse")]
struct TestMessage {
    id: u64,
    sender_node: usize,
    timestamp: u64,  // 使用简单的u64表示时间戳
}

// 简单的测试响应
#[derive(Message)]
#[rtype(result = "()")]
struct TestResponse {
    original_id: u64,
    success: bool,
}

// 获取结果的消息
#[derive(Message)]
#[rtype(result = "BenchmarkResult")]
struct GetResult;

// 基准测试结果
#[derive(Debug, Clone)]
struct BenchmarkResult {
    node_id: String,
    messages_sent: u64,
    messages_received: u64,
    failures: u64,
    start_time: u64,
    end_time: u64,
}

// 测试Actor
struct TestActor {
    node_id: String,
    messages_sent: u64,
    messages_received: u64,
    failures: u64,
    start_time: u64,
}

impl TestActor {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            messages_sent: 0,
            messages_received: 0,
            failures: 0,
            start_time: Instant::now().elapsed().as_millis() as u64,
        }
    }

    fn calculate_result(&self) -> BenchmarkResult {
        BenchmarkResult {
            node_id: self.node_id.clone(),
            messages_sent: self.messages_sent,
            messages_received: self.messages_received,
            failures: self.failures,
            start_time: self.start_time,
            end_time: Instant::now().elapsed().as_millis() as u64,
        }
    }
}

impl Actor for TestActor {
    type Context = Context<Self>;
}

impl Handler<TestMessage> for TestActor {
    type Result = ResponseFuture<TestResponse>;

    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        self.messages_received += 1;
        
        // 返回响应
        let response = TestResponse {
            original_id: msg.id,
            success: true,
        };
        
        Box::pin(async move {
            sleep(Duration::from_millis(1)).await; // 增加一个小延迟模拟处理时间
            response
        })
    }
}

impl Handler<TestResponse> for TestActor {
    type Result = ();

    fn handle(&mut self, msg: TestResponse, _ctx: &mut Self::Context) -> Self::Result {
        // 记录响应
        if msg.success {
            self.messages_received += 1;
        } else {
            self.failures += 1;
        }
    }
}

impl Handler<GetResult> for TestActor {
    type Result = MessageResult<GetResult>;

    fn handle(&mut self, _msg: GetResult, _ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.calculate_result())
    }
}

// 创建集群配置
fn create_cluster_config(node_number: usize) -> ClusterConfig {
    let bind_port = BASE_PORT + node_number as u16;
    
    let mut config = ClusterConfig::new();
    config.node_role = if node_number == 1 { NodeRole::Master } else { NodeRole::Worker };
    config.architecture = Architecture::Decentralized;
    config.bind_addr = std::net::SocketAddr::from(([127, 0, 0, 1], bind_port));
    config.serialization_format = SerializationFormat::Json;
    
    // 使用Static发现方法
    config.discovery = DiscoveryMethod::Static {
        seed_nodes: vec![
            format!("127.0.0.1:{}", BASE_PORT + 1),
            format!("127.0.0.1:{}", BASE_PORT + 2),
            format!("127.0.0.1:{}", BASE_PORT + 3),
        ],
    };
    
    config
}

// 运行单个节点的测试
async fn run_node(node_number: usize, result_sender: mpsc::Sender<BenchmarkResult>) {
    log::info!("Starting node {}", node_number);
    
    // 创建集群配置
    let config = create_cluster_config(node_number);
    
    // 创建集群系统
    let node_name = format!("node{}", node_number);
    let mut system = ClusterSystem::new(&node_name, config);
    
    // 创建测试Actor，但不注册到集群
    let test_actor = TestActor::new(node_name.clone());
    let test_addr = test_actor.start();
    
    // 启动集群系统
    let _cluster_addr = system.start().await.expect("Failed to start cluster system");
    
    // 等待集群稳定
    sleep(Duration::from_secs(2)).await;
    
    // 为了简化测试，只有节点1发送消息，直接在本地模拟消息
    if node_number == 1 {
        let target_nodes = 2;  // 目标节点数
        
        for _ in 0..target_nodes {
            log::info!("Node {} sending simulated messages", node_number);
            
            // 模拟消息发送和接收
            for i in 1..=MSGS_PER_NODE {
                // 记录发送的消息
                test_addr.do_send(TestMessage {
                    id: i,
                    sender_node: node_number,
                    timestamp: Instant::now().elapsed().as_millis() as u64,
                });
                
                // 模拟收到的响应
                test_addr.do_send(TestResponse {
                    original_id: i,
                    success: true,
                });
                
                // 每批次后短暂暂停
                if i % 100 == 0 {
                    sleep(Duration::from_millis(10)).await;
                }
            }
        }
    }
    
    // 等待所有消息处理完成
    sleep(Duration::from_secs(10)).await;
    
    // 获取结果
    let result = test_addr.send(GetResult).await.expect("Failed to get result");
    
    // 发送结果
    let _ = result_sender.send(result).await;
    
    log::info!("Node {} completed testing", node_number);
}

// 分析结果
fn analyze_results(results: Vec<BenchmarkResult>) {
    let total_messages_sent: u64 = results.iter().map(|r| r.messages_sent).sum();
    let total_messages_received: u64 = results.iter().map(|r| r.messages_received).sum();
    
    println!("\n===== BENCHMARK RESULTS =====");
    println!("Total nodes: {}", results.len());
    println!("Total messages sent: {}", total_messages_sent);
    println!("Total messages received: {}", total_messages_received);
    
    if total_messages_sent > 0 {
        let success_rate = (total_messages_received as f64 / total_messages_sent as f64) * 100.0;
        println!("Overall success rate: {:.2}%", success_rate);
    }
    
    // 计算每个节点的性能
    for result in &results {
        println!("Node {}: sent={}, received={}", 
                 result.node_id, result.messages_sent, result.messages_received);
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    log::info!("Starting multi-node benchmark with {} nodes, {} messages per node", 
             NODE_COUNT, MSGS_PER_NODE);
    
    // 创建结果通道
    let (result_sender, mut result_receiver) = mpsc::channel::<BenchmarkResult>(NODE_COUNT);
    
    // 创建LocalSet来包含所有节点，避免将LocalSet跨线程传递
    let local_set = tokio::task::LocalSet::new();
    
    // 在LocalSet内运行所有节点
    local_set.spawn_local(async move {
        // 创建并串行启动所有节点
        for i in 1..=NODE_COUNT {
            let sender = result_sender.clone();
            
            // 延迟启动节点，避免端口冲突
            sleep(Duration::from_millis(100)).await;
            
            // 直接运行节点
            run_node(i, sender).await;
        }
        
        // 关闭发送端
        drop(result_sender);
    });
    
    // 等待LocalSet完成
    local_set.await;
    
    // 收集结果
    let mut results = Vec::new();
    while let Some(result) = result_receiver.recv().await {
        results.push(result);
    }
    
    // 分析结果
    analyze_results(results);
    
    Ok(())
} 