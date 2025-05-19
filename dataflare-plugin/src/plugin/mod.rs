//! 数据集成框架的插件系统
//!
//! 本模块定义了插件系统的接口和功能，支持 WebAssembly (WASM) 插件。
//! 插件系统允许用户扩展框架的功能，包括自定义数据处理器和连接器。

mod wasm;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::fs;

use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use wasmtime::{Engine, Module, Store, Instance, InstancePre, Linker, Caller, Val};

use crate::error::{DataFlareError, Result};
use crate::message::DataRecord;

pub use self::wasm::{WasmMemory, WasmProcessor, create_example_wasm_module};

/// 插件类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginType {
    /// 处理器插件
    Processor,
    /// 源连接器插件
    SourceConnector,
    /// 目标连接器插件
    DestinationConnector,
}

/// 插件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// 插件名称
    pub name: String,
    /// 插件版本
    pub version: String,
    /// 插件描述
    #[serde(default = "default_description")]
    pub description: String,
    /// 插件类型
    #[serde(default = "default_plugin_type")]
    pub plugin_type: PluginType,
    /// 插件作者
    pub author: String,
    /// 插件输入模式
    #[serde(default)]
    pub input_schema: Option<Value>,
    /// 插件输出模式
    #[serde(default)]
    pub output_schema: Option<Value>,
    /// 插件配置模式
    #[serde(default)]
    pub config_schema: Option<Value>,
}

/// 默认插件描述
fn default_description() -> String {
    "WASM 插件".to_string()
}

/// 默认插件类型
fn default_plugin_type() -> PluginType {
    PluginType::Processor
}

/// 插件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// 内存限制（以字节为单位）
    pub memory_limit: Option<usize>,
    /// 超时时间（以毫秒为单位）
    pub timeout_ms: Option<u64>,
    /// 插件特定配置
    pub config: Value,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            memory_limit: Some(16 * 1024 * 1024), // 默认 16MB
            timeout_ms: Some(5000), // 默认 5 秒
            config: json!({}),
        }
    }
}

/// 插件实例
pub struct Plugin {
    /// 插件ID
    pub id: String,
    /// 插件元数据
    pub metadata: PluginMetadata,
    /// 插件配置
    pub config: PluginConfig,
    /// Wasmtime 引擎
    pub(crate) engine: Engine,
    /// Wasmtime 预实例化实例
    pub(crate) instance: InstancePre<PluginState>,
    /// 插件状态
    pub(crate) state: PluginState,
}

/// 插件状态
#[derive(Clone)]
pub struct PluginState {
    /// 内存限制
    pub memory_limit: usize,
    /// 超时时间
    pub timeout_ms: u64,
    /// 插件配置
    pub config: Value,
    /// 插件日志
    pub logs: Vec<String>,
}

/// 初始化插件系统
pub fn init_plugin_system(plugin_dir: PathBuf) -> Result<()> {
    // 检查目录是否存在，不存在则创建
    if !plugin_dir.exists() {
        fs::create_dir_all(&plugin_dir)
            .map_err(|e| DataFlareError::Plugin(format!("创建插件目录时出错: {}", e)))?;
    }

    log::info!("插件系统已初始化，插件目录: {:?}", plugin_dir);
    Ok(())
}

/// 处理器插件接口
pub trait ProcessorPlugin: Send + Sync {
    /// 配置插件
    fn configure(&mut self, config: Value) -> Result<()>;

    /// 处理记录
    fn process(&self, record: DataRecord) -> Result<DataRecord>;

    /// 获取元数据
    fn get_metadata(&self) -> PluginMetadata;
}

/// 插件管理器
pub struct PluginManager {
    /// 插件目录
    plugin_dir: PathBuf,
    /// Wasmtime 引擎
    engine: Engine,
    /// 已加载的插件
    plugins: Arc<RwLock<HashMap<String, Arc<Plugin>>>>,
}

