# DataFlare 插件系统

基于Fluvio SmartModule设计理念的高性能、零拷贝插件系统。

## 🚀 第一阶段完成情况

### ✅ 已实现功能

- **零拷贝数据结构**：`PluginRecord<'a>`支持高性能数据处理
- **核心插件接口**：`SmartPlugin` trait提供统一的插件开发接口
- **适配器桥接**：`SmartPluginAdapter`无缝集成现有DataFlare Processor系统
- **完整错误处理**：从`PluginError`到`DataFlareError`的完整错误链
- **性能基准测试**：验证纳秒级处理性能
- **全面测试覆盖**：17个测试用例，覆盖所有核心功能

### 🏆 性能指标

基准测试结果：
- **Filter插件**：20-55ns 处理时间
- **Map插件**：57-101ns 处理时间  
- **复杂插件**：98ns-1.18μs 处理时间
- **适配器开销**：<47ns（几乎无性能损失）

## 📦 快速开始

### 1. 添加依赖

```toml
[dependencies]
dataflare-plugin = { path = "../plugin" }
```

### 2. 创建过滤器插件

```rust
use dataflare_plugin::{SmartPlugin, PluginRecord, PluginResult, PluginType, Result};

pub struct ErrorFilter;

impl SmartPlugin for ErrorFilter {
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
        let data = record.value_as_str()?;
        Ok(PluginResult::Filtered(data.contains("error")))
    }
    
    fn name(&self) -> &str { "error-filter" }
    fn version(&self) -> &str { "1.0.0" }
    fn plugin_type(&self) -> PluginType { PluginType::Filter }
}
```

### 3. 创建映射插件

```rust
use dataflare_plugin::{SmartPlugin, PluginRecord, PluginResult, PluginType, Result};

pub struct UppercaseMap;

impl SmartPlugin for UppercaseMap {
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
        let data = record.value_as_str()?;
        let result = data.to_uppercase();
        Ok(PluginResult::Mapped(result.into_bytes()))
    }
    
    fn name(&self) -> &str { "uppercase-map" }
    fn version(&self) -> &str { "1.0.0" }
    fn plugin_type(&self) -> PluginType { PluginType::Map }
}
```

### 4. 在DataFlare工作流中使用

```rust
use dataflare_plugin::SmartPluginAdapter;
use dataflare_core::processor::Processor;

// 创建插件适配器
let plugin = Box::new(ErrorFilter);
let mut adapter = SmartPluginAdapter::new(plugin);

// 在工作流中使用
let result = adapter.process_record(&data_record).await?;
```

## 📁 项目结构

```
dataflare/plugin/
├── src/
│   ├── lib.rs              # 主要API导出
│   ├── plugin.rs           # SmartPlugin trait定义
│   ├── record.rs           # PluginRecord数据结构
│   ├── adapter.rs          # SmartPluginAdapter适配器
│   ├── error.rs            # 错误处理
│   └── test_utils.rs       # 测试工具
├── examples/
│   ├── simple_filter.rs    # 过滤器插件示例
│   └── simple_map.rs       # 映射插件示例
├── tests/
│   └── integration_tests.rs # 集成测试
├── benches/
│   └── plugin_performance.rs # 性能基准测试
└── Cargo.toml
```

## 🧪 运行测试

```bash
# 运行所有测试
cargo test

# 运行基准测试
cargo bench

# 查看测试覆盖率
cargo test --lib
```

## 📊 API 文档

### 核心类型

- **`PluginRecord<'a>`**：零拷贝数据记录，包含`value: &[u8]`和`metadata`
- **`SmartPlugin`**：插件开发的核心trait
- **`PluginResult`**：插件处理结果枚举（Filtered/Mapped/Aggregated）
- **`SmartPluginAdapter`**：桥接插件到DataFlare Processor系统

### 工具类型

- **`OwnedPluginRecord`**：拥有所有权的插件记录，解决生命周期限制
- **`PluginError`**：插件专用错误类型
- **`PluginType`**：插件类型枚举（Filter/Map/Aggregate）

## 🔄 下一阶段计划

### P0 - CLI集成（第2周）
- [ ] 实现`dataflare plugin new`命令
- [ ] 实现`dataflare plugin build`命令
- [ ] 实现`dataflare plugin test`命令
- [ ] 集成到现有plan2.md CLI架构

### P1 - 注解驱动系统（第3-4周）
- [ ] 实现`#[dataflare_plugin]`宏
- [ ] 自动代码生成
- [ ] 简化插件开发体验

## 🤝 贡献指南

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 打开 Pull Request

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🙏 致谢

- 感谢 [Fluvio](https://github.com/infinyon/fluvio) 项目的SmartModule设计启发
- 感谢 DataFlare 团队的支持和反馈
