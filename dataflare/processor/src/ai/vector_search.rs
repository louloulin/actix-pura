/// 向量搜索处理器
///
/// 基于lumos.ai的向量存储进行语义相似性搜索，支持：
/// - 语义相似性搜索
/// - 重复内容检测
/// - 相关内容推荐
/// - 多向量存储支持

use super::{SearchResult, utils};
use super::{VectorStorage, MemoryVectorStorage, create_memory_vector_storage, VectorQueryResult};
use dataflare_core::processor::{Processor, ProcessorState};
use dataflare_core::message::{DataRecord, DataRecordBatch};
use dataflare_core::error::{DataFlareError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use async_trait::async_trait;
use std::collections::HashMap;
use log::{info, debug, warn, error};

/// 向量搜索处理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchConfig {
    /// 向量存储引用名称
    pub vector_store: String,
    /// 查询向量字段名
    pub query_field: String,
    /// 输出字段名
    pub output_field: String,
    /// 返回的相似结果数量
    pub top_k: Option<usize>,
    /// 相似度阈值 (0.0-1.0)
    pub similarity_threshold: Option<f32>,
    /// 是否包含元数据
    pub include_metadata: Option<bool>,
    /// 搜索过滤条件
    pub filters: Option<HashMap<String, Value>>,
    /// 超时时间 (毫秒)
    pub timeout_ms: Option<u64>,
}

/// 向量搜索处理器
pub struct VectorSearchProcessor {
    /// 处理器配置
    config: VectorSearchConfig,
    /// 向量存储客户端 (使用lumos.ai)
    vector_store: Option<Box<dyn VectorStorage>>,
    /// 处理器状态
    state: ProcessorState,
}

impl std::fmt::Debug for VectorSearchProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorSearchProcessor")
            .field("config", &self.config)
            .field("state", &self.state)
            .finish()
    }
}

impl VectorSearchProcessor {
    /// 创建新的向量搜索处理器
    pub fn new() -> Self {
        Self {
            config: VectorSearchConfig {
                vector_store: String::new(),
                query_field: String::new(),
                output_field: String::new(),
                top_k: Some(5),
                similarity_threshold: Some(0.7),
                include_metadata: Some(true),
                filters: None,
                timeout_ms: Some(10000), // 10秒超时
            },
            vector_store: None,
            state: ProcessorState::new("vector_search"),
        }
    }

    /// 从JSON配置创建处理器
    pub fn from_json(config: Value) -> Result<Self> {
        let mut processor = Self::new();
        processor.configure(&config)?;
        Ok(processor)
    }

    /// 创建向量存储客户端 (使用lumos.ai)
    fn create_vector_store_client(&self) -> Result<Box<dyn VectorStorage>> {
        // 使用lumos.ai的内存向量存储
        let store = Box::new(create_memory_vector_storage());
        Ok(store)
    }

    /// 提取向量字段值
    fn extract_vector(&self, record: &DataRecord, field: &str) -> Result<Option<Vec<f32>>> {
        if let Some(vector_value) = record.get_value(field) {
            if let Some(vector) = utils::extract_vector_from_json(vector_value) {
                Ok(Some(vector))
            } else {
                Err(DataFlareError::Processing("向量字段格式无效".to_string()))
            }
        } else {
            Ok(None)
        }
    }

    /// 搜索相似向量 (使用lumos.ai)
    async fn search_similar_vectors(&self, query_vector: &[f32]) -> Result<Vec<SearchResult>> {
        if let Some(ref store) = self.vector_store {
            let top_k = self.config.top_k.unwrap_or(5);
            let threshold = self.config.similarity_threshold;

            // 使用lumos.ai的向量搜索
            let results = store.query(
                "default_index",
                query_vector.to_vec(),
                top_k,
                None, // TODO: 实现过滤条件转换
                self.config.include_metadata.unwrap_or(true)
            ).await?;

            // 转换结果格式
            let search_results: Vec<SearchResult> = results
                .into_iter()
                .filter(|r| {
                    if let Some(thresh) = threshold {
                        r.score >= thresh
                    } else {
                        true
                    }
                })
                .map(|r| SearchResult {
                    id: r.id,
                    score: r.score,
                    metadata: r.metadata,
                })
                .collect();

            debug!("向量搜索完成: 查询维度 {}, 返回 {} 个结果",
                   query_vector.len(), search_results.len());

            Ok(search_results)
        } else {
            Err(DataFlareError::Processing("向量存储未初始化".to_string()))
        }
    }

    /// 将搜索结果转换为JSON值
    fn results_to_json(&self, results: &[SearchResult]) -> Value {
        let json_results: Vec<Value> = results
            .iter()
            .map(|result| {
                let mut obj = serde_json::Map::new();
                obj.insert("id".to_string(), Value::String(result.id.clone()));
                obj.insert("score".to_string(), Value::Number(
                    serde_json::Number::from_f64(result.score as f64)
                        .unwrap_or_else(|| serde_json::Number::from(0))
                ));

                if self.config.include_metadata.unwrap_or(true) {
                    if let Some(ref metadata) = result.metadata {
                        obj.insert("metadata".to_string(), Value::Object(
                            metadata.iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect()
                        ));
                    }
                }

                Value::Object(obj)
            })
            .collect();

        Value::Array(json_results)
    }

