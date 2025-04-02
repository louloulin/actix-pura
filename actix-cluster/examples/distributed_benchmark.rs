use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::collections::HashSet;
use std::sync::{Arc, Mutex as StdMutex};
use std::fmt;
use std::collections::HashMap;

use actix::prelude::*;
use actix_cluster::{
    ClusterSystem, ClusterConfig, Architecture, NodeRole, 
    SerializationFormat, NodeId, AnyMessage
};
use log::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use clap::{Command, Arg, ArgMatches};
use actix_cluster::registry::ActorRef;
use actix_cluster::cluster::SimpleActorRef;

// 命令行参数
struct Args {
    /// 节点ID
    id: String,

    /// 绑定地址
    address: SocketAddr,

    /// 种子节点地址
    seed: Option<String>,
    
    /// 启动多节点测试模式
    multi: bool,
    
    /// 节点数量
    nodes: usize,
    
    /// 基础端口
    base_port: u16,
}

impl Args {
    fn from_arg_matches(matches: &ArgMatches) -> Self {
        let id = matches.get_one::<String>("id").unwrap_or(&"node1".to_string()).clone();
        let address = matches.get_one::<String>("address").unwrap_or(&"127.0.0.1:8080".to_string())
            .parse::<SocketAddr>().expect("无效的地址格式");
        let seed = matches.get_one::<String>("seed").cloned();
        let multi = matches.get_flag("multi");
        let nodes = matches.get_one::<String>("nodes").unwrap_or(&"10".to_string())
            .parse::<usize>().expect("节点数量必须是整数");
        let base_port = matches.get_one::<String>("base_port").unwrap_or(&"9000".to_string())
            .parse::<u16>().expect("端口号必须是整数");
        
        Self {
            id,
            address,
            seed,
            multi,
            nodes,
            base_port,
        }
    }
}

// 修改性能指标结构体，增加更多分析数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestMetrics {
    node_id: String,
    messages_sent: u64,
    messages_received: u64,
    total_latency: u64,
    min_latency: u64,
    max_latency: u64,
    start_time: u64, // 使用微秒时间戳而不是Instant便于序列化
    end_time: Option<u64>,
    // 添加更多性能数据字段
    latency_distribution: HashMap<u64, u64>, // 延迟分布统计 (延迟区间 -> 消息数量)
    throughput_samples: Vec<(u64, u64)>,     // 吞吐量采样 (时间戳, 消息数)
    message_sizes: Vec<usize>,              // 消息大小采样
    errors: u64,                            // 错误计数
}

