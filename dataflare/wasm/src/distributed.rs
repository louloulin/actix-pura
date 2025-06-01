//! 分布式WASM插件执行系统
//!
//! 提供WASM插件在分布式环境中的执行、调度和管理功能

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};
use serde::{Deserialize, Serialize};
use log::{info, debug, warn, error};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::{WasmResult, WasmError, WasmPlugin, WasmPluginMetadata, WasmRuntime};

/// 分布式WASM执行器
pub struct DistributedWasmExecutor {
    /// 节点ID
    node_id: String,
    /// 集群配置
    cluster_config: DistributedConfig,
    /// 节点注册表
    node_registry: Arc<RwLock<NodeRegistry>>,
    /// 任务调度器
    task_scheduler: Arc<Mutex<TaskScheduler>>,
    /// 负载均衡器
    load_balancer: Arc<RwLock<LoadBalancer>>,
    /// 故障检测器
    failure_detector: Arc<RwLock<FailureDetector>>,
    /// 本地WASM运行时
    local_runtime: Arc<Mutex<WasmRuntime>>,
    /// 通信管理器
    communication_manager: Arc<Mutex<CommunicationManager>>,
}

/// 分布式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedConfig {
    /// 集群名称
    pub cluster_name: String,
    /// 节点角色
    pub node_role: NodeRole,
    /// 心跳间隔
    pub heartbeat_interval: Duration,
    /// 任务超时时间
    pub task_timeout: Duration,
    /// 最大并发任务数
    pub max_concurrent_tasks: usize,
    /// 负载均衡策略
    pub load_balancing_strategy: LoadBalancingStrategy,
    /// 故障转移配置
    pub failover_config: FailoverConfig,
    /// 网络配置
    pub network_config: NetworkConfig,
}

/// 节点角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NodeRole {
    /// 协调节点 - 负责任务调度和集群管理
    Coordinator,
    /// 工作节点 - 执行WASM插件任务
    Worker,
    /// 混合节点 - 既可以调度也可以执行
    Hybrid,
}

/// 负载均衡策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    /// 轮询
    RoundRobin,
    /// 最少连接
    LeastConnections,
    /// 最少负载
    LeastLoad,
    /// 加权轮询
    WeightedRoundRobin,
    /// 一致性哈希
    ConsistentHash,
}

/// 故障转移配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    /// 启用自动故障转移
    pub enable_auto_failover: bool,
    /// 最大重试次数
    pub max_retry_attempts: u32,
    /// 重试间隔
    pub retry_interval: Duration,
    /// 节点故障检测超时
    pub failure_detection_timeout: Duration,
}

/// 网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// 监听地址
    pub listen_address: String,
    /// 监听端口
    pub listen_port: u16,
    /// 连接超时
    pub connection_timeout: Duration,
    /// 请求超时
    pub request_timeout: Duration,
    /// 最大连接数
    pub max_connections: usize,
}

/// 节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// 节点ID
    pub node_id: String,
    /// 节点角色
    pub role: NodeRole,
    /// 网络地址
    pub address: String,
    /// 端口
    pub port: u16,
    /// 最后心跳时间
    pub last_heartbeat: SystemTime,
    /// 节点状态
    pub status: NodeStatus,
    /// 负载信息
    pub load_info: NodeLoadInfo,
    /// 支持的插件类型
    pub supported_plugins: Vec<String>,
    /// 节点元数据
    pub metadata: HashMap<String, String>,
}

/// 节点状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeStatus {
    /// 健康
    Healthy,
    /// 警告
    Warning,
    /// 不健康
    Unhealthy,
    /// 离线
    Offline,
}

/// 节点负载信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLoadInfo {
    /// CPU使用率 (0.0-1.0)
    pub cpu_usage: f64,
    /// 内存使用率 (0.0-1.0)
    pub memory_usage: f64,
    /// 当前任务数
    pub current_tasks: usize,
    /// 最大任务数
    pub max_tasks: usize,
    /// 网络延迟 (毫秒)
    pub network_latency: u64,
    /// 吞吐量 (任务/秒)
    pub throughput: f64,
}

/// 分布式任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedTask {
    /// 任务ID
    pub task_id: String,
    /// 插件ID
    pub plugin_id: String,
    /// 任务类型
    pub task_type: TaskType,
    /// 输入数据
    pub input_data: Vec<u8>,
    /// 任务优先级
    pub priority: TaskPriority,
    /// 创建时间
    pub created_at: SystemTime,
    /// 超时时间
    pub timeout: Duration,
    /// 重试配置
    pub retry_config: TaskRetryConfig,
    /// 任务元数据
    pub metadata: HashMap<String, String>,
}

/// 任务类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    /// 数据处理任务
    DataProcessing,
    /// 数据转换任务
    DataTransformation,
    /// 数据过滤任务
    DataFiltering,
    /// 数据聚合任务
    DataAggregation,
    /// 自定义任务
    Custom(String),
}

/// 任务优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    /// 低优先级
    Low = 1,
    /// 普通优先级
    Normal = 2,
    /// 高优先级
    High = 3,
    /// 紧急优先级
    Critical = 4,
}

/// 任务重试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRetryConfig {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 重试间隔
    pub retry_interval: Duration,
    /// 指数退避
    pub exponential_backoff: bool,
    /// 最大退避时间
    pub max_backoff: Duration,
}

/// 任务执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionResult {
    /// 任务ID
    pub task_id: String,
    /// 执行状态
    pub status: TaskExecutionStatus,
    /// 输出数据
    pub output_data: Option<Vec<u8>>,
    /// 错误信息
    pub error_message: Option<String>,
    /// 执行时间
    pub execution_time: Duration,
    /// 执行节点
    pub executed_on_node: String,
    /// 完成时间
    pub completed_at: SystemTime,
    /// 执行统计
    pub execution_stats: ExecutionStats,
}

/// 任务执行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskExecutionStatus {
    /// 等待执行
    Pending,
    /// 正在执行
    Running,
    /// 执行成功
    Success,
    /// 执行失败
    Failed,
    /// 任务取消
    Cancelled,
    /// 超时
    Timeout,
}

