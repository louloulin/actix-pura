/// AI分析处理器
/// 
/// 基于lumos.ai提供各种AI驱动的文本分析功能，包括：
/// - 情感分析
/// - 实体提取
/// - 文本分类
/// - 文本摘要
/// - 语言检测

use super::{AIOperation, utils};
use super::{LlmProvider, LlmOptions, Message, Role};
use dataflare_core::processor::{Processor, ProcessorState};
use dataflare_core::message::{DataRecord, DataRecordBatch};
use dataflare_core::error::{DataFlareError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use async_trait::async_trait;
use std::collections::HashMap;
use log::{info, debug, warn, error};

/// AI分析处理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIAnalysisConfig {
    /// 模型引用名称
    pub model: String,
    /// AI操作列表
    pub operations: Vec<AIOperation>,
    /// 批处理大小
    pub batch_size: Option<usize>,
    /// 超时时间 (毫秒)
    pub timeout_ms: Option<u64>,
    /// 是否启用结果缓存
    pub cache_results: Option<bool>,
}

/// AI分析处理器
pub struct AIAnalysisProcessor {
    /// 处理器配置
    config: AIAnalysisConfig,
    /// 结果缓存 (操作+文本 -> 结果)
    result_cache: HashMap<String, Value>,
    /// 处理器状态
    state: ProcessorState,
}

impl std::fmt::Debug for AIAnalysisProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIAnalysisProcessor")
            .field("config", &self.config)
            .field("result_cache", &self.result_cache)
            .field("state", &self.state)
            .finish()
    }
}

impl AIAnalysisProcessor {
    /// 创建新的AI分析处理器
    pub fn new() -> Self {
        Self {
            config: AIAnalysisConfig {
                model: String::new(),
                operations: Vec::new(),
                batch_size: Some(10),
                timeout_ms: Some(30000), // 30秒超时
                cache_results: Some(true),
            },
            result_cache: HashMap::new(),
            state: ProcessorState::new("ai_analysis"),
        }
    }

    /// 从JSON配置创建处理器
    pub fn from_json(config: Value) -> Result<Self> {
        let mut processor = Self::new();
        processor.configure(&config)?;
        Ok(processor)
    }

    /// 执行AI操作 (使用lumos.ai)
    async fn execute_ai_operation(&self, operation: &AIOperation, text: &str) -> Result<Value> {
        // 检查缓存
        let cache_key = format!("{}:{}", operation.operation_type, text);
        if self.config.cache_results.unwrap_or(true) {
            if let Some(cached_result) = self.result_cache.get(&cache_key) {
                debug!("使用缓存的分析结果: {}", operation.operation_type);
                return Ok(cached_result.clone());
            }
        }

        // 执行AI操作 (目前使用模拟实现，实际应该调用lumos.ai的LLM服务)
        let result = match operation.operation_type.as_str() {
            "sentiment_analysis" => {
                let sentiment = self.analyze_sentiment_simple(text);
                Value::String(sentiment)
            },
            "entity_extraction" => {
                let entities = self.extract_entities_simple(text);
                Value::Array(entities.into_iter().map(Value::String).collect())
            },
            "text_classification" => {
                let categories = self.extract_categories(operation)?;
                let category = self.classify_text_simple(text, &categories);
                Value::String(category)
            },
            "summarization" => {
                let max_length = self.extract_max_length(operation);
                let summary = self.summarize_text_simple(text, max_length);
                Value::String(summary)
            },
            "language_detection" => {
                let language = self.detect_language_simple(text);
                Value::String(language)
            },
            "text_generation" => {
                let generated = format!("Generated response for: {}", text);
                Value::String(generated)
            },
            _ => {
                return Err(DataFlareError::Processing(
                    format!("不支持的AI操作类型: {}", operation.operation_type)
                ));
            }
        };

        debug!("AI操作完成: {} -> {:?}", operation.operation_type, result);
        Ok(result)
    }

    /// 缓存分析结果
    fn cache_result(&mut self, operation: &AIOperation, text: &str, result: &Value) {
        if self.config.cache_results.unwrap_or(true) {
            let cache_key = format!("{}:{}", operation.operation_type, text);
            self.result_cache.insert(cache_key, result.clone());
        }
    }

    /// 简单的情感分析实现
    fn analyze_sentiment_simple(&self, text: &str) -> String {
        if text.contains("good") || text.contains("great") || text.contains("excellent") {
            "positive".to_string()
        } else if text.contains("bad") || text.contains("terrible") || text.contains("awful") {
            "negative".to_string()
        } else {
            "neutral".to_string()
        }
    }

    /// 简单的实体提取实现
    fn extract_entities_simple(&self, text: &str) -> Vec<String> {
        text.split_whitespace()
            .filter(|word| word.chars().next().unwrap_or('a').is_uppercase())
            .map(|word| word.to_string())
            .collect()
    }

    /// 简单的文本分类实现
    fn classify_text_simple(&self, text: &str, categories: &[String]) -> String {
        for category in categories {
            if text.to_lowercase().contains(&category.to_lowercase()) {
                return category.clone();
            }
        }
        categories.first().unwrap_or(&"unknown".to_string()).clone()
    }

