use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex as StdMutex};
use structopt::StructOpt;

use actix::prelude::*;
use actix_cluster::{
    ClusterSystem, ClusterConfig, Architecture, NodeRole,
    SerializationFormat, NodeId, AnyMessage, ActorRef
};
use actix_cluster::cluster::SimpleActorRef;
use log::{info, warn, error, debug};

// TestMetrics 结构体
struct TestMetrics {
    node_id: String,
    messages_processed: usize,
    processing_time_ms: u64,
}

impl TestMetrics {
    fn new(node_id: String) -> Self {
        Self {
            node_id,
            messages_processed: 0,
            processing_time_ms: 0,
        }
    }

    fn record_process(&mut self, time_ms: u64) {
        self.messages_processed += 1;
        self.processing_time_ms += time_ms;
    }

    fn avg_process_time(&self) -> f64 {
        if self.messages_processed == 0 {
            return 0.0;
        }
        self.processing_time_ms as f64 / self.messages_processed as f64
    }
}

// TestActor 结构体
struct TestActor {
    node_id: String,
    metrics: Arc<StdMutex<TestMetrics>>,
}

impl TestActor {
    fn new(node_id: String, metrics: Arc<StdMutex<TestMetrics>>) -> Self {
        Self {
            node_id,
            metrics,
        }
    }
}

// TestMessage 消息
#[derive(Message, Clone)]
#[rtype(result = "TestResult")]
struct TestMessage {
    pub data: Vec<u8>,
}

// TestResult 结果
#[derive(Debug)]
struct TestResult {
    pub node_id: String,
    pub processed: bool,
    pub process_time_ms: u64,
}

// Actor 实现
impl Actor for TestActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("TestActor started on node {}", self.node_id);
    }
}

// Message 处理
impl Handler<TestMessage> for TestActor {
    type Result = MessageResult<TestMessage>;

    fn handle(&mut self, msg: TestMessage, _ctx: &mut Self::Context) -> Self::Result {
        // 模拟处理时间根据消息大小
        let start = Instant::now();

        // 简单处理: 对数据进行CRC计算
        let mut sum: u64 = 0;
        for byte in &msg.data {
            sum = sum.wrapping_add(*byte as u64);
        }

        // 记录处理时间
        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;

        // 更新指标
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.record_process(elapsed_ms);
        }

        MessageResult(TestResult {
            node_id: self.node_id.clone(),
            processed: true,
            process_time_ms: elapsed_ms,
        })
    }
}

// Make TestActor handle AnyMessage
impl Handler<AnyMessage> for TestActor {
    type Result = ();

    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(test_msg) = msg.downcast::<TestMessage>() {
            let test_msg_clone = test_msg.clone(); // Clone it
            let result = <TestActor as Handler<TestMessage>>::handle(self, test_msg_clone, ctx);
            // Use a let instead of if let since the pattern is irrefutable
            let MessageResult(test_result) = result;
            info!("Processed message on node {} in {}ms", self.node_id, test_result.process_time_ms);
        } else {
            warn!("Received unknown message type");
        }
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
    let mut sys = ClusterSystem::new(config);

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
    // sys.sync_registry()方法不存在，我们用其他方式来确保节点可以被发现

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
        let message_count = 100000; // 增加到100000条消息
        let message_size = 1024;

        info!("发送 {} 条消息，每条大小 {} 字节", message_count, message_size);

        let mut successful = 0;
        let mut failed = 0;

        // 清楚显示节点发现过程
        info!("开始查找工作节点...");
        // 删除 sys.dump_registry() 调用，因为该方法不存在

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

            // 不使用sync_registry方法，因为它不存在
            // 改为增加延迟，让系统自然地更新注册信息

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

        // ... existing code ...
    } else {
        info!("工作节点已准备就绪，等待消息");
    }

    // 保持节点运行直到收到终止信号
    tokio::signal::ctrl_c().await?;

    Ok(())
}

// Add command line arguments
#[derive(StructOpt, Debug)]
struct Args {
    /// Node ID
    #[structopt(short, long, default_value = "node1")]
    id: String,

    /// Binding address
    #[structopt(short, long, default_value = "127.0.0.1:9021")]
    address: SocketAddr,

    /// Whether to run in multi-node mode
    #[structopt(short, long)]
    multi: bool,

    /// Number of nodes to run in multi-node mode
    #[structopt(short, long, default_value = "2")]
    nodes: usize,

    /// Base port for multi-node mode
    #[structopt(long, default_value = "9021")]
    base_port: u16,
}

// Main function
fn main() -> std::io::Result<()> {
    // Set up logging
    std::env::set_var("RUST_LOG", "info,actix_cluster=debug");
    env_logger::init();

    // Parse command line arguments
    let args = Args::from_args();

    if args.multi {
        // Start multiple nodes in the same process for testing
        start_multi_node_test(args.nodes, args.base_port)
    } else {
        // Start a single node
        let system = System::new();
        system.block_on(async {
            let addr: SocketAddr = args.address;
            let seed_nodes = Vec::new();
            start_node(args.id, addr, seed_nodes, true).await.unwrap();
        });
        Ok(())
    }
}

fn start_multi_node_test(node_count: usize, base_port: u16) -> std::io::Result<()> {
    // Create the master node
    let master_addr = format!("127.0.0.1:{}", base_port);

    // Start the master node in a separate thread
    std::thread::spawn(move || {
        let id = "master".to_string();
        let addr: SocketAddr = master_addr.parse().unwrap();
        let system = System::new();
        system.block_on(async {
            start_node(id, addr, Vec::new(), true).await.unwrap();
        });
    });

    // Wait for the master node to start
    std::thread::sleep(Duration::from_secs(2));

    // Start worker nodes
    for i in 1..node_count {
        let port = base_port + (i as u16);
        let seed = vec![format!("127.0.0.1:{}", base_port)];

        std::thread::spawn(move || {
            let id = format!("worker{}", i);
            let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            let system = System::new();
            system.block_on(async {
                start_node(id, addr, seed, false).await.unwrap();
            });
        });

        // Add a small delay between starting nodes
        std::thread::sleep(Duration::from_millis(500));
    }

    // Wait for Ctrl+C to terminate
    println!("Press Ctrl+C to terminate the benchmark");
    let (tx, rx) = std::sync::mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    }).expect("Error setting Ctrl-C handler");

    // Block until Ctrl+C is pressed
    let _ = rx.recv();

    Ok(())
}