/// 执行统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// 处理的数据量 (字节)
    pub bytes_processed: u64,
    /// 内存峰值使用量 (字节)
    pub peak_memory_usage: u64,
    /// CPU时间 (毫秒)
    pub cpu_time_ms: u64,
    /// 网络IO (字节)
    pub network_io_bytes: u64,
    /// 磁盘IO (字节)
    pub disk_io_bytes: u64,
}

/// 节点注册表
#[derive(Debug)]
pub struct NodeRegistry {
    /// 节点映射
    nodes: HashMap<String, NodeInfo>,
    /// 按角色分组的节点
    nodes_by_role: HashMap<NodeRole, Vec<String>>,
    /// 健康节点列表
    healthy_nodes: Vec<String>,
}

/// 任务调度器
#[derive(Debug)]
pub struct TaskScheduler {
    /// 待执行任务队列
    pending_tasks: Vec<DistributedTask>,
    /// 正在执行的任务
    running_tasks: HashMap<String, TaskExecution>,
    /// 已完成的任务
    completed_tasks: HashMap<String, TaskExecutionResult>,
    /// 调度策略
    scheduling_strategy: SchedulingStrategy,
}

/// 任务执行信息
#[derive(Debug, Clone)]
pub struct TaskExecution {
    /// 任务信息
    pub task: DistributedTask,
    /// 执行节点
    pub assigned_node: String,
    /// 开始时间
    pub started_at: SystemTime,
    /// 当前重试次数
    pub retry_count: u32,
}

/// 调度策略
#[derive(Debug, Clone)]
pub enum SchedulingStrategy {
    /// 先进先出
    FIFO,
    /// 优先级调度
    Priority,
    /// 最短作业优先
    ShortestJobFirst,
    /// 公平调度
    FairShare,
}

/// 负载均衡器
#[derive(Debug)]
pub struct LoadBalancer {
    /// 负载均衡策略
    strategy: LoadBalancingStrategy,
    /// 节点权重
    node_weights: HashMap<String, f64>,
    /// 轮询计数器
    round_robin_counter: usize,
    /// 一致性哈希环
    consistent_hash_ring: Option<ConsistentHashRing>,
}

/// 一致性哈希环
#[derive(Debug)]
pub struct ConsistentHashRing {
    /// 虚拟节点映射
    virtual_nodes: HashMap<u64, String>,
    /// 每个物理节点的虚拟节点数
    virtual_nodes_per_node: usize,
}

/// 故障检测器
#[derive(Debug)]
pub struct FailureDetector {
    /// 节点健康状态
    node_health: HashMap<String, NodeHealthInfo>,
    /// 检测配置
    detection_config: FailureDetectionConfig,
}

/// 节点健康信息
#[derive(Debug, Clone)]
pub struct NodeHealthInfo {
    /// 最后心跳时间
    pub last_heartbeat: SystemTime,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 健康状态
    pub status: NodeStatus,
    /// 响应时间历史
    pub response_times: Vec<Duration>,
}

/// 故障检测配置
#[derive(Debug, Clone)]
pub struct FailureDetectionConfig {
    /// 心跳超时
    pub heartbeat_timeout: Duration,
    /// 最大连续失败次数
    pub max_consecutive_failures: u32,
    /// 健康检查间隔
    pub health_check_interval: Duration,
}

/// 通信管理器
#[derive(Debug)]
pub struct CommunicationManager {
    /// 节点连接池
    connection_pool: HashMap<String, NodeConnection>,
    /// 消息路由表
    message_router: MessageRouter,
    /// 网络配置
    network_config: NetworkConfig,
}

/// 节点连接
#[derive(Debug)]
pub struct NodeConnection {
    /// 节点ID
    pub node_id: String,
    /// 连接状态
    pub status: ConnectionStatus,
    /// 最后活动时间
    pub last_activity: SystemTime,
    /// 发送队列
    pub send_queue: mpsc::UnboundedSender<DistributedMessage>,
}

/// 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    /// 已连接
    Connected,
    /// 连接中
    Connecting,
    /// 已断开
    Disconnected,
    /// 连接失败
    Failed,
}

/// 分布式消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DistributedMessage {
    /// 心跳消息
    Heartbeat {
        node_id: String,
        load_info: NodeLoadInfo,
    },
    /// 任务分配
    TaskAssignment {
        task: DistributedTask,
        target_node: String,
    },
    /// 任务结果
    TaskResult {
        result: TaskExecutionResult,
    },
    /// 节点注册
    NodeRegistration {
        node_info: NodeInfo,
    },
    /// 节点注销
    NodeDeregistration {
        node_id: String,
    },
    /// 集群状态同步
    ClusterStateSync {
        nodes: Vec<NodeInfo>,
    },
}

/// 消息路由器
#[derive(Debug)]
pub struct MessageRouter {
    /// 路由表
    routes: HashMap<String, String>,
    /// 广播组
    broadcast_groups: HashMap<String, Vec<String>>,
}

impl DistributedWasmExecutor {
    /// 创建新的分布式WASM执行器
    pub fn new(node_id: String, cluster_config: DistributedConfig, local_runtime: WasmRuntime) -> Self {
        let load_balancing_strategy = cluster_config.load_balancing_strategy.clone();
        let network_config = cluster_config.network_config.clone();

        Self {
            node_id,
            cluster_config,
            node_registry: Arc::new(RwLock::new(NodeRegistry::new())),
            task_scheduler: Arc::new(Mutex::new(TaskScheduler::new())),
            load_balancer: Arc::new(RwLock::new(LoadBalancer::new(load_balancing_strategy))),
            failure_detector: Arc::new(RwLock::new(FailureDetector::new(FailureDetectionConfig::default()))),
            local_runtime: Arc::new(Mutex::new(local_runtime)),
            communication_manager: Arc::new(Mutex::new(CommunicationManager::new(network_config))),
        }
    }

