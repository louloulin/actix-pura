//! DataFlare处理器注册表集成
//!
//! 将WASM插件集成到DataFlare的处理器注册表中

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, debug, warn, error};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    processor::{Processor, ProcessorState},
    model::Schema,
};

use crate::{
    WasmRuntime, DataFlareWasmProcessor,
    ai_integration::{AIWasmIntegrator, AIWasmConfig},
    dtl_bridge::DTLWasmBridge,
    error::{WasmError, WasmResult},
};

/// WASM处理器注册表
pub struct WasmProcessorRegistry {
    /// WASM运行时
    wasm_runtime: Arc<RwLock<WasmRuntime>>,
    /// DTL桥接器
    dtl_bridge: Arc<RwLock<DTLWasmBridge>>,
    /// AI集成器
    ai_integrator: Arc<RwLock<AIWasmIntegrator>>,
    /// 已注册的处理器类型
    registered_types: HashMap<String, WasmProcessorTypeInfo>,
    /// 处理器实例缓存
    processor_cache: HashMap<String, Arc<RwLock<DataFlareWasmProcessor>>>,
}

/// WASM处理器类型信息
#[derive(Debug, Clone)]
pub struct WasmProcessorTypeInfo {
    /// 处理器类型名称
    pub type_name: String,
    /// 插件ID
    pub plugin_id: String,
    /// 描述
    pub description: String,
    /// 支持的功能
    pub capabilities: Vec<String>,
    /// 配置模式
    pub config_schema: Option<Value>,
    /// 是否支持批处理
    pub supports_batch: bool,
    /// 是否支持流处理
    pub supports_streaming: bool,
}

impl WasmProcessorRegistry {
    /// 创建新的WASM处理器注册表
    pub fn new(
        wasm_runtime: Arc<RwLock<WasmRuntime>>,
        dtl_bridge: Arc<RwLock<DTLWasmBridge>>,
        ai_integrator: Arc<RwLock<AIWasmIntegrator>>,
    ) -> Self {
        Self {
            wasm_runtime,
            dtl_bridge,
            ai_integrator,
            registered_types: HashMap::new(),
            processor_cache: HashMap::new(),
        }
    }

    /// 初始化注册表
    pub async fn initialize(&mut self) -> Result<()> {
        info!("初始化WASM处理器注册表");

        // 注册标准WASM处理器类型
        self.register_standard_types().await?;

        // 尝试初始化AI集成（如果失败，不影响标准类型的注册）
        {
            let mut ai_integrator = self.ai_integrator.write().await;
            if let Err(e) = ai_integrator.initialize().await {
                warn!("AI集成器初始化失败，但标准处理器类型已注册: {}", e);
                // 不返回错误，允许标准类型继续使用
            } else {
                info!("AI集成器初始化成功");
            }
        }

        info!("WASM处理器注册表初始化完成，已注册{}个处理器类型", self.registered_types.len());
        Ok(())
    }

