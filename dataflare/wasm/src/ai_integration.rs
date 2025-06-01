//! AI处理器与WASM插件的深度集成
//!
//! 实现AI功能与WASM插件的无缝协作

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{info, debug, warn, error};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::DataRecord,
};

use crate::{
    WasmPlugin, WasmRuntime,
    interface::{WasmFunctionCall, WasmPluginInterface},
    error::{WasmError, WasmResult},
    dtl_bridge::DTLWasmBridge,
};

/// AI-WASM集成配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AIWasmConfig {
    /// 启用的AI功能
    pub enabled_ai_functions: Vec<String>,
    /// WASM插件映射
    pub wasm_plugin_mappings: HashMap<String, String>,
    /// AI模型配置
    pub ai_model_config: HashMap<String, Value>,
    /// 向量存储配置
    pub vector_store_config: Option<Value>,
    /// 缓存配置
    pub cache_config: Option<Value>,
}

impl Default for AIWasmConfig {
    fn default() -> Self {
        Self {
            enabled_ai_functions: vec![
                "embed".to_string(),
                "sentiment_analysis".to_string(),
                "vector_search".to_string(),
                "llm_summarize".to_string(),
                "text_classification".to_string(),
            ],
            wasm_plugin_mappings: HashMap::new(),
            ai_model_config: HashMap::new(),
            vector_store_config: None,
            cache_config: None,
        }
    }
}

/// AI-WASM集成器
pub struct AIWasmIntegrator {
    /// 配置
    config: AIWasmConfig,
    /// WASM运行时
    wasm_runtime: Arc<RwLock<WasmRuntime>>,
    /// DTL桥接器
    dtl_bridge: Arc<RwLock<DTLWasmBridge>>,
    /// AI函数缓存
    ai_function_cache: HashMap<String, Value>,
    /// 集成统计
    stats: AIWasmStats,
}

/// AI-WASM集成统计
#[derive(Debug, Clone, Default)]
pub struct AIWasmStats {
    /// AI函数调用次数
    pub ai_function_calls: u64,
    /// WASM函数调用次数
    pub wasm_function_calls: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
    /// 总执行时间 (毫秒)
    pub total_execution_time_ms: u64,
    /// 错误次数
    pub error_count: u64,
}

impl AIWasmIntegrator {
    /// 创建新的AI-WASM集成器
    pub fn new(
        config: AIWasmConfig,
        wasm_runtime: Arc<RwLock<WasmRuntime>>,
        dtl_bridge: Arc<RwLock<DTLWasmBridge>>,
    ) -> Self {
        Self {
            config,
            wasm_runtime,
            dtl_bridge,
            ai_function_cache: HashMap::new(),
            stats: AIWasmStats::default(),
        }
    }

    /// 初始化AI-WASM集成
    pub async fn initialize(&mut self) -> Result<()> {
        info!("初始化AI-WASM集成");

        // 克隆配置以避免借用冲突
        let enabled_functions = self.config.enabled_ai_functions.clone();
        let plugin_mappings = self.config.wasm_plugin_mappings.clone();

        // 注册AI函数到DTL桥接器
        for ai_function in &enabled_functions {
            self.register_ai_function(ai_function).await?;
        }

        // 注册WASM插件映射
        for (ai_function, plugin_id) in &plugin_mappings {
            self.register_wasm_mapping(ai_function, plugin_id).await?;
        }

        info!("AI-WASM集成初始化完成");
        Ok(())
    }