    /// 启动分布式执行器
    pub async fn start(&self) -> WasmResult<()> {
        info!("启动分布式WASM执行器: {}", self.node_id);

        // 启动通信管理器
        self.start_communication_manager().await?;

        // 启动心跳服务
        self.start_heartbeat_service().await?;

        // 启动故障检测
        self.start_failure_detection().await?;

        // 启动任务调度器
        self.start_task_scheduler().await?;

        // 注册本地节点
        self.register_local_node().await?;

        info!("分布式WASM执行器启动完成");
        Ok(())
    }

    /// 停止分布式执行器
    pub async fn stop(&self) -> WasmResult<()> {
        info!("停止分布式WASM执行器: {}", self.node_id);

        // 注销本地节点
        self.deregister_local_node().await?;

        // 停止各个服务
        // TODO: 实现服务停止逻辑

        info!("分布式WASM执行器已停止");
        Ok(())
    }

    /// 提交分布式任务
    pub async fn submit_task(&self, task: DistributedTask) -> WasmResult<String> {
        info!("提交分布式任务: {}", task.task_id);

        // 选择最佳执行节点
        let target_node = self.select_best_node(&task).await?;

        if target_node == self.node_id {
            // 在本地执行
            self.execute_task_locally(task).await
        } else {
            // 转发到远程节点
            self.forward_task_to_node(task, target_node).await
        }
    }

    /// 在本地执行任务
    pub async fn execute_task_locally(&self, task: DistributedTask) -> WasmResult<String> {
        debug!("在本地执行任务: {}", task.task_id);

        let task_id = task.task_id.clone();
        let mut scheduler = self.task_scheduler.lock().await;

        // 添加到调度器
        scheduler.add_task(task)?;

        Ok(task_id)
    }

    /// 转发任务到远程节点
    pub async fn forward_task_to_node(&self, task: DistributedTask, target_node: String) -> WasmResult<String> {
        debug!("转发任务到节点 {}: {}", target_node, task.task_id);

        let comm_manager = self.communication_manager.lock().await;
        let message = DistributedMessage::TaskAssignment {
            task: task.clone(),
            target_node: target_node.clone(),
        };

        comm_manager.send_message(&target_node, message).await?;

        Ok(task.task_id)
    }

    /// 选择最佳执行节点
    async fn select_best_node(&self, task: &DistributedTask) -> WasmResult<String> {
        let node_registry = self.node_registry.read()
            .map_err(|_| WasmError::runtime("获取节点注册表锁失败".to_string()))?;

        // 获取支持该插件的健康节点
        let available_nodes = node_registry.get_nodes_supporting_plugin(&task.plugin_id);

        if available_nodes.is_empty() {
            return Err(WasmError::runtime(format!("没有可用节点支持插件: {}", task.plugin_id)));
        }

        // 释放node_registry锁，然后获取load_balancer的写锁
        drop(node_registry);

        let mut load_balancer = self.load_balancer.write()
            .map_err(|_| WasmError::runtime("获取负载均衡器锁失败".to_string()))?;

        let node_registry = self.node_registry.read()
            .map_err(|_| WasmError::runtime("获取节点注册表锁失败".to_string()))?;

        // 使用负载均衡策略选择节点
        let selected_node = load_balancer.select_node(&available_nodes, &node_registry.nodes)?;

        Ok(selected_node)
    }

    /// 启动通信管理器
    async fn start_communication_manager(&self) -> WasmResult<()> {
        debug!("启动通信管理器");
        let mut comm_manager = self.communication_manager.lock().await;
        comm_manager.start().await?;
        Ok(())
    }

