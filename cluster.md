# Actix 分布式集群实现计划

## 当前系统分析

Actix 是一个强大的 Rust Actor 框架，使用消息传递并发模型。当前实现提供：

1. **Actor 系统**：具有消息传递功能的核心 actor 框架
2. **消息代理**：单节点内的消息代理（SystemBroker/ArbiterBroker）
3. **SyncArbiter**：用于并发工作负载的线程池 actors
4. **监督机制**：Actor 生命周期管理和重启能力

该框架目前设计用于单节点操作，所有 actors 在同一进程或机器内运行。

## 分布式 Actix 目标

扩展 Actix 以支持跨多个节点的分布式计算，具备以下能力：

1. ✅ 节点发现和管理
2. 远程 actor 寻址
3. 分布式消息传递
4. 容错和恢复
5. ✅ 支持中心化和去中心化架构

## 架构设计

### 通用组件

#### 1. 节点标识
- ✅ 唯一节点 ID 生成
- ✅ 节点元数据（IP、端口、能力、角色）
- ✅ 节点心跳和健康监控

#### 2. 序列化层
- ✅ 用于网络传输的消息序列化/反序列化
- ✅ 支持多种序列化格式（bincode、JSON、MessagePack）
- ✅ 大型消息的压缩选项

#### 3. 传输层
- TCP/TLS 基于可靠通信
- UDP 用于性能关键、容忍丢失的通信
- WebSocket 支持网络友好部署

#### 4. 分布式 Actor 注册表
- 全局 actor 命名和寻址
- Actor 位置解析
- Actor 迁移跟踪

#### 5. 分布式消息代理
- 扩展现有代理系统以支持远程订阅
- 跨节点消息路由
- 消息传递保证（最多一次、至少一次、恰好一次）

### 中心化架构

在中心化模型中，主节点协调集群：

#### 1. 主节点
- ✅ 集群成员管理
- 全局 actor 注册表
- 工作分配和负载均衡
- ✅ 故障检测和恢复

#### 2. 工作节点
- ✅ 本地 actor 执行
- ✅ 资源利用报告
- 任务执行和结果报告

#### 3. 通信流
- ✅ 工作节点 → 主节点：注册、心跳、状态更新
- 主节点 → 工作节点：Actor 部署、消息路由、控制命令
- 工作节点 → 工作节点：直接消息传递（通过主节点协调）

#### 4. 容错
- ✅ 故障检测
- 主节点冗余的状态复制
- 工作节点故障恢复

### 去中心化架构

在去中心化模型中，所有节点作为对等节点运行：

#### 1. 点对点网络
- ✅ 节点发现
- 用于 actor 位置的分布式哈希表（DHT）
- 用于负载分配的一致性哈希

#### 2. 共识机制
- 用于集群范围决策的 Raft 或类似算法
- 最终一致的 actor 注册表
- 冲突解决策略

#### 3. 通信流
- 直接点对点消息传递
- 通过路由表转发消息
- 用于组通信的多播

#### 4. 容错
- ✅ 无单点故障
- 节点故障时自动重新分配 actors
- 冗余 actor 放置策略

## 实施计划

### 阶段 1：基础设施 - 进行中

1. **节点管理系统** ✅
   - ✅ 实现节点表示和生命周期
   - ✅ 创建节点参数配置系统
   - ✅ 开发本地节点发现机制

2. **网络传输层**
   - 实现 TCP/TLS 传输
   - 设计消息帧和序列化
   - 创建连接池和管理

3. **远程消息传递原语**
   - ✅ 扩展序列化支持
   - 实现远程消息信封
   - 创建远程地址解析

### 阶段 2：分布式代理

1. **NetworkBroker 实现**
   - 扩展代理系统以支持远程订阅
   - 实现分布式消息路由
   - 创建消息传递跟踪和确认

2. **集群感知注册表**
   - 实现分布式 actor 注册表
   - 创建位置透明的 actor 引用
   - 设计 actor 迁移协议

3. **故障检测** ✅
   - ✅ 实现节点故障检测
   - 创建跨节点的 actor 监督
   - 设计故障恢复策略

### 阶段 3：中心化实现

1. **主节点实现** - 部分完成
   - ✅ 创建主节点 actor 系统
   - ✅ 实现集群成员管理
   - 设计工作分配算法

2. **工作节点实现** - 部分完成
   - ✅ 创建工作节点注册
   - ✅ 实现资源报告
   - 设计 actor 部署机制

3. **主节点冗余**
   - 实现主节点选举算法
   - 创建状态复制机制
   - 设计故障转移协议

### 阶段 4：去中心化实现

1. **点对点网络实现** - 部分完成
   - ✅ 实现节点发现
   - 创建分布式哈希表
   - 设计一致性哈希机制

2. **共识层**
   - 实现 Raft 共识算法
   - 创建分布式 actor 注册表
   - 设计冲突解决策略

