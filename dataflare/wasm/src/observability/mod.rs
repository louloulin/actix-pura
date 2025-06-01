//! 可观测性模块 (占位符)

use serde::{Deserialize, Serialize};

/// 可观测性管理器 (占位符)
pub struct ObservabilityManager;

/// 追踪配置 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    pub enabled: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// 追踪指标 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingMetrics {
    pub traces_collected: u64,
}

impl Default for TracingMetrics {
    fn default() -> Self {
        Self { traces_collected: 0 }
    }
}