    /// 启动心跳服务
    async fn start_heartbeat_service(&self) -> WasmResult<()> {
        debug!("启动心跳服务");

        let node_id = self.node_id.clone();
        let interval = self.cluster_config.heartbeat_interval;
        let comm_manager = self.communication_manager.clone();

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                // 收集本地负载信息
                let load_info = Self::collect_local_load_info().await;

                // 发送心跳消息
                let message = DistributedMessage::Heartbeat {
                    node_id: node_id.clone(),
                    load_info,
                };

                let comm_manager = comm_manager.lock().await;
                if let Err(e) = comm_manager.broadcast_message(message).await {
                    warn!("发送心跳失败: {}", e);
                }
            }
        });

        Ok(())
    }

    /// 启动故障检测
    async fn start_failure_detection(&self) -> WasmResult<()> {
        debug!("启动故障检测");

        let failure_detector = self.failure_detector.clone();
        let node_registry = self.node_registry.clone();

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval_timer.tick().await;

                let mut detector = failure_detector.write().unwrap();
                let mut registry = node_registry.write().unwrap();

                detector.check_node_health(&mut registry);
            }
        });

        Ok(())
    }

    /// 启动任务调度器
    async fn start_task_scheduler(&self) -> WasmResult<()> {
        debug!("启动任务调度器");

        let task_scheduler = self.task_scheduler.clone();
        let local_runtime = self.local_runtime.clone();
        let node_id = self.node_id.clone();

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(Duration::from_millis(100));

            loop {
                interval_timer.tick().await;

                let mut scheduler = task_scheduler.lock().await;
                let mut runtime = local_runtime.lock().await;

                // 处理待执行任务
                if let Some(task) = scheduler.get_next_task() {
                    let task_id = task.task_id.clone();

                    // 执行任务
                    match Self::execute_wasm_task(&mut runtime, &task).await {
                        Ok(result) => {
                            scheduler.complete_task(task_id, result);
                        }
                        Err(e) => {
                            let error_result = TaskExecutionResult {
                                task_id: task_id.clone(),
                                status: TaskExecutionStatus::Failed,
                                output_data: None,
                                error_message: Some(e.to_string()),
                                execution_time: Duration::from_secs(0),
                                executed_on_node: node_id.clone(),
                                completed_at: SystemTime::now(),
                                execution_stats: ExecutionStats::default(),
                            };
                            scheduler.complete_task(task_id, error_result);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// 注册本地节点
    async fn register_local_node(&self) -> WasmResult<()> {
        debug!("注册本地节点");

        let node_info = NodeInfo {
            node_id: self.node_id.clone(),
            role: self.cluster_config.node_role.clone(),
            address: self.cluster_config.network_config.listen_address.clone(),
            port: self.cluster_config.network_config.listen_port,
            last_heartbeat: SystemTime::now(),
            status: NodeStatus::Healthy,
            load_info: Self::collect_local_load_info().await,
            supported_plugins: vec![], // TODO: 从本地运行时获取
            metadata: HashMap::new(),
        };

        let mut registry = self.node_registry.write()
            .map_err(|_| WasmError::runtime("获取节点注册表锁失败".to_string()))?;

        registry.register_node(node_info.clone());

        // 广播节点注册消息
        let comm_manager = self.communication_manager.lock().await;
        let message = DistributedMessage::NodeRegistration { node_info };
        comm_manager.broadcast_message(message).await?;

        Ok(())
    }

    /// 注销本地节点
    async fn deregister_local_node(&self) -> WasmResult<()> {
        debug!("注销本地节点");

        let mut registry = self.node_registry.write()
            .map_err(|_| WasmError::runtime("获取节点注册表锁失败".to_string()))?;

        registry.deregister_node(&self.node_id);

        // 广播节点注销消息
        let comm_manager = self.communication_manager.lock().await;
        let message = DistributedMessage::NodeDeregistration {
            node_id: self.node_id.clone(),
        };
        comm_manager.broadcast_message(message).await?;

        Ok(())
    }

    /// 收集本地负载信息
    async fn collect_local_load_info() -> NodeLoadInfo {
        // TODO: 实现真实的系统负载收集
        NodeLoadInfo {
            cpu_usage: 0.3,
            memory_usage: 0.4,
            current_tasks: 2,
            max_tasks: 10,
            network_latency: 5,
            throughput: 100.0,
        }
    }

    /// 执行WASM任务
    async fn execute_wasm_task(runtime: &mut WasmRuntime, task: &DistributedTask) -> WasmResult<TaskExecutionResult> {
        let start_time = SystemTime::now();

        // TODO: 实现真实的WASM任务执行
        // 这里是模拟实现
        tokio::time::sleep(Duration::from_millis(100)).await;

        let execution_time = start_time.elapsed().unwrap_or(Duration::from_secs(0));

        Ok(TaskExecutionResult {
            task_id: task.task_id.clone(),
            status: TaskExecutionStatus::Success,
            output_data: Some(b"processed data".to_vec()),
            error_message: None,
            execution_time,
            executed_on_node: "local".to_string(),
            completed_at: SystemTime::now(),
            execution_stats: ExecutionStats {
                bytes_processed: task.input_data.len() as u64,
                peak_memory_usage: 1024 * 1024, // 1MB
                cpu_time_ms: execution_time.as_millis() as u64,
                network_io_bytes: 0,
                disk_io_bytes: 0,
            },
        })
    }
}

impl NodeRegistry {
    /// 创建新的节点注册表
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            nodes_by_role: HashMap::new(),
            healthy_nodes: Vec::new(),
        }
    }

    /// 注册节点
    pub fn register_node(&mut self, node_info: NodeInfo) {
        let node_id = node_info.node_id.clone();
        let role = node_info.role.clone();

        // 添加到节点映射
        self.nodes.insert(node_id.clone(), node_info);

        // 添加到角色分组
        self.nodes_by_role
            .entry(role)
            .or_insert_with(Vec::new)
            .push(node_id.clone());

        // 如果是健康节点，添加到健康列表
        if let Some(node) = self.nodes.get(&node_id) {
            if node.status == NodeStatus::Healthy {
                if !self.healthy_nodes.contains(&node_id) {
                    self.healthy_nodes.push(node_id);
                }
            }
        }
    }

    /// 注销节点
    pub fn deregister_node(&mut self, node_id: &str) {
        if let Some(node) = self.nodes.remove(node_id) {
            // 从角色分组中移除
            if let Some(role_nodes) = self.nodes_by_role.get_mut(&node.role) {
                role_nodes.retain(|id| id != node_id);
            }

            // 从健康节点列表中移除
            self.healthy_nodes.retain(|id| id != node_id);
        }
    }

    /// 更新节点状态
    pub fn update_node_status(&mut self, node_id: &str, status: NodeStatus) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            let old_status = node.status.clone();
            node.status = status.clone();

            // 更新健康节点列表
            match (old_status, status) {
                (NodeStatus::Healthy, NodeStatus::Healthy) => {
                    // 保持不变
                }
                (_, NodeStatus::Healthy) => {
                    // 变为健康，添加到列表
                    if !self.healthy_nodes.contains(&node_id.to_string()) {
                        self.healthy_nodes.push(node_id.to_string());
                    }
                }
                (NodeStatus::Healthy, _) => {
                    // 不再健康，从列表移除
                    self.healthy_nodes.retain(|id| id != node_id);
                }
                _ => {
                    // 其他状态变化，确保不在健康列表中
                    self.healthy_nodes.retain(|id| id != node_id);
                }
            }
        }
    }

    /// 获取支持指定插件的节点
    pub fn get_nodes_supporting_plugin(&self, plugin_id: &str) -> Vec<String> {
        self.healthy_nodes
            .iter()
            .filter(|node_id| {
                if let Some(node) = self.nodes.get(*node_id) {
                    node.supported_plugins.contains(&plugin_id.to_string())
                        || node.supported_plugins.is_empty() // 空列表表示支持所有插件
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }

    /// 获取指定角色的节点
    pub fn get_nodes_by_role(&self, role: &NodeRole) -> Vec<String> {
        self.nodes_by_role
            .get(role)
            .cloned()
            .unwrap_or_default()
    }

    /// 获取健康节点列表
    pub fn get_healthy_nodes(&self) -> &Vec<String> {
        &self.healthy_nodes
    }
}

impl TaskScheduler {
    /// 创建新的任务调度器
    pub fn new() -> Self {
        Self {
            pending_tasks: Vec::new(),
            running_tasks: HashMap::new(),
            completed_tasks: HashMap::new(),
            scheduling_strategy: SchedulingStrategy::Priority,
        }
    }

    /// 添加任务
    pub fn add_task(&mut self, task: DistributedTask) -> WasmResult<()> {
        self.pending_tasks.push(task);

        // 根据调度策略排序
        match self.scheduling_strategy {
            SchedulingStrategy::Priority => {
                self.pending_tasks.sort_by(|a, b| b.priority.cmp(&a.priority));
            }
            SchedulingStrategy::FIFO => {
                // FIFO不需要排序
            }
            SchedulingStrategy::ShortestJobFirst => {
                // 根据预估执行时间排序（这里简化为按输入数据大小）
                self.pending_tasks.sort_by(|a, b| a.input_data.len().cmp(&b.input_data.len()));
            }
            SchedulingStrategy::FairShare => {
                // 公平调度的实现较复杂，这里简化处理
                self.pending_tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            }
        }

        Ok(())
    }

    /// 获取下一个待执行任务
    pub fn get_next_task(&mut self) -> Option<DistributedTask> {
        if !self.pending_tasks.is_empty() {
            // 从前面取出最高优先级的任务（因为我们按降序排序）
            let task = self.pending_tasks.remove(0);

            // 添加到运行中任务
            let execution = TaskExecution {
                task: task.clone(),
                assigned_node: "local".to_string(),
                started_at: SystemTime::now(),
                retry_count: 0,
            };
            self.running_tasks.insert(task.task_id.clone(), execution);

            Some(task)
        } else {
            None
        }
    }

    /// 完成任务
    pub fn complete_task(&mut self, task_id: String, result: TaskExecutionResult) {
        // 从运行中任务移除
        self.running_tasks.remove(&task_id);

        // 添加到已完成任务
        self.completed_tasks.insert(task_id, result);
    }

    /// 获取任务统计信息
    pub fn get_task_stats(&self) -> TaskStats {
        TaskStats {
            pending_count: self.pending_tasks.len(),
            running_count: self.running_tasks.len(),
            completed_count: self.completed_tasks.len(),
            success_count: self.completed_tasks.values()
                .filter(|r| r.status == TaskExecutionStatus::Success)
                .count(),
            failed_count: self.completed_tasks.values()
                .filter(|r| r.status == TaskExecutionStatus::Failed)
                .count(),
        }
    }
}

/// 任务统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStats {
    /// 待执行任务数
    pub pending_count: usize,
    /// 运行中任务数
    pub running_count: usize,
    /// 已完成任务数
    pub completed_count: usize,
    /// 成功任务数
    pub success_count: usize,
    /// 失败任务数
    pub failed_count: usize,
}

impl LoadBalancer {
    /// 创建新的负载均衡器
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            node_weights: HashMap::new(),
            round_robin_counter: 0,
            consistent_hash_ring: None,
        }
    }

    /// 选择节点
    pub fn select_node(&mut self, available_nodes: &[String], nodes: &HashMap<String, NodeInfo>) -> WasmResult<String> {
        if available_nodes.is_empty() {
            return Err(WasmError::runtime("没有可用节点".to_string()));
        }

        match &self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                let index = self.round_robin_counter % available_nodes.len();
                self.round_robin_counter += 1;
                Ok(available_nodes[index].clone())
            }
            LoadBalancingStrategy::LeastConnections => {
                // 选择当前任务数最少的节点
                let best_node = available_nodes
                    .iter()
                    .min_by_key(|node_id| {
                        nodes.get(*node_id)
                            .map(|node| node.load_info.current_tasks)
                            .unwrap_or(usize::MAX)
                    })
                    .ok_or_else(|| WasmError::runtime("无法选择节点".to_string()))?;

                Ok(best_node.clone())
            }
            LoadBalancingStrategy::LeastLoad => {
                // 选择CPU和内存使用率最低的节点
                let best_node = available_nodes
                    .iter()
                    .min_by(|a, b| {
                        let load_a = nodes.get(*a)
                            .map(|node| node.load_info.cpu_usage + node.load_info.memory_usage)
                            .unwrap_or(f64::MAX);
                        let load_b = nodes.get(*b)
                            .map(|node| node.load_info.cpu_usage + node.load_info.memory_usage)
                            .unwrap_or(f64::MAX);
                        load_a.partial_cmp(&load_b).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .ok_or_else(|| WasmError::runtime("无法选择节点".to_string()))?;

                Ok(best_node.clone())
            }
            LoadBalancingStrategy::WeightedRoundRobin => {
                // 加权轮询（简化实现）
                let weighted_nodes: Vec<_> = available_nodes
                    .iter()
                    .map(|node_id| {
                        let weight = self.node_weights.get(node_id).copied().unwrap_or(1.0);
                        (node_id.clone(), weight)
                    })
                    .collect();

                if weighted_nodes.is_empty() {
                    return Err(WasmError::runtime("没有加权节点".to_string()));
                }

                // 简化的加权选择
                let total_weight: f64 = weighted_nodes.iter().map(|(_, w)| w).sum();
                let mut target = (self.round_robin_counter as f64 % total_weight) + 1.0;
                self.round_robin_counter += 1;

                for (node_id, weight) in weighted_nodes {
                    target -= weight;
                    if target <= 0.0 {
                        return Ok(node_id);
                    }
                }

                Ok(available_nodes[0].clone())
            }
            LoadBalancingStrategy::ConsistentHash => {
                // 一致性哈希（简化实现）
                if available_nodes.len() == 1 {
                    Ok(available_nodes[0].clone())
                } else {
                    // 简化为随机选择
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};

                    let mut hasher = DefaultHasher::new();
                    SystemTime::now().hash(&mut hasher);
                    let hash = hasher.finish();
                    let index = (hash as usize) % available_nodes.len();

                    Ok(available_nodes[index].clone())
                }
            }
        }
    }

    /// 设置节点权重
    pub fn set_node_weight(&mut self, node_id: String, weight: f64) {
        self.node_weights.insert(node_id, weight);
    }
}

