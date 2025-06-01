//! # DataFlare WASM Plugin System
//!
//! 统一的WASM插件系统，支持DataFlare完整的DSL生态：
//! - Source连接器
//! - Destination连接器
//! - Processor处理器
//! - Transformer转换器
//! - Filter过滤器
//! - Aggregator聚合器
//! - 自定义组件
//!
//! ## 特性
//! - 安全的沙箱执行环境
//! - 统一的插件接口
//! - 热插拔支持
//! - 多语言插件开发
//! - 资源限制和权限控制
//! - 高性能异步执行

#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

pub mod runtime;
pub mod plugin;
pub mod interface;
pub mod components;
pub mod registry;
pub mod sandbox;
pub mod host_functions;
pub mod error;
pub mod processor;
pub mod wit_runtime;
pub mod dtl_bridge;
pub mod ai_integration;
pub mod registry_integration;
pub mod profiler;
pub mod metrics;
pub mod dev_tools;
pub mod marketplace;
pub mod distributed;

// Re-exports for convenience
pub use runtime::{WasmRuntime, WasmRuntimeConfig, WasmRuntimeBuilder};
pub use plugin::{WasmPlugin, WasmPluginConfig, WasmPluginMetadata, WasmPluginStatus};
pub use interface::{WasmPluginInterface, WasmPluginCapabilities, WasmFunctionCall, WasmFunctionResult};
pub use components::{
    WasmComponent, WasmComponentType, WasmComponentConfig, WasmComponentFactory, WasmComponentManager,
    source::WasmSourceConnector,
    destination::WasmDestinationConnector,
    processor::WasmProcessorComponent,
    transformer::WasmTransformerComponent,
    filter::WasmFilterComponent,
    aggregator::WasmAggregatorComponent,
};
pub use processor::{DataFlareWasmProcessor, WasmProcessorConfig, register_wasm_processor};
pub use registry::{WasmPluginRegistry, PluginRegistration};
pub use sandbox::{WasmSandbox, SandboxConfig, SecurityPolicy};
pub use host_functions::{HostFunctionRegistry, HostFunction, HostFunctionCall};
pub use error::{WasmError, WasmResult};
pub use wit_runtime::{WitRuntime, WitRuntimeConfig, WitComponent};
pub use dtl_bridge::{DTLWasmBridge, WasmFunctionInfo, DTLWasmFunction};
pub use ai_integration::{AIWasmIntegrator, AIWasmConfig, AIWasmStats};
pub use registry_integration::{WasmProcessorRegistry, WasmProcessorFactory, WasmProcessorTypeInfo};
pub use profiler::{WasmProfiler, PerformanceReport, ExecutionPhase};
pub use metrics::{MetricsCollector, SystemMetrics, PluginMetrics};
pub use dev_tools::{PluginDevTools, BuildConfig, TestConfig, TemplateType};
pub use marketplace::{
    DataFlareMarketplace, PluginMarketplace, MarketplaceConfig,
    PluginRegistry, PluginSearchEngine, PluginPackageManager,
    PluginSecurityScanner, PluginQualityScorer, PluginRecommendationEngine,
    PluginMetadata, PluginVersion, PluginStats, PluginSummary, PluginDetails,
    SearchQuery, SearchResult, SortBy, Pagination,
    SecurityRating, LicenseInfo, PluginDependency, PublishStatus,
};

use dataflare_core::error::Result;

/// DataFlare WASM模块版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 初始化WASM插件系统
///
/// # Examples
///
/// ```rust
/// use dataflare_wasm::init_wasm_system;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     init_wasm_system().await?;
///     println!("WASM插件系统初始化完成");
///     Ok(())
/// }
/// ```
pub async fn init_wasm_system() -> Result<()> {
    log::info!("初始化DataFlare WASM插件系统 v{}", VERSION);

    // 初始化运行时
    let _runtime = WasmRuntime::new(WasmRuntimeConfig::default())?;

    // 初始化插件注册表
    let _registry = WasmPluginRegistry::new();

    // 初始化组件工厂
    let _factory = WasmComponentFactory::new();

    log::info!("WASM插件系统初始化完成");
    Ok(())
}

/// 创建默认的WASM运行时
///
/// # Examples
///
/// ```rust
/// use dataflare_wasm::create_default_runtime;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let runtime = create_default_runtime().await?;
///     println!("WASM运行时创建完成");
///     Ok(())
/// }
/// ```
pub async fn create_default_runtime() -> Result<WasmRuntime> {
    let config = WasmRuntimeConfig::default();
    let mut runtime = WasmRuntime::new(config)?;
    runtime.initialize().await?;
    Ok(runtime)
}

/// 创建默认的组件管理器
///
/// # Examples
///
/// ```rust
/// use dataflare_wasm::create_default_component_manager;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let manager = create_default_component_manager().await?;
///     println!("组件管理器创建完成");
///     Ok(())
/// }
/// ```
pub async fn create_default_component_manager() -> Result<WasmComponentManager> {
    Ok(WasmComponentManager::new())
}