    /// 注册标准处理器类型
    async fn register_standard_types(&mut self) -> Result<()> {
        // 注册WASM映射处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "wasm_mapping".to_string(),
            plugin_id: "wasm_mapping_plugin".to_string(),
            description: "WASM数据映射处理器".to_string(),
            capabilities: vec!["transform".to_string(), "map".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "mapping_config": {"type": "object"}
                },
                "required": ["plugin_path"]
            })),
            supports_batch: true,
            supports_streaming: true,
        }).await?;

        // 注册WASM过滤处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "wasm_filter".to_string(),
            plugin_id: "wasm_filter_plugin".to_string(),
            description: "WASM数据过滤处理器".to_string(),
            capabilities: vec!["filter".to_string(), "validate".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "filter_config": {"type": "object"}
                },
                "required": ["plugin_path"]
            })),
            supports_batch: true,
            supports_streaming: true,
        }).await?;

        // 注册WASM聚合处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "wasm_aggregate".to_string(),
            plugin_id: "wasm_aggregate_plugin".to_string(),
            description: "WASM数据聚合处理器".to_string(),
            capabilities: vec!["aggregate".to_string(), "group".to_string(), "reduce".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "aggregate_config": {"type": "object"}
                },
                "required": ["plugin_path"]
            })),
            supports_batch: true,
            supports_streaming: false,
        }).await?;

        // 注册WASM丰富化处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "wasm_enrichment".to_string(),
            plugin_id: "wasm_enrichment_plugin".to_string(),
            description: "WASM数据丰富化处理器".to_string(),
            capabilities: vec!["enrich".to_string(), "lookup".to_string(), "join".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "enrichment_config": {"type": "object"}
                },
                "required": ["plugin_path"]
            })),
            supports_batch: true,
            supports_streaming: true,
        }).await?;

        // 注册WASM连接处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "wasm_join".to_string(),
            plugin_id: "wasm_join_plugin".to_string(),
            description: "WASM数据连接处理器".to_string(),
            capabilities: vec!["join".to_string(), "merge".to_string(), "combine".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "join_config": {"type": "object"}
                },
                "required": ["plugin_path"]
            })),
            supports_batch: true,
            supports_streaming: false,
        }).await?;

        // 注册WASM DTL处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "wasm_dtl".to_string(),
            plugin_id: "wasm_dtl_plugin".to_string(),
            description: "WASM DTL扩展处理器".to_string(),
            capabilities: vec!["transform".to_string(), "dtl".to_string(), "script".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "dtl_script": {"type": "string"},
                    "wasm_functions": {"type": "array"}
                },
                "required": ["plugin_path", "dtl_script"]
            })),
            supports_batch: true,
            supports_streaming: true,
        }).await?;

        // 注册AI相关的WASM处理器
        self.register_ai_processor_types().await?;

        Ok(())
    }

    /// 注册AI处理器类型
    async fn register_ai_processor_types(&mut self) -> Result<()> {
        // 注册AI嵌入处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "ai_embedding_wasm".to_string(),
            plugin_id: "ai_embedding_wasm_plugin".to_string(),
            description: "基于WASM的AI嵌入处理器".to_string(),
            capabilities: vec!["ai".to_string(), "embedding".to_string(), "vector".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "model_config": {"type": "object"},
                    "embedding_config": {"type": "object"}
                },
                "required": ["plugin_path"]
            })),
            supports_batch: true,
            supports_streaming: true,
        }).await?;

        // 注册AI分析处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "ai_analysis_wasm".to_string(),
            plugin_id: "ai_analysis_wasm_plugin".to_string(),
            description: "基于WASM的AI分析处理器".to_string(),
            capabilities: vec!["ai".to_string(), "analysis".to_string(), "nlp".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "analysis_config": {"type": "object"}
                },
                "required": ["plugin_path"]
            })),
            supports_batch: true,
            supports_streaming: true,
        }).await?;

        // 注册向量搜索处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "ai_vector_search_wasm".to_string(),
            plugin_id: "ai_vector_search_wasm_plugin".to_string(),
            description: "基于WASM的向量搜索处理器".to_string(),
            capabilities: vec!["ai".to_string(), "vector".to_string(), "search".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "vector_store_config": {"type": "object"},
                    "search_config": {"type": "object"}
                },
                "required": ["plugin_path"]
            })),
            supports_batch: true,
            supports_streaming: true,
        }).await?;

        // 注册AI路由处理器
        self.register_processor_type(WasmProcessorTypeInfo {
            type_name: "ai_router_wasm".to_string(),
            plugin_id: "ai_router_wasm_plugin".to_string(),
            description: "基于WASM的AI路由处理器".to_string(),
            capabilities: vec!["ai".to_string(), "router".to_string(), "decision".to_string()],
            config_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "plugin_path": {"type": "string"},
                    "routing_config": {"type": "object"}
                },
                "required": ["plugin_path"]
            })),
            supports_batch: true,
            supports_streaming: true,
        }).await?;

        Ok(())
    }

    /// 注册处理器类型
    pub async fn register_processor_type(&mut self, type_info: WasmProcessorTypeInfo) -> Result<()> {
        let type_name = type_info.type_name.clone();
        self.registered_types.insert(type_name.clone(), type_info);
        info!("WASM处理器类型注册成功: {}", type_name);
        Ok(())
    }

    /// 创建处理器实例
    pub async fn create_processor(
        &mut self,
        processor_type: &str,
        config: Value,
    ) -> Result<Arc<RwLock<DataFlareWasmProcessor>>> {
        let type_info = self.registered_types.get(processor_type)
            .ok_or_else(|| DataFlareError::Config(format!("未知的WASM处理器类型: {}", processor_type)))?
            .clone();

        // 检查缓存
        let cache_key = format!("{}:{}", processor_type, serde_json::to_string(&config).unwrap_or_default());
        if let Some(cached_processor) = self.processor_cache.get(&cache_key) {
            return Ok(cached_processor.clone());
        }

        // 创建新的处理器实例
        let processor = DataFlareWasmProcessor::from_config(&config)
            .map_err(|e| DataFlareError::Plugin(format!("创建WASM处理器失败: {}", e)))?;

        let processor_arc = Arc::new(RwLock::new(processor));

        // 缓存处理器实例
        self.processor_cache.insert(cache_key, processor_arc.clone());

        info!("WASM处理器实例创建成功: {}", processor_type);
        Ok(processor_arc)
    }

    /// 获取已注册的处理器类型
    pub fn get_registered_types(&self) -> Vec<String> {
        self.registered_types.keys().cloned().collect()
    }

    /// 获取处理器类型信息
    pub fn get_type_info(&self, processor_type: &str) -> Option<&WasmProcessorTypeInfo> {
        self.registered_types.get(processor_type)
    }

    /// 检查处理器类型是否支持批处理
    pub fn supports_batch(&self, processor_type: &str) -> bool {
        self.registered_types.get(processor_type)
            .map(|info| info.supports_batch)
            .unwrap_or(false)
    }

    /// 检查处理器类型是否支持流处理
    pub fn supports_streaming(&self, processor_type: &str) -> bool {
        self.registered_types.get(processor_type)
            .map(|info| info.supports_streaming)
            .unwrap_or(false)
    }

    /// 获取处理器配置模式
    pub fn get_config_schema(&self, processor_type: &str) -> Option<&Value> {
        self.registered_types.get(processor_type)
            .and_then(|info| info.config_schema.as_ref())
    }

    /// 清除处理器缓存
    pub fn clear_cache(&mut self) {
        self.processor_cache.clear();
        info!("WASM处理器缓存已清除");
    }

    /// 获取缓存大小
    pub fn get_cache_size(&self) -> usize {
        self.processor_cache.len()
    }

    /// 注销处理器类型
    pub fn unregister_processor_type(&mut self, processor_type: &str) -> bool {
        let removed = self.registered_types.remove(processor_type).is_some();
        if removed {
            // 清除相关的缓存
            self.processor_cache.retain(|key, _| !key.starts_with(&format!("{}:", processor_type)));
            info!("WASM处理器类型注销成功: {}", processor_type);
        }
        removed
    }
}

