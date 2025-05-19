//! 聚合处理器
//!
//! 提供数据聚合功能，支持分组和聚合操作。

use std::collections::{HashMap, HashSet};
use async_trait::async_trait;
use serde_json::{Value, json, Map};
use log::{debug, error, info, warn};

use crate::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    processor::{Processor, ProcessorState},
};

/// 聚合函数类型
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateFunction {
    /// 计数
    Count,
    /// 求和
    Sum,
    /// 平均值
    Average,
    /// 最小值
    Min,
    /// 最大值
    Max,
    /// 第一个值
    First,
    /// 最后一个值
    Last,
    /// 列表（收集所有值）
    List,
    /// 自定义函数
    Custom(String),
}

impl AggregateFunction {
    /// 从字符串解析聚合函数
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "count" => Ok(AggregateFunction::Count),
            "sum" => Ok(AggregateFunction::Sum),
            "avg" | "average" => Ok(AggregateFunction::Average),
            "min" => Ok(AggregateFunction::Min),
            "max" => Ok(AggregateFunction::Max),
            "first" => Ok(AggregateFunction::First),
            "last" => Ok(AggregateFunction::Last),
            "list" | "collect" => Ok(AggregateFunction::List),
            s if s.starts_with("custom:") => {
                let custom_name = s.trim_start_matches("custom:").to_string();
                Ok(AggregateFunction::Custom(custom_name))
            },
            _ => Err(DataFlareError::Config(format!("Función de agregación no válida: {}", s))),
        }
    }
}

/// 聚合配置
#[derive(Debug, Clone)]
pub struct AggregateConfig {
    /// 分组字段
    pub group_by: Vec<String>,
    
    /// 聚合操作
    pub aggregations: Vec<AggregationOperation>,
    
    /// 是否保留原始字段
    pub keep_original_fields: bool,
    
    /// 窗口大小（可选）
    pub window_size: Option<usize>,
}

/// 聚合操作
#[derive(Debug, Clone)]
pub struct AggregationOperation {
    /// 源字段
    pub source_field: String,
    
    /// 目标字段
    pub destination_field: String,
    
    /// 聚合函数
    pub function: AggregateFunction,
}

/// 聚合处理器
pub struct AggregateProcessor {
    /// 配置
    config: AggregateConfig,
    
    /// 处理器状态
    state: ProcessorState,
    
    /// 聚合缓冲区
    buffer: HashMap<String, Vec<DataRecord>>,
    
    /// 已处理记录计数
    processed_count: usize,
}

impl AggregateProcessor {
    /// 创建新的聚合处理器
    pub fn new(config: AggregateConfig) -> Self {
        Self {
            config,
            state: ProcessorState::new(),
            buffer: HashMap::new(),
            processed_count: 0,
        }
    }
    