impl PluginManager {
    /// 创建新的插件管理器
    pub fn new(plugin_dir: PathBuf) -> Self {
        // 创建 Wasmtime 引擎
        let engine = Engine::default();

        Self {
            plugin_dir,
            engine,
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 加载插件
    pub fn load_plugin(&self, plugin_id: &str, config: PluginConfig) -> Result<Arc<Plugin>> {
        let plugin_path = self.plugin_dir.join(format!("{}.wasm", plugin_id));

        // 检查插件文件是否存在
        if !plugin_path.exists() {
            return Err(DataFlareError::Plugin(format!("插件文件不存在: {:?}", plugin_path)));
        }

        // 读取插件文件
        let wasm_bytes = fs::read(&plugin_path)
            .map_err(|e| DataFlareError::Plugin(format!("读取插件文件时出错: {}", e)))?;

        self.load_plugin_from_bytes(plugin_id, &wasm_bytes, config)
    }

    /// 从字节加载插件
    pub fn load_plugin_from_bytes(&self, plugin_id: &str, wasm_bytes: &[u8], config: PluginConfig) -> Result<Arc<Plugin>> {
        // 编译 WASM 模块
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| DataFlareError::Plugin(format!("编译 WASM 模块时出错: {}", e)))?;

        // 创建插件状态
        let plugin_state = PluginState {
            memory_limit: config.memory_limit.unwrap_or(16 * 1024 * 1024), // 默认 16MB
            timeout_ms: config.timeout_ms.unwrap_or(5000), // 默认 5 秒
            config: config.config.clone(),
            logs: Vec::new(),
        };

        // 创建 Wasmtime 存储
        let mut store = Store::new(&self.engine, plugin_state);

        // 创建链接器
        let mut linker = Linker::new(&self.engine);

        // 添加宿主函数
        self.add_host_functions(&mut linker)?;

        // 实例化 WASM 模块
        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| DataFlareError::Plugin(format!("实例化 WASM 模块时出错: {}", e)))?;

        // 获取元数据
        let metadata = self.get_plugin_metadata(&mut store, &instance)?;

        // 创建新的实例，确保它与 store 分离
        let instance = linker.instantiate_pre(&module)
            .map_err(|e| DataFlareError::Plugin(format!("预实例化 WASM 模块时出错: {}", e)))?;

        // 获取插件状态
        let state = store.data().clone();

        // 创建插件实例
        let plugin = Arc::new(Plugin {
            id: plugin_id.to_string(),
            metadata,
            config,
            engine: self.engine.clone(),
            instance,
            state,
        });

        // 存储插件
        self.plugins.write().unwrap().insert(plugin_id.to_string(), plugin.clone());

        log::info!("已加载插件: {}", plugin_id);
        Ok(plugin)
    }

    /// 卸载插件
    pub fn unload_plugin(&self, plugin_id: &str) -> Result<()> {
        let mut plugins = self.plugins.write().unwrap();

        if plugins.remove(plugin_id).is_some() {
            log::info!("已卸载插件: {}", plugin_id);
            Ok(())
        } else {
            Err(DataFlareError::Plugin(format!("插件不存在: {}", plugin_id)))
        }
    }

    /// 获取插件
    pub fn get_plugin(&self, plugin_id: &str) -> Result<Arc<Plugin>> {
        let plugins = self.plugins.read().unwrap();

        plugins.get(plugin_id)
            .cloned()
            .ok_or_else(|| DataFlareError::Plugin(format!("插件不存在: {}", plugin_id)))
    }

    /// 列出可用插件
    pub fn list_plugins(&self) -> Result<Vec<String>> {
        let plugins = self.plugins.read().unwrap();
        Ok(plugins.keys().cloned().collect())
    }

    /// 扫描插件目录
    pub fn scan_plugin_directory(&self) -> Result<Vec<PathBuf>> {
        let mut plugin_files = Vec::new();

        // 检查目录是否存在
        if !self.plugin_dir.exists() {
            return Ok(plugin_files);
        }

        // 遍历目录
        for entry in fs::read_dir(&self.plugin_dir)
            .map_err(|e| DataFlareError::Plugin(format!("读取插件目录时出错: {}", e)))? {

            let entry = entry
                .map_err(|e| DataFlareError::Plugin(format!("读取目录条目时出错: {}", e)))?;

            let path = entry.path();

            // 检查是否是 WASM 文件
            if path.is_file() && path.extension().map_or(false, |ext| ext == "wasm") {
                plugin_files.push(path);
            }
        }

        Ok(plugin_files)
    }