/// WASM插件系统构建器
pub struct WasmSystemBuilder {
    runtime_config: WasmRuntimeConfig,
    sandbox_config: SandboxConfig,
    enable_hot_reload: bool,
    enable_metrics: bool,
}

impl WasmSystemBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            runtime_config: WasmRuntimeConfig::default(),
            sandbox_config: SandboxConfig::default(),
            enable_hot_reload: false,
            enable_metrics: true,
        }
    }

    /// 设置运行时配置
    pub fn with_runtime_config(mut self, config: WasmRuntimeConfig) -> Self {
        self.runtime_config = config;
        self
    }

    /// 设置沙箱配置
    pub fn with_sandbox_config(mut self, config: SandboxConfig) -> Self {
        self.sandbox_config = config;
        self
    }

    /// 启用热重载
    pub fn enable_hot_reload(mut self) -> Self {
        self.enable_hot_reload = true;
        self
    }

    /// 启用指标收集
    pub fn enable_metrics(mut self) -> Self {
        self.enable_metrics = true;
        self
    }

    /// 构建WASM系统
    pub async fn build(self) -> Result<WasmSystem> {
        let runtime = WasmRuntime::new(self.runtime_config)?;
        let registry = WasmPluginRegistry::new();
        let component_manager = WasmComponentManager::new();
        let sandbox = WasmSandbox::new(self.sandbox_config)?;

        Ok(WasmSystem {
            runtime,
            registry,
            component_manager,
            sandbox,
            hot_reload_enabled: self.enable_hot_reload,
            metrics_enabled: self.enable_metrics,
        })
    }
}

impl Default for WasmSystemBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 完整的WASM插件系统
pub struct WasmSystem {
    /// WASM运行时
    pub runtime: WasmRuntime,
    /// 插件注册表
    pub registry: WasmPluginRegistry,
    /// 组件管理器
    pub component_manager: WasmComponentManager,
    /// 沙箱
    pub sandbox: WasmSandbox,
    /// 是否启用热重载
    pub hot_reload_enabled: bool,
    /// 是否启用指标收集
    pub metrics_enabled: bool,
}

impl WasmSystem {
    /// 创建新的WASM系统
    pub async fn new() -> Result<Self> {
        WasmSystemBuilder::new().build().await
    }

    /// 加载插件
    pub async fn load_plugin(&mut self, path: &str, config: WasmPluginConfig) -> Result<String> {
        let plugin = self.runtime.load_plugin(path).await?;
        let plugin_id = plugin.get_metadata().name.clone();

        self.registry.register_plugin(plugin_id.clone(), plugin).await?;

        log::info!("插件加载成功: {}", plugin_id);
        Ok(plugin_id)
    }

    /// 创建组件
    pub async fn create_component(&mut self, config: WasmComponentConfig) -> Result<String> {
        let component_id = config.name.clone();
        self.component_manager.load_component(component_id.clone(), &config).await?;

        log::info!("组件创建成功: {}", component_id);
        Ok(component_id)
    }

    /// 获取系统统计信息
    pub fn get_stats(&self) -> WasmSystemStats {
        WasmSystemStats {
            loaded_plugins: self.registry.list_plugins().len(),
            active_components: self.component_manager.list_components().len(),
            component_stats: self.component_manager.get_stats(),
            hot_reload_enabled: self.hot_reload_enabled,
            metrics_enabled: self.metrics_enabled,
        }
    }

    /// 清理系统
    pub async fn cleanup(&mut self) -> Result<()> {
        self.component_manager.cleanup_all().await?;
        self.registry.cleanup().await?;
        self.runtime.cleanup().await?;

        log::info!("WASM系统清理完成");
        Ok(())
    }
}

/// WASM系统统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WasmSystemStats {
    /// 已加载插件数量
    pub loaded_plugins: usize,
    /// 活跃组件数量
    pub active_components: usize,
    /// 组件统计信息
    pub component_stats: components::WasmComponentStats,
    /// 是否启用热重载
    pub hot_reload_enabled: bool,
    /// 是否启用指标收集
    pub metrics_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wasm_system_initialization() {
        let result = init_wasm_system().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_default_runtime_creation() {
        let result = create_default_runtime().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wasm_system_builder() {
        let builder = WasmSystemBuilder::new()
            .enable_hot_reload()
            .enable_metrics();

        let result = builder.build().await;
        assert!(result.is_ok());

        let system = result.unwrap();
        assert!(system.hot_reload_enabled);
        assert!(system.metrics_enabled);
    }

    #[tokio::test]
    async fn test_wasm_system_stats() {
        let system = WasmSystem::new().await.unwrap();
        let stats = system.get_stats();

        assert_eq!(stats.loaded_plugins, 0);
        assert_eq!(stats.active_components, 0);
    }
}
