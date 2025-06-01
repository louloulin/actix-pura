//! {{plugin_name}} - DataFlare WASM插件
//!
//! 这是一个基础的DataFlare WASM插件模板

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 插件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// 插件名称
    pub name: String,
    /// 插件版本
    pub version: String,
    /// 自定义配置参数
    pub parameters: HashMap<String, String>,
}

/// 数据记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    /// 记录ID
    pub id: String,
    /// 数据内容
    pub data: serde_json::Value,
    /// 元数据
    pub metadata: HashMap<String, String>,
    /// 时间戳
    pub timestamp: u64,
}

/// 处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingResult {
    /// 成功处理
    Success(DataRecord),
    /// 处理失败
    Error(String),
    /// 跳过记录
    Skip,
}

/// 插件状态
#[derive(Debug, Clone)]
pub struct PluginState {
    /// 是否已初始化
    pub initialized: bool,
    /// 处理计数
    pub process_count: u64,
    /// 错误计数
    pub error_count: u64,
    /// 配置
    pub config: Option<PluginConfig>,
}

/// 全局插件状态
static mut PLUGIN_STATE: PluginState = PluginState {
    initialized: false,
    process_count: 0,
    error_count: 0,
    config: None,
};

/// 初始化插件
#[no_mangle]
pub extern "C" fn init(config_ptr: *const u8, config_len: usize) -> i32 {
    unsafe {
        if PLUGIN_STATE.initialized {
            return 0; // 已经初始化
        }

        // 解析配置
        let config_slice = std::slice::from_raw_parts(config_ptr, config_len);
        let config_str = match std::str::from_utf8(config_slice) {
            Ok(s) => s,
            Err(_) => return -1,
        };

        let config: PluginConfig = match serde_json::from_str(config_str) {
            Ok(c) => c,
            Err(_) => return -2,
        };

        PLUGIN_STATE.config = Some(config);
        PLUGIN_STATE.initialized = true;
        
        log_info("{{plugin_name}} 插件初始化成功");
        0
    }
}

/// 处理数据
#[no_mangle]
pub extern "C" fn process(input_ptr: *const u8, input_len: usize, output_ptr: *mut u8, output_len: *mut usize) -> i32 {
    unsafe {
        if !PLUGIN_STATE.initialized {
            return -1; // 未初始化
        }

        // 解析输入数据
        let input_slice = std::slice::from_raw_parts(input_ptr, input_len);
        let input_str = match std::str::from_utf8(input_slice) {
            Ok(s) => s,
            Err(_) => {
                PLUGIN_STATE.error_count += 1;
                return -2;
            }
        };

        let input_record: DataRecord = match serde_json::from_str(input_str) {
            Ok(r) => r,
            Err(_) => {
                PLUGIN_STATE.error_count += 1;
                return -3;
            }
        };

        // 处理数据
        let result = process_record(input_record);
        PLUGIN_STATE.process_count += 1;

        // 序列化结果
        let output_str = match serde_json::to_string(&result) {
            Ok(s) => s,
            Err(_) => {
                PLUGIN_STATE.error_count += 1;
                return -4;
            }
        };

        let output_bytes = output_str.as_bytes();
        if output_bytes.len() > *output_len {
            *output_len = output_bytes.len();
            return -5; // 输出缓冲区太小
        }

        // 复制输出数据
        std::ptr::copy_nonoverlapping(output_bytes.as_ptr(), output_ptr, output_bytes.len());
        *output_len = output_bytes.len();

        0
    }
}

/// 获取插件信息
#[no_mangle]
pub extern "C" fn get_info(output_ptr: *mut u8, output_len: *mut usize) -> i32 {
    let info = serde_json::json!({
        "name": "{{plugin_name}}",
        "version": "1.0.0",
        "description": "DataFlare基础插件模板",
        "author": "DataFlare Team",
        "capabilities": [
            "process",
            "transform"
        ]
    });

    let info_str = info.to_string();
    let info_bytes = info_str.as_bytes();

    unsafe {
        if info_bytes.len() > *output_len {
            *output_len = info_bytes.len();
            return -1; // 缓冲区太小
        }

        std::ptr::copy_nonoverlapping(info_bytes.as_ptr(), output_ptr, info_bytes.len());
        *output_len = info_bytes.len();
    }

    0
}

