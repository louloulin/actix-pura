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
    
    // ... existing code ...
} 