    /// 创建处理器插件
    pub fn create_processor(&self, plugin_id: &str) -> Result<Box<dyn ProcessorPlugin>> {
        let plugin = self.get_plugin(plugin_id)?;

        // 检查插件类型
        if plugin.metadata.plugin_type != PluginType::Processor {
            return Err(DataFlareError::Plugin(format!("插件 {} 不是处理器插件", plugin_id)));
        }

        // 创建处理器
        let processor = Box::new(WasmProcessor::new(plugin));

        Ok(processor)
    }

    /// 添加宿主函数
    fn add_host_functions(&self, linker: &mut Linker<PluginState>) -> Result<()> {
        // 添加日志函数
        linker.func_wrap("env", "log", |mut caller: Caller<'_, PluginState>, level: i32, ptr: i32, len: i32| {
            let memory = match caller.get_export("memory") {
                Some(wasmtime::Extern::Memory(mem)) => mem,
                _ => {
                    log::error!("[WASM] 模块没有导出内存");
                    return;
                },
            };

            if ptr < 0 || len <= 0 {
                log::error!("[WASM] 无效的内存指针或长度: ptr={}, len={}", ptr, len);
                return;
            }

            // 读取消息
            let data = match memory.data(&caller).get(ptr as usize..(ptr as usize + len as usize)) {
                Some(data) => data,
                None => {
                    log::error!("[WASM] 内存访问越界: ptr={}, len={}", ptr, len);
                    return;
                }
            };

            let message = String::from_utf8_lossy(data).to_string();

            // 根据级别记录日志
            match level {
                1 => log::error!("[WASM] {}", message),
                2 => log::warn!("[WASM] {}", message),
                3 => log::info!("[WASM] {}", message),
                4 => log::debug!("[WASM] {}", message),
                _ => log::trace!("[WASM] {}", message),
            }

            // 存储日志
            caller.data_mut().logs.push(message);
        })
        .map_err(|e| DataFlareError::Plugin(format!("添加日志函数时出错: {}", e)))?;

        Ok(())
    }

    /// 获取插件元数据
    fn get_plugin_metadata(&self, store: &mut Store<PluginState>, instance: &Instance) -> Result<PluginMetadata> {
        // 获取元数据函数
        let get_metadata = instance.get_func(&mut *store, "get_metadata")
            .ok_or_else(|| DataFlareError::Plugin("插件没有导出 get_metadata 函数".to_string()))?;

        // 检查函数类型
        let func_type = get_metadata.ty(&*store);
        if func_type.params().len() != 0 || func_type.results().len() != 2 {
            return Err(DataFlareError::Plugin("get_metadata 函数签名不正确".to_string()));
        }

        // 调用函数
        let mut results = [Val::I32(0), Val::I32(0)];
        get_metadata.call(&mut *store, &[], &mut results)
            .map_err(|e| DataFlareError::Plugin(format!("调用 get_metadata 函数时出错: {}", e)))?;

        // 获取返回值
        let ptr = if let Val::I32(val) = results[0] { val } else { 0 };
        let len = if let Val::I32(val) = results[1] { val } else { 0 };

        if ptr <= 0 || len <= 0 {
            return Err(DataFlareError::Plugin(format!("get_metadata 返回了无效的指针或长度: ptr={}, len={}", ptr, len)));
        }

        // 获取内存
        let memory = instance.get_memory(&mut *store, "memory")
            .ok_or_else(|| DataFlareError::Plugin("插件没有导出内存".to_string()))?;

        // 读取元数据 JSON
        let data = memory.data(&*store)
            .get(ptr as usize..(ptr as usize + len as usize))
            .ok_or_else(|| DataFlareError::Plugin("内存访问越界".to_string()))?;

        let metadata_json = String::from_utf8(data.to_vec())
            .map_err(|e| DataFlareError::Plugin(format!("解析元数据 JSON 时出错: {}", e)))?;

        // 打印元数据 JSON 以便调试
        log::debug!("元数据 JSON: {}", metadata_json);

        // 解析元数据
        let metadata: PluginMetadata = match serde_json::from_str(&metadata_json) {
            Ok(metadata) => metadata,
            Err(e) => {
                log::error!("解析插件元数据时出错: {}, JSON: {}", e, metadata_json);
                return Err(DataFlareError::Plugin(format!("解析插件元数据时出错: {}", e)));
            }
        };

        Ok(metadata)
    }
}
