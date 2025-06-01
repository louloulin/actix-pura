# DataFlare Plugin 使用指南

## 🚀 快速开始

### 基本使用

```rust
use dataflare_plugin::{
    PluginManager, PluginManagerConfig,
    core::{PluginRecord, PluginResult, PluginInfo, DataFlarePlugin},
    interface::{PluginType, PluginState, PluginError},
};

// 1. 创建插件管理器
let config = PluginManagerConfig::default();
let manager = PluginManager::new(config)?;

// 2. 实现自定义插件
struct MyPlugin;

impl DataFlarePlugin for MyPlugin {
    fn process(&self, record: &PluginRecord) -> Result<PluginResult, PluginError> {
        // 处理数据
        let processed_data = process_data(record.value)?;
        Ok(PluginResult::success(processed_data))
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "my_plugin".to_string(),
            version: "1.0.0".to_string(),
            plugin_type: PluginType::Transformer,
            description: "My custom plugin".to_string(),
            author: "Your Name".to_string(),
            api_version: "1.0".to_string(),
        }
    }

    fn get_state(&self) -> PluginState {
        PluginState::Ready
    }
}

// 3. 注册和使用插件
manager.register_plugin("my_plugin".to_string(), MyPlugin)?;
let result = manager.process_record("my_plugin", &record)?;
```

## 🧠 内存池优化

### 配置内存池

```rust
use dataflare_plugin::{MemoryPool, MemoryPoolConfig, BufferSize};
use std::time::Duration;

let config = MemoryPoolConfig {
    max_tiny_buffers: 1000,    // 256B 缓冲区
    max_small_buffers: 500,    // 4KB 缓冲区
    max_medium_buffers: 100,   // 64KB 缓冲区
    max_large_buffers: 50,     // 1MB 缓冲区
    max_huge_buffers: 10,      // 4MB 缓冲区
    cleanup_interval: Duration::from_secs(60),
    max_idle_time: Duration::from_secs(300),
    enable_metrics: true,
};

let pool = MemoryPool::new(config);
pool.warmup(); // 预热内存池
```

### 使用内存池

```rust
// 获取缓冲区
let buffer = pool.get_buffer(1024); // 自动选择合适的缓冲区类型

// 使用缓冲区
// ... 处理数据 ...

// 归还缓冲区
pool.return_buffer(buffer);

// 查看统计信息
let stats = pool.get_stats();
println!("缓存命中率: {:.1}%", stats.cache_hit_rate * 100.0);
```

## 📦 批处理优化

### 实现批处理插件

```rust
use dataflare_plugin::{BatchPlugin, BatchResult};

impl BatchPlugin for MyPlugin {
    fn process_batch(&self, records: &[PluginRecord]) -> Result<BatchResult> {
        let start_time = Instant::now();
        let mut results = Vec::new();
        let mut errors = Vec::new();

        // 批量处理记录
        for (idx, record) in records.iter().enumerate() {
            match self.process(record) {
                Ok(result) => results.push(result),
                Err(error) => errors.push((idx, error.into())),
            }
        }

        let processing_time = start_time.elapsed();
        
        Ok(BatchResult {
            results,
            errors,
            processed_count: records.len(),
            success_count: results.len(),
            error_count: errors.len(),
            processing_time,
            throughput_per_second: records.len() as f64 / processing_time.as_secs_f64(),
        })
    }

    fn optimize_batch_size(&self, current_size: usize, latency: Duration, throughput: f64) -> usize {
        // 智能批次大小优化
        let target_latency = Duration::from_millis(100);
        
        if latency > target_latency && current_size > 10 {
            current_size * 80 / 100  // 减少批次大小
        } else if throughput < 1000.0 && current_size < 1000 {
            current_size * 120 / 100 // 增加批次大小
        } else {
            current_size
        }
    }
}
```

### 使用批处理

```rust
// 批量处理记录
let result = manager.process_batch("my_plugin", &records)?;

println!("处理了 {} 条记录", result.processed_count);
println!("成功 {} 条，失败 {} 条", result.success_count, result.error_count);
println!("吞吐量: {:.0} records/sec", result.throughput_per_second);
```