    /// 简单的文本摘要实现
    fn summarize_text_simple(&self, text: &str, max_length: Option<u32>) -> String {
        let max_len = max_length.unwrap_or(100) as usize;
        if text.len() <= max_len {
            text.to_string()
        } else {
            format!("{}...", &text[..max_len])
        }
    }

    /// 简单的语言检测
    fn detect_language_simple(&self, text: &str) -> String {
        let english_words = ["the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by"];
        let chinese_chars = text.chars().any(|c| c as u32 >= 0x4e00 && c as u32 <= 0x9fff);
        
        if chinese_chars {
            "zh".to_string()
        } else if english_words.iter().any(|&word| text.to_lowercase().contains(word)) {
            "en".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 从操作参数中提取分类类别
    fn extract_categories(&self, operation: &AIOperation) -> Result<Vec<String>> {
        if let Some(ref params) = operation.parameters {
            if let Some(categories) = params.get("categories") {
                match categories {
                    Value::Array(arr) => {
                        let cats: std::result::Result<Vec<String>, DataFlareError> = arr
                            .iter()
                            .map(|v| v.as_str().ok_or_else(|| 
                                DataFlareError::Config("分类类别必须是字符串".to_string())
                            ).map(|s| s.to_string()))
                            .collect();
                        return cats;
                    },
                    _ => return Err(DataFlareError::Config("分类类别必须是数组".to_string())),
                }
            }
        }
        
        // 默认分类类别
        Ok(vec!["positive".to_string(), "negative".to_string(), "neutral".to_string()])
    }

    /// 从操作参数中提取最大长度
    fn extract_max_length(&self, operation: &AIOperation) -> Option<u32> {
        operation.parameters
            .as_ref()
            .and_then(|params| params.get("max_length"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
    }
}

#[async_trait]
impl Processor for AIAnalysisProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = serde_json::from_value(config.clone())
            .map_err(|e| DataFlareError::Config(format!("无效的AI分析处理器配置: {}", e)))?;

        // 验证配置
        if self.config.operations.is_empty() {
            return Err(DataFlareError::Config("至少需要配置一个AI操作".to_string()));
        }

        // 验证操作类型
        for operation in &self.config.operations {
            match operation.operation_type.as_str() {
                "sentiment_analysis" | "entity_extraction" | "text_classification" | 
                "summarization" | "language_detection" | "text_generation" => {},
                _ => {
                    return Err(DataFlareError::Config(
                        format!("不支持的AI操作类型: {}", operation.operation_type)
                    ));
                }
            }
        }

        info!("AI分析处理器配置成功，模型: {}, 操作数: {}", 
              self.config.model, self.config.operations.len());
        Ok(())
    }

    async fn initialize(&mut self) -> Result<()> {
        info!("初始化AI分析处理器");
        // TODO: 初始化lumos.ai LLM客户端
        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord) -> Result<DataRecord> {
        let mut new_record = record.clone();
        let operations = self.config.operations.clone();

        for operation in &operations {
            if let Some(text) = utils::extract_text_value(record, &operation.input_field) {
                if !text.trim().is_empty() {
                    match self.execute_ai_operation(operation, &text).await {
                        Ok(result) => {
                            // 缓存结果
                            self.cache_result(operation, &text, &result);
                            
                            // 设置输出字段
                            new_record.set_value(&operation.output_field, result)?;
                            
                            debug!("AI操作 '{}' 完成: {} -> {}", 
                                   operation.operation_type, operation.input_field, operation.output_field);
                        },
                        Err(e) => {
                            error!("AI操作 '{}' 失败: {}", operation.operation_type, e);
                            // 设置错误标记但继续处理
                            new_record.set_value(&operation.output_field, Value::Null)?;
                        }
                    }
                } else {
                    warn!("字段 '{}' 的文本为空，跳过AI分析", operation.input_field);
                    new_record.set_value(&operation.output_field, Value::Null)?;
                }
            } else {
                warn!("字段 '{}' 不存在或不是文本类型", operation.input_field);
                new_record.set_value(&operation.output_field, Value::Null)?;
            }
        }

        // 更新处理器状态
        self.state.records_processed += 1;

        Ok(new_record)
    }

    async fn process_batch(&mut self, batch: &DataRecordBatch) -> Result<DataRecordBatch> {
        let mut processed_records = Vec::with_capacity(batch.records.len());

        for record in &batch.records {
            let processed = self.process_record(record).await?;
            processed_records.push(processed);
        }

        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        info!("批量AI分析完成: {} 条记录", batch.records.len());

        Ok(new_batch)
    }

    async fn finalize(&mut self) -> Result<()> {
        info!("AI分析处理器完成，共处理 {} 条记录，缓存 {} 个结果", 
              self.state.records_processed, self.result_cache.len());
        Ok(())
    }

    fn get_state(&self) -> ProcessorState {
        self.state.clone()
    }

    fn get_input_schema(&self) -> Option<dataflare_core::Schema> {
        None // TODO: 实现输入模式定义
    }

    fn get_output_schema(&self) -> Option<dataflare_core::Schema> {
        None // TODO: 实现输出模式定义
    }
}

impl Default for AIAnalysisProcessor {
    fn default() -> Self {
        Self::new()
    }
}
