//! 批处理工具函数
//! 
//! 提供批处理操作的辅助函数，如大小估算和切片

use dataflare_core::{
    error::Result,
    message::DataRecordBatch,
};

/// 估算批次大小（字节）
/// 
/// 此函数估算DataRecordBatch在内存中的近似大小
pub fn estimate_batch_size(batch: &DataRecordBatch) -> usize {
    // 基础大小（批次结构本身）
    let mut size = std::mem::size_of::<DataRecordBatch>();
    
    // 添加记录数组大小
    size += std::mem::size_of::<Vec<_>>() + 
        batch.records.capacity() * std::mem::size_of::<*const ()>();
    
    // 每条记录的估算大小
    for record in &batch.records {
        // 基础记录大小
        size += std::mem::size_of_val(record);
        
        // 估算JSON数据大小 - 使用JSON字符串长度作为近似值
        let json_str = serde_json::to_string(&record.data).unwrap_or_default();
        size += json_str.len();
    }
    
    // 添加元数据大小
    let metadata_str = serde_json::to_string(&batch.metadata).unwrap_or_default();
    size += metadata_str.len();
    
    // 添加模式大小
    if let Some(schema) = &batch.schema {
        // 基础模式大小
        size += std::mem::size_of_val(schema);
        
        // 字段大小
        size += schema.fields.len() * std::mem::size_of::<dataflare_core::model::Field>();
    }
    
    size
}

/// 创建批次的切片
/// 
/// 此函数创建DataRecordBatch的子集，包含从start_index到end_index（不含）的记录
pub fn slice_batch(batch: &DataRecordBatch, start_index: usize, end_index: usize) -> DataRecordBatch {
    let start = start_index.min(batch.records.len());
    let end = end_index.min(batch.records.len());
    
    // 如果切片为空或无效，返回空批次
    if start >= end {
        return DataRecordBatch {
            records: Vec::new(),
            schema: batch.schema.clone(),
            metadata: batch.metadata.clone(),
        };
    }
    
    // 创建子集记录
    let records = batch.records[start..end].to_vec();
    
    // 创建新批次，保留原始模式和元数据
    DataRecordBatch {
        records,
        schema: batch.schema.clone(),
        metadata: batch.metadata.clone(),
    }
}

/// 合并多个批次
/// 
/// 此函数将多个批次合并为一个批次，保持第一个批次的元数据和模式
pub fn merge_batches(batches: &[DataRecordBatch]) -> DataRecordBatch {
    if batches.is_empty() {
        return DataRecordBatch {
            records: Vec::new(),
            schema: None,
            metadata: serde_json::Map::new(),
        };
    }
    
    if batches.len() == 1 {
        return batches[0].clone();
    }
    
    // 计算总记录数
    let total_records: usize = batches.iter()
        .map(|batch| batch.records.len())
        .sum();
    
    // 创建容量足够的记录向量
    let mut merged_records = Vec::with_capacity(total_records);
    
    // 合并所有记录
    for batch in batches {
        merged_records.extend_from_slice(&batch.records);
    }
    
    // 使用第一个批次的模式和元数据
    DataRecordBatch {
        records: merged_records,
        schema: batches[0].schema.clone(),
        metadata: batches[0].metadata.clone(),
    }
}

/// 分割批次为多个子批次
/// 
/// 此函数将一个大批次分割为多个子批次，每个子批次最多包含max_size条记录
pub fn split_batch(batch: &DataRecordBatch, max_size: usize) -> Vec<DataRecordBatch> {
    if batch.records.is_empty() || max_size == 0 {
        return vec![batch.clone()];
    }
    
    // 计算需要的批次数
    let batch_count = (batch.records.len() + max_size - 1) / max_size;
    let mut result = Vec::with_capacity(batch_count);
    
    // 分割记录
    for i in 0..batch_count {
        let start = i * max_size;
        let end = (start + max_size).min(batch.records.len());
        
        let sub_batch = slice_batch(batch, start, end);
        result.push(sub_batch);
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use dataflare_core::message::DataRecord;
    
    #[test]
    fn test_estimate_batch_size() {
        // 创建测试批次
        let records = vec![
            DataRecord::new(json!({"id": 1, "name": "test1"})),
            DataRecord::new(json!({"id": 2, "name": "test2"})),
            DataRecord::new(json!({"id": 3, "name": "test3"})),
        ];
        
        let batch = DataRecordBatch::new(records);
        
        // 估算大小
        let size = estimate_batch_size(&batch);
        
        // 验证大小合理
        assert!(size > 0, "估算大小应该为正数");
        assert!(size < 10 * 1024 * 1024, "估算大小不应该过大"); // 不应超过10MB
        
        // 验证与记录数的关系
        let empty_batch = DataRecordBatch::new(Vec::new());
        let empty_size = estimate_batch_size(&empty_batch);
        assert!(size > empty_size, "非空批次应该大于空批次");
    }
    
    #[test]
    fn test_slice_batch() {
        // 创建测试批次
        let mut records = Vec::new();
        for i in 0..10 {
            records.push(DataRecord::new(json!({"id": i})));
        }
        
        let batch = DataRecordBatch::new(records);
        
        // 测试正常切片
        let slice = slice_batch(&batch, 2, 5);
        assert_eq!(slice.records.len(), 3);
        assert_eq!(slice.records[0].get::<i64>("id").unwrap(), 2);
        assert_eq!(slice.records[2].get::<i64>("id").unwrap(), 4);
        
        // 测试边界情况
        let empty_slice = slice_batch(&batch, 5, 5);
        assert_eq!(empty_slice.records.len(), 0);
        
        let out_of_bounds = slice_batch(&batch, 8, 15);
        assert_eq!(out_of_bounds.records.len(), 2);
    }
    
    #[test]
    fn test_merge_batches() {
        // 创建测试批次
        let batch1 = DataRecordBatch::new(vec![
            DataRecord::new(json!({"id": 1})),
            DataRecord::new(json!({"id": 2})),
        ]);
        
        let batch2 = DataRecordBatch::new(vec![
            DataRecord::new(json!({"id": 3})),
            DataRecord::new(json!({"id": 4})),
            DataRecord::new(json!({"id": 5})),
        ]);
        
        // 合并批次
        let merged = merge_batches(&[batch1, batch2]);
        
        // 验证合并结果
        assert_eq!(merged.records.len(), 5);
        assert_eq!(merged.records[0].get::<i64>("id").unwrap(), 1);
        assert_eq!(merged.records[4].get::<i64>("id").unwrap(), 5);
    }
    
    #[test]
    fn test_split_batch() {
        // 创建测试批次
        let mut records = Vec::new();
        for i in 0..10 {
            records.push(DataRecord::new(json!({"id": i})));
        }
        
        let batch = DataRecordBatch::new(records);
        
        // 分割批次
        let split = split_batch(&batch, 3);
        
        // 验证分割结果
        assert_eq!(split.len(), 4); // 应该有4个子批次
        assert_eq!(split[0].records.len(), 3);
        assert_eq!(split[1].records.len(), 3);
        assert_eq!(split[2].records.len(), 3);
        assert_eq!(split[3].records.len(), 1);
        
        // 验证内容
        assert_eq!(split[0].records[0].get::<i64>("id").unwrap(), 0);
        assert_eq!(split[1].records[0].get::<i64>("id").unwrap(), 3);
        assert_eq!(split[3].records[0].get::<i64>("id").unwrap(), 9);
    }
} 