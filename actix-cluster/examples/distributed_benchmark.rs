use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::collections::HashSet;

use actix::prelude::*;
use actix_cluster::{
    ClusterSystem, ClusterConfig, Architecture, NodeRole, 
    SerializationFormat, NodeId, AnyMessage
};
use log::{info, warn, error};
use serde::{Serialize, Deserialize};

// 简单的测试消息
#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "TestResponse")]
struct TestMessage {
    id: u64,
    timestamp: u64,
    payload: Vec<u8>,
    sender_node: String,
}

// 响应消息
#[derive(MessageResponse, Serialize, Deserialize, Clone)]
struct TestResponse {
    id: u64,
    original_timestamp: u64,
    response_timestamp: u64,
}

// 测试Actor
#[derive(Debug)]
struct TestActor {
    id: String,
    messages_received: u64,
    start_time: Instant,
}

impl TestActor {
    fn new(id: String) -> Self {
        Self {
            id,
            messages_received: 0,
            start_time: Instant::now(),
        }
    }
    
    // 获取当前时间戳（微秒）
    fn now_micros() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_micros() as u64
    }
}

impl Actor for TestActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("TestActor {} started", self.id);
    }
}

impl Handler<TestMessage> for TestActor {
    type Result = TestResponse;
    
    fn handle(&mut self, msg: TestMessage, _: &mut Self::Context) -> Self::Result {
        self.messages_received += 1;
        
        if self.messages_received % 100 == 0 {
            info!("Actor {} processed {} messages", self.id, self.messages_received);
        }
        
        // 创建响应
        TestResponse {
            id: msg.id,
            original_timestamp: msg.timestamp,
            response_timestamp: TestActor::now_micros(),
        }
    }
}

// 实现AnyMessage的Handler，使其能够被注册到集群
impl Handler<AnyMessage> for TestActor {
    type Result = ();
    
    fn handle(&mut self, _: AnyMessage, _: &mut Self::Context) -> Self::Result {
        // 处理通用消息
        self.messages_received += 1;
        info!("Received AnyMessage");
    }
}

// 节点信息信息
#[derive(Message)]
#[rtype(result = "Vec<String>")]
struct GetLocalActors;

// 节点信息查询
#[derive(Message)]
#[rtype(result = "Vec<String>")]
struct GetClusterNodes;

// 协调Actor
struct CoordinatorActor {
    node_id: String,
}

impl CoordinatorActor {
    fn new(node_id: String) -> Self {
        Self { node_id }
    }
}

impl Actor for CoordinatorActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _: &mut Self::Context) {
        info!("CoordinatorActor started on node {}", self.node_id);
    }
}

impl Handler<GetClusterNodes> for CoordinatorActor {
    type Result = Vec<String>;
    
    fn handle(&mut self, _: GetClusterNodes, _: &mut Self::Context) -> Self::Result {
        // 此处简化为返回空列表，实际应该查询集群
        Vec::new()
    }
}

