# DataFlare Enterprise
## 企业级高性能流处理平台

DataFlare Enterprise 是基于 `@dataflare/plugin` 构建的企业级流处理平台，专注于为大型企业提供高性能、高可用、安全合规的数据流处理能力。

## 🎯 核心定位

### 企业级流处理平台
- **高性能流处理**: 借鉴 Fluvio 设计，支持 >1M 记录/秒吞吐量
- **分布式计算**: 基于 WASM 的分布式数据处理引擎
- **企业级监控**: 全面的可观测性、告警和运维体系
- **安全合规**: 企业级安全、加密、审计和权限管理

### 与 @dataflare/plugin 的关系
- **基础依赖**: 基于 `@dataflare/plugin` 的插件系统构建
- **功能互补**: 专注企业级特性，避免功能重复
- **架构协同**: 利用插件系统的零拷贝和高性能特性

## 🚀 企业级功能

### 🌊 高性能流处理
- **零拷贝处理**: 基于 Fluvio 设计的零拷贝数据处理
- **智能背压控制**: 自适应流量控制和负载管理
- **流式聚合**: 实时窗口计算和聚合分析
- **事件驱动**: 完整的事件处理和重放机制

### 🏗️ 分布式计算
- **集群管理**: 1000+ 节点的大规模集群管理
- **任务调度**: 智能任务分发和负载均衡
- **故障恢复**: 自动故障检测和快速恢复
- **弹性扩缩**: 基于负载的自动扩缩容

### 📊 企业级监控
- **全面监控**: 业务、系统、性能、安全多维度监控
- **分布式追踪**: 端到端的请求追踪和性能分析
- **智能告警**: 基于机器学习的智能告警和预测
- **运维自动化**: 自动化运维和故障处理

### 🔒 安全合规
- **多层安全**: 认证、授权、加密、审计全方位安全
- **合规管理**: SOC2、GDPR、HIPAA 等合规要求支持
- **数据保护**: 端到端数据加密和隐私保护
- **审计追踪**: 100% 操作审计和合规报告

## 快速开始

### 1. 基本使用

```rust
use dataflare_wasm::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 WASM 运行时
    let mut runtime = WasmRuntime::new().await?;

    // 加载插件
    let plugin_config = WasmPluginConfig {
        name: "my_plugin".to_string(),
        module_path: "./plugins/my_plugin.wasm".to_string(),
        config: HashMap::new(),
        security_policy: sandbox::SecurityPolicy::default(),
        memory_limit: 16 * 1024 * 1024, // 16MB
        timeout_ms: 10000,
    };

    let plugin_id = runtime.load_plugin("./plugins/my_plugin.wasm", plugin_config).await?;

    // 执行插件函数
    let result = runtime.call_function(&plugin_id, "process_data", vec![
        serde_json::json!({"input": "test_data"})
    ]).await?;

    println!("插件执行结果: {:?}", result);
    Ok(())
}
```

### 2. 组件管理

```rust
use dataflare_wasm::components::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建组件管理器
    let mut manager = WasmComponentManager::new();

    // 注册 Source 组件
    let source_config = WasmComponentConfig {
        component_type: WasmComponentType::Source,
        name: "csv_source".to_string(),
        module_path: "./plugins/csv_source.wasm".to_string(),
        config: Some({
            let mut config = HashMap::new();
            config.insert("file_path".to_string(), serde_json::json!("./data/input.csv"));
            config
        }),
        runtime_config: None,
        metadata: None,
    };

    manager.register_component(source_config).await?;

    // 获取组件统计信息
    let stats = manager.get_stats();
    println!("组件统计: {:?}", stats);

    Ok(())
}
```

## 插件开发

### 1. 插件结构

WASM 插件需要实现以下标准接口：

```rust
// 插件初始化
#[no_mangle]
pub extern "C" fn plugin_init(config_ptr: *const u8, config_len: usize) -> i32 {
    // 初始化插件
    0 // 返回 0 表示成功
}

// 数据处理
#[no_mangle]
pub extern "C" fn process_data(data_ptr: *const u8, data_len: usize) -> i32 {
    // 处理数据
    0 // 返回处理结果的指针
}

// 插件清理
#[no_mangle]
pub extern "C" fn plugin_cleanup() -> i32 {
    // 清理资源
    0
}
```