impl FailureDetector {
    /// 创建新的故障检测器
    pub fn new(config: FailureDetectionConfig) -> Self {
        Self {
            node_health: HashMap::new(),
            detection_config: config,
        }
    }

    /// 检查节点健康状态
    pub fn check_node_health(&mut self, registry: &mut NodeRegistry) {
        let now = SystemTime::now();
        let mut unhealthy_nodes = Vec::new();

        for (node_id, health_info) in &mut self.node_health {
            // 检查心跳超时
            if let Ok(elapsed) = now.duration_since(health_info.last_heartbeat) {
                if elapsed > self.detection_config.heartbeat_timeout {
                    health_info.consecutive_failures += 1;

                    // 判断是否需要标记为不健康
                    if health_info.consecutive_failures >= self.detection_config.max_consecutive_failures {
                        health_info.status = NodeStatus::Unhealthy;
                        unhealthy_nodes.push(node_id.clone());
                    }
                } else {
                    // 重置失败计数
                    health_info.consecutive_failures = 0;
                    if health_info.status != NodeStatus::Healthy {
                        health_info.status = NodeStatus::Healthy;
                    }
                }
            }
        }

        // 更新注册表中的节点状态
        for node_id in unhealthy_nodes {
            registry.update_node_status(&node_id, NodeStatus::Unhealthy);
        }
    }

