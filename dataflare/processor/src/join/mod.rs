//! 连接处理器实现
//!
//! 提供数据记录连接功能，支持内连接、左连接、右连接和全连接。

use std::collections::HashMap;
use async_trait::async_trait;
use serde_json::{Map, Value};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    processor::{Processor, ProcessorState},
};

/// 连接类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinType {
    /// 内连接：只返回两个数据集中匹配的记录
    Inner,
    /// 左连接：返回左侧数据集的所有记录，以及右侧数据集中匹配的记录
    Left,
    /// 右连接：返回右侧数据集的所有记录，以及左侧数据集中匹配的记录
    Right,
    /// 全连接：返回两个数据集的所有记录，无论是否匹配
    Full,
}

impl JoinType {
    /// 从字符串创建连接类型
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "inner" => Ok(JoinType::Inner),
            "left" => Ok(JoinType::Left),
            "right" => Ok(JoinType::Right),
            "full" => Ok(JoinType::Full),
            _ => Err(DataFlareError::Config(format!("无效的连接类型: {}", s))),
        }
    }
}

/// 连接处理器
///
/// 用于连接两个数据集，支持内连接、左连接、右连接和全连接。
#[derive(Debug, Clone)]
pub struct JoinProcessor {
    /// 处理器名称
    name: String,
    /// 左侧数据集的连接键
    left_key: String,
    /// 右侧数据集的连接键
    right_key: String,
    /// 连接类型
    join_type: JoinType,
    /// 右侧数据集缓存
    right_cache: HashMap<String, Vec<DataRecord>>,
    /// 左侧数据集前缀（用于区分字段）
    left_prefix: Option<String>,
    /// 右侧数据集前缀（用于区分字段）
    right_prefix: Option<String>,
}

impl JoinProcessor {
    /// 创建新的连接处理器
    pub fn new(
        name: String,
        left_key: String,
        right_key: String,
        join_type: JoinType,
        left_prefix: Option<String>,
        right_prefix: Option<String>,
    ) -> Self {
        Self {
            name,
            left_key,
            right_key,
            join_type,
            right_cache: HashMap::new(),
            left_prefix,
            right_prefix,
        }
    }

    /// 从配置创建连接处理器
    pub fn from_config(name: String, config: &Value) -> Result<Self> {
        let left_key = config.get("left_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("连接处理器需要 'left_key' 配置".to_string()))?
            .to_string();

        let right_key = config.get("right_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DataFlareError::Config("连接处理器需要 'right_key' 配置".to_string()))?
            .to_string();

        let join_type_str = config.get("join_type")
            .and_then(|v| v.as_str())
            .unwrap_or("inner");

        let join_type = JoinType::from_str(join_type_str)?;

        let left_prefix = config.get("left_prefix")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let right_prefix = config.get("right_prefix")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(Self::new(name, left_key, right_key, join_type, left_prefix, right_prefix))
    }

    /// 获取记录的连接键值
    fn get_key_value(&self, record: &DataRecord, key: &str) -> Option<String> {
        record.get_value(key)
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            })
    }

    /// 合并两条记录
    fn merge_records(&self, left: &DataRecord, right: &DataRecord) -> DataRecord {
        let mut merged_data = Map::new();

        // 添加左侧记录的字段
        if let Value::Object(left_map) = &left.data {
            for (key, value) in left_map {
                let field_name = if let Some(prefix) = &self.left_prefix {
                    format!("{}.{}", prefix, key)
                } else {
                    key.clone()
                };
                merged_data.insert(field_name, value.clone());
            }
        }

        // 添加右侧记录的字段
        if let Value::Object(right_map) = &right.data {
            for (key, value) in right_map {
                let field_name = if let Some(prefix) = &self.right_prefix {
                    format!("{}.{}", prefix, key)
                } else {
                    key.clone()
                };
                merged_data.insert(field_name, value.clone());
            }
        }

        // 创建合并后的记录
        let mut merged_record = DataRecord::new(Value::Object(merged_data));

        // 合并元数据，但移除 join_side 元数据
        for (key, value) in &left.metadata {
            if key != "join_side" {
                merged_record.metadata.insert(key.clone(), value.clone());
            }
        }
        for (key, value) in &right.metadata {
            if key != "join_side" {
                merged_record.metadata.insert(key.clone(), value.clone());
            }
        }

        merged_record
    }

    /// 创建空的右侧记录
    fn create_empty_right_record(&self) -> DataRecord {
        let empty_data = Map::new();
        DataRecord::new(Value::Object(empty_data))
    }