    /// 从配置创建聚合处理器
    pub fn from_config(config: &Value) -> Result<Self> {
        // 解析分组字段
        let group_by = config.get("group_by")
            .and_then(|g| g.as_array())
            .ok_or_else(|| DataFlareError::Config("Se requiere el campo 'group_by'".to_string()))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>();
        
        // 解析聚合操作
        let aggregations = config.get("aggregations")
            .and_then(|a| a.as_array())
            .ok_or_else(|| DataFlareError::Config("Se requiere el campo 'aggregations'".to_string()))?
            .iter()
            .map(|agg| {
                let source = agg.get("source")
                    .and_then(|s| s.as_str())
                    .ok_or_else(|| DataFlareError::Config("Cada agregación requiere un campo 'source'".to_string()))?;
                
                let destination = agg.get("destination")
                    .and_then(|d| d.as_str())
                    .ok_or_else(|| DataFlareError::Config("Cada agregación requiere un campo 'destination'".to_string()))?;
                
                let function_str = agg.get("function")
                    .and_then(|f| f.as_str())
                    .ok_or_else(|| DataFlareError::Config("Cada agregación requiere un campo 'function'".to_string()))?;
                
                let function = AggregateFunction::from_str(function_str)?;
                
                Ok(AggregationOperation {
                    source_field: source.to_string(),
                    destination_field: destination.to_string(),
                    function,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        
        // 解析其他选项
        let keep_original_fields = config.get("keep_original_fields")
            .and_then(|k| k.as_bool())
            .unwrap_or(false);
        
        let window_size = config.get("window_size")
            .and_then(|w| w.as_u64())
            .map(|w| w as usize);
        
        Ok(Self::new(AggregateConfig {
            group_by,
            aggregations,
            keep_original_fields,
            window_size,
        }))
    }
    
    /// 获取记录的分组键
    fn get_group_key(&self, record: &DataRecord) -> Result<String> {
        if self.config.group_by.is_empty() {
            return Ok("_all_".to_string());
        }
        
        let mut key_parts = Vec::with_capacity(self.config.group_by.len());
        
        for field in &self.config.group_by {
            let value = self.get_field_value(record, field)?;
            key_parts.push(value.to_string());
        }
        
        Ok(key_parts.join("::"))
    }
    
    /// 获取字段值
    fn get_field_value(&self, record: &DataRecord, field: &str) -> Result<Value> {
        let parts = field.split('.').collect::<Vec<_>>();
        let mut current = &record.data;
        
        for part in parts {
            current = match current {
                Value::Object(obj) => obj.get(part).ok_or_else(|| {
                    DataFlareError::Field(format!("Campo no encontrado: {}", part))
                })?,
                _ => return Err(DataFlareError::Field(format!("No se puede acceder al campo {} en un valor no objeto", part))),
            };
        }
        
        Ok(current.clone())
    }
    
    /// 执行聚合操作
    fn aggregate_records(&self, records: &[DataRecord]) -> Result<DataRecord> {
        if records.is_empty() {
            return Err(DataFlareError::Processing("No hay registros para agregar".to_string()));
        }
        
        let mut result = Map::new();
        
        // 如果保留原始字段，使用第一条记录的字段
        if self.config.keep_original_fields {
            if let Value::Object(obj) = &records[0].data {
                for (k, v) in obj {
                    result.insert(k.clone(), v.clone());
                }
            }
        }
        
        // 执行每个聚合操作
        for agg in &self.config.aggregations {
            let agg_value = match agg.function {
                AggregateFunction::Count => {
                    json!(records.len())
                },
                AggregateFunction::Sum => {
                    let mut sum = 0.0;
                    for record in records {
                        if let Ok(value) = self.get_field_value(record, &agg.source_field) {
                            if let Some(num) = value.as_f64() {
                                sum += num;
                            } else if let Some(num) = value.as_i64() {
                                sum += num as f64;
                            }
                        }
                    }
                    json!(sum)
                },
                AggregateFunction::Average => {
                    let mut sum = 0.0;
                    let mut count = 0;
                    for record in records {
                        if let Ok(value) = self.get_field_value(record, &agg.source_field) {
                            if let Some(num) = value.as_f64() {
                                sum += num;
                                count += 1;
                            } else if let Some(num) = value.as_i64() {
                                sum += num as f64;
                                count += 1;
                            }
                        }
                    }
                    if count > 0 {
                        json!(sum / count as f64)
                    } else {
                        Value::Null
                    }
                },
                AggregateFunction::Min => {
                    let mut min_value = None;
                    for record in records {
                        if let Ok(value) = self.get_field_value(record, &agg.source_field) {
                            if let Some(num) = value.as_f64() {
                                min_value = Some(min_value.map_or(num, |m: f64| m.min(num)));
                            } else if let Some(num) = value.as_i64() {
                                let num_f64 = num as f64;
                                min_value = Some(min_value.map_or(num_f64, |m: f64| m.min(num_f64)));
                            }
                        }
                    }
                    min_value.map_or(Value::Null, |m| json!(m))
                },
                AggregateFunction::Max => {
                    let mut max_value = None;
                    for record in records {
                        if let Ok(value) = self.get_field_value(record, &agg.source_field) {
                            if let Some(num) = value.as_f64() {
                                max_value = Some(max_value.map_or(num, |m: f64| m.max(num)));
                            } else if let Some(num) = value.as_i64() {
                                let num_f64 = num as f64;
                                max_value = Some(max_value.map_or(num_f64, |m: f64| m.max(num_f64)));
                            }
                        }
                    }
                    max_value.map_or(Value::Null, |m| json!(m))
                },
                AggregateFunction::First => {
                    if let Ok(value) = self.get_field_value(&records[0], &agg.source_field) {
                        value
                    } else {
                        Value::Null
                    }
                },
                AggregateFunction::Last => {
                    if let Ok(value) = self.get_field_value(records.last().unwrap(), &agg.source_field) {
                        value
                    } else {
                        Value::Null
                    }
                },
                AggregateFunction::List => {
                    let values = records.iter()
                        .filter_map(|record| {
                            self.get_field_value(record, &agg.source_field).ok()
                        })
                        .collect::<Vec<_>>();
                    json!(values)
                },
                AggregateFunction::Custom(ref _name) => {
                    // 自定义函数暂未实现
                    Value::Null
                },
            };
            
            // 设置目标字段
            let parts = agg.destination_field.split('.').collect::<Vec<_>>();
            if parts.len() == 1 {
                result.insert(agg.destination_field.clone(), agg_value);
            } else {
                // 处理嵌套字段
                let mut current = &mut result;
                for (i, part) in parts.iter().enumerate() {
                    if i == parts.len() - 1 {
                        current.insert(part.to_string(), agg_value);
                        break;
                    }
                    
                    if !current.contains_key(*part) {
                        current.insert(part.to_string(), json!({}));
                    }
                    
                    if let Some(Value::Object(ref mut obj)) = current.get_mut(*part) {
                        current = obj;
                    } else {
                        return Err(DataFlareError::Field(format!("No se puede establecer el campo anidado: {}", agg.destination_field)));
                    }
                }
            }
        }
        
        // 添加分组字段
        for (i, field) in self.config.group_by.iter().enumerate() {
            if let Ok(value) = self.get_field_value(&records[0], field) {
                result.insert(format!("group_{}", i), value);
            }
        }
        
        Ok(DataRecord::new(Value::Object(result)))
    }
}

#[async_trait]
impl Processor for AggregateProcessor {
    fn configure(&mut self, config: &Value) -> Result<()> {
        let new_processor = Self::from_config(config)?;
        self.config = new_processor.config;
        Ok(())
    }
    
    async fn process_record(&mut self, record: &DataRecord, _state: Option<ProcessorState>) -> Result<Vec<DataRecord>> {
        // 获取记录的分组键
        let group_key = self.get_group_key(record)?;
        
        // 将记录添加到缓冲区
        self.buffer.entry(group_key).or_default().push(record.clone());
        self.processed_count += 1;
        
        // 如果有窗口大小限制，检查是否需要输出
        if let Some(window_size) = self.config.window_size {
            if self.processed_count >= window_size {
                return self.flush_buffer().await;
            }
        }
        
        // 默认不输出任何记录，等待批处理
        Ok(vec![])
    }
    
    async fn process_batch(&mut self, batch: &DataRecordBatch, state: Option<ProcessorState>) -> Result<DataRecordBatch> {
        // 处理每条记录
        for record in &batch.records {
            self.process_record(record, state.clone()).await?;
        }
        
        // 刷新缓冲区并创建新批次
        let records = self.flush_buffer().await?;
        let mut new_batch = DataRecordBatch::new(records);
        new_batch.schema = batch.schema.clone();
        new_batch.metadata = batch.metadata.clone();
        
        Ok(new_batch)
    }
    
    fn get_state(&self) -> Result<ProcessorState> {
        Ok(self.state.clone())
    }
    
    async fn finalize(&mut self) -> Result<()> {
        // 确保所有缓冲的记录都被处理
        self.flush_buffer().await?;
        Ok(())
    }
}

impl AggregateProcessor {
    /// 刷新缓冲区，执行聚合并返回结果
    async fn flush_buffer(&mut self) -> Result<Vec<DataRecord>> {
        if self.buffer.is_empty() {
            return Ok(vec![]);
        }
        
        let mut results = Vec::with_capacity(self.buffer.len());
        
        // 对每个分组执行聚合
        for (_key, records) in std::mem::take(&mut self.buffer) {
            if !records.is_empty() {
                let aggregated = self.aggregate_records(&records)?;
                results.push(aggregated);
            }
        }
        
        // 重置计数器
        self.processed_count = 0;
        
        Ok(results)
    }
}
