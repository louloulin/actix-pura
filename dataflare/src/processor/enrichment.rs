//! 丰富处理器
//!
//! 提供数据丰富功能，支持查找和数据扩充操作。

use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::{Value, json, Map};

use crate::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    processor::{Processor, ProcessorState},
};

/// 丰富处理器
pub struct EnrichmentProcessor {
    /// 配置
    config: Option<Value>,
    /// 处理器状态
    state: ProcessorState,
    /// 查找表
    lookup_table: HashMap<String, Value>,
    /// 查找键
    lookup_key: String,
    /// 目标字段
    target_field: String,
    /// 源字段
    source_field: String,
    /// 是否保留原始字段
    keep_original_fields: bool,
    /// 默认值
    default_value: Option<Value>,
}

impl EnrichmentProcessor {
    /// 创建新的丰富处理器
    pub fn new() -> Self {
        Self {
            config: None,
            state: ProcessorState::new(),
            lookup_table: HashMap::new(),
            lookup_key: String::new(),
            target_field: String::new(),
            source_field: String::new(),
            keep_original_fields: true,
            default_value: None,
        }
    }

    /// 从配置创建丰富处理器
    pub fn from_config(config: &Value) -> Result<Self> {
        let mut processor = Self::new();
        processor.configure(config)?;
        Ok(processor)
    }

    /// 加载查找表
    fn load_lookup_table(&mut self, data: &Value) -> Result<()> {
        // 清空现有查找表
        self.lookup_table.clear();

        // 获取查找表数据
        let lookup_data = data.as_array().ok_or_else(|| {
            DataFlareError::Config("查找表数据必须是数组".to_string())
        })?;

        // 获取查找键
        let lookup_key = self.lookup_key.clone();
        if lookup_key.is_empty() {
            return Err(DataFlareError::Config("未指定查找键".to_string()));
        }

        // 构建查找表
        for item in lookup_data {
            if let Some(key_value) = Self::get_nested_value(item, &lookup_key) {
                let key = key_value.to_string();
                self.lookup_table.insert(key, item.clone());
            }
        }

        Ok(())
    }

    /// 获取嵌套值
    fn get_nested_value<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            match current {
                Value::Object(obj) => {
                    if let Some(next) = obj.get(part) {
                        current = next;
                    } else {
                        return None;
                    }
                },
                _ => return None,
            }
        }

        Some(current)
    }

    /// 设置嵌套值
    fn set_nested_value(obj: &mut Map<String, Value>, path: &str, value: Value) -> Result<()> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = obj;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // 最后一部分，设置值
                current.insert(part.to_string(), value);
                return Ok(());
            }

            // 确保路径中的对象存在
            if !current.contains_key(*part) {
                current.insert(part.to_string(), json!({}));
            }

            // 获取下一级对象
            match current.get_mut(*part) {
                Some(Value::Object(ref mut next_obj)) => {
                    current = next_obj;
                },
                _ => {
                    // 替换非对象值为对象
                    let new_obj = json!({});
                    current.insert(part.to_string(), new_obj);

                    if let Some(Value::Object(ref mut next_obj)) = current.get_mut(*part) {
                        current = next_obj;
                    } else {
                        return Err(DataFlareError::Config(format!("无法创建嵌套路径: {}", path)));
                    }
                }
            }
        }

        Ok(())
    }

    /// 丰富记录
    fn enrich_record(&self, record: &DataRecord) -> Result<DataRecord> {
        // 创建新记录
        let mut new_record = if self.keep_original_fields {
            record.clone()
        } else {
            DataRecord::new(json!({}))
        };

        // 获取源字段值
        let source_value = Self::get_nested_value(&record.data, &self.source_field);

        if let Some(source_value) = source_value {
            let lookup_key = source_value.to_string();

            // 查找匹配值
            if let Some(lookup_data) = self.lookup_table.get(&lookup_key) {
                // 如果目标字段包含点，表示嵌套路径
                if self.target_field.contains('.') {
                    if let Value::Object(ref mut obj) = new_record.data {
                        Self::set_nested_value(obj, &self.target_field, lookup_data.clone())?;
                    }
                } else {
                    // 简单字段
                    if let Value::Object(ref mut obj) = new_record.data {
                        obj.insert(self.target_field.clone(), lookup_data.clone());
                    }
                }
            } else if let Some(ref default_value) = self.default_value {
                // 使用默认值
                if self.target_field.contains('.') {
                    if let Value::Object(ref mut obj) = new_record.data {
                        Self::set_nested_value(obj, &self.target_field, default_value.clone())?;
                    }
                } else {
                    if let Value::Object(ref mut obj) = new_record.data {
                        obj.insert(self.target_field.clone(), default_value.clone());
                    }
                }
            }
        }

        Ok(new_record)
    }
}

#[async_trait]
impl Processor for EnrichmentProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = Some(config.clone());

        // 获取查找键
        self.lookup_key = config.get("lookup_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("未指定查找键".to_string()))?
            .to_string();

        // 获取源字段
        self.source_field = config.get("source_field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("未指定源字段".to_string()))?
            .to_string();

        // 获取目标字段
        self.target_field = config.get("target_field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("未指定目标字段".to_string()))?
            .to_string();

        // 获取是否保留原始字段
        if let Some(keep) = config.get("keep_original_fields").and_then(|v| v.as_bool()) {
            self.keep_original_fields = keep;
        }

        // 获取默认值
        self.default_value = config.get("default_value").cloned();

        // 加载查找表
        if let Some(lookup_data) = config.get("lookup_data") {
            self.load_lookup_table(lookup_data)?;
        }

        Ok(())
    }

    async fn process_record(&mut self, record: &DataRecord, _state: Option<ProcessorState>) -> Result<Vec<DataRecord>> {
        let enriched_record = self.enrich_record(record)?;
        Ok(vec![enriched_record])
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

impl Default for EnrichmentProcessor {
    fn default() -> Self {
        Self::new()
    }
}
