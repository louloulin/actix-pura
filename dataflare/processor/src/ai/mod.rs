/// AI增强处理器模块
///
/// 基于lumos.ai提供AI原生的数据处理能力，包括：
/// - 文本嵌入生成
/// - 语义分析
/// - 向量搜索
/// - 智能路由

pub mod embedding;
pub mod analysis;
pub mod vector_search;
pub mod router;

// Re-exports
pub use embedding::AIEmbeddingProcessor;
pub use analysis::AIAnalysisProcessor;
pub use vector_search::VectorSearchProcessor;
pub use router::AIRouterProcessor;

use dataflare_core::error::Result;
use serde_json::Value;
use std::collections::HashMap;

// Re-export lumos.ai types for convenience
pub use lomusai_core::{
    LlmProvider, LlmOptions, Message, Role,
    VectorStorage, MemoryVectorStorage, SimilarityMetric, VectorStorageConfig,
    create_vector_storage, create_memory_vector_storage,
    EmbeddingService, create_random_embedding,
    Vector, IndexStats, VectorQueryResult, FilterCondition, Document
};

/// AI模型配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AIModelConfig {
    /// 模型提供商 (openai, anthropic, local, etc.)
    pub provider: String,
    /// 模型名称
    pub model: String,
    /// 模型配置参数
    pub config: HashMap<String, Value>,
}

/// AI操作类型
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AIOperation {
    /// 操作类型
    #[serde(rename = "type")]
    pub operation_type: String,
    /// 输入字段
    pub input_field: String,
    /// 输出字段
    pub output_field: String,
    /// 操作参数
    pub parameters: Option<HashMap<String, Value>>,
}

/// 路由规则
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoutingRule {
    /// 规则名称
    pub name: String,
    /// 条件表达式
    pub condition: String,
    /// 目标路由
    pub destination: String,
    /// 优先级 (数字越小优先级越高)
    pub priority: u32,
}

/// 嵌入字段配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingFieldConfig {
    /// 输入字段名
    pub field: String,
    /// 输出字段名
    pub output: String,
}

/// 向量搜索结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    /// 文档ID
    pub id: String,
    /// 相似度分数
    pub score: f32,
    /// 元数据
    pub metadata: Option<HashMap<String, Value>>,
}

/// 路由决策结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoutingDecision {
    /// 选择的路由
    pub route: String,
    /// 匹配的规则名称
    pub matched_rule: Option<String>,
    /// 决策原因
    pub reason: Option<String>,
    /// 规则优先级
    pub priority: Option<u32>,
    /// 决策时间戳
    pub timestamp: String,
}

/// AI处理器工厂
pub struct AIProcessorFactory;

impl AIProcessorFactory {
    /// 创建AI嵌入处理器
    pub fn create_embedding_processor(config: Value) -> Result<AIEmbeddingProcessor> {
        AIEmbeddingProcessor::from_json(config)
    }

    /// 创建AI分析处理器
    pub fn create_analysis_processor(config: Value) -> Result<AIAnalysisProcessor> {
        AIAnalysisProcessor::from_json(config)
    }

    /// 创建向量搜索处理器
    pub fn create_vector_search_processor(config: Value) -> Result<VectorSearchProcessor> {
        VectorSearchProcessor::from_json(config)
    }

    /// 创建AI路由处理器
    pub fn create_router_processor(config: Value) -> Result<AIRouterProcessor> {
        AIRouterProcessor::from_json(config)
    }
}

/// 计算余弦相似度
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// AI处理器通用工具函数
pub mod utils {
    use super::*;

    /// 提取文本字段值
    pub fn extract_text_value(record: &dataflare_core::message::DataRecord, field: &str) -> Option<String> {
        if let Some(v) = record.get_value(field) {
            match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    /// 将嵌入向量转换为JSON值
    pub fn embedding_to_json(embedding: &[f32]) -> Value {
        Value::Array(
            embedding
                .iter()
                .map(|&f| Value::Number(
                    serde_json::Number::from_f64(f as f64)
                        .unwrap_or_else(|| serde_json::Number::from(0))
                ))
                .collect()
        )
    }

    /// 从JSON值提取向量
    pub fn extract_vector_from_json(value: &Value) -> Option<Vec<f32>> {
        if let Value::Array(arr) = value {
            let vector: Option<Vec<f32>> = arr
                .iter()
                .map(|v| v.as_f64().map(|f| f as f32))
                .collect();
            vector
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let vec1 = vec![1.0, 2.0, 3.0];
        let vec2 = vec![1.0, 2.0, 3.0];
        let similarity = cosine_similarity(&vec1, &vec2);
        assert!((similarity - 1.0).abs() < f32::EPSILON);

        let vec3 = vec![1.0, 0.0, 0.0];
        let vec4 = vec![0.0, 1.0, 0.0];
        let similarity2 = cosine_similarity(&vec3, &vec4);
        assert!((similarity2 - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_embedding_to_json() {
        let embedding = vec![0.1, 0.2, 0.3];
        let json = utils::embedding_to_json(&embedding);

        if let Value::Array(arr) = json {
            assert_eq!(arr.len(), 3);
            assert!((arr[0].as_f64().unwrap() - 0.1).abs() < 1e-6);
            assert!((arr[1].as_f64().unwrap() - 0.2).abs() < 1e-6);
            assert!((arr[2].as_f64().unwrap() - 0.3).abs() < 1e-6);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_extract_vector_from_json() {
        let json = serde_json::json!([0.1, 0.2, 0.3]);
        let vector = utils::extract_vector_from_json(&json).unwrap();
        assert_eq!(vector, vec![0.1, 0.2, 0.3]);
    }
}
