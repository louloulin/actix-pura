//! 合规模块 (占位符)

use serde::{Deserialize, Serialize};

/// 合规管理器 (占位符)
pub struct ComplianceManager;

/// 合规配置 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceConfig {
    pub enabled: bool,
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// 合规指标 (占位符)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceMetrics {
    pub compliance_checks: u64,
}

impl Default for ComplianceMetrics {
    fn default() -> Self {
        Self { compliance_checks: 0 }
    }
}
