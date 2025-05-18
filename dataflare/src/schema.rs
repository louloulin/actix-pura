//! 模式模块
//!
//! 定义用于描述和验证数据模式的功能。

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::{
    error::{DataFlareError, Result},
    model::{DataType, Field, Schema},
};

/// 模式验证器
pub struct SchemaValidator {
    /// 模式
    schema: Schema,
}

impl SchemaValidator {
    /// 创建新的模式验证器
    pub fn new(schema: Schema) -> Self {
        Self { schema }
    }
    
    /// 验证数据记录
    pub fn validate(&self, data: &serde_json::Value) -> Result<()> {
        // 验证数据是否为对象
        if !data.is_object() {
            return Err(DataFlareError::Validation("数据必须是对象".to_string()));
        }
        
        // 获取数据对象
        let data_obj = data.as_object().unwrap();
        
        // 验证每个字段
        for field in &self.schema.fields {
            // 检查必填字段
            if !field.nullable && !data_obj.contains_key(&field.name) {
                return Err(DataFlareError::Validation(
                    format!("缺少必填字段: {}", field.name)
                ));
            }
            
            // 如果字段存在，验证类型
            if let Some(value) = data_obj.get(&field.name) {
                if !value.is_null() {
                    self.validate_type(value, &field.data_type, &field.name)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// 验证数据类型
    fn validate_type(&self, value: &serde_json::Value, data_type: &DataType, field_name: &str) -> Result<()> {
        match data_type {
            DataType::Null => {
                if !value.is_null() {
                    return Err(DataFlareError::Validation(
                        format!("字段 {} 必须为 null", field_name)
                    ));
                }
            },
            DataType::Boolean => {
                if !value.is_boolean() {
                    return Err(DataFlareError::Validation(
                        format!("字段 {} 必须为布尔值", field_name)
                    ));
                }
            },
            DataType::Int32 | DataType::Int64 => {
                if !value.is_i64() {
                    return Err(DataFlareError::Validation(
                        format!("字段 {} 必须为整数", field_name)
                    ));
                }
            },
            DataType::Float32 | DataType::Float64 => {
                if !value.is_number() {
                    return Err(DataFlareError::Validation(
                        format!("字段 {} 必须为数字", field_name)
                    ));
                }
            },
            DataType::String => {
                if !value.is_string() {
                    return Err(DataFlareError::Validation(
                        format!("字段 {} 必须为字符串", field_name)
                    ));
                }
            },
            DataType::Array(item_type) => {
                if !value.is_array() {
                    return Err(DataFlareError::Validation(
                        format!("字段 {} 必须为数组", field_name)
                    ));
                }
                
                // 验证数组中的每个元素
                for (i, item) in value.as_array().unwrap().iter().enumerate() {
                    self.validate_type(item, item_type, &format!("{}[{}]", field_name, i))?;
                }
            },
            DataType::Object => {
                if !value.is_object() {
                    return Err(DataFlareError::Validation(
                        format!("字段 {} 必须为对象", field_name)
                    ));
                }
            },
            _ => {
                // 其他类型暂不验证
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Field;
    
    #[test]
    fn test_schema_validator() {
        // 创建模式
        let mut schema = Schema::new();
        schema.add_field(Field::new("id", DataType::Int64).nullable(false));
        schema.add_field(Field::new("name", DataType::String).nullable(false));
        schema.add_field(Field::new("email", DataType::String).nullable(true));
        schema.add_field(Field::new("tags", DataType::Array(Box::new(DataType::String))).nullable(true));
        
        // 创建验证器
        let validator = SchemaValidator::new(schema);
        
        // 有效数据
        let valid_data = serde_json::json!({
            "id": 1,
            "name": "Test User",
            "email": "test@example.com",
            "tags": ["tag1", "tag2"]
        });
        
        assert!(validator.validate(&valid_data).is_ok());
        
        // 缺少必填字段
        let missing_required = serde_json::json!({
            "id": 1
        });
        
        assert!(validator.validate(&missing_required).is_err());
        
        // 类型错误
        let wrong_type = serde_json::json!({
            "id": "1", // 应该是整数
            "name": "Test User"
        });
        
        assert!(validator.validate(&wrong_type).is_err());
        
        // 数组元素类型错误
        let wrong_array_type = serde_json::json!({
            "id": 1,
            "name": "Test User",
            "tags": ["tag1", 2] // 应该全是字符串
        });
        
        assert!(validator.validate(&wrong_array_type).is_err());
    }
}
