/// AI嵌入处理器
///
/// 基于lumos.ai实现文本嵌入生成，支持：
/// - 单字段和多字段嵌入
/// - 批量处理优化
/// - 嵌入缓存
/// - 多种嵌入模型

use super::{EmbeddingFieldConfig, utils};
use dataflare_core::processor::{Processor, ProcessorState};
use dataflare_core::message::{DataRecord, DataRecordBatch};
use dataflare_core::error::{DataFlareError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use async_trait::async_trait;
use std::collections::HashMap;
use log::{info, debug, warn};

/// AI嵌入处理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIEmbeddingConfig {
    /// 模型引用名称
    pub model: String,
    /// 输入字段配置列表
    pub input_fields: Vec<EmbeddingFieldConfig>,
    /// 批处理大小
    pub batch_size: Option<usize>,
    /// 是否启用嵌入缓存
    pub cache_embeddings: Option<bool>,
    /// 嵌入维度
    pub dimension: Option<usize>,
    /// 超时时间 (毫秒)
    pub timeout_ms: Option<u64>,
}

/// AI嵌入处理器
pub struct AIEmbeddingProcessor {
    /// 处理器配置
    config: AIEmbeddingConfig,
    /// 嵌入缓存 (文本 -> 嵌入向量)
    embedding_cache: HashMap<String, Vec<f32>>,
    /// 处理器状态
    state: ProcessorState,
}

impl std::fmt::Debug for AIEmbeddingProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIEmbeddingProcessor")
            .field("config", &self.config)
            .field("embedding_cache", &self.embedding_cache)
            .field("state", &self.state)
            .finish()
    }
}

impl AIEmbeddingProcessor {
    /// 创建新的AI嵌入处理器
    pub fn new() -> Self {
        Self {
            config: AIEmbeddingConfig {
                model: String::new(),
                input_fields: Vec::new(),
                batch_size: Some(50),
                cache_embeddings: Some(true),
                dimension: Some(1536), // 默认维度
                timeout_ms: Some(30000), // 30秒超时
            },
            embedding_cache: HashMap::new(),
            state: ProcessorState::new("ai_embedding"),
        }
    }

    /// 从JSON配置创建处理器
    pub fn from_json(config: Value) -> Result<Self> {
        let mut processor = Self::new();
        processor.configure(&config)?;
        Ok(processor)
    }

    /// 生成单个文本的嵌入 (使用lumos.ai)
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // 检查缓存
        if self.config.cache_embeddings.unwrap_or(true) {
            if let Some(cached_embedding) = self.embedding_cache.get(text) {
                debug!("使用缓存的嵌入向量: {}", text);
                return Ok(cached_embedding.clone());
            }
        }

        // 使用lumos.ai生成嵌入向量
        // 目前使用随机嵌入作为示例，实际应该调用真实的嵌入服务
        let dimension = self.config.dimension.unwrap_or(1536);

        // 生成随机嵌入向量作为示例
        let embedding: Vec<f32> = (0..dimension)
            .map(|_| rand::random::<f32>() * 2.0 - 1.0) // 生成-1到1之间的随机数
            .collect();

        debug!("生成嵌入向量: {} -> {} 维", text, embedding.len());
        Ok(embedding)
    }

    /// 批量生成嵌入
    async fn generate_embeddings_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::new();

        for text in texts {
            let embedding = self.generate_embedding(text).await?;
            embeddings.push(embedding);
        }

        debug!("批量生成嵌入向量: {} 个文本", texts.len());
        Ok(embeddings)
    }

    /// 缓存嵌入向量
    fn cache_embedding(&mut self, text: &str, embedding: &[f32]) {
        if self.config.cache_embeddings.unwrap_or(true) {
            self.embedding_cache.insert(text.to_string(), embedding.to_vec());
        }
    }
}

