
# 基于actix-cluster和Raft构建股票交易OMS系统设计

基于我们对actix-cluster的修复和观察，我设计一个分布式高可用的股票交易订单管理系统(OMS)架构：

## 系统架构

```
┌────────────────────────────────────────────────────┐
│                  接入层 (Gateway)                   │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌────────┐ │
│  │ HTTP API│  │  FIX    │  │ Binary  │  │  gRPC  │ │
│  └─────────┘  └─────────┘  └─────────┘  └────────┘ │
└────────────────────────────────────────────────────┘
                        │
┌────────────────────────────────────────────────────┐
│                  OMS核心服务层                      │
│                                                    │
│  ┌───────────────────┐    ┌───────────────────┐    │
│  │   OrderActor集群   │    │   风控Actor集群    │    │
│  └───────────────────┘    └───────────────────┘    │
│                                                    │
│  ┌───────────────────┐    ┌───────────────────┐    │
│  │   账户Actor集群    │    │  执行引擎Actor集群  │    │
│  └───────────────────┘    └───────────────────┘    │
│                                                    │
│  ┌───────────────────┐    ┌───────────────────┐    │
│  │  行情Actor集群     │    │   报表Actor集群    │    │
│  └───────────────────┘    └───────────────────┘    │
└────────────────────────────────────────────────────┘
                        │
┌────────────────────────────────────────────────────┐
│                  Raft一致性层                       │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐             │
│  │ Node 1  │  │ Node 2  │  │ Node 3  │ ...         │
│  └─────────┘  └─────────┘  └─────────┘             │
└────────────────────────────────────────────────────┘
                        │
┌────────────────────────────────────────────────────┐
│                  持久化层                           │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐             │
│  │ 订单数据 │  │ 账户数据 │  │ 交易日志 │ ...         │
│  └─────────┘  └─────────┘  └─────────┘             │
└────────────────────────────────────────────────────┘
```

## 核心组件实现

### 1. 订单Actor (OrderActor)

```rust
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
#[rtype(result = "OrderResult")]
struct OrderRequest {
    order_id: String,
    symbol: String,
    side: OrderSide,
    price: Option<f64>,  // None表示市价单
    quantity: u64,
    client_id: String,
    order_type: OrderType,
}

struct OrderActor {
    node_id: String,
    order_store: Arc<RwLock<HashMap<String, Order>>>,
    raft_client: Arc<RaftClient>,
    execution_engine: Addr<ExecutionActor>,
    risk_manager: Addr<RiskActor>,
}

impl Handler<OrderRequest> for OrderActor {
    type Result = OrderResult;
    
    fn handle(&mut self, msg: OrderRequest, ctx: &mut Self::Context) -> Self::Result {
        // 1. 记录订单到Raft日志确保一致性
        let log_entry = LogEntry::OrderRequest(msg.clone());
        if let Err(e) = self.raft_client.append_log(log_entry) {
            return OrderResult::Error(format!("Failed to append to Raft log: {}", e));
        }
        
        // 2. 风控检查
        if let Err(e) = self.risk_manager.send(RiskCheck::new(msg.clone())).await {
            return OrderResult::Rejected(format!("Risk check failed: {}", e));
        }
        
        // 3. 创建订单对象并存储
        let order = Order::from_request(msg.clone());
        {
            let mut orders = self.order_store.write().unwrap();
            orders.insert(order.order_id.clone(), order.clone());
        }
        
        // 4. 发送到执行引擎
        match self.execution_engine.send(ExecuteOrder::new(order)).await {
            Ok(result) => result,
            Err(e) => OrderResult::Error(format!("Execution error: {}", e))
        }
    }
}

impl Handler<AnyMessage> for OrderActor {
    type Result = ();
    
    fn handle(&mut self, msg: AnyMessage, ctx: &mut Self::Context) -> Self::Result {
        if let Some(order_req) = msg.downcast::<OrderRequest>() {
            let order_req_clone = order_req.clone();
            let _ = self.handle(order_req_clone, ctx);
        } else if let Some(query) = msg.downcast::<OrderQuery>() {
            let query_clone = query.clone();
            let _ = self.handle(query_clone, ctx);
        } else {
            warn!("OrderActor received unknown message type");
        }
    }
}
```

### 2. Raft集成层 (RaftService)

```rust
struct RaftService {
    node_id: NodeId,
    state: Arc<RwLock<RaftState>>,
    log: Arc<RwLock<Vec<LogEntry>>>,
    cluster: Addr<ClusterSystemActor>,
    apply_ch: mpsc::Sender<LogEntry>,
}

impl RaftService {
    async fn start(&self) -> Result<()> {
        // 初始化Raft服务
        info!("Starting Raft service on node {}", self.node_id);
        
        // 开始选举计时器
        self.start_election_timer();
        
        // 订阅集群成员变更事件
        self.cluster.do_send(SubscribeClusterEvents(self.addr.clone()));
        
        Ok(())
    }
    
    async fn append_log(&mut self, entry: LogEntry) -> Result<()> {
        // 如果是Leader，复制日志到其他节点
        let is_leader = {
            let state = self.state.read().unwrap();
            state.current_role == RaftRole::Leader
        };
        
        if is_leader {
            // 添加到本地日志
            {
                let mut log = self.log.write().unwrap();
                log.push(entry.clone());
            }
            
            // 发送AppendEntries RPC到所有follower
            self.replicate_logs().await?;
            
            // 应用到状态机
            self.apply_ch.send(entry).await?;
            
            Ok(())
        } else {
            // 转发请求到Leader
            let leader_id = {
                let state = self.state.read().unwrap();
                state.current_leader.clone()
            };
            
            if let Some(leader) = leader_id {
                // 通过cluster找到Leader Actor并转发
                if let Some(leader_actor) = self.cluster.lookup(&format!("/user/raft/{}", leader)).await {
                    leader_actor.send_any(Box::new(AppendRequest::new(entry)))?;
                    Ok(())
                } else {
                    Err(anyhow!("Leader not found"))
                }
            } else {
                Err(anyhow!("No leader elected yet"))
            }
        }
    }
}
```