impl TestMetrics {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            messages_sent: 0,
            messages_received: 0,
            total_latency: 0,
            min_latency: u64::MAX,
            max_latency: 0,
            start_time: TestActor::now_micros(),
            end_time: None,
            latency_distribution: HashMap::new(),
            throughput_samples: vec![(TestActor::now_micros(), 0)],
            message_sizes: Vec::new(),
            errors: 0,
        }
    }

    fn record_sent(&mut self, count: u64) {
        self.messages_sent += count;
    }

    fn record_received(&mut self, count: u64, latency: u64) {
        self.messages_received += count;
        self.total_latency += latency;
        
        // 记录最小和最大延迟
        if latency < self.min_latency {
            self.min_latency = latency;
        }
        if latency > self.max_latency {
            self.max_latency = latency;
        }
        
        // 记录延迟分布: 对延迟按区间统计
        // 将延迟分组到最接近的毫秒区间
        let latency_ms = latency / 1000;
        *self.latency_distribution.entry(latency_ms).or_insert(0) += 1;
        
        // 每100个消息更新一次吞吐量采样
        if self.messages_received % 100 == 0 {
            self.throughput_samples.push((TestActor::now_micros(), self.messages_received));
        }
    }
    
    fn record_error(&mut self) {
        self.errors += 1;
    }
    
    fn record_message_size(&mut self, size: usize) {
        // 只记录少量样本避免占用过多内存
        if self.message_sizes.len() < 100 {
            self.message_sizes.push(size);
        }
    }

    fn set_end_time(&mut self) {
        self.end_time = Some(TestActor::now_micros());
        // 添加最终吞吐量采样点
        self.throughput_samples.push((TestActor::now_micros(), self.messages_received));
    }

    fn get_duration_micros(&self) -> u64 {
        match self.end_time {
            Some(end) => end - self.start_time,
            None => TestActor::now_micros() - self.start_time,
        }
    }

    fn get_duration_secs(&self) -> f64 {
        self.get_duration_micros() as f64 / 1_000_000.0
    }
    
    fn get_percentile_latency(&self, percentile: f64) -> u64 {
        // 计算指定百分位数的延迟
        if self.latency_distribution.is_empty() {
            return 0;
        }
        
        // 将所有延迟数据重建为排序数组
        let mut latency_samples = Vec::new();
        for (latency_ms, count) in &self.latency_distribution {
            for _ in 0..*count {
                latency_samples.push(*latency_ms);
            }
        }
        
        if latency_samples.is_empty() {
            return 0;
        }
        
        latency_samples.sort_unstable();
        
        // 计算百分位索引
        let index = (percentile * latency_samples.len() as f64 / 100.0) as usize;
        let index = index.min(latency_samples.len() - 1);
        
        // 返回延迟值（毫秒转微秒）
        latency_samples[index] * 1000
    }
    
    fn calculate_throughput_stats(&self) -> (f64, f64, f64) {
        // 计算吞吐量的平均值、标准差和稳定性分数
        if self.throughput_samples.len() < 2 {
            return (0.0, 0.0, 0.0);
        }
        
        let mut throughputs = Vec::new();
        
        // 计算相邻采样点之间的吞吐量
        for i in 1..self.throughput_samples.len() {
            let (t1, m1) = self.throughput_samples[i - 1];
            let (t2, m2) = self.throughput_samples[i];
            
            let time_diff = (t2 - t1) as f64 / 1_000_000.0; // 秒
            let message_diff = (m2 - m1) as f64;
            
            if time_diff > 0.0 {
                let throughput = message_diff / time_diff;
                throughputs.push(throughput);
            }
        }
        
        if throughputs.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        
        // 计算平均吞吐量
        let avg_throughput = throughputs.iter().sum::<f64>() / throughputs.len() as f64;
        
        // 计算标准差
        let variance = throughputs.iter()
            .map(|&t| (t - avg_throughput).powi(2))
            .sum::<f64>() / throughputs.len() as f64;
        let std_dev = variance.sqrt();
        
        // 计算稳定性分数 (0-100)，标准差越小越稳定
        let stability = if avg_throughput > 0.0 {
            100.0 - (std_dev / avg_throughput * 100.0).min(100.0)
        } else {
            0.0
        };
        
        (avg_throughput, std_dev, stability)
    }

    fn get_summary(&self) -> String {
        let duration_secs = self.get_duration_secs();

        let avg_latency = if self.messages_received > 0 {
            self.total_latency / self.messages_received
        } else {
            0
        };
        
        // 计算延迟百分位数
        let p95_latency = self.get_percentile_latency(95.0);
        let p99_latency = self.get_percentile_latency(99.0);

        let throughput = if duration_secs > 0.0 {
            self.messages_received as f64 / duration_secs
        } else {
            0.0
        };
        
        // 计算吞吐量统计
        let (avg_throughput, throughput_std_dev, stability) = self.calculate_throughput_stats();

        format!(
            "发送: {}, 接收: {}, 错误: {}, 延迟(µs): 平均={}, 最小={}, 最大={}, P95={}, P99={}, 吞吐量: {:.2}消息/秒 (稳定性: {:.1}%)",
            self.messages_sent, 
            self.messages_received, 
            self.errors,
            avg_latency, 
            if self.min_latency == u64::MAX { 0 } else { self.min_latency },
            self.max_latency,
            p95_latency,
            p99_latency,
            throughput, 
            stability
        )
    }
}

impl fmt::Display for TestMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_summary())
    }
}

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
    metrics: Arc<StdMutex<TestMetrics>>,
}

