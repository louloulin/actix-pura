# Actix-Pura 开发计划

## 当前系统分析

Actix-Pura 是一个具有分布式能力的 Rust 基础 actor 框架，受到 Akka 和 Proto.Actor 等框架的启发。当前实现包括：

1. **核心 Actor 系统**：强大的 actor 框架，具有消息传递能力
2. **分布式集群**：支持中心化和去中心化架构
3. **消息代理**：用于节点内和节点间消息分发
4. **发现机制**：用于节点发现和管理
5. **序列化**：支持多种序列化格式
6. **性能测试**：用于优化的基准测试工具

## 框架比较

### Akka.Net vs Actix-Pura

| 特性 | Akka.Net | Actix-Pura（当前） | 备注 |
|------|----------|-------------------|------|
| 核心 Actor 模型 | 传统 actor 层次结构，监督 | 带有监督的 actor 层次结构 | 相似的核心模型 |
| 集群 | 集群分片（基于组） | Actor 级别分发 | 不同的集群方法 |
| 序列化 | 自定义（带有 Hyperion 替代方案） | 多种格式（bincode、JSON） | Actix-Pura 更灵活 |
| 配置 | 基于 HOCON | 基于 Rust 结构体 | Actix-Pura 对 Rust 更符合习惯 |
| 远程通信 | 自定义协议 | 自定义协议 | 类似的方法 |
| 持久化 | 内置（事件溯源） | 未完全实现 | Actix-Pura 的差距 |
| 监控 | 商业工具（Phobos） | 基本指标 | Actix-Pura 的差距 |
| 成熟度 | 生产就绪 | 开发中 | Akka.Net 更成熟 |

### Proto.Actor vs Actix-Pura

| 特性 | Proto.Actor | Actix-Pura（当前） | 备注 |
|------|------------|-------------------|------|
| 理念 | "不重新发明轮子" | 自定义实现 | 不同的方法 |
| 序列化 | gRPC 与 protobuf | 自定义序列化 | Proto.Actor 利用标准 |
| 集群 | 标准提供者（Consul 等） | 自定义实现 | Proto.Actor 更集成 |
| Actor 模型 | 虚拟 actors 作为一等公民 | 新兴的虚拟 actor 支持 | Actix-Pura 的差距 |
| 本地亲和性 | 支持 | 已实现 ✅ | 功能已添加 (2024-04-06) |
| 编程 API | 清晰，对开发者友好 | 功能性但复杂 | 改进机会 |
| 跨语言 | 有 Go 版本 | 仅 Rust | 不同的目标用例 |

## 改进领域

基于与 Akka 和 Proto.Actor 的比较，确定了以下改进领域：

### 1. 核心架构改进

- [x] **编程 API 优化**：简化 actor 创建和消息传递 API，使其更加开发者友好（受 Proto.Actor 方法启发）
- [x] **本地亲和性机制**：实现类似 Proto.Actor 的本地亲和性机制，优化本地与远程通信
- [x] **增强监督**：使用更复杂的恢复策略改进监督层次结构

### 2. 序列化与通信

- [ ] **标准集成**：不使用自定义序列化，而是利用行业标准如 gRPC 与 Protocol Buffers（受 Proto.Actor 启发）
- [ ] **可插拔传输**：实现更模块化的传输系统，允许不同的通信机制
- [ ] **消息压缩**：为大型消息添加自动压缩
- [ ] **通信安全**：为安全的 actor 通信实现 TLS 和认证

### 3. 集群增强

- [ ] **集群分片**：实现类 Akka 的集群分片以提高可扩展性
- [ ] **外部服务集成**：支持外部服务发现（Consul、Zookeeper、Kubernetes API）
- [ ] **高级放置策略**：实现更复杂的 actor 放置算法
- [ ] **动态集群重配置**：允许在不停机的情况下添加/移除节点

### 4. 持久化与状态管理

- [ ] **事件溯源**：为 actor 状态持久化实现事件溯源
- [ ] **状态快照**：支持状态快照以改善恢复时间
- [ ] **自定义存储后端**：为不同环境提供可插拔存储后端
- [ ] **事务支持**：为某些操作提供可选的事务保证

### 5. 可观察性与监控

- [ ] **OpenTelemetry 集成**：使用 OpenTelemetry 添加跟踪和指标支持
- [ ] **性能仪表板**：创建基于 Web 的集群监控仪表板
- [ ] **日志增强**：具有不同严重级别的结构化日志
- [ ] **警报与通知**：基于系统健康状况的可配置警报

### 6. AI 集成功能

- [ ] **AI Agent 框架**：AI 驱动的 actors 的核心抽象
- [ ] **LLM 集成**：与大型语言模型的原生集成
- [ ] **Agent 通信协议**：专用于 agent 间通信的协议
- [ ] **多 Agent 协调**：agent 合作与协调的原语
- [ ] **工具使用能力**：agents 使用外部工具的框架
- [ ] **记忆管理**：agents 的短期和长期记忆
- [ ] **观察能力**：agents 观察其环境的系统
- [ ] **规划机制**：支持 agents 中的分层规划

