# DataFlare插件体系重新设计

## 🔍 Fluvio vs DataFlare 架构分析

### Fluvio设计优势

#### 1. SmartModule简洁设计
```rust
// Fluvio SmartModule - 极简接口
#[smartmodule(filter)]
pub fn filter(record: &SmartModuleRecord) -> Result<bool> {
    let string = std::str::from_utf8(record.value.as_ref())?;
    Ok(string.contains('a'))
}

#[smartmodule(map)]
pub fn map(record: &SmartModuleRecord) -> Result<(Option<RecordData>, RecordData)> {
    let key = record.key.clone();
    let mut value = Vec::from(record.value.as_ref());
    value.make_ascii_uppercase();
    Ok((key, value.into()))
}

#[smartmodule(aggregate)]
pub fn aggregate(accumulator: RecordData, current: &SmartModuleRecord) -> Result<RecordData> {
    let mut acc = String::from_utf8(accumulator.as_ref().to_vec())?;
    let next = std::str::from_utf8(current.value.as_ref())?;
    acc.push_str(next);
    Ok(acc.into())
}
```

**优势**：
- **单一职责**：每个SmartModule只做一件事
- **零拷贝**：直接操作字节数据，无序列化开销
- **类型安全**：编译时类型检查
- **性能优异**：同步执行，无异步开销

#### 2. Connector简洁设计
```rust
// Fluvio Source Connector
#[async_trait]
impl<'a> Source<'a, String> for TestJsonSource {
    async fn connect(self, _offset: Option<Offset>) -> Result<LocalBoxStream<'a, String>> {
        Ok(self.boxed_local())
    }
}

// Fluvio Sink Connector
#[async_trait]
impl Sink<String> for TestSink {
    async fn connect(self, offset: Option<Offset>) -> Result<LocalBoxSink<String>> {
        // 简单的连接逻辑
    }
}
```

**优势**：
- **流式设计**：天然支持流处理
- **背压处理**：内置背压机制
- **资源管理**：自动资源清理
- **错误恢复**：优雅的错误处理

### DataFlare设计问题

#### 1. 过度复杂的抽象层次
```rust
// DataFlare当前设计 - 层次过多
WasmSystem -> WasmRuntime -> WasmPlugin -> WasmComponent -> WasmProcessor

// 接口过于复杂
#[async_trait]
pub trait SourceConnector: Send + Sync + 'static {
    fn configure(&mut self, config: &Value) -> Result<()>;
    async fn check_connection(&self) -> Result<bool>;
    async fn discover_schema(&self) -> Result<Schema>;
    async fn read(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>>;
    fn get_state(&self) -> Result<SourceState>;
    fn get_extraction_mode(&self) -> ExtractionMode;
    async fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64>;
}
```

**问题**：
- **职责不清**：一个接口承担太多职责
- **配置复杂**：多层配置嵌套
- **性能开销**：过多的抽象层
- **维护困难**：代码复杂度高

#### 2. 数据转换开销大
```rust
// DataFlare当前数据流
DataRecord -> JSON -> WASM bytes -> WASM processing -> WASM bytes -> JSON -> DataRecord
```

**问题**：
- **序列化开销**：多次序列化/反序列化
- **内存拷贝**：数据多次拷贝
- **类型转换**：复杂的类型转换逻辑

## 🎯 新插件体系设计

### 核心设计原则

1. **简单优于复杂**：参考Fluvio的极简设计
2. **性能优先**：零拷贝，最小化开销
3. **类型安全**：编译时保证正确性
4. **易于扩展**：插件开发简单直观

### 1. 统一的插件接口设计

#### 1.1 核心插件类型
```rust
// dataflare/plugin/src/core.rs

/// 插件类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum PluginType {
    /// 数据源插件
    Source,
    /// 数据目标插件
    Sink,
    /// 过滤器插件
    Filter,
    /// 转换器插件
    Transform,
    /// 聚合器插件
    Aggregate,
}

/// 插件记录 - 统一的数据结构
#[derive(Debug, Clone)]
pub struct PluginRecord {
    /// 记录键
    pub key: Option<Vec<u8>>,
    /// 记录值
    pub value: Vec<u8>,
    /// 时间戳
    pub timestamp: Option<i64>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// 插件结果
pub type PluginResult<T> = std::result::Result<T, PluginError>;

/// 插件错误
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Processing error: {0}")]
    Processing(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}
```

