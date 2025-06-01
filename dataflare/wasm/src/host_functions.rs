//! WASM主机函数系统

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::{WasmError, WasmResult};

/// 主机函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFunctionCall {
    /// 函数名称
    pub function_name: String,
    /// 参数列表
    pub parameters: Vec<serde_json::Value>,
    /// 调用上下文
    pub context: Option<HashMap<String, serde_json::Value>>,
}

/// 主机函数定义
pub trait HostFunction: Send + Sync {
    /// 函数名称
    fn name(&self) -> &str;

    /// 函数描述
    fn description(&self) -> &str;

    /// 执行函数
    fn execute(&self, params: Vec<serde_json::Value>) -> WasmResult<serde_json::Value>;

    /// 是否需要权限检查
    fn requires_permission(&self) -> bool {
        false
    }

    /// 所需权限
    fn required_permission(&self) -> Option<&str> {
        None
    }
}

/// 主机函数注册表
pub struct HostFunctionRegistry {
    /// 已注册的函数
    functions: HashMap<String, Box<dyn HostFunction>>,
}

impl HostFunctionRegistry {
    /// 创建新的主机函数注册表
    pub fn new() -> Self {
        let mut registry = Self {
            functions: HashMap::new(),
        };

        // 注册默认主机函数
        registry.register_default_functions();
        registry
    }

    /// 注册默认主机函数
    fn register_default_functions(&mut self) {
        self.register_function(Box::new(LogFunction));
        self.register_function(Box::new(GetTimeFunction));
        self.register_function(Box::new(RandomFunction));
    }

    /// 注册主机函数
    pub fn register_function(&mut self, function: Box<dyn HostFunction>) {
        let name = function.name().to_string();
        self.functions.insert(name.clone(), function);
        log::info!("主机函数注册成功: {}", name);
    }

    /// 调用主机函数
    pub fn call_function(&self, call: HostFunctionCall) -> WasmResult<serde_json::Value> {
        let function = self.functions.get(&call.function_name)
            .ok_or_else(|| WasmError::FunctionNotFound(call.function_name.clone()))?;

        function.execute(call.parameters)
    }

    /// 检查函数是否存在
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// 列出所有函数
    pub fn list_functions(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }
}

impl Default for HostFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 日志函数
struct LogFunction;

impl HostFunction for LogFunction {
    fn name(&self) -> &str {
        "log"
    }

    fn description(&self) -> &str {
        "记录日志消息"
    }

    fn execute(&self, params: Vec<serde_json::Value>) -> WasmResult<serde_json::Value> {
        if params.is_empty() {
            return Err(WasmError::FunctionNotFound("log函数需要至少一个参数".to_string()));
        }

        let message = params[0].as_str()
            .ok_or_else(|| WasmError::TypeConversion("log参数必须是字符串".to_string()))?;

        let level = if params.len() > 1 {
            params[1].as_str().unwrap_or("info")
        } else {
            "info"
        };

        match level {
            "error" => log::error!("[WASM] {}", message),
            "warn" => log::warn!("[WASM] {}", message),
            "info" => log::info!("[WASM] {}", message),
            "debug" => log::debug!("[WASM] {}", message),
            _ => log::info!("[WASM] {}", message),
        }

        Ok(serde_json::Value::Null)
    }
}

/// 获取时间函数
struct GetTimeFunction;

impl HostFunction for GetTimeFunction {
    fn name(&self) -> &str {
        "get_time"
    }

    fn description(&self) -> &str {
        "获取当前时间戳"
    }

    fn execute(&self, _params: Vec<serde_json::Value>) -> WasmResult<serde_json::Value> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        Ok(serde_json::Value::Number(serde_json::Number::from(timestamp)))
    }
}

/// 随机数函数
struct RandomFunction;

impl HostFunction for RandomFunction {
    fn name(&self) -> &str {
        "random"
    }

    fn description(&self) -> &str {
        "生成随机数"
    }

    fn execute(&self, params: Vec<serde_json::Value>) -> WasmResult<serde_json::Value> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let max = if !params.is_empty() {
            params[0].as_u64().unwrap_or(100)
        } else {
            100
        };

        // 简单的伪随机数生成
        let mut hasher = DefaultHasher::new();
        chrono::Utc::now().timestamp_nanos().hash(&mut hasher);
        let random_value = hasher.finish() % max;

        Ok(serde_json::Value::Number(serde_json::Number::from(random_value)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_function_registry() {
        let registry = HostFunctionRegistry::new();

        assert!(registry.has_function("log"));
        assert!(registry.has_function("get_time"));
        assert!(registry.has_function("random"));
        assert!(!registry.has_function("nonexistent"));

        let functions = registry.list_functions();
        assert!(functions.contains(&"log".to_string()));
        assert!(functions.contains(&"get_time".to_string()));
        assert!(functions.contains(&"random".to_string()));
    }

    #[test]
    fn test_log_function() {
        let registry = HostFunctionRegistry::new();

        let call = HostFunctionCall {
            function_name: "log".to_string(),
            parameters: vec![serde_json::Value::String("test message".to_string())],
            context: None,
        };

        let result = registry.call_function(call);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn test_get_time_function() {
        let registry = HostFunctionRegistry::new();

        let call = HostFunctionCall {
            function_name: "get_time".to_string(),
            parameters: vec![],
            context: None,
        };

        let result = registry.call_function(call);
        assert!(result.is_ok());
        assert!(result.unwrap().is_number());
    }

    #[test]
    fn test_random_function() {
        let registry = HostFunctionRegistry::new();

        let call = HostFunctionCall {
            function_name: "random".to_string(),
            parameters: vec![serde_json::Value::Number(serde_json::Number::from(50))],
            context: None,
        };

        let result = registry.call_function(call);
        assert!(result.is_ok());

        let value = result.unwrap().as_u64().unwrap();
        assert!(value < 50);
    }

    #[test]
    fn test_nonexistent_function() {
        let registry = HostFunctionRegistry::new();

        let call = HostFunctionCall {
            function_name: "nonexistent".to_string(),
            parameters: vec![],
            context: None,
        };

        let result = registry.call_function(call);
        assert!(result.is_err());
    }
}