## 实施计划

### 阶段 1：基础改进（2024 年第二季度）

1. **API 优化**
   - 简化 actor 创建和注册
   - 改进消息发送模式
   - 创建更直观的错误处理

2. **序列化增强**
   - 集成 Protocol Buffers 用于消息序列化
   - 实现基于 gRPC 的通信
   - 添加消息压缩

3. **核心集群改进**
   - 增强节点发现机制
   - 改进 actor 位置透明性
   - 实现更稳健的故障检测

### 阶段 2：高级集群（2024 年第三季度）

1. **集群分片**
   - 基于一致性哈希实现分片
   - 添加分片重平衡支持
   - 为 actors 创建迁移策略

2. **外部集成**
   - 添加 Consul/Zookeeper/Kubernetes 集成
   - 实现云服务提供商服务发现
   - 为常见环境创建部署模板

3. **持久化框架**
   - 设计事件溯源抽象
   - 实现状态快照
   - 为常见数据库添加存储后端

### 阶段 3：可观察性与生产就绪（2024 年第四季度）

1. **监控系统**
   - 实现 OpenTelemetry 集成
   - 创建性能仪表板
   - 添加告警能力

2. **安全增强**
   - 为通信添加 TLS 支持
   - 实现认证和授权
   - 为敏感数据添加加密

3. **生产工具**
   - 创建部署文档
   - 实现混沌测试工具
   - 添加性能调优指南

### 阶段 4：AI Agent 集成（2025 年第一、二季度）

1. **Agent 核心**
   - 设计 AI agents 的基本抽象
   - 实现 LLM 集成
   - 创建 agent 生命周期管理

2. **Agent 能力**
   - 实现记忆管理
   - 添加工具使用框架
   - 创建观察系统

3. **多 Agent 系统**
   - 设计协调协议
   - 实现协作原语
   - 添加竞争和谈判支持

## AI Agent 集成详情

### Agent Actor 模型

```rust
// 具有 LLM 能力的 Agent actor
struct AgentActor<T: LLMProvider> {
    llm: T,
    memory: AgentMemory,
    tools: ToolRegistry,
    state: AgentState,
    config: AgentConfig,
}

impl<T: LLMProvider> DistributedActor for AgentActor<T> {
    // 实现细节
}

// 用于执行任务的 Agent 消息
#[derive(Message, Serialize, Deserialize)]
#[rtype(result = "TaskResult")]
struct ExecuteTask {
    instruction: String,
    context: TaskContext,
    constraints: Vec<Constraint>,
}

// Agent 记忆管理
struct AgentMemory {
    short_term: VecDeque<Memory>,
    long_term: Box<dyn MemoryStore>,
    working: HashMap<String, any::Any>,
}
```

### 多 Agent 协调

```rust
// 用于管理多个 agents 的 Agent 协调器
struct AgentCoordinator {
    agents: HashMap<AgentId, Addr<AgentActor>>,
    tasks: TaskQueue,
    strategy: CoordinationStrategy,
}

// 任务分配机制
enum CoordinationStrategy {
    Hierarchical,  // 领导者分配任务
    Collaborative, // Agents 协商
    Competitive,   // Agents 竞标任务
    Market,        // 基于价格的分配
}

// Agents 之间的通信
#[derive(Message, Serialize, Deserialize)]
#[rtype(result = "ProposalResponse")]
struct Proposal {
    proposal_id: Uuid,
    content: String,
    deadline: DateTime<Utc>,
}
```

### 工具集成框架

```rust
// Agent 能力的工具注册表
struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

// Agent 能力的工具特质
trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value, ToolError>;
    fn parameter_schema(&self) -> serde_json::Value;
}
```

## 已实现功能详情

### 本地亲和性机制 (2024年4月6日)

本地亲和性机制允许相关的Actor优先部署在同一节点上，以优化通信效率。该实现包括：

1. **PlacementStrategy扩展**：添加了LocalAffinity策略选项，支持指定亲和性分组和回退策略
   ```rust
   PlacementStrategy::LocalAffinity {
       fallback: Box<PlacementStrategy>,
       group: Option<String>,
   }
   ```

2. **位置选择算法**：
   - 优先选择本地节点
   - 如果指定了分组，查找运行同一分组Actor的其他节点
   - 如果前两种策略都不可用，使用回退策略

3. **分组支持**：通过亲和性组ID将相关Actor分组，帮助它们保持在同一节点

4. **全面的测试**：添加了单元测试验证不同场景下的行为：
   - 本地节点可用时选择本地节点
   - 有分组亲和性要求时选择同组节点
   - 无匹配节点时使用回退策略

5. **示例应用**：提供了LocalAffinity使用示例 (`local_affinity_example.rs`)，演示了如何:
   - 创建使用LocalAffinity的Actor
   - 配置亲和性分组
   - 利用本地亲和性优化相关Actor的部署

此功能的实现显著提高了集群中的Actor通信效率，特别是对于频繁交互的Actor组。通过将相关Actor放置在同一节点上，可以减少网络通信开销，提高系统整体性能。