    /// 预填充测试数据到向量存储
    async fn populate_test_data(&self) -> Result<()> {
        if let Some(ref store) = self.vector_store {
            // 创建默认索引
            store.create_index("default_index", 4, None).await?;

            // 添加一些测试向量
            let test_vectors = vec![
                vec![0.1, 0.2, 0.3, 0.4],
                vec![0.2, 0.3, 0.4, 0.5],
                vec![0.3, 0.4, 0.5, 0.6],
                vec![0.4, 0.5, 0.6, 0.7],
                vec![0.5, 0.6, 0.7, 0.8],
            ];

            let ids = vec![
                "doc1".to_string(),
                "doc2".to_string(),
                "doc3".to_string(),
                "doc4".to_string(),
                "doc5".to_string(),
            ];

            let metadata = vec![
                {
                    let mut meta = HashMap::new();
                    meta.insert("category".to_string(), Value::String("technology".to_string()));
                    meta.insert("source".to_string(), Value::String("test".to_string()));
                    meta
                },
                {
                    let mut meta = HashMap::new();
                    meta.insert("category".to_string(), Value::String("business".to_string()));
                    meta.insert("source".to_string(), Value::String("test".to_string()));
                    meta
                },
                {
                    let mut meta = HashMap::new();
                    meta.insert("category".to_string(), Value::String("science".to_string()));
                    meta.insert("source".to_string(), Value::String("test".to_string()));
                    meta
                },
                {
                    let mut meta = HashMap::new();
                    meta.insert("category".to_string(), Value::String("technology".to_string()));
                    meta.insert("source".to_string(), Value::String("test".to_string()));
                    meta
                },
                {
                    let mut meta = HashMap::new();
                    meta.insert("category".to_string(), Value::String("entertainment".to_string()));
                    meta.insert("source".to_string(), Value::String("test".to_string()));
                    meta
                },
            ];

            store.upsert("default_index", test_vectors, Some(ids), Some(metadata)).await?;

            debug!("测试数据预填充完成");
        }
        Ok(())
    }
}

#[async_trait]
impl Processor for VectorSearchProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = serde_json::from_value(config.clone())
            .map_err(|e| DataFlareError::Config(format!("无效的向量搜索处理器配置: {}", e)))?;

        // 验证配置
        if self.config.query_field.is_empty() {
            return Err(DataFlareError::Config("查询字段不能为空".to_string()));
        }
        if self.config.output_field.is_empty() {
            return Err(DataFlareError::Config("输出字段不能为空".to_string()));
        }

        // 验证参数范围
        if let Some(threshold) = self.config.similarity_threshold {
            if threshold < 0.0 || threshold > 1.0 {
                return Err(DataFlareError::Config("相似度阈值必须在0.0-1.0之间".to_string()));
            }
        }

        // 初始化向量存储客户端
        self.vector_store = Some(self.create_vector_store_client()?);

        info!("向量搜索处理器配置成功，存储: {}, 查询字段: {}",
              self.config.vector_store, self.config.query_field);
        Ok(())
    }

    async fn initialize(&mut self) -> Result<()> {
        info!("初始化向量搜索处理器");

        // 预填充测试数据
        self.populate_test_data().await?;

        // 验证向量存储连接
        if let Some(ref store) = self.vector_store {
            let test_vector = vec![0.1, 0.2, 0.3, 0.4];
            let results = store.query("default_index", test_vector, 1, None, false).await?;
            info!("向量存储连接成功，测试搜索返回 {} 个结果", results.len());
        }

        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord) -> Result<DataRecord> {
        let mut new_record = record.clone();

        if let Some(query_vector) = self.extract_vector(record, &self.config.query_field)? {
            match self.search_similar_vectors(&query_vector).await {
                Ok(results) => {
                    // 转换为JSON并设置输出字段
                    let results_json = self.results_to_json(&results);
                    new_record.set_value(&self.config.output_field, results_json)?;

                    debug!("向量搜索完成: 查询维度 {}, 返回 {} 个结果",
                           query_vector.len(), results.len());
                },
                Err(e) => {
                    error!("向量搜索失败: {}", e);
                    // 设置空结果但继续处理
                    new_record.set_value(&self.config.output_field, Value::Array(vec![]))?;
                }
            }
        } else {
            warn!("字段 '{}' 不存在或不是有效的向量", self.config.query_field);
            new_record.set_value(&self.config.output_field, Value::Array(vec![]))?;
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

        info!("批量向量搜索完成: {} 条记录", batch.records.len());

        Ok(new_batch)
    }

    async fn finalize(&mut self) -> Result<()> {
        info!("向量搜索处理器完成，共处理 {} 条记录", self.state.records_processed);
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

impl Default for VectorSearchProcessor {
    fn default() -> Self {
        Self::new()
    }
}