### 2. 组件类型

#### Source 组件
```rust
// 数据源组件示例
#[no_mangle]
pub extern "C" fn read_data() -> i32 {
    // 从数据源读取数据
    // 返回数据指针
}
```

#### Transformer 组件
```rust
// 数据转换组件示例
#[no_mangle]
pub extern "C" fn transform_data(input_ptr: *const u8, input_len: usize) -> i32 {
    // 转换数据格式
    // 返回转换后的数据
}
```

#### Filter 组件
```rust
// 数据过滤组件示例
#[no_mangle]
pub extern "C" fn filter_data(input_ptr: *const u8, input_len: usize) -> i32 {
    // 过滤数据
    // 返回过滤结果
}
```

## 配置说明

### 插件配置

```json
{
  "name": "my_plugin",
  "type": "transformer",
  "module_path": "./plugins/my_plugin.wasm",
  "config": {
    "param1": "value1",
    "param2": 42
  },
  "security_policy": {
    "allow_network": false,
    "allow_file_system": true,
    "allow_env_vars": false,
    "max_memory": 16777216,
    "max_execution_time": 10000,
    "allowed_paths": ["./data/"]
  },
  "memory_limit": 16777216,
  "timeout_ms": 10000,
  "metadata": {
    "author": "Your Name",
    "version": "1.0.0",
    "description": "插件描述"
  }
}
```

### 安全策略

```rust
use dataflare_wasm::sandbox::SecurityPolicy;

let policy = SecurityPolicy {
    allow_network: false,        // 禁止网络访问
    allow_file_system: true,     // 允许文件系统访问
    allow_env_vars: false,       // 禁止环境变量访问
    max_memory: 16 * 1024 * 1024, // 最大内存 16MB
    max_execution_time: 10000,   // 最大执行时间 10秒
    allowed_paths: vec!["./data/".to_string()], // 允许访问的路径
    allowed_hosts: vec![],       // 允许访问的主机
    allowed_env_vars: vec![],    // 允许访问的环境变量
};
```

## 主机函数

WASM 插件可以调用以下主机函数：

### 日志函数
```rust
// 在插件中调用
host_log("info", "这是一条日志消息");
```

### 时间函数
```rust
// 获取当前时间戳
let timestamp = host_get_time();
```

### 随机数函数
```rust
// 生成随机数
let random_value = host_random();
```

## 错误处理

```rust
use dataflare_wasm::{WasmError, WasmResult};

fn handle_plugin_error(result: WasmResult<String>) {
    match result {
        Ok(value) => println!("成功: {}", value),
        Err(WasmError::Configuration(msg)) => eprintln!("配置错误: {}", msg),
        Err(WasmError::Runtime(msg)) => eprintln!("运行时错误: {}", msg),
        Err(WasmError::PluginLoad(msg)) => eprintln!("插件加载错误: {}", msg),
        Err(WasmError::PluginExecution(msg)) => eprintln!("插件执行错误: {}", msg),
        Err(e) => eprintln!("其他错误: {}", e),
    }
}
```

## 性能优化

### 1. 内存管理
- 设置合适的内存限制
- 及时释放不需要的资源
- 使用流式处理处理大数据

### 2. 执行优化
- 设置合理的超时时间
- 使用异步执行避免阻塞
- 批量处理数据提高效率

### 3. 缓存策略
- 缓存编译后的 WASM 模块
- 复用插件实例
- 预加载常用插件

## 测试

运行测试：

```bash
# 运行所有测试
cargo test --manifest-path dataflare/wasm/Cargo.toml

# 运行集成测试
cargo test --manifest-path dataflare/wasm/Cargo.toml integration_tests

# 运行特定测试
cargo test --manifest-path dataflare/wasm/Cargo.toml test_wasm_runtime_creation
```

## 示例

查看 `examples/` 目录中的示例配置和插件代码。

## 贡献

欢迎贡献代码和提出建议！请查看项目的贡献指南。

## 许可证

本项目采用 MIT 许可证。