    /// 更新节点心跳
    pub fn update_node_heartbeat(&mut self, node_id: &str, response_time: Duration) {
        let health_info = self.node_health.entry(node_id.to_string()).or_insert_with(|| {
            NodeHealthInfo {
                last_heartbeat: SystemTime::now(),
                consecutive_failures: 0,
                status: NodeStatus::Healthy,
                response_times: Vec::new(),
            }
        });

        health_info.last_heartbeat = SystemTime::now();
        health_info.response_times.push(response_time);

        // 保持最近的响应时间记录
        if health_info.response_times.len() > 100 {
            health_info.response_times.remove(0);
        }
    }

    /// 获取节点平均响应时间
    pub fn get_average_response_time(&self, node_id: &str) -> Option<Duration> {
        self.node_health.get(node_id).and_then(|health_info| {
            if health_info.response_times.is_empty() {
                None
            } else {
                let total: Duration = health_info.response_times.iter().sum();
                Some(total / health_info.response_times.len() as u32)
            }
        })
    }
}

impl FailureDetectionConfig {
    /// 创建默认配置
    pub fn default() -> Self {
        Self {
            heartbeat_timeout: Duration::from_secs(30),
            max_consecutive_failures: 3,
            health_check_interval: Duration::from_secs(10),
        }
    }
}

impl CommunicationManager {
    /// 创建新的通信管理器
    pub fn new(network_config: NetworkConfig) -> Self {
        Self {
            connection_pool: HashMap::new(),
            message_router: MessageRouter::new(),
            network_config,
        }
    }

    /// 启动通信管理器
    pub async fn start(&mut self) -> WasmResult<()> {
        info!("启动通信管理器，监听地址: {}:{}",
              self.network_config.listen_address,
              self.network_config.listen_port);

        // TODO: 实现真实的网络监听和连接管理
        // 这里是模拟实现
        Ok(())
    }

    /// 发送消息到指定节点
    pub async fn send_message(&self, target_node: &str, message: DistributedMessage) -> WasmResult<()> {
        debug!("发送消息到节点 {}: {:?}", target_node, message);

        // 获取节点连接
        if let Some(connection) = self.connection_pool.get(target_node) {
            if connection.status == ConnectionStatus::Connected {
                // 发送消息
                if let Err(_) = connection.send_queue.send(message) {
                    return Err(WasmError::runtime(format!("发送消息到节点 {} 失败", target_node)));
                }
            } else {
                return Err(WasmError::runtime(format!("节点 {} 连接不可用", target_node)));
            }
        } else {
            return Err(WasmError::runtime(format!("节点 {} 连接不存在", target_node)));
        }

        Ok(())
    }

    /// 广播消息到所有节点
    pub async fn broadcast_message(&self, message: DistributedMessage) -> WasmResult<()> {
        debug!("广播消息: {:?}", message);

        let mut errors = Vec::new();

        for (node_id, connection) in &self.connection_pool {
            if connection.status == ConnectionStatus::Connected {
                if let Err(_) = connection.send_queue.send(message.clone()) {
                    errors.push(format!("发送到节点 {} 失败", node_id));
                }
            }
        }

        if !errors.is_empty() {
            warn!("广播消息部分失败: {:?}", errors);
        }

        Ok(())
    }

    /// 添加节点连接
    pub fn add_node_connection(&mut self, node_id: String, connection: NodeConnection) {
        self.connection_pool.insert(node_id, connection);
    }

    /// 移除节点连接
    pub fn remove_node_connection(&mut self, node_id: &str) {
        self.connection_pool.remove(node_id);
    }

    /// 获取连接统计信息
    pub fn get_connection_stats(&self) -> ConnectionStats {
        let total_connections = self.connection_pool.len();
        let connected_count = self.connection_pool.values()
            .filter(|conn| conn.status == ConnectionStatus::Connected)
            .count();
        let disconnected_count = self.connection_pool.values()
            .filter(|conn| conn.status == ConnectionStatus::Disconnected)
            .count();
        let failed_count = self.connection_pool.values()
            .filter(|conn| conn.status == ConnectionStatus::Failed)
            .count();

        ConnectionStats {
            total_connections,
            connected_count,
            disconnected_count,
            failed_count,
        }
    }
}

/// 连接统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    /// 总连接数
    pub total_connections: usize,
    /// 已连接数
    pub connected_count: usize,
    /// 已断开数
    pub disconnected_count: usize,
    /// 失败连接数
    pub failed_count: usize,
}

impl MessageRouter {
    /// 创建新的消息路由器
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
            broadcast_groups: HashMap::new(),
        }
    }

    /// 添加路由
    pub fn add_route(&mut self, destination: String, next_hop: String) {
        self.routes.insert(destination, next_hop);
    }

    /// 获取下一跳节点
    pub fn get_next_hop(&self, destination: &str) -> Option<&String> {
        self.routes.get(destination)
    }

    /// 添加广播组
    pub fn add_broadcast_group(&mut self, group_name: String, members: Vec<String>) {
        self.broadcast_groups.insert(group_name, members);
    }

    /// 获取广播组成员
    pub fn get_broadcast_group(&self, group_name: &str) -> Option<&Vec<String>> {
        self.broadcast_groups.get(group_name)
    }
}