    /// 注册AI函数
    async fn register_ai_function(&mut self, function_name: &str) -> Result<()> {
        let mut bridge = self.dtl_bridge.write().await;

        match function_name {
            "embed" => {
                bridge.register_wasm_function(
                    "embed".to_string(),
                    "ai_embedding_plugin".to_string(),
                    "generate_embedding".to_string(),
                    Some("生成文本嵌入向量".to_string()),
                ).await.map_err(|e| DataFlareError::Plugin(format!("注册embed函数失败: {}", e)))?;
            },
            "sentiment_analysis" => {
                bridge.register_wasm_function(
                    "sentiment_analysis".to_string(),
                    "ai_sentiment_plugin".to_string(),
                    "analyze_sentiment".to_string(),
                    Some("分析文本情感".to_string()),
                ).await.map_err(|e| DataFlareError::Plugin(format!("注册sentiment_analysis函数失败: {}", e)))?;
            },
            "vector_search" => {
                bridge.register_wasm_function(
                    "vector_search".to_string(),
                    "ai_vector_plugin".to_string(),
                    "search_vectors".to_string(),
                    Some("向量相似性搜索".to_string()),
                ).await.map_err(|e| DataFlareError::Plugin(format!("注册vector_search函数失败: {}", e)))?;
            },
            "llm_summarize" => {
                bridge.register_wasm_function(
                    "llm_summarize".to_string(),
                    "ai_llm_plugin".to_string(),
                    "summarize_text".to_string(),
                    Some("LLM文本摘要".to_string()),
                ).await.map_err(|e| DataFlareError::Plugin(format!("注册llm_summarize函数失败: {}", e)))?;
            },
            "text_classification" => {
                bridge.register_wasm_function(
                    "text_classification".to_string(),
                    "ai_classification_plugin".to_string(),
                    "classify_text".to_string(),
                    Some("文本分类".to_string()),
                ).await.map_err(|e| DataFlareError::Plugin(format!("注册text_classification函数失败: {}", e)))?;
            },
            _ => {
                warn!("未知的AI函数: {}", function_name);
                return Ok(());
            }
        }

        info!("AI函数注册成功: {}", function_name);
        Ok(())
    }

    /// 注册WASM插件映射
    async fn register_wasm_mapping(&mut self, ai_function: &str, plugin_id: &str) -> Result<()> {
        let mut bridge = self.dtl_bridge.write().await;

        bridge.register_wasm_function(
            ai_function.to_string(),
            plugin_id.to_string(),
            "process".to_string(), // 默认处理函数
            Some(format!("AI函数 {} 的WASM实现", ai_function)),
        ).await.map_err(|e| DataFlareError::Plugin(format!("注册WASM映射失败: {}", e)))?;

        info!("WASM插件映射注册成功: {} -> {}", ai_function, plugin_id);
        Ok(())
    }

    /// 调用AI函数（支持WASM实现）
    pub async fn call_ai_function(
        &mut self,
        function_name: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        let start_time = std::time::Instant::now();
        self.stats.ai_function_calls += 1;

        // 检查缓存
        let cache_key = format!("{}:{}", function_name, serde_json::to_string(&args).unwrap_or_default());
        if let Some(cached_result) = self.ai_function_cache.get(&cache_key) {
            self.stats.cache_hits += 1;
            return Ok(cached_result.clone());
        }
        self.stats.cache_misses += 1;

        // 调用函数
        let result = match function_name {
            "embed" => self.call_embed_function(args).await,
            "sentiment_analysis" => self.call_sentiment_analysis(args).await,
            "vector_search" => self.call_vector_search(args).await,
            "llm_summarize" => self.call_llm_summarize(args).await,
            "text_classification" => self.call_text_classification(args).await,
            _ => {
                // 尝试调用自定义WASM函数
                self.call_custom_wasm_function(function_name, args).await
            }
        };

        // 更新统计
        let execution_time = start_time.elapsed().as_millis() as u64;
        self.stats.total_execution_time_ms += execution_time;

        match &result {
            Ok(value) => {
                // 缓存结果
                if self.should_cache_result(function_name) {
                    self.ai_function_cache.insert(cache_key, value.clone());
                }
            },
            Err(_) => {
                self.stats.error_count += 1;
            }
        }

        result
    }

