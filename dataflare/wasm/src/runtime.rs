//! WASM运行时实现
//!
//! 基于wasmtime的安全WASM执行环境，支持DataFlare完整DSL

use crate::{WasmPlugin, WasmPluginConfig, WasmError, WasmResult, sandbox::SecurityPolicy, interface::WasmPluginInterface};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{info, debug, warn, error};
use wasmtime::*;

/// WASM运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRuntimeConfig {
    /// 内存限制 (字节)
    pub memory_limit: usize,
    /// 执行超时 (毫秒)
    pub timeout_ms: u64,
    /// 是否启用调试模式
    pub debug: bool,
    /// 安全策略
    pub security_policy: SecurityPolicy,
    /// 最大并发插件数
    pub max_concurrent_plugins: usize,
    /// 是否启用JIT编译
    pub enable_jit: bool,
    /// 是否启用WASI
    pub enable_wasi: bool,
}

impl Default for WasmRuntimeConfig {
    fn default() -> Self {
        Self {
            memory_limit: 64 * 1024 * 1024, // 64MB
            timeout_ms: 5000, // 5秒
            debug: false,
            security_policy: SecurityPolicy::default(),
            max_concurrent_plugins: 10,
            enable_jit: true,
            enable_wasi: false,
        }
    }
}

/// WASM运行时构建器
pub struct WasmRuntimeBuilder {
    config: WasmRuntimeConfig,
}

impl WasmRuntimeBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: WasmRuntimeConfig::default(),
        }
    }

    /// 设置内存限制
    pub fn memory_limit(mut self, limit: usize) -> Self {
        self.config.memory_limit = limit;
        self
    }

    /// 设置超时时间
    pub fn timeout(mut self, timeout_ms: u64) -> Self {
        self.config.timeout_ms = timeout_ms;
        self
    }

    /// 启用调试模式
    pub fn debug(mut self, enable: bool) -> Self {
        self.config.debug = enable;
        self
    }

    /// 设置安全策略
    pub fn security_policy(mut self, policy: SecurityPolicy) -> Self {
        self.config.security_policy = policy;
        self
    }

    /// 设置最大并发插件数
    pub fn max_concurrent_plugins(mut self, max: usize) -> Self {
        self.config.max_concurrent_plugins = max;
        self
    }

    /// 启用JIT编译
    pub fn enable_jit(mut self, enable: bool) -> Self {
        self.config.enable_jit = enable;
        self
    }

    /// 启用WASI
    pub fn enable_wasi(mut self, enable: bool) -> Self {
        self.config.enable_wasi = enable;
        self
    }

    /// 构建运行时
    pub fn build(self) -> WasmResult<WasmRuntime> {
        WasmRuntime::new(self.config)
    }
}

impl Default for WasmRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// WASM运行时
pub struct WasmRuntime {
    /// 运行时配置
    config: WasmRuntimeConfig,
    /// Wasmtime引擎
    engine: Option<Engine>,
    /// 已加载的插件
    plugins: HashMap<String, WasmPlugin>,
    /// 是否已初始化
    initialized: bool,
    /// 运行时统计
    stats: WasmRuntimeStats,
}

impl WasmRuntime {
    /// 创建新的WASM运行时
    pub fn new(config: WasmRuntimeConfig) -> WasmResult<Self> {
        // 验证配置
        if config.memory_limit == 0 {
            return Err(WasmError::configuration("内存限制不能为0"));
        }

        if config.timeout_ms == 0 {
            return Err(WasmError::configuration("超时时间不能为0"));
        }

        let runtime = Self {
            config,
            engine: None,
            plugins: HashMap::new(),
            initialized: false,
            stats: WasmRuntimeStats::default(),
        };

        info!("WASM运行时创建成功，内存限制: {}MB, 超时: {}ms",
              runtime.config.memory_limit / (1024 * 1024),
              runtime.config.timeout_ms);

        Ok(runtime)
    }