### 3. 执行引擎Actor (ExecutionActor)

```rust
struct ExecutionActor {
    node_id: String,
    order_book: HashMap<String, OrderBook>,  // symbol -> order book
    matcher: Arc<OrderMatcher>,
    execution_store: Arc<RwLock<HashMap<String, Execution>>>,
}

impl Handler<ExecuteOrder> for ExecutionActor {
    type Result = OrderResult;
    
    fn handle(&mut self, msg: ExecuteOrder, _: &mut Self::Context) -> Self::Result {
        let order = msg.order;
        
        // 获取或创建订单簿
        let order_book = self.order_book
            .entry(order.symbol.clone())
            .or_insert_with(|| OrderBook::new(order.symbol.clone()));
        
        // 添加订单到订单簿
        order_book.add_order(order.clone());
        
        // 尝试撮合
        let executions = self.matcher.match_orders(order_book);
        
        // 处理执行结果
        for exec in executions {
            // 存储执行结果
            {
                let mut execs = self.execution_store.write().unwrap();
                execs.insert(exec.execution_id.clone(), exec.clone());
            }
            
            // 发送执行通知
            self.send_execution_notifications(&exec);
        }
        
        OrderResult::Accepted(order.order_id)
    }
}
```

### 4. 系统启动与配置

```rust
async fn start_oms_system() -> Result<()> {
    // 解析配置
    let config = Config::from_file("oms_config.yaml")?;
    
    // 初始化日志系统
    init_logger(&config);
    
    // 创建并启动集群系统
    let mut cluster_config = ClusterConfig::new()
        .architecture(Architecture::Decentralized)
        .node_role(NodeRole::Peer)  // 使用Peer角色
        .bind_addr(config.bind_address)
        .cluster_name("oms-cluster")
        .serialization_format(SerializationFormat::Bincode);
    
    // 添加种子节点
    if !config.seed_nodes.is_empty() {
        cluster_config = cluster_config.seed_nodes(config.seed_nodes);
    }
    
    let cluster_config = cluster_config.build()?;
    let mut sys = ClusterSystem::new(&config.node_id, cluster_config);
    let cluster_addr = sys.start().await?;
    
    // 初始化Raft服务
    let (apply_tx, apply_rx) = mpsc::channel(100);
    let raft_service = RaftService::new(
        config.node_id.clone(),
        cluster_addr.clone(),
        apply_tx
    );
    let raft_addr = raft_service.start();
    
    // 启动状态机应用器
    tokio::spawn(run_state_machine(apply_rx));
    
    // 初始化和注册各类Actor
    let risk_actor = RiskActor::new(&config).start();
    let exec_actor = ExecutionActor::new(&config).start();
    let order_actor = OrderActor::new(
        config.node_id.clone(),
        raft_addr.clone(),
        exec_actor.clone(),
        risk_actor.clone()
    ).start();
    
    // 注册Actor到集群
    sys.register("/user/risk", risk_actor.clone()).await?;
    sys.register("/user/execution", exec_actor.clone()).await?;
    sys.register("/user/order", order_actor.clone()).await?;
    sys.register("/user/raft", raft_addr.clone()).await?;
    
    // 启动API服务器
    start_api_server(config.api_port, order_actor.clone())?;
    
    info!("OMS System started on node {}", config.node_id);
    
    // 等待终止信号
    tokio::signal::ctrl_c().await?;
    info!("Shutting down OMS System");
    
    Ok(())
}
```

## 关键设计考量

1. **Actor模型与Raft结合**：
   - 使用actix-cluster的Actor模型处理并发请求和消息传递
   - 关键状态变更通过Raft共识算法确保一致性

2. **容错与高可用**：
   - 多节点部署，支持自动故障转移
   - Raft提供的领导者选举确保单一写入点，避免脑裂

3. **性能优化**：
   - 读操作可直接访问本地状态，不需共识
   - 批量写入Raft日志减少网络开销

4. **扩展性设计**：
   - 基于角色的Actor系统便于水平扩展
   - 根据负载动态调整节点角色和职责

5. **监控与可观察性**：
   - 内置健康检查与指标收集
   - 状态变更事件追踪与日志记录

该架构适合处理高并发、要求强一致性的股票交易场景，并充分利用了actix-cluster的分布式Actor能力与Raft的一致性保证。