#### 1.2 简化的插件接口
```rust
// dataflare/plugin/src/traits.rs

/// 过滤器插件接口
pub trait FilterPlugin: Send + Sync {
    /// 过滤记录
    fn filter(&self, record: &PluginRecord) -> PluginResult<bool>;

    /// 配置插件
    fn configure(&mut self, config: &HashMap<String, String>) -> PluginResult<()>;
}

/// 转换器插件接口
pub trait TransformPlugin: Send + Sync {
    /// 转换记录
    fn transform(&self, record: &PluginRecord) -> PluginResult<PluginRecord>;

    /// 配置插件
    fn configure(&mut self, config: &HashMap<String, String>) -> PluginResult<()>;
}

/// 聚合器插件接口
pub trait AggregatePlugin: Send + Sync {
    /// 聚合记录
    fn aggregate(&self, accumulator: &PluginRecord, current: &PluginRecord) -> PluginResult<PluginRecord>;

    /// 初始化累加器
    fn init_accumulator(&self) -> PluginResult<PluginRecord>;

    /// 配置插件
    fn configure(&mut self, config: &HashMap<String, String>) -> PluginResult<()>;
}

/// 数据源插件接口
#[async_trait]
pub trait SourcePlugin: Send + Sync {
    /// 连接数据源
    async fn connect(&mut self) -> PluginResult<()>;

    /// 读取数据流
    async fn read(&mut self) -> PluginResult<Option<PluginRecord>>;

    /// 配置插件
    fn configure(&mut self, config: &HashMap<String, String>) -> PluginResult<()>;

    /// 关闭连接
    async fn close(&mut self) -> PluginResult<()>;
}

/// 数据目标插件接口
#[async_trait]
pub trait SinkPlugin: Send + Sync {
    /// 连接数据目标
    async fn connect(&mut self) -> PluginResult<()>;

    /// 写入记录
    async fn write(&mut self, record: &PluginRecord) -> PluginResult<()>;

    /// 批量写入
    async fn write_batch(&mut self, records: &[PluginRecord]) -> PluginResult<()> {
        for record in records {
            self.write(record).await?;
        }
        Ok(())
    }

    /// 刷新缓冲区
    async fn flush(&mut self) -> PluginResult<()>;

    /// 配置插件
    fn configure(&mut self, config: &HashMap<String, String>) -> PluginResult<()>;

    /// 关闭连接
    async fn close(&mut self) -> PluginResult<()>;
}
```

### 2. WASM插件集成

#### 2.1 WASM插件运行时
```rust
// dataflare/plugin/src/wasm/runtime.rs

use wasmtime::{Engine, Module, Store, Instance, Func};

/// WASM插件运行时
pub struct WasmPluginRuntime {
    engine: Engine,
    module: Module,
    store: Store<()>,
    instance: Instance,
    plugin_type: PluginType,
}

impl WasmPluginRuntime {
    /// 创建新的WASM运行时
    pub fn new(wasm_bytes: &[u8], plugin_type: PluginType) -> PluginResult<Self> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm_bytes)?;
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;

        Ok(Self {
            engine,
            module,
            store,
            instance,
            plugin_type,
        })
    }

    /// 调用WASM函数
    pub fn call_function(&mut self, func_name: &str, input: &[u8]) -> PluginResult<Vec<u8>> {
        // 获取函数
        let func = self.instance
            .get_func(&mut self.store, func_name)
            .ok_or_else(|| PluginError::Processing(format!("Function {} not found", func_name)))?;

        // 分配内存
        let memory = self.instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| PluginError::Processing("Memory not found".to_string()))?;

        // 写入输入数据
        let input_ptr = self.allocate_memory(input.len())?;
        memory.write(&mut self.store, input_ptr, input)?;

        // 调用函数
        let results = func.call(&mut self.store, &[input_ptr.into(), input.len().into()])?;

        // 读取输出数据
        let output_ptr = results[0].i32().unwrap() as usize;
        let output_len = results[1].i32().unwrap() as usize;

        let mut output = vec![0u8; output_len];
        memory.read(&self.store, output_ptr, &mut output)?;

        Ok(output)
    }

    fn allocate_memory(&mut self, size: usize) -> PluginResult<usize> {
        // 简化的内存分配逻辑
        Ok(0) // 实际实现需要更复杂的内存管理
    }
}
```

#### 2.2 WASM插件适配器
```rust
// dataflare/plugin/src/wasm/adapter.rs

/// WASM过滤器适配器
pub struct WasmFilterAdapter {
    runtime: WasmPluginRuntime,
    config: HashMap<String, String>,
}

impl WasmFilterAdapter {
    pub fn new(wasm_bytes: &[u8]) -> PluginResult<Self> {
        let runtime = WasmPluginRuntime::new(wasm_bytes, PluginType::Filter)?;
        Ok(Self {
            runtime,
            config: HashMap::new(),
        })
    }
}

impl FilterPlugin for WasmFilterAdapter {
    fn filter(&self, record: &PluginRecord) -> PluginResult<bool> {
        // 序列化记录为字节
        let input_bytes = bincode::serialize(record)
            .map_err(|e| PluginError::Serialization(e.to_string()))?;

        // 调用WASM函数
        let output_bytes = self.runtime.call_function("filter", &input_bytes)?;

        // 反序列化结果
        let result: bool = bincode::deserialize(&output_bytes)
            .map_err(|e| PluginError::Serialization(e.to_string()))?;

        Ok(result)
    }

    fn configure(&mut self, config: &HashMap<String, String>) -> PluginResult<()> {
        self.config = config.clone();

        // 将配置传递给WASM模块
        let config_bytes = bincode::serialize(&self.config)
            .map_err(|e| PluginError::Serialization(e.to_string()))?;

        self.runtime.call_function("configure", &config_bytes)?;
        Ok(())
    }
}
```