/// WASM处理器工厂
pub struct WasmProcessorFactory {
    /// 注册表
    registry: Arc<RwLock<WasmProcessorRegistry>>,
}

impl WasmProcessorFactory {
    /// 创建新的工厂
    pub fn new(registry: Arc<RwLock<WasmProcessorRegistry>>) -> Self {
        Self { registry }
    }

    /// 创建处理器
    pub async fn create_processor(
        &self,
        processor_type: &str,
        config: Value,
    ) -> Result<Arc<RwLock<DataFlareWasmProcessor>>> {
        let mut registry = self.registry.write().await;
        registry.create_processor(processor_type, config).await
    }

    /// 获取支持的处理器类型
    pub async fn get_supported_types(&self) -> Vec<String> {
        let registry = self.registry.read().await;
        registry.get_registered_types()
    }

    /// 验证处理器配置
    pub async fn validate_config(&self, processor_type: &str, config: &Value) -> Result<()> {
        let registry = self.registry.read().await;

        let type_info = registry.get_type_info(processor_type)
            .ok_or_else(|| DataFlareError::Config(format!("未知的处理器类型: {}", processor_type)))?;

        // 这里可以添加更详细的配置验证逻辑
        // 例如使用JSON Schema验证

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::WasmRuntimeConfig;

    #[tokio::test]
    async fn test_wasm_processor_registry_creation() {
        let wasm_config = WasmRuntimeConfig::default();
        let wasm_runtime = Arc::new(RwLock::new(WasmRuntime::new(wasm_config).unwrap()));
        let dtl_bridge = Arc::new(RwLock::new(DTLWasmBridge::new(wasm_runtime.clone())));
        let ai_config = AIWasmConfig::default();
        let ai_integrator = Arc::new(RwLock::new(AIWasmIntegrator::new(ai_config, wasm_runtime.clone(), dtl_bridge.clone())));

        let registry = WasmProcessorRegistry::new(wasm_runtime, dtl_bridge, ai_integrator);
        assert_eq!(registry.get_registered_types().len(), 0);
    }
}