/// 获取插件统计信息
#[no_mangle]
pub extern "C" fn get_stats(output_ptr: *mut u8, output_len: *mut usize) -> i32 {
    unsafe {
        let stats = serde_json::json!({
            "initialized": PLUGIN_STATE.initialized,
            "process_count": PLUGIN_STATE.process_count,
            "error_count": PLUGIN_STATE.error_count,
            "error_rate": if PLUGIN_STATE.process_count > 0 {
                PLUGIN_STATE.error_count as f64 / PLUGIN_STATE.process_count as f64
            } else {
                0.0
            }
        });

        let stats_str = stats.to_string();
        let stats_bytes = stats_str.as_bytes();

        if stats_bytes.len() > *output_len {
            *output_len = stats_bytes.len();
            return -1; // 缓冲区太小
        }

        std::ptr::copy_nonoverlapping(stats_bytes.as_ptr(), output_ptr, stats_bytes.len());
        *output_len = stats_bytes.len();
    }

    0
}

/// 清理插件
#[no_mangle]
pub extern "C" fn cleanup() -> i32 {
    unsafe {
        PLUGIN_STATE.initialized = false;
        PLUGIN_STATE.process_count = 0;
        PLUGIN_STATE.error_count = 0;
        PLUGIN_STATE.config = None;
        
        log_info("{{plugin_name}} 插件清理完成");
    }
    0
}

/// 处理单个记录的核心逻辑
fn process_record(mut record: DataRecord) -> ProcessingResult {
    // 在这里实现你的数据处理逻辑
    
    // 示例：添加处理时间戳
    record.metadata.insert(
        "processed_at".to_string(),
        chrono::Utc::now().timestamp().to_string()
    );
    
    // 示例：添加插件标识
    record.metadata.insert(
        "processed_by".to_string(),
        "{{plugin_name}}".to_string()
    );

    // 示例：简单的数据转换
    if let Some(obj) = record.data.as_object_mut() {
        obj.insert("plugin_processed".to_string(), serde_json::Value::Bool(true));
    }

    ProcessingResult::Success(record)
}

/// 记录信息日志
fn log_info(message: &str) {
    // 在实际实现中，这里会调用主机提供的日志函数
    // 现在只是一个占位符
    #[cfg(debug_assertions)]
    eprintln!("[INFO] {}", message);
}

/// 记录错误日志
fn log_error(message: &str) {
    // 在实际实现中，这里会调用主机提供的日志函数
    // 现在只是一个占位符
    #[cfg(debug_assertions)]
    eprintln!("[ERROR] {}", message);
}

/// 获取当前时间戳
fn get_timestamp() -> u64 {
    // 在实际实现中，这里会调用主机提供的时间函数
    // 现在返回一个模拟值
    1640995200 // 2022-01-01 00:00:00 UTC
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_record() {
        let input_record = DataRecord {
            id: "test_001".to_string(),
            data: serde_json::json!({"value": 42}),
            metadata: HashMap::new(),
            timestamp: get_timestamp(),
        };

        let result = process_record(input_record);
        
        match result {
            ProcessingResult::Success(record) => {
                assert!(record.metadata.contains_key("processed_at"));
                assert!(record.metadata.contains_key("processed_by"));
                assert_eq!(record.data["plugin_processed"], true);
            }
            _ => panic!("Expected successful processing"),
        }
    }

    #[test]
    fn test_plugin_config() {
        let config = PluginConfig {
            name: "{{plugin_name}}".to_string(),
            version: "1.0.0".to_string(),
            parameters: HashMap::new(),
        };

        assert_eq!(config.name, "{{plugin_name}}");
        assert_eq!(config.version, "1.0.0");
    }

    #[test]
    fn test_data_record_serialization() {
        let record = DataRecord {
            id: "test".to_string(),
            data: serde_json::json!({"test": true}),
            metadata: HashMap::new(),
            timestamp: 1640995200,
        };

        let serialized = serde_json::to_string(&record).unwrap();
        let deserialized: DataRecord = serde_json::from_str(&serialized).unwrap();

        assert_eq!(record.id, deserialized.id);
        assert_eq!(record.timestamp, deserialized.timestamp);
    }

    #[test]
    fn test_processing_result_variants() {
        // 测试成功结果
        let success_record = DataRecord {
            id: "success".to_string(),
            data: serde_json::json!({}),
            metadata: HashMap::new(),
            timestamp: 0,
        };
        let success_result = ProcessingResult::Success(success_record);
        
        match success_result {
            ProcessingResult::Success(_) => {},
            _ => panic!("Expected Success variant"),
        }

        // 测试错误结果
        let error_result = ProcessingResult::Error("Test error".to_string());
        match error_result {
            ProcessingResult::Error(msg) => assert_eq!(msg, "Test error"),
            _ => panic!("Expected Error variant"),
        }

        // 测试跳过结果
        let skip_result = ProcessingResult::Skip;
        match skip_result {
            ProcessingResult::Skip => {},
            _ => panic!("Expected Skip variant"),
        }
    }
}