    /// 初始化运行时
    pub async fn initialize(&mut self) -> WasmResult<()> {
        if self.initialized {
            return Ok(());
        }

        info!("初始化WASM运行时");

        // 创建Wasmtime引擎配置
        let mut engine_config = Config::new();

        // 配置JIT编译
        if self.config.enable_jit {
            engine_config.strategy(Strategy::Cranelift);
        } else {
            engine_config.strategy(Strategy::Winch);
        }

        // 配置调试模式
        if self.config.debug {
            engine_config.debug_info(true);
        }

        // 配置内存限制
        engine_config.max_wasm_stack(1024 * 1024); // 1MB stack

        // 配置安全特性
        engine_config.wasm_bulk_memory(true);
        engine_config.wasm_reference_types(true);
        engine_config.wasm_multi_value(true);
        engine_config.wasm_simd(true); // 启用SIMD支持

        // 创建引擎
        let engine = Engine::new(&engine_config)
            .map_err(|e| WasmError::runtime(format!("创建Wasmtime引擎失败: {}", e)))?;

        self.engine = Some(engine);
        self.initialized = true;
        self.stats.initialization_time = Some(chrono::Utc::now());

        info!("WASM运行时初始化完成");
        Ok(())
    }

    /// 加载WASM插件
    pub async fn load_plugin(&mut self, module_path: &str) -> WasmResult<WasmPlugin> {
        if !self.initialized {
            self.initialize().await?;
        }

        info!("加载WASM插件: {}", module_path);

        // 检查并发限制
        if self.plugins.len() >= self.config.max_concurrent_plugins {
            return Err(WasmError::runtime("已达到最大并发插件数限制"));
        }

        // 验证文件存在
        if !std::path::Path::new(module_path).exists() {
            return Err(WasmError::plugin_load(format!("WASM模块文件不存在: {}", module_path)));
        }

        // 读取WASM字节码
        let wasm_bytes = std::fs::read(module_path)
            .map_err(|e| WasmError::plugin_load(format!("读取WASM文件失败: {}", e)))?;

        // 验证WASM格式
        if !self.validate_wasm_format(&wasm_bytes) {
            return Err(WasmError::plugin_load("无效的WASM文件格式"));
        }

        // 编译WASM模块
        let engine = self.engine.as_ref().unwrap();
        let module = Module::new(engine, &wasm_bytes)
            .map_err(|e| WasmError::plugin_load(format!("WASM模块编译失败: {}", e)))?;

        // 创建插件配置
        let plugin_config = WasmPluginConfig {
            name: std::path::Path::new(module_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string(),
            module_path: module_path.to_string(),
            config: HashMap::new(),
            security_policy: self.config.security_policy.clone(),
            memory_limit: self.config.memory_limit,
            timeout_ms: self.config.timeout_ms,
        };

        // 创建插件实例
        let plugin = WasmPlugin::new(plugin_config, module, engine.clone()).await?;
        let plugin_id = plugin.get_metadata().name.clone();

        // 存储插件
        self.plugins.insert(plugin_id.clone(), plugin.clone());
        self.stats.loaded_plugins += 1;

        info!("WASM插件加载成功: {} (ID: {})", module_path, plugin_id);
        Ok(plugin)
    }

    /// 卸载插件
    pub async fn unload_plugin(&mut self, plugin_id: &str) -> WasmResult<()> {
        if let Some(mut plugin) = self.plugins.remove(plugin_id) {
            plugin.cleanup().await?;
            self.stats.loaded_plugins = self.stats.loaded_plugins.saturating_sub(1);
            info!("插件卸载成功: {}", plugin_id);
        } else {
            warn!("插件不存在: {}", plugin_id);
        }
        Ok(())
    }

    /// 获取插件
    pub fn get_plugin(&self, plugin_id: &str) -> Option<&WasmPlugin> {
        self.plugins.get(plugin_id)
    }

    /// 列出所有插件
    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// 验证WASM文件格式
    fn validate_wasm_format(&self, bytes: &[u8]) -> bool {
        // WASM文件以魔数 0x00 0x61 0x73 0x6D 开头
        if bytes.len() < 8 {
            return false;
        }

        // 检查魔数
        if !(bytes[0] == 0x00 && bytes[1] == 0x61 && bytes[2] == 0x73 && bytes[3] == 0x6D) {
            return false;
        }

        // 检查版本号 (应该是 0x01 0x00 0x00 0x00)
        bytes[4] == 0x01 && bytes[5] == 0x00 && bytes[6] == 0x00 && bytes[7] == 0x00
    }

    /// 获取运行时配置
    pub fn get_config(&self) -> &WasmRuntimeConfig {
        &self.config
    }

    /// 获取运行时统计信息
    pub fn get_stats(&self) -> &WasmRuntimeStats {
        &self.stats
    }

    /// 更新配置
    pub fn update_config(&mut self, config: WasmRuntimeConfig) -> WasmResult<()> {
        if self.initialized {
            return Err(WasmError::runtime("运行时已初始化，无法更新配置"));
        }

        self.config = config;
        Ok(())
    }

    /// 清理运行时资源
    pub async fn cleanup(&mut self) -> WasmResult<()> {
        if !self.initialized {
            return Ok(());
        }

        info!("清理WASM运行时资源");

        // 清理所有插件
        for (plugin_id, mut plugin) in self.plugins.drain() {
            if let Err(e) = plugin.cleanup().await {
                error!("清理插件 {} 失败: {}", plugin_id, e);
            }
        }

        // 清理引擎
        self.engine = None;
        self.initialized = false;
        self.stats.cleanup_time = Some(chrono::Utc::now());

        info!("WASM运行时资源清理完成");
        Ok(())
    }

    /// 检查是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// 获取已加载插件数量
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

/// WASM运行时统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRuntimeStats {
    /// 已加载插件数量
    pub loaded_plugins: usize,
    /// 初始化时间
    pub initialization_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 清理时间
    pub cleanup_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 总执行次数
    pub total_executions: u64,
    /// 总执行时间 (毫秒)
    pub total_execution_time_ms: u64,
    /// 错误次数
    pub error_count: u64,
}

impl Default for WasmRuntimeStats {
    fn default() -> Self {
        Self {
            loaded_plugins: 0,
            initialization_time: None,
            cleanup_time: None,
            total_executions: 0,
            total_execution_time_ms: 0,
            error_count: 0,
        }
    }
}

impl Drop for WasmRuntime {
    fn drop(&mut self) {
        if self.initialized {
            debug!("WASM运行时正在销毁");
            // 在析构函数中进行同步清理
            // 注意：这里不能使用async
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wasm_runtime_creation() {
        let config = WasmRuntimeConfig::default();
        let runtime = WasmRuntime::new(config).unwrap();
        assert!(!runtime.initialized);
    }

    #[tokio::test]
    async fn test_wasm_runtime_initialization() {
        let config = WasmRuntimeConfig::default();
        let mut runtime = WasmRuntime::new(config).unwrap();

        runtime.initialize().await.unwrap();
        assert!(runtime.initialized);
    }

    #[test]
    fn test_wasm_format_validation() {
        let config = WasmRuntimeConfig::default();
        let runtime = WasmRuntime::new(config).unwrap();

        // 有效的WASM魔数和版本
        let valid_wasm = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        assert!(runtime.validate_wasm_format(&valid_wasm));

        // 无效的魔数
        let invalid_wasm = vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        assert!(!runtime.validate_wasm_format(&invalid_wasm));

        // 太短的字节数组
        let short_bytes = vec![0x00, 0x61];
        assert!(!runtime.validate_wasm_format(&short_bytes));
    }

    #[test]
    fn test_runtime_builder() {
        let runtime = WasmRuntimeBuilder::new()
            .memory_limit(128 * 1024 * 1024)
            .timeout(10000)
            .debug(true)
            .max_concurrent_plugins(20)
            .build()
            .unwrap();

        assert_eq!(runtime.config.memory_limit, 128 * 1024 * 1024);
        assert_eq!(runtime.config.timeout_ms, 10000);
        assert!(runtime.config.debug);
        assert_eq!(runtime.config.max_concurrent_plugins, 20);
    }
}
