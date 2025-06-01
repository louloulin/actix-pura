//! Plugin Backend Layer
//!
//! 第3层：Native/WASM后端抽象
//!
//! 这一层提供了统一的后端抽象，支持：
//! - Native插件：直接在主进程中执行
//! - WASM插件：在沙箱环境中执行
//! - 统一的后端接口

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

use crate::interface::{DataFlarePlugin, PluginRecord, PluginResult, PluginError, PluginInfo, PluginState};

/// 插件后端类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    /// Native插件（直接执行）
    Native,
    /// WASM插件（沙箱执行）
    Wasm,
}

/// 插件后端接口
///
/// 所有后端实现都必须实现这个trait
#[async_trait]
pub trait PluginBackend: Send + Sync {
    /// 后端类型
    fn backend_type(&self) -> BackendType;

    /// 加载插件
    async fn load_plugin(&mut self, config: &PluginBackendConfig) -> Result<Box<dyn DataFlarePlugin>, PluginError>;

    /// 卸载插件
    async fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError>;

    /// 获取已加载的插件列表
    fn list_plugins(&self) -> Vec<String>;

    /// 检查插件是否已加载
    fn is_plugin_loaded(&self, plugin_id: &str) -> bool;

    /// 获取后端统计信息
    fn get_stats(&self) -> BackendStats;

    /// 清理后端资源
    async fn cleanup(&mut self) -> Result<(), PluginError>;
}

/// 插件后端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginBackendConfig {
    /// 插件ID
    pub plugin_id: String,
    /// 插件路径或模块路径
    pub plugin_path: String,
    /// 插件配置
    pub plugin_config: HashMap<String, serde_json::Value>,
    /// 后端特定配置
    pub backend_config: BackendSpecificConfig,
    /// 安全策略
    pub security_policy: SecurityPolicy,
    /// 资源限制
    pub resource_limits: ResourceLimits,
}

/// 后端特定配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendSpecificConfig {
    /// Native后端配置
    Native {
        /// 动态库路径
        library_path: Option<String>,
        /// 符号名称
        symbol_name: Option<String>,
    },
    /// WASM后端配置
    Wasm {
        /// WASM模块路径
        module_path: String,
        /// 运行时配置
        runtime_config: WasmRuntimeConfig,
    },
}

/// WASM运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRuntimeConfig {
    /// 内存限制（字节）
    pub memory_limit: usize,
    /// 执行超时（毫秒）
    pub timeout_ms: u64,
    /// 是否启用调试
    pub debug: bool,
    /// 是否启用JIT编译
    pub enable_jit: bool,
    /// 是否启用WASI
    pub enable_wasi: bool,
}

impl Default for WasmRuntimeConfig {
    fn default() -> Self {
        Self {
            memory_limit: 64 * 1024 * 1024, // 64MB
            timeout_ms: 5000,               // 5秒
            debug: false,
            enable_jit: true,
            enable_wasi: false,
        }
    }
}

/// 安全策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    /// 是否允许文件系统访问
    pub allow_filesystem: bool,
    /// 是否允许网络访问
    pub allow_network: bool,
    /// 是否允许环境变量访问
    pub allow_env: bool,
    /// 允许的文件路径列表
    pub allowed_paths: Vec<String>,
    /// 允许的网络地址列表
    pub allowed_hosts: Vec<String>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            allow_filesystem: false,
            allow_network: false,
            allow_env: false,
            allowed_paths: Vec::new(),
            allowed_hosts: Vec::new(),
        }
    }
}

/// 资源限制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// 最大内存使用（字节）
    pub max_memory: usize,
    /// 最大CPU时间（毫秒）
    pub max_cpu_time: u64,
    /// 最大执行时间（毫秒）
    pub max_execution_time: u64,
    /// 最大文件大小（字节）
    pub max_file_size: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 128 * 1024 * 1024, // 128MB
            max_cpu_time: 10000,           // 10秒
            max_execution_time: 30000,     // 30秒
            max_file_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// 后端统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendStats {
    /// 后端类型
    pub backend_type: BackendType,
    /// 已加载插件数量
    pub loaded_plugins: usize,
    /// 总处理次数
    pub total_executions: u64,
    /// 总处理时间（毫秒）
    pub total_execution_time: u64,
    /// 平均处理时间（毫秒）
    pub average_execution_time: f64,
    /// 错误次数
    pub error_count: u64,
    /// 内存使用（字节）
    pub memory_usage: usize,
    /// 最后更新时间
    pub last_updated: i64,
}

impl Default for BackendStats {
    fn default() -> Self {
        Self {
            backend_type: BackendType::Native,
            loaded_plugins: 0,
            total_executions: 0,
            total_execution_time: 0,
            average_execution_time: 0.0,
            error_count: 0,
            memory_usage: 0,
            last_updated: chrono::Utc::now().timestamp(),
        }
    }
}

/// Native插件后端
pub struct NativeBackend {
    /// 已加载的插件
    plugins: HashMap<String, Box<dyn DataFlarePlugin>>,
    /// 统计信息
    stats: BackendStats,
}

impl NativeBackend {
    /// 创建新的Native后端
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            stats: BackendStats {
                backend_type: BackendType::Native,
                ..Default::default()
            },
        }
    }
}

