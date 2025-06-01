//! JSON Transformer WASM Plugin for DataFlare
//!
//! This plugin provides advanced JSON transformation capabilities using JSONPath

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, Map};
use std::collections::HashMap;
use log::{info, error, debug};

// 当 `console_error_panic_hook` 特性被启用时，我们可以调用
// `set_panic_hook` 函数至少一次在初始化期间，然后我们将在
// 我们的 wasm 中获得更好的错误消息。
#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_panic_hook;

// 当 `wee_alloc` 特性被启用时，使用 `wee_alloc` 作为全局分配器。
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// 插件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// JSONPath 转换规则
    pub transformations: Vec<TransformRule>,
    /// 是否保留原始字段
    pub preserve_original: bool,
    /// 错误处理策略
    pub error_strategy: ErrorStrategy,
}

/// 转换规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformRule {
    /// 源JSONPath
    pub source_path: String,
    /// 目标字段名
    pub target_field: String,
    /// 转换类型
    pub transform_type: TransformType,
    /// 可选的转换参数
    pub parameters: Option<Map<String, Value>>,
}

/// 转换类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransformType {
    /// 直接复制
    Copy,
    /// 字符串转换
    ToString,
    /// 数字转换
    ToNumber,
    /// 布尔转换
    ToBoolean,
    /// 数组转换
    ToArray,
    /// 对象转换
    ToObject,
    /// 自定义函数
    Custom(String),
}

/// 错误处理策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorStrategy {
    /// 忽略错误
    Ignore,
    /// 使用默认值
    UseDefault(Value),
    /// 抛出错误
    Throw,
}

/// 插件状态
static mut PLUGIN_CONFIG: Option<PluginConfig> = None;

/// 初始化插件
#[wasm_bindgen]
pub fn initialize(config_json: &str) -> Result<(), JsValue> {
    // 设置panic hook以获得更好的错误消息
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_panic_hook();

    // 初始化日志
    wasm_logger::init(wasm_logger::Config::default());

    info!("初始化JSON转换插件");

    // 解析配置
    let config: PluginConfig = serde_json::from_str(config_json)
        .map_err(|e| JsValue::from_str(&format!("配置解析失败: {}", e)))?;

    debug!("插件配置: {:?}", config);

    // 存储配置
    unsafe {
        PLUGIN_CONFIG = Some(config);
    }

    info!("JSON转换插件初始化成功");
    Ok(())
}

/// 获取插件元数据
#[wasm_bindgen]
pub fn get_metadata() -> String {
    let metadata = serde_json::json!({
        "name": "json-transformer",
        "version": "1.0.0",
        "description": "Advanced JSON transformation plugin with JSONPath support",
        "author": "DataFlare Team",
        "license": "MIT",
        "plugin_type": "transformer",
        "language": "rust",
        "capabilities": {
            "supported_functions": [
                {
                    "name": "transform",
                    "description": "Transform JSON data using JSONPath rules",
                    "input_type": "json",
                    "output_type": "json"
                },
                {
                    "name": "validate",
                    "description": "Validate JSON data structure",
                    "input_type": "json",
                    "output_type": "boolean"
                }
            ],
            "supported_formats": ["json"],
            "max_memory_mb": 16,
            "timeout_seconds": 30
        },
        "dependencies": ["serde", "jsonpath"],
        "compatibility": ["dataflare-4.0"]
    });

    metadata.to_string()
}

/// 转换JSON数据
#[wasm_bindgen]
pub fn transform(input_json: &str) -> Result<String, JsValue> {
    debug!("开始转换JSON数据");

    // 获取配置
    let config = unsafe {
        PLUGIN_CONFIG.as_ref()
            .ok_or_else(|| JsValue::from_str("插件未初始化"))?
    };

    // 解析输入JSON
    let mut input_data: Value = serde_json::from_str(input_json)
        .map_err(|e| JsValue::from_str(&format!("输入JSON解析失败: {}", e)))?;

    // 创建输出对象
    let mut output_data = if config.preserve_original {
        input_data.clone()
    } else {
        Value::Object(Map::new())
    };

    // 应用转换规则
    for rule in &config.transformations {
        match apply_transform_rule(&input_data, &mut output_data, rule) {
            Ok(_) => debug!("转换规则应用成功: {}", rule.target_field),
            Err(e) => {
                error!("转换规则应用失败: {}", e);
                match &config.error_strategy {
                    ErrorStrategy::Ignore => continue,
                    ErrorStrategy::UseDefault(default_value) => {
                        if let Value::Object(ref mut obj) = output_data {
                            obj.insert(rule.target_field.clone(), default_value.clone());
                        }
                    }
                    ErrorStrategy::Throw => {
                        return Err(JsValue::from_str(&format!("转换失败: {}", e)));
                    }
                }
            }
        }
    }

    // 序列化输出
    let output_json = serde_json::to_string(&output_data)
        .map_err(|e| JsValue::from_str(&format!("输出JSON序列化失败: {}", e)))?;

    debug!("JSON转换完成");
    Ok(output_json)
}