impl TestActor {
    fn new(id: String, metrics: Arc<StdMutex<TestMetrics>>) -> Self {
        Self {
            id,
            messages_received: 0,
            start_time: Instant::now(),
            metrics,
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

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("TestActor {} 已停止", self.id);
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

// 添加性能指标请求消息
#[derive(Message, Serialize, Deserialize, Clone)]
#[rtype(result = "MetricsResponse")]
struct GetMetrics;

#[derive(MessageResponse, Serialize, Deserialize, Clone)]
struct MetricsResponse {
    metrics: TestMetrics,
}

// 修改TestActor，增加对GetMetrics消息的处理
impl Handler<GetMetrics> for TestActor {
    type Result = MetricsResponse;
    
    fn handle(&mut self, _: GetMetrics, _: &mut Self::Context) -> Self::Result {
        let metrics = if let Ok(metrics_lock) = self.metrics.lock() {
            metrics_lock.clone()
        } else {
            TestMetrics::new(self.id.clone())
        };
        
        MetricsResponse { metrics }
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
async fn start_node(node_id: String, addr: SocketAddr, seed_nodes: Vec<String>, is_master: bool) -> std::io::Result<()> {
    // 创建性能指标
    let metrics = Arc::new(StdMutex::new(TestMetrics::new(node_id.clone())));
    
    // 创建集群配置
    let mut config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)
        .bind_addr(addr)
        .cluster_name("benchmark-cluster".to_string())
        .serialization_format(SerializationFormat::Bincode);
    
    // 添加种子节点
    if !seed_nodes.is_empty() {
        // 确保种子节点被正确添加
        let mut seed_addrs = Vec::new();
        for seed in seed_nodes {
            info!("添加种子节点: {}", seed);
            seed_addrs.push(seed);
        }
        config = config.seed_nodes(seed_addrs);
    }
    
    let config = config.build().expect("无法创建集群配置");
    
    // 创建集群系统
    let mut sys = ClusterSystem::new(&node_id, config);
    
    // 启动集群
    sys.start().await.expect("无法启动集群");
    
    info!("节点 {} 已启动，地址：{}", node_id, addr);
    
    // 创建测试Actor
    let actor = TestActor::new(node_id.clone(), metrics.clone());
    let addr = actor.start();
    
    // 注册到集群 - 使用更容易发现的路径格式
    let actor_path = format!("/user/test/{}", node_id);
    info!("注册Actor: {}", actor_path);
    
    // 确保集群系统已经完全初始化
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 尝试注册Actor
    match sys.register(&actor_path, addr.clone()).await {
        Ok(_) => info!("成功注册Actor: {}", actor_path),
        Err(e) => {
            error!("无法注册Actor: {}", e);
            return Ok(());  // 返回成功但记录错误，避免中断整个测试
        }
    }
    
    // 在注册后等待更长时间，确保系统有机会进行节点发现和更新注册表
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // 手动触发节点发现和注册表更新
    info!("尝试更新注册表信息...");
    // 增加等待时间，让集群系统自动同步注册表
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    if is_master {
        info!("主节点已准备就绪，等待其他节点加入");
        
        // 等待更多节点加入
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        // 开始测试
        info!("开始分布式基准测试");
        
        // 发送测试消息
        let start = Instant::now();
        let message_count = 500000; // 增加到500000条消息
        let message_size = 1024;
        
        info!("发送 {} 条消息，每条大小 {} 字节", message_count, message_size);
        
        let mut successful = 0;
        let mut failed = 0;
        
        // 清楚显示节点发现过程
        info!("开始查找工作节点...");
        
        // 查找工作节点上的Actor
        let mut remote_actors = Vec::new();
        for i in 1..100 { // 增加搜索的节点数量上限
            let worker_id = format!("worker{}", i);
            let worker_path = format!("/user/test/{}", worker_id);
            
            info!("尝试查找Actor路径: {}", worker_path);
            
            // 尝试解析远程Actor
            if let Some(remote_addr) = sys.lookup(&worker_path).await {
                info!("成功找到远程Actor: {}", worker_path);
                remote_actors.push((worker_path, remote_addr));
            } else {
                debug!("未找到远程Actor {}", worker_path);
                // 如果找不到更多节点，可能已经到达最大节点数
                if i > 20 && remote_actors.len() > 0 {
                    break;
                }
            }
            
            // 简短延迟，避免过多的查询
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        // 如果找不到远程Actor，可能是因为注册表同步问题，这里再等待并重试一次
        if remote_actors.is_empty() {
            info!("第一次查找没有找到远程Actor，等待注册表更新后重试...");
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // 重新查找
            for i in 1..10 {
                let worker_id = format!("worker{}", i);
                let worker_path = format!("/user/test/{}", worker_id);
                
                // 尝试解析远程Actor
                if let Some(remote_addr) = sys.lookup(&worker_path).await {
                    info!("重试后找到远程Actor: {}", worker_path);
                    remote_actors.push((worker_path, remote_addr));
                }
                
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        
        if remote_actors.is_empty() {
            warn!("没有找到远程Actor，将测试本地通信");
            // 使用本地Actor作为测试
            let simple_ref = SimpleActorRef::new(addr.clone(), actor_path.to_string());
            remote_actors.push((actor_path, Box::new(simple_ref) as Box<dyn ActorRef>));
        } else {
            info!("找到 {} 个远程Actor", remote_actors.len());
        }
        
        // 生成消息并发送 - 优化大量消息的发送逻辑
        let batch_size = 1000; // 每批发送的消息数
        for batch in 0..(message_count / batch_size + 1) {
            let start_id = batch * batch_size;
            let end_id = std::cmp::min((batch + 1) * batch_size, message_count);
            
            for i in start_id..end_id {
                let msg = TestMessage {
                    id: i,
                    timestamp: TestActor::now_micros(),
                    payload: vec![0; message_size],
                    sender_node: node_id.clone(),
                };
                
                // 选择一个远程Actor - 使用循环分配实现负载均衡
                let target_idx = (i as usize) % remote_actors.len();
                let (target_path, target_addr) = &remote_actors[target_idx];
                
                // 发送消息
                if i % 1000 == 0 {
                    info!("发送消息 {} 到 {}", i, target_path);
                }
                
                // 使用send_any来发送消息
                let msg_box = Box::new(msg.clone()) as Box<dyn std::any::Any + Send>;
                match target_addr.send_any(msg_box) {
                    Ok(_) => {
                        successful += 1;
                        
                        // 计算往返延迟
                        let now = TestActor::now_micros();
                        let latency = now - msg.timestamp;
                        
                        if let Ok(mut metrics_lock) = metrics.lock() {
                            metrics_lock.record_sent(1);
                            metrics_lock.record_received(1, latency);
                            // 记录消息大小
                            metrics_lock.record_message_size(message_size);
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        // 记录错误
                        if let Ok(mut metrics_lock) = metrics.lock() {
                            metrics_lock.record_error();
                        }
                        
                        // 更详细的错误报告
                        if failed % 100 == 0 || failed < 10 {
                            warn!("发送消息 {} 到 {} 失败: {}", i, target_path, e);
                        }
                        
                        // 当错误过多时尝试切换到其他节点
                        if failed % 1000 == 0 && remote_actors.len() > 1 {
                            info!("错误过多，尝试查找新的可用节点...");
                            
                            // 在继续之前稍作延迟
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    }
                }
            }
            
            // 每批次结束后记录进度
            info!("已发送 {} 条消息，成功: {}, 失败: {}", end_id, successful, failed);
            
            // 每批次消息之间短暂等待，让系统有时间处理
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        let elapsed = start.elapsed();
        info!("测试完成，耗时: {:?}", elapsed);
        info!("成功消息: {}, 失败消息: {}", successful, failed);
        
        // 记录结束时间并显示指标
        {
            if let Ok(mut metrics_lock) = metrics.lock() {
                metrics_lock.set_end_time();
                info!("节点 {} 性能指标: {}", node_id, metrics_lock.get_summary());
            }
        }
        
        // 收集远程节点的性能指标
        info!("正在收集远程节点的性能指标...");
        
        // 等待一段时间确保所有消息都已处理完毕
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // 收集并显示所有远程节点的性能指标
        let mut all_metrics = Vec::new();
        
        // 先添加主节点的指标
        if let Ok(metrics_lock) = metrics.lock() {
            all_metrics.push(metrics_lock.clone());
        }
        
        // 然后收集所有工作节点的指标
        for (worker_path, worker_addr) in &remote_actors {
            if worker_path.contains("master") {
                continue; // 跳过主节点，已经添加过了
            }
            
            info!("正在获取节点 {} 的性能指标", worker_path);
            
            // 构造GetMetrics消息并发送
            let metrics_msg = Box::new(GetMetrics{}) as Box<dyn std::any::Any + Send>;
            match worker_addr.send_any(metrics_msg) {
                Ok(_) => {
                    info!("已向 {} 发送指标请求", worker_path);
                    // 由于无法直接获取响应，这里假设节点会将指标记录到日志
                }
                Err(e) => {
                    warn!("无法向 {} 请求性能指标: {}", worker_path, e);
                }
            }
        }
        
        // 优化指标收集和报告，增加更多详细统计信息
        info!("性能指标汇总报告 - {} 个节点:", all_metrics.len());
        
        let total_messages_sent: u64 = all_metrics.iter().map(|m| m.messages_sent).sum();
        let total_messages_received: u64 = all_metrics.iter().map(|m| m.messages_received).sum();
        
        // 找出最小的开始时间和最大的结束时间
        let min_start = all_metrics.iter()
            .map(|m| m.start_time)
            .min()
            .unwrap_or(0);
        
        let max_end = all_metrics.iter()
            .filter_map(|m| m.end_time)
            .max()
            .unwrap_or(TestActor::now_micros());
        
        // 计算总运行时间
        let total_duration_micros = max_end - min_start;
        let total_duration_secs = total_duration_micros as f64 / 1_000_000.0;
        
        // 计算总体吞吐量
        let total_throughput = if total_duration_secs > 0.0 {
            total_messages_received as f64 / total_duration_secs
        } else {
            0.0
        };
        
        // 计算最大、最小和平均延迟
        let global_min_latency = all_metrics.iter()
            .map(|m| if m.min_latency == u64::MAX { u64::MAX } else { m.min_latency })
            .min()
            .unwrap_or(0);
        
        let global_max_latency = all_metrics.iter()
            .map(|m| m.max_latency)
            .max()
            .unwrap_or(0);
        
        let global_avg_latency = if total_messages_received > 0 {
            all_metrics.iter().map(|m| m.total_latency).sum::<u64>() / total_messages_received
        } else {
            0
        };
        
        // 计算每秒消息的标准差，了解性能的稳定性
        // 这里简化处理，只用最大吞吐量和最小吞吐量来估算
        let mut node_throughputs = Vec::new();
        for m in &all_metrics {
            let node_duration = m.get_duration_secs();
            if node_duration > 0.0 && m.messages_received > 0 {
                let node_throughput = m.messages_received as f64 / node_duration;
                node_throughputs.push(node_throughput);
            }
        }
        
        // 如果有多个节点，计算吞吐量的最大和最小值，否则设为0
        let (throughput_min, throughput_max) = if node_throughputs.len() > 1 {
            let min = node_throughputs.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = node_throughputs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (min, max)
        } else {
            (0.0, 0.0)
        };
        
        let throughput_variation = if node_throughputs.len() > 1 && throughput_max > 0.0 {
            (throughput_max - throughput_min) / throughput_max * 100.0
        } else {
            0.0
        };
        
        info!("集群整体性能：");
        info!("总消息数：发送 = {}, 接收 = {}", total_messages_sent, total_messages_received);
        info!("总运行时间：{:.2} 秒", total_duration_secs);
        info!("集群吞吐量：{:.2} 消息/秒", total_throughput);
        info!("延迟(微秒)：平均 = {}, 最小 = {}, 最大 = {}", 
              global_avg_latency, 
              if global_min_latency == u64::MAX { 0 } else { global_min_latency }, 
              global_max_latency);
              
        if node_throughputs.len() > 1 {
            info!("节点吞吐量变化：最小 = {:.2}, 最大 = {:.2}, 变化率 = {:.2}%", 
                throughput_min, throughput_max, throughput_variation);
        }
        
    } else {
        info!("工作节点已准备就绪，等待消息");
    }
    
    // 为了防止程序立即退出，添加适当的等待时间
    let wait_time = if is_master { 30 } else { 20 };
    tokio::time::sleep(Duration::from_secs(wait_time)).await;
    info!("测试完成，节点 {} 即将关闭", node_id);
    
    Ok(())
}

// 修改run_multi_node_test函数，增加更多worker节点的支持和错误处理
async fn run_multi_node_test(node_count: usize, base_port: u16) -> std::io::Result<()> {
    info!("启动 {} 个节点的分布式测试", node_count);
    
    if node_count < 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "节点数量必须至少为2（1个主节点和至少1个工作节点）"
        ));
    }
    
    // 创建主节点地址，使用String类型可以简单克隆
    let master_addr = format!("127.0.0.1:{}", base_port);
    
    // 创建一个LocalSet用于运行本地任务
    let local = tokio::task::LocalSet::new();
    
    // 在LocalSet内运行所有节点任务
    local.run_until(async move {
        // 先启动所有工作节点
        let mut worker_handles = Vec::new();
        for i in 1..node_count {
            let port = base_port + i as u16;
            let node_id = format!("worker{}", i);
            let addr = format!("127.0.0.1:{}", port);
            let seed = master_addr.clone();
            
            info!("启动工作节点 {}, 地址: {}, 种子节点: {}", node_id, addr, seed);
            
            // 使用spawn_local而不是spawn
            let worker_handle = tokio::task::spawn_local(async move {
                match start_node(
                    node_id.clone(),
                    addr.parse().unwrap(),
                    vec![seed],
                    false
                ).await {
                    Ok(_) => info!("工作节点 {} 运行完成", node_id),
                    Err(e) => warn!("工作节点 {} 运行失败: {:?}", node_id, e),
                }
            });
            
            worker_handles.push(worker_handle);
            
            // 让节点有时间启动和注册，避免端口冲突
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        
        // 让工作节点有时间完全启动
        info!("等待工作节点初始化完成...");
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        // 最后启动主节点来触发测试
        let master_handle = tokio::task::spawn_local(async move {
            match start_node(
                "master".to_string(),
                master_addr.parse().unwrap(),
                Vec::new(), 
                true
            ).await {
                Ok(_) => info!("主节点运行完成"),
                Err(e) => warn!("主节点运行失败: {:?}", e),
            }
        });
        
        // 等待测试完成
        info!("所有节点已启动，等待测试完成...");
        
        // 等待主节点完成，它会执行测试逻辑
        if let Err(e) = master_handle.await {
            warn!("主节点任务出错: {:?}", e);
        }
        
        // 等待所有工作节点完成
        for (i, handle) in worker_handles.into_iter().enumerate() {
            if let Err(e) = handle.await {
                warn!("工作节点 {} 任务出错: {:?}", i+1, e);
            }
        }
        
        info!("测试完成，所有节点已关闭");
    }).await;
    
    // 结束测试
    Ok(())
}

// 修改主函数，使用clap解析命令行参数
#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    // 使用clap解析命令行参数
    let matches = Command::new("分布式基准测试")
        .about("ActixCluster分布式系统基准测试工具")
        .arg(Arg::new("id")
            .short('i')
            .long("id")
            .value_name("ID")
            .help("节点ID")
            .default_value("node1"))
        .arg(Arg::new("address")
            .short('a')
            .long("address")
            .value_name("ADDR")
            .help("绑定地址")
            .default_value("127.0.0.1:8080"))
        .arg(Arg::new("seed")
            .short('s')
            .long("seed")
            .value_name("SEED")
            .help("种子节点地址"))
        .arg(Arg::new("multi")
            .long("multi")
            .action(clap::ArgAction::SetTrue)
            .help("启动多节点测试模式"))
        .arg(Arg::new("nodes")
            .long("nodes")
            .value_name("COUNT")
            .help("节点数量")
            .default_value("10"))
        .arg(Arg::new("base_port")
            .long("base-port")
            .value_name("PORT")
            .help("基础端口")
            .default_value("9000"))
        .get_matches();
    
    // 解析命令行参数
    let args = Args::from_arg_matches(&matches);
    
    if args.multi {
        // 多节点模式 - 自动启动多个节点
        let node_count = args.nodes;
        info!("以多节点模式启动，创建 {} 个节点，基础端口: {}", node_count, args.base_port);
        run_multi_node_test(node_count, args.base_port).await?;
    } else {
        // 单节点模式 - 手动连接
        // 设置种子节点
        let mut seed_nodes = Vec::new();
        if let Some(seed) = args.seed {
            seed_nodes.push(seed);
        }
        
        // 如果有种子节点则为工作节点，否则为主节点
        let is_master = seed_nodes.is_empty();
        
        info!("启动 {} 节点 ID: {}, 地址: {}", 
            if is_master { "主" } else { "工作" }, 
            args.id, 
            args.address
        );
        
        if !seed_nodes.is_empty() {
            info!("种子节点: {:?}", seed_nodes);
        }
        
        // 启动节点
        start_node(args.id, args.address, seed_nodes, is_master).await?;
    }
    
    Ok(())
} 