// 启动分布式节点
async fn start_node(node_id: String, addr: SocketAddr, seed_nodes: Vec<String>, is_master: bool) {
    // 创建集群配置
    let mut config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(addr)
        .cluster_name("benchmark-cluster".to_string())
        .serialization_format(SerializationFormat::Bincode);
    
    // 添加种子节点
    if !seed_nodes.is_empty() {
        config = config.seed_nodes(seed_nodes.clone());
    }
    
    let config = config.build().expect("无法创建集群配置");
    
    // 创建集群系统
    let mut sys = ClusterSystem::new(&node_id, config);
    
    // 启动集群
    sys.start().await.expect("无法启动集群");
    
    info!("节点 {} 已启动，地址：{}", node_id, addr);
    
    // 创建测试Actor
    let actor = TestActor::new(node_id.clone());
    let addr = actor.start();
    
    // 注册到集群
    let actor_path = format!("/test/actor/{}", node_id);
    info!("注册Actor: {}", actor_path);
    
    // 尝试注册Actor
    match sys.register(&actor_path, addr.clone()).await {
        Ok(_) => info!("成功注册Actor: {}", actor_path),
        Err(e) => {
            error!("无法注册Actor: {}", e);
            return;
        }
    }
    
    // 等待集群初始化
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    if is_master {
        info!("主节点已准备就绪，等待其他节点加入");
        
        // 等待更多节点加入
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // 开始测试
        info!("开始分布式基准测试");
        
        // 发送测试消息
        let start = Instant::now();
        let message_count = 1000;
        let message_size = 1024;
        
        info!("发送 {} 条消息，每条大小 {} 字节", message_count, message_size);
        
        let mut successful = 0;
        let mut failed = 0;
        let mut total_latency = 0;
        
        // 查找种子节点上的Actor
        let mut remote_actors = Vec::new();
        if !seed_nodes.is_empty() {
            for seed in seed_nodes.iter() {
                // 假设每个种子节点都有一个Actor
                let addr_parts: Vec<&str> = seed.split(':').collect();
                if addr_parts.len() == 2 {
                    let host = addr_parts[0];
                    let path = format!("/test/actor/{}", host);
                    remote_actors.push(path);
                }
            }
        }
        
        if remote_actors.is_empty() {
            warn!("没有找到远程Actor，将测试本地通信");
            // 使用本地Actor作为测试
            remote_actors.push(actor_path);
        } else {
            info!("找到 {} 个远程Actor", remote_actors.len());
        }
        
        // 生成消息并发送
        for i in 0..message_count {
            let msg = TestMessage {
                id: i,
                timestamp: TestActor::now_micros(),
                payload: vec![0; message_size],
                sender_node: node_id.clone(),
            };
            
            // 选择一个远程Actor
            let target_idx = (i as usize) % remote_actors.len();
            let target_path = &remote_actors[target_idx];
            
            // 通过本地Actor发送消息
            let send_result = addr.send(msg.clone()).await;
            match send_result {
                Ok(_) => {
                    successful += 1;
                    let now = TestActor::now_micros();
                    let latency = now - msg.timestamp;
                    total_latency += latency;
                }
                Err(e) => {
                    failed += 1;
                    warn!("发送消息失败: {}", e);
                }
            }
            
            // 每发送100条消息记录一次
            if i % 100 == 0 {
                info!("已发送 {} 条消息，成功: {}, 失败: {}", i, successful, failed);
            }
            
            // 延迟一小段时间避免消息风暴
            if i % 10 == 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
        
        let elapsed = start.elapsed();
        info!("测试完成，耗时: {:?}", elapsed);
        info!("成功消息: {}, 失败消息: {}", successful, failed);
        
        if successful > 0 {
            let avg_latency = total_latency / successful;
            let throughput = successful as f64 / elapsed.as_secs_f64();
            
            info!("平均延迟: {} 微秒", avg_latency);
            info!("吞吐量: {:.2} 消息/秒", throughput);
        }
    } else {
        info!("工作节点已准备就绪，等待消息");
    }
    
    // 保持节点运行
    tokio::signal::ctrl_c().await.ok();
    info!("关闭节点 {}", node_id);
}

// 主函数
#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // 解析命令行参数 (简化版，仅支持基本的参数)
    let args: Vec<String> = std::env::args().collect();
    
    let mut node_id = "node1".to_string();
    let mut addr = "127.0.0.1:8080".parse::<SocketAddr>().unwrap();
    let mut is_master = false;
    let mut seed_nodes = Vec::new();
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--id" | "-i" => {
                if i + 1 < args.len() {
                    node_id = args[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--address" | "-a" => {
                if i + 1 < args.len() {
                    addr = args[i + 1].parse().unwrap_or(addr);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--master" | "-m" => {
                is_master = true;
                i += 1;
            }
            "--seed" | "-s" => {
                if i + 1 < args.len() {
                    seed_nodes.push(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    
    info!("启动节点 {}, 地址: {}, 主节点: {}", node_id, addr, is_master);
    if !seed_nodes.is_empty() {
        info!("种子节点: {:?}", seed_nodes);
    }
    
    // 启动集群节点
    start_node(node_id, addr, seed_nodes, is_master).await;
    
    Ok(())
} 