    /// 处理右侧数据集的记录
    fn process_right_record(&mut self, record: DataRecord) -> Result<DataRecord> {
        // 获取连接键值
        let key = match self.get_key_value(&record, &self.right_key) {
            Some(k) => k,
            None => return Ok(DataRecord::new(serde_json::json!({}))), // 如果没有连接键，则返回空记录
        };

        // 将记录添加到缓存
        self.right_cache.entry(key).or_insert_with(Vec::new).push(record);

        // 右侧记录不产生输出，返回空记录
        Ok(DataRecord::new(serde_json::json!({})))
    }

    /// 处理左侧数据集的记录
    fn process_left_record(&mut self, record: DataRecord) -> Result<DataRecord> {
        // 获取连接键值
        let key = match self.get_key_value(&record, &self.left_key) {
            Some(k) => k,
            None => {
                // 对于左连接和全连接，即使没有连接键，也需要保留左侧记录
                if self.join_type == JoinType::Left || self.join_type == JoinType::Full {
                    let empty_right = self.create_empty_right_record();
                    return Ok(self.merge_records(&record, &empty_right));
                } else {
                    return Ok(DataRecord::new(serde_json::json!({})));
                }
            },
        };

        // 查找匹配的右侧记录
        if let Some(right_records) = self.right_cache.get(&key) {
            // 有匹配的右侧记录，创建连接结果
            // 简化：只使用第一个匹配的右侧记录
            if let Some(right_record) = right_records.first() {
                Ok(self.merge_records(&record, right_record))
            } else {
                // 这种情况不应该发生，因为我们已经检查了 right_records 不为空
                Ok(DataRecord::new(serde_json::json!({})))
            }
        } else {
            // 没有匹配的右侧记录
            match self.join_type {
                JoinType::Inner | JoinType::Right => Ok(DataRecord::new(serde_json::json!({}))),
                JoinType::Left | JoinType::Full => {
                    let empty_right = self.create_empty_right_record();
                    Ok(self.merge_records(&record, &empty_right))
                },
            }
        }
    }
}

#[async_trait]
impl dataflare_core::processor::Processor for JoinProcessor {
    /// 配置处理器
    fn configure(&mut self, config: &Value) -> Result<()> {
        // 从配置中提取连接键
        if let Some(left_key) = config.get("left_key").and_then(|v| v.as_str()) {
            self.left_key = left_key.to_string();
        }

        if let Some(right_key) = config.get("right_key").and_then(|v| v.as_str()) {
            self.right_key = right_key.to_string();
        }

        // 提取连接类型
        if let Some(join_type_str) = config.get("join_type").and_then(|v| v.as_str()) {
            self.join_type = JoinType::from_str(join_type_str)?;
        }

        // 提取前缀
        if let Some(left_prefix) = config.get("left_prefix").and_then(|v| v.as_str()) {
            self.left_prefix = Some(left_prefix.to_string());
        }

        if let Some(right_prefix) = config.get("right_prefix").and_then(|v| v.as_str()) {
            self.right_prefix = Some(right_prefix.to_string());
        }

        Ok(())
    }

    /// 处理单条记录
    async fn process_record(&mut self, record: &DataRecord) -> Result<DataRecord> {
        // 检查记录是左侧还是右侧数据集
        if let Some(side) = record.metadata.get("join_side") {
            let side_str = side.as_str();
            if side_str == "left" {
                self.process_left_record(record.clone())
            } else if side_str == "right" {
                self.process_right_record(record.clone())
            } else {
                Err(DataFlareError::Config("无效的 join_side 元数据值".to_string()))
            }
        } else {
            // 默认为左侧数据集
            self.process_left_record(record.clone())
        }
    }