impl Default for ExecutionStats {
    fn default() -> Self {
        Self {
            bytes_processed: 0,
            peak_memory_usage: 0,
            cpu_time_ms: 0,
            network_io_bytes: 0,
            disk_io_bytes: 0,
        }
    }
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            cluster_name: "dataflare-cluster".to_string(),
            node_role: NodeRole::Hybrid,
            heartbeat_interval: Duration::from_secs(10),
            task_timeout: Duration::from_secs(300),
            max_concurrent_tasks: 10,
            load_balancing_strategy: LoadBalancingStrategy::LeastLoad,
            failover_config: FailoverConfig::default(),
            network_config: NetworkConfig::default(),
        }
    }
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            enable_auto_failover: true,
            max_retry_attempts: 3,
            retry_interval: Duration::from_secs(5),
            failure_detection_timeout: Duration::from_secs(30),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            listen_port: 8080,
            connection_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_connections: 1000,
        }
    }
}

impl Default for TaskRetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            retry_interval: Duration::from_secs(1),
            exponential_backoff: true,
            max_backoff: Duration::from_secs(60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::WasmRuntime;
    use tempfile::TempDir;

    fn create_test_runtime() -> WasmRuntime {
        use crate::runtime::WasmRuntimeConfig;
        use crate::sandbox::SecurityPolicy;
        let _temp_dir = TempDir::new().unwrap();
        let config = WasmRuntimeConfig {
            memory_limit: 64 * 1024 * 1024, // 64MB
            timeout_ms: 30000, // 30 seconds
            debug: false,
            security_policy: SecurityPolicy::default(),
            max_concurrent_plugins: 10,
            enable_jit: false,
            enable_wasi: false,
        };
        WasmRuntime::new(config).unwrap()
    }

    fn create_test_config() -> DistributedConfig {
        DistributedConfig {
            cluster_name: "test-cluster".to_string(),
            node_role: NodeRole::Worker,
            heartbeat_interval: Duration::from_millis(100),
            task_timeout: Duration::from_secs(10),
            max_concurrent_tasks: 5,
            load_balancing_strategy: LoadBalancingStrategy::RoundRobin,
            failover_config: FailoverConfig::default(),
            network_config: NetworkConfig {
                listen_address: "127.0.0.1".to_string(),
                listen_port: 9999,
                connection_timeout: Duration::from_secs(5),
                request_timeout: Duration::from_secs(10),
                max_connections: 100,
            },
        }
    }

    fn create_test_task() -> DistributedTask {
        DistributedTask {
            task_id: Uuid::new_v4().to_string(),
            plugin_id: "test-plugin".to_string(),
            task_type: TaskType::DataProcessing,
            input_data: b"test data".to_vec(),
            priority: TaskPriority::Normal,
            created_at: SystemTime::now(),
            timeout: Duration::from_secs(30),
            retry_config: TaskRetryConfig::default(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_distributed_executor_creation() {
        let runtime = create_test_runtime();
        let config = create_test_config();
        let node_id = "test-node-1".to_string();

        let executor = DistributedWasmExecutor::new(node_id.clone(), config, runtime);

        assert_eq!(executor.node_id, node_id);
        assert_eq!(executor.cluster_config.cluster_name, "test-cluster");
    }

    #[test]
    fn test_node_registry() {
        let mut registry = NodeRegistry::new();

        let node_info = NodeInfo {
            node_id: "node-1".to_string(),
            role: NodeRole::Worker,
            address: "127.0.0.1".to_string(),
            port: 8080,
            last_heartbeat: SystemTime::now(),
            status: NodeStatus::Healthy,
            load_info: NodeLoadInfo {
                cpu_usage: 0.3,
                memory_usage: 0.4,
                current_tasks: 2,
                max_tasks: 10,
                network_latency: 5,
                throughput: 100.0,
            },
            supported_plugins: vec!["test-plugin".to_string()],
            metadata: HashMap::new(),
        };

        // 注册节点
        registry.register_node(node_info.clone());

        // 验证节点已注册
        assert_eq!(registry.nodes.len(), 1);
        assert_eq!(registry.healthy_nodes.len(), 1);
        assert!(registry.nodes.contains_key("node-1"));

        // 获取支持插件的节点
        let supporting_nodes = registry.get_nodes_supporting_plugin("test-plugin");
        assert_eq!(supporting_nodes.len(), 1);
        assert_eq!(supporting_nodes[0], "node-1");

        // 注销节点
        registry.deregister_node("node-1");
        assert_eq!(registry.nodes.len(), 0);
        assert_eq!(registry.healthy_nodes.len(), 0);
    }

    #[test]
    fn test_task_scheduler() {
        let mut scheduler = TaskScheduler::new();

        let task1 = DistributedTask {
            task_id: "task-1".to_string(),
            priority: TaskPriority::High,
            ..create_test_task()
        };

        let task2 = DistributedTask {
            task_id: "task-2".to_string(),
            priority: TaskPriority::Low,
            ..create_test_task()
        };

        // 添加任务
        scheduler.add_task(task1).unwrap();
        scheduler.add_task(task2).unwrap();

        // 验证任务按优先级排序
        let next_task = scheduler.get_next_task().unwrap();
        assert_eq!(next_task.task_id, "task-1"); // 高优先级任务先执行

        let stats = scheduler.get_task_stats();
        assert_eq!(stats.pending_count, 1);
        assert_eq!(stats.running_count, 1);
    }

    #[test]
    fn test_load_balancer_round_robin() {
        let mut load_balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        let nodes = HashMap::new();
        let available_nodes = vec!["node-1".to_string(), "node-2".to_string(), "node-3".to_string()];

        // 测试轮询选择
        let selected1 = load_balancer.select_node(&available_nodes, &nodes).unwrap();
        let selected2 = load_balancer.select_node(&available_nodes, &nodes).unwrap();
        let selected3 = load_balancer.select_node(&available_nodes, &nodes).unwrap();
        let selected4 = load_balancer.select_node(&available_nodes, &nodes).unwrap();

        assert_eq!(selected1, "node-1");
        assert_eq!(selected2, "node-2");
        assert_eq!(selected3, "node-3");
        assert_eq!(selected4, "node-1"); // 回到第一个节点
    }

    #[test]
    fn test_load_balancer_least_connections() {
        let mut load_balancer = LoadBalancer::new(LoadBalancingStrategy::LeastConnections);
        let mut nodes = HashMap::new();

        // 创建测试节点
        let node1 = NodeInfo {
            node_id: "node-1".to_string(),
            role: NodeRole::Worker,
            address: "127.0.0.1".to_string(),
            port: 8080,
            last_heartbeat: SystemTime::now(),
            status: NodeStatus::Healthy,
            load_info: NodeLoadInfo {
                cpu_usage: 0.3,
                memory_usage: 0.4,
                current_tasks: 5, // 较多任务
                max_tasks: 10,
                network_latency: 5,
                throughput: 100.0,
            },
            supported_plugins: vec![],
            metadata: HashMap::new(),
        };

        let node2 = NodeInfo {
            node_id: "node-2".to_string(),
            role: NodeRole::Worker,
            address: "127.0.0.1".to_string(),
            port: 8081,
            last_heartbeat: SystemTime::now(),
            status: NodeStatus::Healthy,
            load_info: NodeLoadInfo {
                cpu_usage: 0.2,
                memory_usage: 0.3,
                current_tasks: 2, // 较少任务
                max_tasks: 10,
                network_latency: 3,
                throughput: 120.0,
            },
            supported_plugins: vec![],
            metadata: HashMap::new(),
        };

        nodes.insert("node-1".to_string(), node1);
        nodes.insert("node-2".to_string(), node2);

        let available_nodes = vec!["node-1".to_string(), "node-2".to_string()];

        // 应该选择任务数较少的节点
        let selected = load_balancer.select_node(&available_nodes, &nodes).unwrap();
        assert_eq!(selected, "node-2");
    }

    #[test]
    fn test_failure_detector() {
        let mut detector = FailureDetector::new(FailureDetectionConfig {
            heartbeat_timeout: Duration::from_millis(100),
            max_consecutive_failures: 2,
            health_check_interval: Duration::from_millis(50),
        });

        let mut registry = NodeRegistry::new();

        // 添加测试节点
        let node_info = NodeInfo {
            node_id: "test-node".to_string(),
            role: NodeRole::Worker,
            address: "127.0.0.1".to_string(),
            port: 8080,
            last_heartbeat: SystemTime::now(),
            status: NodeStatus::Healthy,
            load_info: NodeLoadInfo {
                cpu_usage: 0.3,
                memory_usage: 0.4,
                current_tasks: 2,
                max_tasks: 10,
                network_latency: 5,
                throughput: 100.0,
            },
            supported_plugins: vec![],
            metadata: HashMap::new(),
        };

        registry.register_node(node_info);

        // 更新心跳
        detector.update_node_heartbeat("test-node", Duration::from_millis(10));

        // 验证节点健康
        assert_eq!(detector.node_health.get("test-node").unwrap().status, NodeStatus::Healthy);

        // 模拟心跳超时
        std::thread::sleep(Duration::from_millis(150));
        detector.check_node_health(&mut registry);

        // 验证节点状态更新
        let health_info = detector.node_health.get("test-node").unwrap();
        assert!(health_info.consecutive_failures > 0);
    }

    #[test]
    fn test_communication_manager() {
        let config = NetworkConfig::default();
        let mut comm_manager = CommunicationManager::new(config);

        // 测试连接统计
        let stats = comm_manager.get_connection_stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.connected_count, 0);

        // 添加模拟连接
        let (tx, _rx) = mpsc::unbounded_channel();
        let connection = NodeConnection {
            node_id: "test-node".to_string(),
            status: ConnectionStatus::Connected,
            last_activity: SystemTime::now(),
            send_queue: tx,
        };

        comm_manager.add_node_connection("test-node".to_string(), connection);

        let stats = comm_manager.get_connection_stats();
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.connected_count, 1);
    }

    #[test]
    fn test_message_router() {
        let mut router = MessageRouter::new();

        // 添加路由
        router.add_route("destination-1".to_string(), "next-hop-1".to_string());

        // 验证路由
        let next_hop = router.get_next_hop("destination-1");
        assert_eq!(next_hop, Some(&"next-hop-1".to_string()));

        // 添加广播组
        router.add_broadcast_group("group-1".to_string(), vec!["node-1".to_string(), "node-2".to_string()]);

        // 验证广播组
        let group_members = router.get_broadcast_group("group-1");
        assert_eq!(group_members.unwrap().len(), 2);
    }

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::Critical > TaskPriority::High);
        assert!(TaskPriority::High > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Low);
    }

    #[test]
    fn test_node_status_transitions() {
        let mut registry = NodeRegistry::new();

        let node_info = NodeInfo {
            node_id: "test-node".to_string(),
            role: NodeRole::Worker,
            address: "127.0.0.1".to_string(),
            port: 8080,
            last_heartbeat: SystemTime::now(),
            status: NodeStatus::Healthy,
            load_info: NodeLoadInfo {
                cpu_usage: 0.3,
                memory_usage: 0.4,
                current_tasks: 2,
                max_tasks: 10,
                network_latency: 5,
                throughput: 100.0,
            },
            supported_plugins: vec![],
            metadata: HashMap::new(),
        };

        registry.register_node(node_info);
        assert_eq!(registry.healthy_nodes.len(), 1);

        // 更新为不健康状态
        registry.update_node_status("test-node", NodeStatus::Unhealthy);
        assert_eq!(registry.healthy_nodes.len(), 0);

        // 恢复为健康状态
        registry.update_node_status("test-node", NodeStatus::Healthy);
        assert_eq!(registry.healthy_nodes.len(), 1);
    }
}