impl Default for NativeBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PluginBackend for NativeBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Native
    }

    async fn load_plugin(&mut self, config: &PluginBackendConfig) -> Result<Box<dyn DataFlarePlugin>, PluginError> {
        // 对于Native插件，这里需要动态加载共享库
        // 这是一个简化的实现，实际需要使用libloading等库

        // 检查是否已加载
        if self.is_plugin_loaded(&config.plugin_id) {
            return Err(PluginError::configuration(format!("Plugin {} already loaded", config.plugin_id)));
        }

        // TODO: 实现动态库加载逻辑
        // 这里返回一个示例插件
        let plugin = Box::new(ExampleNativePlugin::new(&config.plugin_id));

        self.plugins.insert(config.plugin_id.clone(), plugin.clone());
        self.stats.loaded_plugins = self.plugins.len();
        self.stats.last_updated = chrono::Utc::now().timestamp();

        Ok(plugin)
    }

    async fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(mut plugin) = self.plugins.remove(plugin_id) {
            plugin.cleanup()?;
            self.stats.loaded_plugins = self.plugins.len();
            self.stats.last_updated = chrono::Utc::now().timestamp();
            Ok(())
        } else {
            Err(PluginError::configuration(format!("Plugin {} not found", plugin_id)))
        }
    }

    fn list_plugins(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    fn is_plugin_loaded(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }

    fn get_stats(&self) -> BackendStats {
        self.stats.clone()
    }

    async fn cleanup(&mut self) -> Result<(), PluginError> {
        for (_, mut plugin) in self.plugins.drain() {
            plugin.cleanup()?;
        }
        self.stats.loaded_plugins = 0;
        self.stats.last_updated = chrono::Utc::now().timestamp();
        Ok(())
    }
}

/// 示例Native插件（用于测试）
#[derive(Clone)]
struct ExampleNativePlugin {
    name: String,
    state: PluginState,
}

impl ExampleNativePlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            state: PluginState::Uninitialized,
        }
    }
}

impl DataFlarePlugin for ExampleNativePlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn plugin_type(&self) -> crate::interface::PluginType {
        crate::interface::PluginType::Processor
    }

    fn description(&self) -> &str {
        "Example Native Plugin"
    }

    fn initialize(&mut self, _config: &HashMap<String, serde_json::Value>) -> Result<(), PluginError> {
        self.state = PluginState::Ready;
        Ok(())
    }

    fn process(&self, record: &PluginRecord) -> Result<PluginResult, PluginError> {
        // 简单的示例：返回原始数据
        Ok(PluginResult::success(record.value.to_vec()))
    }

    fn get_state(&self) -> PluginState {
        self.state.clone()
    }

    fn cleanup(&mut self) -> Result<(), PluginError> {
        self.state = PluginState::Stopped;
        Ok(())
    }
}

/// WASM插件后端
pub struct WasmBackend {
    /// 已加载的插件
    plugins: HashMap<String, Box<dyn DataFlarePlugin>>,
    /// 统计信息
    stats: BackendStats,
    /// WASM运行时配置
    runtime_config: WasmRuntimeConfig,
}

impl WasmBackend {
    /// 创建新的WASM后端
    pub fn new(runtime_config: WasmRuntimeConfig) -> Self {
        Self {
            plugins: HashMap::new(),
            stats: BackendStats {
                backend_type: BackendType::Wasm,
                ..Default::default()
            },
            runtime_config,
        }
    }
}

impl Default for WasmBackend {
    fn default() -> Self {
        Self::new(WasmRuntimeConfig::default())
    }
}

#[async_trait]
impl PluginBackend for WasmBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Wasm
    }

    async fn load_plugin(&mut self, config: &PluginBackendConfig) -> Result<Box<dyn DataFlarePlugin>, PluginError> {
        // 检查是否已加载
        if self.is_plugin_loaded(&config.plugin_id) {
            return Err(PluginError::configuration(format!("Plugin {} already loaded", config.plugin_id)));
        }

        // TODO: 实现WASM插件加载逻辑
        // 这里返回一个示例插件
        let plugin = Box::new(ExampleWasmPlugin::new(&config.plugin_id));

        self.plugins.insert(config.plugin_id.clone(), plugin.clone());
        self.stats.loaded_plugins = self.plugins.len();
        self.stats.last_updated = chrono::Utc::now().timestamp();

        Ok(plugin)
    }

    async fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(mut plugin) = self.plugins.remove(plugin_id) {
            plugin.cleanup()?;
            self.stats.loaded_plugins = self.plugins.len();
            self.stats.last_updated = chrono::Utc::now().timestamp();
            Ok(())
        } else {
            Err(PluginError::configuration(format!("Plugin {} not found", plugin_id)))
        }
    }

    fn list_plugins(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    fn is_plugin_loaded(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }

    fn get_stats(&self) -> BackendStats {
        self.stats.clone()
    }

    async fn cleanup(&mut self) -> Result<(), PluginError> {
        for (_, mut plugin) in self.plugins.drain() {
            plugin.cleanup()?;
        }
        self.stats.loaded_plugins = 0;
        self.stats.last_updated = chrono::Utc::now().timestamp();
        Ok(())
    }
}

/// 示例WASM插件（用于测试）
#[derive(Clone)]
struct ExampleWasmPlugin {
    name: String,
    state: PluginState,
}

impl ExampleWasmPlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            state: PluginState::Uninitialized,
        }
    }
}

impl DataFlarePlugin for ExampleWasmPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn plugin_type(&self) -> crate::interface::PluginType {
        crate::interface::PluginType::Processor
    }

    fn description(&self) -> &str {
        "Example WASM Plugin"
    }

    fn initialize(&mut self, _config: &HashMap<String, serde_json::Value>) -> Result<(), PluginError> {
        self.state = PluginState::Ready;
        Ok(())
    }

    fn process(&self, record: &PluginRecord) -> Result<PluginResult, PluginError> {
        // 简单的示例：返回原始数据
        Ok(PluginResult::success(record.value.to_vec()))
    }

    fn get_state(&self) -> PluginState {
        self.state.clone()
    }

    fn cleanup(&mut self) -> Result<(), PluginError> {
        self.state = PluginState::Stopped;
        Ok(())
    }
}