/// 验证JSON数据
#[wasm_bindgen]
pub fn validate(input_json: &str) -> Result<bool, JsValue> {
    debug!("验证JSON数据");

    // 尝试解析JSON
    match serde_json::from_str::<Value>(input_json) {
        Ok(_) => {
            debug!("JSON验证成功");
            Ok(true)
        }
        Err(e) => {
            debug!("JSON验证失败: {}", e);
            Ok(false)
        }
    }
}

/// 应用转换规则
fn apply_transform_rule(
    input: &Value,
    output: &mut Value,
    rule: &TransformRule,
) -> Result<(), String> {
    // 使用JSONPath提取值
    let extracted_value = extract_value_by_path(input, &rule.source_path)?;

    // 应用转换
    let transformed_value = apply_transformation(&extracted_value, &rule.transform_type, &rule.parameters)?;

    // 设置到输出对象
    set_value_by_field(output, &rule.target_field, transformed_value)?;

    Ok(())
}

/// 使用JSONPath提取值
fn extract_value_by_path(data: &Value, path: &str) -> Result<Value, String> {
    // 简化的JSONPath实现
    if path == "$" {
        return Ok(data.clone());
    }

    if path.starts_with("$.") {
        let field_path = &path[2..];
        let fields: Vec<&str> = field_path.split('.').collect();
        
        let mut current = data;
        for field in fields {
            match current {
                Value::Object(obj) => {
                    current = obj.get(field)
                        .ok_or_else(|| format!("字段不存在: {}", field))?;
                }
                _ => return Err(format!("无法在非对象类型中查找字段: {}", field)),
            }
        }
        
        Ok(current.clone())
    } else {
        Err(format!("不支持的JSONPath: {}", path))
    }
}

/// 应用转换
fn apply_transformation(
    value: &Value,
    transform_type: &TransformType,
    _parameters: &Option<Map<String, Value>>,
) -> Result<Value, String> {
    match transform_type {
        TransformType::Copy => Ok(value.clone()),
        TransformType::ToString => Ok(Value::String(value.to_string())),
        TransformType::ToNumber => {
            match value {
                Value::Number(n) => Ok(Value::Number(n.clone())),
                Value::String(s) => {
                    let num: f64 = s.parse()
                        .map_err(|_| format!("无法将字符串转换为数字: {}", s))?;
                    Ok(serde_json::Number::from_f64(num)
                        .map(Value::Number)
                        .unwrap_or(Value::Null))
                }
                _ => Err("无法转换为数字".to_string()),
            }
        }
        TransformType::ToBoolean => {
            match value {
                Value::Bool(b) => Ok(Value::Bool(*b)),
                Value::String(s) => Ok(Value::Bool(!s.is_empty())),
                Value::Number(n) => Ok(Value::Bool(n.as_f64().unwrap_or(0.0) != 0.0)),
                Value::Null => Ok(Value::Bool(false)),
                _ => Ok(Value::Bool(true)),
            }
        }
        TransformType::ToArray => {
            match value {
                Value::Array(arr) => Ok(Value::Array(arr.clone())),
                _ => Ok(Value::Array(vec![value.clone()])),
            }
        }
        TransformType::ToObject => {
            match value {
                Value::Object(obj) => Ok(Value::Object(obj.clone())),
                _ => Err("无法转换为对象".to_string()),
            }
        }
        TransformType::Custom(func_name) => {
            // 这里可以实现自定义转换函数
            Err(format!("自定义函数未实现: {}", func_name))
        }
    }
}

/// 设置字段值
fn set_value_by_field(output: &mut Value, field: &str, value: Value) -> Result<(), String> {
    match output {
        Value::Object(ref mut obj) => {
            obj.insert(field.to_string(), value);
            Ok(())
        }
        _ => Err("输出不是对象类型".to_string()),
    }
}

/// 清理插件资源
#[wasm_bindgen]
pub fn cleanup() {
    info!("清理JSON转换插件资源");
    unsafe {
        PLUGIN_CONFIG = None;
    }
}

/// 获取插件状态
#[wasm_bindgen]
pub fn get_status() -> String {
    let status = unsafe {
        if PLUGIN_CONFIG.is_some() {
            "initialized"
        } else {
            "uninitialized"
        }
    };

    serde_json::json!({
        "status": status,
        "memory_usage": "unknown",
        "last_error": null
    }).to_string()
}