### 3. 统一配置格式

#### 3.1 简化配置
```yaml
# 新的简化配置格式
processors:
  # 原生插件
  - id: "user_filter"
    type: "plugin"
    config:
      plugin_type: "filter"
      plugin_name: "json_filter"
      params:
        field: "status"
        value: "active"

  # WASM插件
  - id: "data_transform"
    type: "plugin"
    config:
      plugin_type: "transform"
      plugin_source: "wasm"
      plugin_path: "plugins/transform.wasm"
      params:
        format: "json"
        schema: "user_schema"

sources:
  # 原生数据源插件
  - id: "postgres_source"
    type: "plugin"
    config:
      plugin_type: "source"
      plugin_name: "postgres"
      connection:
        host: "localhost"
        port: 5432
        database: "mydb"

sinks:
  # 原生数据目标插件
  - id: "elasticsearch_sink"
    type: "plugin"
    config:
      plugin_type: "sink"
      plugin_name: "elasticsearch"
      connection:
        url: "http://localhost:9200"
        index: "logs"
```

### 4. 与DataFlare处理器集成

#### 4.1 插件处理器适配器
```rust
// dataflare/plugin/src/processor_adapter.rs

use dataflare_core::{
    message::DataRecord,
    processor::Processor,
    error::Result as DataFlareResult,
};

/// 插件处理器适配器
pub struct PluginProcessorAdapter {
    plugin_type: PluginType,
    filter_plugin: Option<Box<dyn FilterPlugin>>,
    transform_plugin: Option<Box<dyn TransformPlugin>>,
}

impl PluginProcessorAdapter {
    pub fn new_filter(plugin: Box<dyn FilterPlugin>) -> Self {
        Self {
            plugin_type: PluginType::Filter,
            filter_plugin: Some(plugin),
            transform_plugin: None,
        }
    }

    /// 转换DataRecord到PluginRecord
    fn to_plugin_record(&self, record: &DataRecord) -> PluginResult<PluginRecord> {
        Ok(PluginRecord {
            key: None,
            value: serde_json::to_vec(record)?,
            timestamp: Some(chrono::Utc::now().timestamp()),
            metadata: HashMap::new(),
        })
    }

    /// 转换PluginRecord到DataRecord
    fn from_plugin_record(&self, plugin_record: &PluginRecord) -> PluginResult<DataRecord> {
        let record: DataRecord = serde_json::from_slice(&plugin_record.value)?;
        Ok(record)
    }
}

#[async_trait]
impl Processor for PluginProcessorAdapter {
    async fn process_record(&mut self, record: &DataRecord) -> DataFlareResult<DataRecord> {
        let plugin_record = self.to_plugin_record(record)
            .map_err(|e| dataflare_core::error::DataFlareError::ProcessingError(e.to_string()))?;

        match self.plugin_type {
            PluginType::Filter => {
                if let Some(ref filter) = self.filter_plugin {
                    let should_keep = filter.filter(&plugin_record)
                        .map_err(|e| dataflare_core::error::DataFlareError::ProcessingError(e.to_string()))?;

                    if should_keep {
                        Ok(record.clone())
                    } else {
                        Err(dataflare_core::error::DataFlareError::ProcessingError("Record filtered out".to_string()))
                    }
                } else {
                    Err(dataflare_core::error::DataFlareError::ProcessingError("Filter plugin not found".to_string()))
                }
            }

            PluginType::Transform => {
                if let Some(ref transform) = self.transform_plugin {
                    let transformed = transform.transform(&plugin_record)
                        .map_err(|e| dataflare_core::error::DataFlareError::ProcessingError(e.to_string()))?;

                    self.from_plugin_record(&transformed)
                        .map_err(|e| dataflare_core::error::DataFlareError::ProcessingError(e.to_string()))
                } else {
                    Err(dataflare_core::error::DataFlareError::ProcessingError("Transform plugin not found".to_string()))
                }
            }

            _ => Err(dataflare_core::error::DataFlareError::ProcessingError("Unsupported plugin type".to_string()))
        }
    }
}
```

## 🎯 设计优势总结

### 1. 简化的架构
- **3层架构**：Plugin Interface → Plugin Implementation → WASM/Native Runtime
- **统一接口**：所有插件类型使用一致的接口设计
- **清晰职责**：每个组件职责单一明确

### 2. 高性能设计
- **零拷贝**：支持零拷贝数据处理
- **对象池**：插件实例复用
- **最小序列化**：减少数据转换开销

### 3. 易于开发
- **简单接口**：参考Fluvio的极简设计
- **模板支持**：提供开发模板
- **宏支持**：自动注册机制

### 4. 灵活扩展
- **多种插件源**：支持原生和WASM插件
- **统一配置**：一致的配置格式
- **热插拔**：支持运行时插件管理

这个新的插件体系设计将DataFlare的插件系统从复杂的多层抽象简化为清晰的三层架构，同时保持了高性能和易用性，真正实现了"简单而强大"的设计目标。