### 编程 API 优化 (2024年4月6日)

API优化实现使Actor创建和使用更加直观和开发者友好，参考了Proto.Actor的设计理念。具体改进包括：

1. **ActorProps构建器模式**：添加了流式API用于配置Actor
   ```rust
   let addr = actor.props()
       .with_path("/user/custom-path")
       .with_local_affinity(Some("group1"), PlacementStrategy::RoundRobin)
       .with_format(SerializationFormat::Json)
       .start();
   ```

2. **简化的Actor创建**：添加了factory函数简化创建过程
   ```rust
   let addr = actor(MyActor::new())
       .with_round_robin()
       .start();
   ```

3. **异步API增强**：添加了更好的异步支持和超时处理
   ```rust
   // 使用超时的ask模式
   let result = actor_ref.ask_async(&addr, SomeMessage, Duration::from_secs(5)).await?;
   ```

4. **策略配置简化**：添加了描述性方法简化放置策略配置
   ```rust
   let props = actor.props()
       .with_local_affinity(...)   // 本地亲和性
       .with_round_robin()         // 轮询策略
       .with_least_loaded()        // 最小负载策略
       .with_redundancy(3);        // 冗余部署
   ```

此功能的实现大大提高了框架的可用性和开发体验，使复杂的分布式actor配置变得更加简单和直观。新API减少了样板代码，并通过流式接口使代码更具可读性。

### 增强监督 (2024年7月1日)

增强监督功能实现了更复杂的故障恢复策略和分布式环境下的actor监督机制，参考了Akka的监督模型。实现包括：

1. **灵活的监督策略**：提供了多种故障处理策略
   ```rust
   enum SupervisionStrategy {
       // 在同一节点重启actor
       Restart { max_restarts: usize, window: Duration, delay: Duration },
       // 停止actor不尝试恢复
       Stop,
       // 将故障上报给父级监督者
       Escalate,
       // 在不同节点重启actor
       Relocate { placement: PlacementStrategy, max_relocations: usize, delay: Duration },
       // 忽略故障继续执行
       Resume,
       // 根据错误类型选择不同策略
       Match { default: Box<SupervisionStrategy>, matchers: Vec<SupervisionMatcher> },
   }
   ```

2. **基于错误类型的监督**：支持根据不同错误类型应用不同的监督策略
   ```rust
   let strategy = SupervisionStrategy::Match {
       default: Box::new(SupervisionStrategy::Restart { ... }),
       matchers: vec![
           SupervisionMatcher {
               error_type: "std::io::Error".to_string(),
               strategy: Box::new(SupervisionStrategy::Relocate { ... }),
           },
           SupervisionMatcher {
               error_type: "MyCustomError".to_string(),
               strategy: Box::new(SupervisionStrategy::Stop),
           },
       ],
   };
   ```

3. **监督生命周期钩子**：为分布式actor提供监督生命周期事件通知
   ```rust
   impl SupervisedDistributedActor for MyActor {
       fn before_restart(&mut self, ctx: &mut Context<Self>, failure: Option<FailureInfo>) {
           // 重启前的清理逻辑
       }
       
       fn after_restart(&mut self, ctx: &mut Context<Self>, failure: Option<FailureInfo>) {
           // 重启后的初始化逻辑
       }
       
       fn before_relocate(&mut self, ctx: &mut Context<Self>, target_node: NodeId, failure: Option<FailureInfo>) {
           // 迁移前的准备逻辑
       }
   }
   ```

4. **故障信息跟踪**：记录详细的故障信息，包括失败时间、故障类型和重启次数
   ```rust
   pub struct FailureInfo {
       pub actor_path: String,
       pub node_id: NodeId,
       pub time: u64,
       pub error: String,
       pub error_type: String,
       pub restart_count: usize,
       pub failure_id: Uuid,
   }
   ```

5. **用于监督的Fluent API**：为actor监督提供简洁的配置接口
   ```rust
   let supervised_actor = actor.props()
       .with_supervision(
           SupervisionStrategy::Restart {
               max_restarts: 5,
               window: Duration::from_secs(60),
               delay: Duration::from_millis(500),
           }
       )
       .start();
   ```

6. **示例应用**：提供了增强监督示例 (`supervised_actor.rs`)，演示了如何:
   - 配置不同的监督策略
   - 处理actor故障
   - 实现故障恢复机制
   - 监控故障历史

此功能实现大大提升了系统的弹性，使分布式actor系统能更优雅地处理各种故障场景。新的监督机制使开发者能够为不同类型的actor配置最适合的恢复策略，从而提高系统的整体可用性和稳定性。

## 结论

Actix-Pura 有潜力成为领先的基于 Rust 的分布式 actor 框架，结合了 Akka 和 Proto.Actor 等既定系统的最佳特性，同时保持 Rust 的性能和安全保证。通过解决已确定的改进领域并添加 AI agent 能力，Actix-Pura 可以同时作为传统分布式计算平台和高级 AI 系统的基础。

该实施计划提供了在未来 18 个月内增强框架的结构化方法，从核心改进到高级 AI 集成的明确进展。 