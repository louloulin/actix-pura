//! 执行器模块 (占位符)

use serde::{Deserialize, Serialize};

/// WASM 执行器 (占位符)
pub struct WasmExecutor;

/// 执行器配置 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    pub max_concurrent: usize,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self { max_concurrent: 100 }
    }
}

/// 执行器指标 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorMetrics {
    pub tasks_executed: u64,
}

impl Default for ExecutorMetrics {
    fn default() -> Self {
        Self { tasks_executed: 0 }
    }
}