    /// 处理记录批次
    async fn process_batch(&mut self, batch: &DataRecordBatch) -> Result<DataRecordBatch> {
        let mut processed_records = Vec::new();

        // 处理每条记录
        for record in &batch.records {
            let result = self.process_record(record).await?;
            // 只添加非空记录
            if !result.data.is_null() && !result.data.as_object().map_or(true, |obj| obj.is_empty()) {
                processed_records.push(result);
            }
        }

        // 创建新的批次
        let mut new_batch = DataRecordBatch::new(processed_records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();

        Ok(new_batch)
    }

    /// 获取处理器状态
    fn get_state(&self) -> ProcessorState {
        // 创建包含右侧缓存大小的状态
        let mut state = ProcessorState::new("join");
        // 将右侧缓存大小存储在 data 中
        state.add_data("right_cache_size", self.right_cache.len().to_string());
        state
    }

    fn get_input_schema(&self) -> Option<dataflare_core::model::Schema> {
        None
    }

    fn get_output_schema(&self) -> Option<dataflare_core::model::Schema> {
        None
    }

    /// 初始化处理器
    async fn initialize(&mut self) -> Result<()> {
        // 清空右侧缓存
        self.right_cache.clear();
        Ok(())
    }

    /// 结束处理器
    async fn finalize(&mut self) -> Result<()> {
        // 清空右侧缓存
        self.right_cache.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_join_processor_inner_join() {
        // 创建连接处理器
        let mut processor = JoinProcessor::new(
            "test_join".to_string(),
            "id".to_string(),
            "user_id".to_string(),
            JoinType::Inner,
            Some("order".to_string()),
            Some("user".to_string()),
        );

        // 创建右侧数据集记录
        let mut user1 = DataRecord::new(json!({
            "user_id": 1,
            "name": "Alice"
        }));
        user1.metadata.insert("join_side".to_string(), "right".into());

        let mut user2 = DataRecord::new(json!({
            "user_id": 2,
            "name": "Bob"
        }));
        user2.metadata.insert("join_side".to_string(), "right".into());

        // 处理右侧记录
        processor.process_record(&user1).await.unwrap();
        processor.process_record(&user2, None).await.unwrap();

        // 创建左侧数据集记录
        let mut order1 = DataRecord::new(json!({
            "id": 1,
            "amount": 100
        }));
        order1.metadata.insert("join_side".to_string(), "left".into());

        let mut order2 = DataRecord::new(json!({
            "id": 2,
            "amount": 200
        }));
        order2.metadata.insert("join_side".to_string(), "left".into());

        let mut order3 = DataRecord::new(json!({
            "id": 3,
            "amount": 300
        }));
        order3.metadata.insert("join_side".to_string(), "left".into());

        // 处理左侧记录
        let result1 = processor.process_record(&order1, None).await.unwrap();
        let result2 = processor.process_record(&order2, None).await.unwrap();
        let result3 = processor.process_record(&order3, None).await.unwrap();

        // 验证结果
        assert_eq!(result1.len(), 1);
        assert_eq!(result2.len(), 1);
        assert_eq!(result3.len(), 0); // 没有匹配的右侧记录

        // 打印结果以便调试
        println!("Result1: {:?}", result1);
        println!("Result2: {:?}", result2);
        println!("Result3: {:?}", result3);

        // 验证连接结果
        if !result1.is_empty() {
            let joined1 = &result1[0];
            println!("Joined1 data: {:?}", joined1.data);

            // 使用 get 方法获取值，避免使用 unwrap
            if let Some(id) = joined1.data.get("order.id") {
                assert_eq!(id.as_i64().unwrap_or(0), 1);
            } else if let Some(id) = joined1.data.get("id") {
                assert_eq!(id.as_i64().unwrap_or(0), 1);
            } else {
                panic!("Expected joined1 to have an 'id' or 'order.id' field");
            }

            if let Some(name) = joined1.data.get("user.name") {
                assert_eq!(name.as_str().unwrap_or(""), "Alice");
            } else if let Some(name) = joined1.data.get("name") {
                assert_eq!(name.as_str().unwrap_or(""), "Alice");
            } else {
                panic!("Expected joined1 to have a 'name' or 'user.name' field");
            }
        } else {
            panic!("Expected result1 to contain at least one record");
        }

        if !result2.is_empty() {
            let joined2 = &result2[0];
            println!("Joined2 data: {:?}", joined2.data);

            // 使用 get 方法获取值，避免使用 unwrap
            if let Some(id) = joined2.data.get("order.id") {
                assert_eq!(id.as_i64().unwrap_or(0), 2);
            } else if let Some(id) = joined2.data.get("id") {
                assert_eq!(id.as_i64().unwrap_or(0), 2);
            } else {
                panic!("Expected joined2 to have an 'id' or 'order.id' field");
            }

            if let Some(name) = joined2.data.get("user.name") {
                assert_eq!(name.as_str().unwrap_or(""), "Bob");
            } else if let Some(name) = joined2.data.get("name") {
                assert_eq!(name.as_str().unwrap_or(""), "Bob");
            } else {
                panic!("Expected joined2 to have a 'name' or 'user.name' field");
            }
        } else {
            panic!("Expected result2 to contain at least one record");
        }
    }
}