    /// 调用嵌入函数
    async fn call_embed_function(&mut self, args: Vec<Value>) -> Result<Value> {
        if args.is_empty() {
            return Err(DataFlareError::Plugin("embed函数需要文本参数".to_string()));
        }

        let text = args[0].as_str()
            .ok_or_else(|| DataFlareError::Plugin("embed函数的第一个参数必须是字符串".to_string()))?;

        // 检查是否有WASM实现
        if self.config.wasm_plugin_mappings.contains_key("embed") {
            return self.call_wasm_ai_function("embed", args).await;
        }

        // 使用默认实现（这里应该调用实际的AI服务）
        // 为了演示，返回一个模拟的嵌入向量
        let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5]; // 模拟嵌入向量
        Ok(serde_json::to_value(embedding)?)
    }

    /// 调用情感分析函数
    async fn call_sentiment_analysis(&mut self, args: Vec<Value>) -> Result<Value> {
        if args.is_empty() {
            return Err(DataFlareError::Plugin("sentiment_analysis函数需要文本参数".to_string()));
        }

        // 检查是否有WASM实现
        if self.config.wasm_plugin_mappings.contains_key("sentiment_analysis") {
            return self.call_wasm_ai_function("sentiment_analysis", args).await;
        }

        // 使用默认实现
        let sentiment_result = serde_json::json!({
            "sentiment": "positive",
            "confidence": 0.85,
            "scores": {
                "positive": 0.85,
                "negative": 0.10,
                "neutral": 0.05
            }
        });
        Ok(sentiment_result)
    }

    /// 调用向量搜索函数
    async fn call_vector_search(&mut self, args: Vec<Value>) -> Result<Value> {
        if args.len() < 2 {
            return Err(DataFlareError::Plugin("vector_search函数需要查询向量和搜索参数".to_string()));
        }

        // 检查是否有WASM实现
        if self.config.wasm_plugin_mappings.contains_key("vector_search") {
            return self.call_wasm_ai_function("vector_search", args).await;
        }

        // 使用默认实现
        let search_results = serde_json::json!({
            "results": [
                {
                    "id": "doc1",
                    "score": 0.95,
                    "metadata": {"title": "相关文档1"}
                },
                {
                    "id": "doc2",
                    "score": 0.87,
                    "metadata": {"title": "相关文档2"}
                }
            ],
            "total": 2
        });
        Ok(search_results)
    }

    /// 调用LLM摘要函数
    async fn call_llm_summarize(&mut self, args: Vec<Value>) -> Result<Value> {
        if args.is_empty() {
            return Err(DataFlareError::Plugin("llm_summarize函数需要文本参数".to_string()));
        }

        // 检查是否有WASM实现
        if self.config.wasm_plugin_mappings.contains_key("llm_summarize") {
            return self.call_wasm_ai_function("llm_summarize", args).await;
        }

        // 使用默认实现
        let summary_result = serde_json::json!({
            "summary": "这是一个自动生成的摘要",
            "key_points": ["要点1", "要点2", "要点3"],
            "confidence": 0.92
        });
        Ok(summary_result)
    }

    /// 调用文本分类函数
    async fn call_text_classification(&mut self, args: Vec<Value>) -> Result<Value> {
        if args.is_empty() {
            return Err(DataFlareError::Plugin("text_classification函数需要文本参数".to_string()));
        }

        // 检查是否有WASM实现
        if self.config.wasm_plugin_mappings.contains_key("text_classification") {
            return self.call_wasm_ai_function("text_classification", args).await;
        }

        // 使用默认实现
        let classification_result = serde_json::json!({
            "category": "technology",
            "confidence": 0.89,
            "all_scores": {
                "technology": 0.89,
                "business": 0.08,
                "science": 0.03
            }
        });
        Ok(classification_result)
    }

    /// 调用WASM AI函数
    async fn call_wasm_ai_function(&mut self, function_name: &str, args: Vec<Value>) -> Result<Value> {
        self.stats.wasm_function_calls += 1;

        let mut bridge = self.dtl_bridge.write().await;
        bridge.call_wasm_function(function_name, args).await
    }

    /// 调用自定义WASM函数
    async fn call_custom_wasm_function(&mut self, function_name: &str, args: Vec<Value>) -> Result<Value> {
        self.stats.wasm_function_calls += 1;

        let mut bridge = self.dtl_bridge.write().await;
        bridge.call_wasm_function(function_name, args).await
    }

    /// 检查是否应该缓存结果
    fn should_cache_result(&self, function_name: &str) -> bool {
        // 对于计算密集型的AI函数，启用缓存
        matches!(function_name, "embed" | "sentiment_analysis" | "text_classification")
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &AIWasmStats {
        &self.stats
    }

    /// 清除缓存
    pub fn clear_cache(&mut self) {
        self.ai_function_cache.clear();
        info!("AI函数缓存已清除");
    }

    /// 获取缓存大小
    pub fn get_cache_size(&self) -> usize {
        self.ai_function_cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::WasmRuntimeConfig;

    #[tokio::test]
    async fn test_ai_wasm_integrator_creation() {
        let config = AIWasmConfig::default();
        let wasm_config = WasmRuntimeConfig::default();
        let wasm_runtime = Arc::new(RwLock::new(WasmRuntime::new(wasm_config).unwrap()));
        let dtl_bridge = Arc::new(RwLock::new(DTLWasmBridge::new(wasm_runtime.clone())));

        let integrator = AIWasmIntegrator::new(config, wasm_runtime, dtl_bridge);
        assert_eq!(integrator.get_cache_size(), 0);
    }
}
