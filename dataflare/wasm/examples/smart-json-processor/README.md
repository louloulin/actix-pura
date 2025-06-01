# Smart JSON Processor - 高内聚低耦合设计示例

## 📖 概述

这个示例展示了如何基于Fluvio SmartModule和Spin Framework的设计理念，构建一个高内聚、低耦合的DataFlare WASM插件。

## 🎯 设计原则

### 高内聚 (High Cohesion)

#### 1. 单一职责原则
```rust
pub struct SmartJsonProcessor {
    config: JsonProcessorConfig,     // 配置管理
    stats: ProcessingStats,          // 状态管理  
    initialized: bool,               // 生命周期管理
}
```

每个结构体和方法都有明确的单一职责：
- `JsonProcessorConfig`: 专门负责配置管理
- `ProcessingStats`: 专门负责统计信息
- `transform_json()`: 专门负责JSON转换逻辑
- `validate_schema()`: 专门负责模式验证

#### 2. 功能内聚
所有相关的JSON处理功能都封装在同一个模块中：
- JSON解析和序列化
- 字段映射和过滤
- 模式验证
- 错误处理
- 统计收集

#### 3. 数据内聚
相关的数据结构组织在一起：
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonProcessorConfig {
    pub field_mappings: HashMap<String, String>,
    pub filter_fields: Vec<String>,
    pub required_fields: Vec<String>,
    pub default_values: HashMap<String, Value>,
    pub validate_schema: bool,
    pub max_record_size: usize,
}
```

### 低耦合 (Low Coupling)

#### 1. 接口标准化
通过WIT接口定义标准化的组件边界：
```wit
interface smart-module {
    init: func(config: list<tuple<string, string>>) -> result<_, string>;
    process: func(record: data-record) -> result<processing-result, string>;
    process-batch: func(records: list<data-record>) -> result<batch-result, string>;
    get-state: func() -> string;
    cleanup: func() -> result<_, string>;
}
```

#### 2. 依赖最小化
- 只依赖必要的外部库 (serde, serde_json, chrono)
- 通过WIT接口与DataFlare核心系统交互
- 不直接依赖DataFlare内部实现

#### 3. 数据传递解耦
使用标准化的数据结构进行通信：
```rust
pub fn process_record(&mut self, record: DataRecord) -> Result<ProcessingResult, String>
```

#### 4. 配置外部化
配置通过键值对传入，不硬编码：
```rust
pub fn initialize(&mut self, config_pairs: Vec<(String, String)>) -> Result<(), String>
```

## 🏗️ 架构特点

### 借鉴Fluvio SmartModule设计

#### 1. 流式处理能力
```rust
/// 支持单条记录处理 (类似Fluvio SmartModule)
pub fn process_record(&mut self, record: DataRecord) -> Result<ProcessingResult, String>

/// 支持批量处理优化
pub fn process_batch(records: Vec<DataRecord>) -> Result<BatchResult, String>
```

#### 2. 状态管理
```rust
#[derive(Debug, Clone, Default)]
pub struct ProcessingStats {
    pub records_processed: u64,
    pub records_success: u64,
    pub records_error: u64,
    pub records_filtered: u64,
    // ... 更多统计信息
}
```

#### 3. 错误处理策略
```rust
/// 多种处理结果类型 (借鉴Fluvio)
variant processing-result {
    success(data-record),
    error(string),
    skip,
    retry(string),
    filtered,                    // Fluvio-inspired
    multiple(list<data-record>), // Fluvio flat-map inspired
}
```

### 参考Spin Framework设计

#### 1. 组件生命周期
```rust
impl SmartModuleGuest for SmartJsonProcessorImpl {
    fn init(config: Vec<(String, String)>) -> Result<(), String> {
        // 初始化逻辑
    }
    
    fn cleanup() -> Result<(), String> {
        // 清理逻辑
    }
}
```

#### 2. 配置驱动
类似Spin的配置方式，支持灵活的配置管理：
```json
{
  "field_mappings": {"old_name": "new_name"},
  "filter_fields": ["sensitive_field"],
  "required_fields": ["id", "timestamp"],
  "validate_schema": true
}
```

## 🔧 使用示例

### 1. 构建插件
```bash
# 使用cargo component构建
cargo component build --release

# 生成的WASM文件
ls target/wasm32-wasip1/release/smart_json_processor.wasm
```

### 2. 配置示例
```yaml
# DataFlare工作流配置
tasks:
  - id: json-processor
    type: wasm
    config:
      module_path: "./smart_json_processor.wasm"
      init_params:
        field_mappings: '{"user_name": "name", "user_email": "email"}'
        filter_fields: '["password", "secret"]'
        required_fields: '["id", "timestamp"]'
        validate_schema: "true"
        max_record_size: "1048576"
```

### 3. 数据处理流程
```
输入JSON记录
    ↓
大小验证 (max_record_size)
    ↓
JSON解析
    ↓
模式验证 (required_fields)
    ↓
字段映射 (field_mappings)
    ↓
字段过滤 (filter_fields)
    ↓
默认值填充 (default_values)
    ↓
输出处理结果
```

## 📊 性能特点

### 1. 内存效率
- 使用`wee_alloc`优化内存分配
- 零拷贝数据传递 (借鉴Fluvio)
- 批处理减少函数调用开销

### 2. 处理效率
- 单次JSON解析和序列化
- 就地字段操作
- 早期错误检测和返回

### 3. 监控能力
- 详细的处理统计
- 错误分类和计数
- 性能指标收集

## 🔒 安全特性

### 1. 输入验证
- 记录大小限制
- JSON格式验证
- 模式验证

### 2. 错误隔离
- 单条记录错误不影响批处理
- 详细的错误信息
- 优雅的错误恢复

### 3. 资源限制
- 内存使用限制
- 处理时间监控
- 状态大小控制

## 🚀 扩展性

### 1. 配置扩展
可以轻松添加新的配置选项：
```rust
pub struct JsonProcessorConfig {
    // 现有配置...
    pub custom_transformations: HashMap<String, String>,
    pub output_format: OutputFormat,
    pub compression_enabled: bool,
}
```

### 2. 功能扩展
可以添加新的处理功能：
```rust
impl SmartJsonProcessor {
    fn apply_custom_transformations(&self, obj: &mut Map<String, Value>) {
        // 自定义转换逻辑
    }
    
    fn compress_output(&self, data: &[u8]) -> Vec<u8> {
        // 压缩逻辑
    }
}
```

### 3. 接口扩展
可以实现额外的WIT接口：
```wit
interface advanced-processor {
    transform-schema: func(schema: string) -> result<string, string>;
    validate-data: func(data: data-record, schema: string) -> result<bool, string>;
}
```

这个示例展示了如何在DataFlare WASM插件系统中实现高内聚、低耦合的设计，为构建可维护、可扩展的数据处理组件提供了最佳实践。