## 🔧 高级配置

### 插件管理器配置

```rust
use dataflare_plugin::{PluginManagerConfig, MemoryPoolConfig, BatchConfig};

let config = PluginManagerConfig {
    memory_pool_config: MemoryPoolConfig {
        max_tiny_buffers: 2000,
        max_small_buffers: 1000,
        // ... 其他配置
        enable_metrics: true,
    },
    batch_config: BatchConfig {
        default_batch_size: 100,
        max_batch_size: 1000,
        target_latency: Duration::from_millis(50),
        min_throughput: 2000.0,
        enable_auto_optimization: true,
        enable_parallel: true,
        parallelism: 4,
    },
    enable_auto_optimization: true,
    monitoring_interval: Duration::from_secs(30),
    max_plugin_instances: 10,
    health_check_interval: Duration::from_secs(60),
};

let manager = PluginManager::new(config)?;
```

## 📊 性能监控

### 获取性能指标

```rust
// 获取插件指标
let metrics = manager.get_metrics("my_plugin");
if let Some(metrics) = metrics {
    println!("总处理次数: {}", metrics.total_processed);
    println!("平均延迟: {:.2}ms", metrics.average_latency.as_millis());
    println!("平均吞吐量: {:.0} records/sec", metrics.average_throughput);
    println!("错误率: {:.2}%", metrics.error_rate * 100.0);
}

// 获取内存池统计
let pool_stats = manager.get_memory_pool_stats();
println!("内存使用效率: {:.1}%", pool_stats.memory_efficiency * 100.0);
println!("缓存命中率: {:.1}%", pool_stats.cache_hit_rate * 100.0);
```

### 健康检查

```rust
// 执行健康检查
manager.health_check()?;

// 获取系统状态
let status = manager.get_system_status();
println!("系统状态: {:?}", status);
```

## 🎯 性能优化建议

### 1. 内存优化
- **预热内存池**: 在高负载前调用 `pool.warmup()`
- **合理配置缓冲区大小**: 根据数据特征调整各级缓冲区数量
- **监控内存使用**: 定期检查内存效率和缓存命中率

### 2. 批处理优化
- **选择合适的批次大小**: 平衡延迟和吞吐量
- **启用自动优化**: 让系统根据实际性能动态调整
- **并行处理**: 对于CPU密集型任务启用并行处理

### 3. 插件设计
- **避免阻塞操作**: 使用异步处理或快速返回
- **最小化内存分配**: 复用缓冲区和对象
- **错误处理**: 提供详细的错误信息便于调试

### 4. 监控和调优
- **定期监控指标**: 关注延迟、吞吐量和错误率
- **性能基准测试**: 使用 `cargo bench` 验证优化效果
- **负载测试**: 在生产环境前进行充分测试

## 🔍 故障排除

### 常见问题

1. **内存使用过高**
   - 检查缓冲区配置是否合理
   - 确认缓冲区正确归还到池中
   - 调整清理间隔和空闲时间

2. **性能不达预期**
   - 启用性能监控查看瓶颈
   - 调整批次大小和并行度
   - 检查插件实现是否有阻塞操作

3. **错误率过高**
   - 查看详细错误信息
   - 检查输入数据格式
   - 验证插件逻辑正确性

### 调试工具

```rust
// 启用详细日志
env_logger::init();

// 使用调试模式
let config = PluginManagerConfig {
    enable_debug_mode: true,
    // ...
};

// 导出性能报告
manager.export_performance_report("performance_report.json")?;
```

## 📚 示例代码

完整的示例代码请参考：
- `examples/performance_demo.rs` - 性能演示
- `benches/performance_benchmark.rs` - 性能基准测试
- `tests/integration_tests.rs` - 集成测试

## 🚀 下一步

1. **运行性能演示**: `cargo run --example performance_demo`
2. **执行基准测试**: `cargo bench`
3. **查看测试覆盖**: `cargo test`
4. **阅读API文档**: `cargo doc --open`

---

更多信息请参考 [DataFlare 官方文档](https://docs.dataflare.dev) 或提交 [Issue](https://github.com/dataflare/dataflare/issues)。
