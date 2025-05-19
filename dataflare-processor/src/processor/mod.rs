//! 处理器模块
//!
//! 定义数据处理器接口和功能。

pub mod aggregate;
pub mod enrichment;
pub mod join;
pub mod registry;

use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::Value;

use crate::{
    error::Result,
    message::{DataRecord, DataRecordBatch},
};

/// 处理器状态
#[derive(Debug, Clone)]
pub struct ProcessorState {
    /// 状态数据
    pub data: Value,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl ProcessorState {
    /// 创建新的处理器状态
    pub fn new() -> Self {
        Self {
            data: Value::Null,
            metadata: HashMap::new(),
        }
    }

    /// 创建带数据的状态
    pub fn with_data(data: Value) -> Self {
        Self {
            data,
            metadata: HashMap::new(),
        }
    }

    /// 添加元数据
    pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl Default for ProcessorState {
    fn default() -> Self {
        Self::new()
    }
}

/// 数据处理器接口
#[async_trait]
pub trait Processor: Send + Sync + 'static {
    /// 配置处理器
    fn configure(&mut self, config: &Value) -> Result<()>;

    /// 处理单条记录
    async fn process_record(&mut self, record: &DataRecord, state: Option<ProcessorState>) -> Result<Vec<DataRecord>>;

    /// 处理记录批次
    async fn process_batch(&mut self, batch: &DataRecordBatch, state: Option<ProcessorState>) -> Result<DataRecordBatch>;

    /// 获取处理器状态
    fn get_state(&self) -> Result<ProcessorState>;

    /// 初始化处理器
    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    /// 结束处理器
    async fn finalize(&mut self) -> Result<()> {
        Ok(())
    }
}

/// 映射处理器
pub struct MappingProcessor {
    /// 处理器配置
    config: Option<Value>,
    /// 字段映射
    mappings: Vec<FieldMapping>,
    /// 处理器状态
    state: ProcessorState,
}

/// 字段映射
#[derive(Debug, Clone)]
pub struct FieldMapping {
    /// 源字段
    pub source: String,
    /// 目标字段
    pub destination: String,
    /// 转换函数
    pub transform: Option<String>,
}

impl MappingProcessor {
    /// 创建新的映射处理器
    pub fn new() -> Self {
        Self {
            config: None,
            mappings: Vec::new(),
            state: ProcessorState::new(),
        }
    }

    /// 应用映射到记录
    fn apply_mapping(&self, record: &DataRecord, mapping: &FieldMapping) -> Result<Option<(String, Value)>> {
        // 获取源字段值
        let source_value = if mapping.source.contains('.') {
            // 嵌套路径
            record.get_value(&mapping.source).cloned()
        } else {
            // 简单字段
            record.data.get(&mapping.source).cloned()
        };

        // 如果没有值，返回 None
        let source_value = match source_value {
            Some(value) => value,
            None => return Ok(None),
        };

        // 应用转换（如果有）
        let transformed_value = if let Some(transform) = &mapping.transform {
            match transform.as_str() {
                "lowercase" => {
                    if let Some(s) = source_value.as_str() {
                        Value::String(s.to_lowercase())
                    } else {
                        source_value
                    }
                },
                "uppercase" => {
                    if let Some(s) = source_value.as_str() {
                        Value::String(s.to_uppercase())
                    } else {
                        source_value
                    }
                },
                "trim" => {
                    if let Some(s) = source_value.as_str() {
                        Value::String(s.trim().to_string())
                    } else {
                        source_value
                    }
                },
                _ => source_value,
            }
        } else {
            source_value
        };

        Ok(Some((mapping.destination.clone(), transformed_value)))
    }
}

#[async_trait]
impl Processor for MappingProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = Some(config.clone());