3. **自愈机制**
   - 实现自动 actor 重新分配
   - 创建冗余 actor 放置
   - 设计动态集群重配置

## API 设计

### 集群配置

```rust
// 在 Cargo.toml 中
// actix-cluster = "0.1"

// 在代码中
use actix::prelude::*;
use actix_cluster::{ClusterConfig, NodeRole, DiscoveryMethod};

// 中心化集群配置
let config = ClusterConfig::new()
    .architecture(Architecture::Centralized)
    .node_role(NodeRole::Worker) // 或 Master
    .master_nodes(vec!["master-1.example.com:8000"])
    .discovery(DiscoveryMethod::Static)
    .build();

// 去中心化集群配置
let config = ClusterConfig::new()
    .architecture(Architecture::Decentralized)
    .seed_nodes(vec!["node-1.example.com:8000", "node-2.example.com:8000"])
    .discovery(DiscoveryMethod::Gossip)
    .build();

// 启动集群系统
let system = ClusterSystem::new("my-cluster", config).start();
```

### 分布式 Actors

```rust
// 定义可分发的 actor
#[derive(Clone, Serialize, Deserialize)]
struct MyDistributedActor {
    state: String,
}

impl DistributedActor for MyDistributedActor {
    type Context = DistributedContext<Self>;
    
    // 可选：指定放置策略
    fn placement_strategy(&self) -> PlacementStrategy {
        PlacementStrategy::RoundRobin
    }
    
    // 可选：自定义序列化
    fn serialization_format(&self) -> SerializationFormat {
        SerializationFormat::Bincode
    }
}

// 启动分布式 actor
let addr = MyDistributedActor { state: "initial".to_string() }
    .start_distributed();

// 在特定节点上创建远程 actor
let addr = system.remote_actor::<MyDistributedActor>("node-2", |ctx| {
    MyDistributedActor { state: "remote".to_string() }
});
```

### 分布式消息代理

```rust
// 定义分布式消息
#[derive(Clone, Message, Serialize, Deserialize)]
#[rtype(result = "()")]
struct MyDistributedMessage(String);

// 订阅集群范围的消息
actor.subscribe_cluster::<MyDistributedMessage>(ctx);

// 向集群中所有订阅者发布消息
system.publish(MyDistributedMessage("Hello Cluster".to_string()));

// 使用传递保证发布
system.publish_with_guarantee(
    MyDistributedMessage("Important message".to_string()),
    DeliveryGuarantee::ExactlyOnce
);
```

## 测试策略

1. **单元测试** ✅
   - ✅ 测试序列化/反序列化
   - ✅ 测试消息路由逻辑
   - ✅ 测试故障检测算法

2. **集成测试**
   - ✅ 测试节点发现和注册
   - 测试远程 actor 通信
   - 测试代理订阅和发布

3. **分布式测试**
   - 测试多节点集群形成
   - 测试节点故障和恢复
   - 测试负载下的性能

4. **混沌测试**
   - 随机节点故障
   - 网络分区
   - 消息延迟和丢弃

## 部署考虑

1. **Docker 集成**
   - 容器友好配置
   - Docker Compose 示例
   - Kubernetes StatefulSet 模板

2. **云部署**
   - AWS/GCP/Azure 部署指南
   - 自动扩展配置
   - 负载均衡器集成

3. **监控**
   - Prometheus 指标集成
   - 集群健康监控
   - 性能仪表板

## 安全考虑

1. **认证**
   - 节点认证机制
   - Actor 系统访问控制
   - 传输安全的 TLS

2. **授权**
   - Actor 级别权限
   - 消息类型限制
   - 资源使用限制

3. **加密**
   - 传输加密
   - 消息负载加密
   - 安全密钥分发

## 时间线和优先级

1. **阶段 1：基础设施** - 8 周
   - 建立核心分布式基础设施
   - 确保稳健的网络通信
   - 构建序列化框架

2. **阶段 2：分布式代理** - 6 周
   - 创建可靠的消息路由
   - 实现分布式注册表
   - 构建故障检测

3. **阶段 3：中心化架构** - 4 周
   - 实现主从模式
   - 构建负载均衡
   - 创建主节点冗余

4. **阶段 4：去中心化架构** - 6 周
   - 实现点对点网络
   - 构建共识算法
   - 创建自愈机制

5. **测试和文档** - 4 周
   - 综合测试套件
   - 用户和 API 文档
   - 示例应用

总计估计：完整实现需要 28 周。

## 当前实现进度

- ✅ 基础结构设计完成
- ✅ 节点标识和管理系统实现
- ✅ 序列化层实现，支持 Bincode 和 JSON
- ✅ 服务发现基础设施实现，支持静态节点列表
- ✅ 集群系统基础实现，支持中心化和去中心化架构
- ✅ 节点健康检查和故障检测
- ⬜ 远程消息传递
- ⬜ 分布式 Actor 注册
- ⬜ 分布式消息代理
- ⬜ Actor 迁移和放置策略 