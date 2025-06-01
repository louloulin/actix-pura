//! 告警模块 (占位符)

use serde::{Deserialize, Serialize};

/// 告警管理器 (占位符)
pub struct AlertingManager;

/// 告警配置 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    pub enabled: bool,
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// 告警指标 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingMetrics {
    pub alerts_sent: u64,
}

impl Default for AlertingMetrics {
    fn default() -> Self {
        Self { alerts_sent: 0 }
    }
}