        // 从配置中提取映射
        if let Some(mappings) = config.get("mappings").and_then(|m| m.as_array()) {
            self.mappings.clear();

            for mapping in mappings {
                if let (Some(source), Some(destination)) = (
                    mapping.get("source").and_then(|s| s.as_str()),
                    mapping.get("destination").and_then(|d| d.as_str()),
                ) {
                    let transform = mapping.get("transform").and_then(|t| t.as_str()).map(String::from);

                    self.mappings.push(FieldMapping {
                        source: source.to_string(),
                        destination: destination.to_string(),
                        transform,
                    });
                }
            }
        }

        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord, _state: Option<ProcessorState>) -> Result<Vec<DataRecord>> {
        // 创建具有相同元数据的新记录
        let mut new_record = DataRecord::new(serde_json::json!({}));
        new_record.metadata = record.metadata.clone();
        new_record.created_at = record.created_at;

        // 应用映射
        for mapping in &self.mappings {
            if let Some((dest_path, value)) = self.apply_mapping(record, mapping)? {
                // 设置新记录中的值
                new_record.set_value(&dest_path, value)?;
            }
        }

        Ok(vec![new_record])
    }

    async fn process_batch(&mut self, batch: &DataRecordBatch, state: Option<ProcessorState>) -> Result<DataRecordBatch> {
        let mut processed_records = Vec::with_capacity(batch.records.len());

        // 处理每条记录
        for record in &batch.records {
            let mut new_records = self.process_record(record, state.clone()).await?;
            processed_records.append(&mut new_records);
        }

        // 创建包含处理后记录的新批次
        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        Ok(new_batch)
    }

    fn get_state(&self) -> Result<ProcessorState> {
        Ok(self.state.clone())
    }
}

/// 过滤处理器
pub struct FilterProcessor {
    /// 处理器配置
    config: Option<Value>,
    /// 过滤条件
    condition: Option<String>,
    /// 处理器状态
    state: ProcessorState,
}

impl FilterProcessor {
    /// 创建新的过滤处理器
    pub fn new() -> Self {
        Self {
            config: None,
            condition: None,
            state: ProcessorState::new(),
        }
    }

    /// 评估记录上的简单条件
    fn evaluate_condition(&self, record: &DataRecord, condition: &str) -> Result<bool> {
        // 简单条件评估实现
        // 在实际实现中，应使用更完整的表达式引擎

        // 示例："user.email != null"
        let parts: Vec<&str> = condition.split_whitespace().collect();
        if parts.len() != 3 {
            return Ok(false);
        }

        let field_path = parts[0];
        let operator = parts[1];
        let value = parts[2];

        // 获取字段值
        let field_value = record.get_value(field_path);

        // 评估条件
        match operator {
            "==" => Ok(match field_value {
                Some(v) => match value {
                    "null" => v.is_null(),
                    _ => v.as_str().map(|s| s == value).unwrap_or(false),
                },
                None => value == "null",
            }),
            "!=" => Ok(match field_value {
                Some(v) => match value {
                    "null" => !v.is_null(),
                    _ => v.as_str().map(|s| s != value).unwrap_or(true),
                },
                None => value != "null",
            }),
            _ => Ok(false),
        }
    }
}

#[async_trait]
impl Processor for FilterProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = Some(config.clone());

        // 从配置中提取条件
        if let Some(condition) = config.get("condition").and_then(|c| c.as_str()) {
            self.condition = Some(condition.to_string());
        }

        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord, _state: Option<ProcessorState>) -> Result<Vec<DataRecord>> {
        // 如果没有条件，不做更改地传递记录
        if self.condition.is_none() {
            return Ok(vec![record.clone()]);
        }

        // 评估条件
        let condition = self.condition.as_ref().unwrap();
        let passes_filter = self.evaluate_condition(record, condition)?;

        // 如果通过过滤器，包含记录；否则，排除它
        if passes_filter {
            Ok(vec![record.clone()])
        } else {
            Ok(vec![])
        }
    }

    async fn process_batch(&mut self, batch: &DataRecordBatch, state: Option<ProcessorState>) -> Result<DataRecordBatch> {
        let mut processed_records = Vec::new();

        // 处理每条记录
        for record in &batch.records {
            let mut new_records = self.process_record(record, state.clone()).await?;
            processed_records.append(&mut new_records);
        }

        // 创建包含处理后记录的新批次
        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        Ok(new_batch)
    }

    fn get_state(&self) -> Result<ProcessorState> {
        Ok(self.state.clone())
    }
}

// 重新导出处理器
pub use aggregate::AggregateProcessor;
pub use enrichment::EnrichmentProcessor;

#[cfg(test)]
use mockall::mock;

#[cfg(test)]
mock! {
    pub Processor {}

    #[async_trait]
    impl Processor for Processor {
        fn configure(&mut self, config: &Value) -> Result<()>;
        async fn process_record(&mut self, record: &DataRecord, state: Option<ProcessorState>) -> Result<Vec<DataRecord>>;
        async fn process_batch(&mut self, batch: &DataRecordBatch, state: Option<ProcessorState>) -> Result<DataRecordBatch>;
        fn get_state(&self) -> Result<ProcessorState>;
        async fn initialize(&mut self) -> Result<()>;
        async fn finalize(&mut self) -> Result<()>;
    }
}