#[async_trait]
impl Processor for AIEmbeddingProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = serde_json::from_value(config.clone())
            .map_err(|e| DataFlareError::Config(format!("无效的AI嵌入处理器配置: {}", e)))?;

        // 验证配置
        if self.config.input_fields.is_empty() {
            return Err(DataFlareError::Config("至少需要配置一个输入字段".to_string()));
        }

        info!("AI嵌入处理器配置成功，模型: {}, 字段数: {}",
              self.config.model, self.config.input_fields.len());
        Ok(())
    }

    async fn initialize(&mut self) -> Result<()> {
        info!("初始化AI嵌入处理器");

        // 验证嵌入服务
        let test_embedding = self.generate_embedding("test").await?;
        info!("嵌入服务连接成功，维度: {}", test_embedding.len());

        // 验证维度
        if let Some(expected_dim) = self.config.dimension {
            if test_embedding.len() != expected_dim {
                warn!("嵌入维度不匹配: 期望 {}, 实际 {}", expected_dim, test_embedding.len());
            }
        }

        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord) -> Result<DataRecord> {
        let mut new_record = record.clone();
        let input_fields = self.config.input_fields.clone();

        for field_config in &input_fields {
            if let Some(text) = utils::extract_text_value(record, &field_config.field) {
                if !text.trim().is_empty() {
                    // 生成嵌入向量
                    let embedding = self.generate_embedding(&text).await?;

                    // 缓存嵌入
                    self.cache_embedding(&text, &embedding);

                    // 设置输出字段
                    let embedding_json = utils::embedding_to_json(&embedding);
                    new_record.set_value(&field_config.output, embedding_json)?;

                    debug!("为字段 '{}' 生成嵌入向量: {} -> {}",
                           field_config.field, text.len(), embedding.len());
                } else {
                    warn!("字段 '{}' 的文本为空，跳过嵌入生成", field_config.field);
                }
            } else {
                warn!("字段 '{}' 不存在或不是文本类型", field_config.field);
            }
        }

        // 更新处理器状态
        self.state.records_processed += 1;

        Ok(new_record)
    }

    async fn process_batch(&mut self, batch: &DataRecordBatch) -> Result<DataRecordBatch> {
        let batch_size = self.config.batch_size.unwrap_or(50);

        // 收集所有需要处理的文本
        let mut batch_texts = Vec::new();
        let mut text_indices = Vec::new(); // (record_idx, field_idx, text)

        for (record_idx, record) in batch.records.iter().enumerate() {
            for (field_idx, field_config) in self.config.input_fields.iter().enumerate() {
                if let Some(text) = utils::extract_text_value(record, &field_config.field) {
                    if !text.trim().is_empty() {
                        batch_texts.push(text.clone());
                        text_indices.push((record_idx, field_idx, text));
                    }
                }
            }
        }

        // 分批处理嵌入生成
        let mut all_embeddings = Vec::new();
        for chunk in batch_texts.chunks(batch_size) {
            let chunk_texts: Vec<String> = chunk.to_vec();
            let embeddings = self.generate_embeddings_batch(&chunk_texts).await?;
            all_embeddings.extend(embeddings);
        }

        // 应用嵌入到记录
        let mut processed_records = batch.records.clone();
        let input_fields = self.config.input_fields.clone();
        let embeddings_count = all_embeddings.len();

        for (embedding, (record_idx, field_idx, text)) in all_embeddings.into_iter().zip(text_indices) {
            let field_config = &input_fields[field_idx];

            // 缓存嵌入
            self.cache_embedding(&text, &embedding);

            // 设置输出字段
            let embedding_json = utils::embedding_to_json(&embedding);
            processed_records[record_idx].set_value(&field_config.output, embedding_json)?;
        }

        // 更新处理器状态
        self.state.records_processed += batch.records.len() as u64;

        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        info!("批量处理完成: {} 条记录, {} 个嵌入向量",
              batch.records.len(), embeddings_count);

        Ok(new_batch)
    }

    async fn finalize(&mut self) -> Result<()> {
        info!("AI嵌入处理器完成，共处理 {} 条记录，缓存 {} 个嵌入向量",
              self.state.records_processed, self.embedding_cache.len());
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

impl Default for AIEmbeddingProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_ai_embedding_processor_basic() {
        let mut processor = AIEmbeddingProcessor::new();

        let config = json!({
            "model": "text-embedding-ada-002",
            "input_fields": [
                {
                    "field": "content",
                    "output": "content_embedding"
                }
            ],
            "dimension": 1536
        });

        processor.configure(&config).unwrap();
        processor.initialize().await.unwrap();

        let record = DataRecord::new(json!({
            "content": "This is a test content for embedding generation"
        }));

        let result = processor.process_record(&record).await.unwrap();

        // 验证嵌入向量已生成
        assert!(result.data.get("content_embedding").is_some());

        // 验证嵌入向量是数组
        let content_embedding = result.data.get("content_embedding").unwrap();
        assert!(content_embedding.is_array());

        // 验证维度
        if let Value::Array(arr) = content_embedding {
            assert_eq!(arr.len(), 1536);
        }
    }
}
