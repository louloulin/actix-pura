//! 许可证管理系统

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use crate::{WasmError, WasmResult};

/// 许可证类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LicenseType {
    /// 开源许可证
    OpenSource {
        /// 许可证名称 (MIT, Apache-2.0, GPL-3.0等)
        name: String,
        /// 是否允许商业使用
        allows_commercial: bool,
        /// 是否需要署名
        requires_attribution: bool,
        /// 是否需要开源衍生作品
        requires_share_alike: bool,
    },
    /// 商业许可证
    Commercial {
        /// 许可证名称
        name: String,
        /// 价格模式
        pricing_model: PricingModel,
        /// 使用限制
        usage_limits: UsageLimits,
    },
    /// 免费许可证
    Freeware {
        /// 许可证名称
        name: String,
        /// 使用限制
        restrictions: Vec<String>,
    },
    /// 试用许可证
    Trial {
        /// 试用期天数
        trial_days: u32,
        /// 功能限制
        feature_limits: Vec<String>,
    },
}

/// 价格模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PricingModel {
    /// 一次性购买
    OneTime {
        /// 价格（美分）
        price_cents: u64,
        /// 货币代码
        currency: String,
    },
    /// 订阅模式
    Subscription {
        /// 月费（美分）
        monthly_price_cents: u64,
        /// 年费（美分）
        yearly_price_cents: Option<u64>,
        /// 货币代码
        currency: String,
    },
    /// 按使用量计费
    PayPerUse {
        /// 单位价格（美分）
        price_per_unit_cents: u64,
        /// 计费单位
        unit: String,
        /// 货币代码
        currency: String,
    },
    /// 免费
    Free,
}

/// 使用限制
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageLimits {
    /// 最大用户数
    pub max_users: Option<u32>,
    /// 最大数据处理量（MB）
    pub max_data_mb: Option<u64>,
    /// 最大API调用次数
    pub max_api_calls: Option<u64>,
    /// 地理限制
    pub geographic_restrictions: Vec<String>,
    /// 行业限制
    pub industry_restrictions: Vec<String>,
}

/// 许可证实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInstance {
    /// 许可证ID
    pub id: String,
    /// 插件ID
    pub plugin_id: String,
    /// 用户ID
    pub user_id: String,
    /// 许可证类型
    pub license_type: LicenseType,
    /// 激活时间
    pub activated_at: DateTime<Utc>,
    /// 过期时间
    pub expires_at: Option<DateTime<Utc>>,
    /// 许可证状态
    pub status: LicenseStatus,
    /// 使用统计
    pub usage_stats: UsageStats,
    /// 许可证密钥
    pub license_key: String,
}

/// 许可证状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LicenseStatus {
    /// 活跃
    Active,
    /// 已过期
    Expired,
    /// 已暂停
    Suspended,
    /// 已撤销
    Revoked,
    /// 试用中
    Trial,
}

/// 使用统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    /// 数据处理量（MB）
    pub data_processed_mb: u64,
    /// API调用次数
    pub api_calls: u64,
    /// 活跃用户数
    pub active_users: u32,
    /// 最后使用时间
    pub last_used: Option<DateTime<Utc>>,
}

/// 许可证验证结果
#[derive(Debug, Clone)]
pub struct LicenseValidation {
    /// 是否有效
    pub is_valid: bool,
    /// 验证消息
    pub message: String,
    /// 剩余使用量
    pub remaining_usage: Option<UsageLimits>,
    /// 过期时间
    pub expires_at: Option<DateTime<Utc>>,
}

/// 许可证管理器
pub struct LicenseManager {
    /// 许可证实例存储
    licenses: HashMap<String, LicenseInstance>,
    /// 用户许可证映射
    user_licenses: HashMap<String, Vec<String>>,
    /// 插件许可证映射
    plugin_licenses: HashMap<String, Vec<String>>,
}

impl LicenseManager {
    /// 创建新的许可证管理器
    pub fn new() -> Self {
        Self {
            licenses: HashMap::new(),
            user_licenses: HashMap::new(),
            plugin_licenses: HashMap::new(),
        }
    }

    /// 激活许可证
    pub async fn activate_license(
        &mut self,
        plugin_id: String,
        user_id: String,
        license_type: LicenseType,
    ) -> WasmResult<LicenseInstance> {
        let license_id = Uuid::new_v4().to_string();
        let license_key = self.generate_license_key(&plugin_id, &user_id);

        // 计算过期时间
        let expires_at = match &license_type {
            LicenseType::Trial { trial_days, .. } => {
                Some(Utc::now() + Duration::days(*trial_days as i64))
            }
            LicenseType::Commercial { pricing_model, .. } => {
                match pricing_model {
                    PricingModel::Subscription { .. } => {
                        Some(Utc::now() + Duration::days(30)) // 默认月订阅
                    }
                    _ => None,
                }
            }
            _ => None,
        };

        let license = LicenseInstance {
            id: license_id.clone(),
            plugin_id: plugin_id.clone(),
            user_id: user_id.clone(),
            license_type,
            activated_at: Utc::now(),
            expires_at,
            status: LicenseStatus::Active,
            usage_stats: UsageStats::default(),
            license_key,
        };

        // 存储许可证
        self.licenses.insert(license_id.clone(), license.clone());

        // 更新索引
        self.user_licenses
            .entry(user_id)
            .or_insert_with(Vec::new)
            .push(license_id.clone());

        self.plugin_licenses
            .entry(plugin_id)
            .or_insert_with(Vec::new)
            .push(license_id);

        Ok(license)
    }

    /// 验证许可证
    pub async fn validate_license(
        &self,
        plugin_id: &str,
        user_id: &str,
        usage_request: &UsageRequest,
    ) -> WasmResult<LicenseValidation> {
        // 查找用户的许可证
        let license = if let Some(user_license_ids) = self.user_licenses.get(user_id) {
            user_license_ids
                .iter()
                .filter_map(|id| self.licenses.get(id))
                .find(|l| l.plugin_id == plugin_id)
        } else {
            None
        };

        let Some(license) = license else {
            return Ok(LicenseValidation {
                is_valid: false,
                message: "未找到有效许可证".to_string(),
                remaining_usage: None,
                expires_at: None,
            });
        };

        // 检查许可证状态
        if license.status != LicenseStatus::Active && license.status != LicenseStatus::Trial {
            return Ok(LicenseValidation {
                is_valid: false,
                message: format!("许可证状态无效: {:?}", license.status),
                remaining_usage: None,
                expires_at: license.expires_at,
            });
        }

        // 检查过期时间
        if let Some(expires_at) = license.expires_at {
            if Utc::now() > expires_at {
                return Ok(LicenseValidation {
                    is_valid: false,
                    message: "许可证已过期".to_string(),
                    remaining_usage: None,
                    expires_at: Some(expires_at),
                });
            }
        }

        // 检查使用限制
        if let LicenseType::Commercial { usage_limits, .. } = &license.license_type {
            if let Some(validation) = self.check_usage_limits(usage_limits, &license.usage_stats, usage_request) {
                return Ok(validation);
            }
        }

        Ok(LicenseValidation {
            is_valid: true,
            message: "许可证有效".to_string(),
            remaining_usage: self.calculate_remaining_usage(license),
            expires_at: license.expires_at,
        })
    }

    /// 更新使用统计
    pub async fn update_usage_stats(
        &mut self,
        plugin_id: &str,
        user_id: &str,
        usage: &UsageUpdate,
    ) -> WasmResult<()> {
        if let Some(user_license_ids) = self.user_licenses.get(user_id) {
            if let Some(license_id) = user_license_ids
                .iter()
                .find(|id| {
                    self.licenses.get(*id)
                        .map(|l| l.plugin_id == plugin_id)
                        .unwrap_or(false)
                }) {

                if let Some(license) = self.licenses.get_mut(license_id) {
                    license.usage_stats.data_processed_mb += usage.data_processed_mb;
                    license.usage_stats.api_calls += usage.api_calls;
                    license.usage_stats.last_used = Some(Utc::now());

                    if usage.active_users > license.usage_stats.active_users {
                        license.usage_stats.active_users = usage.active_users;
                    }
                }
            }
        }

        Ok(())
    }

    /// 撤销许可证
    pub async fn revoke_license(&mut self, license_id: &str) -> WasmResult<()> {
        if let Some(license) = self.licenses.get_mut(license_id) {
            license.status = LicenseStatus::Revoked;
        }
        Ok(())
    }

    /// 获取用户许可证
    pub async fn get_user_licenses(&self, user_id: &str) -> Vec<LicenseInstance> {
        if let Some(user_license_ids) = self.user_licenses.get(user_id) {
            user_license_ids
                .iter()
                .filter_map(|id| self.licenses.get(id))
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 生成许可证密钥
    fn generate_license_key(&self, plugin_id: &str, user_id: &str) -> String {
        format!("DF-{}-{}-{}",
                plugin_id.chars().take(8).collect::<String>().to_uppercase(),
                user_id.chars().take(8).collect::<String>().to_uppercase(),
                Uuid::new_v4().to_string().replace("-", "").chars().take(16).collect::<String>().to_uppercase())
    }

    /// 检查使用限制
    fn check_usage_limits(
        &self,
        limits: &UsageLimits,
        current_usage: &UsageStats,
        request: &UsageRequest,
    ) -> Option<LicenseValidation> {
        // 检查数据处理量限制
        if let Some(max_data) = limits.max_data_mb {
            if current_usage.data_processed_mb + request.data_mb > max_data {
                return Some(LicenseValidation {
                    is_valid: false,
                    message: "数据处理量超出限制".to_string(),
                    remaining_usage: None,
                    expires_at: None,
                });
            }
        }

        // 检查API调用限制
        if let Some(max_calls) = limits.max_api_calls {
            if current_usage.api_calls + request.api_calls > max_calls {
                return Some(LicenseValidation {
                    is_valid: false,
                    message: "API调用次数超出限制".to_string(),
                    remaining_usage: None,
                    expires_at: None,
                });
            }
        }

        // 检查用户数限制
        if let Some(max_users) = limits.max_users {
            if request.users > max_users {
                return Some(LicenseValidation {
                    is_valid: false,
                    message: "用户数超出限制".to_string(),
                    remaining_usage: None,
                    expires_at: None,
                });
            }
        }

        None
    }

    /// 计算剩余使用量
    fn calculate_remaining_usage(&self, license: &LicenseInstance) -> Option<UsageLimits> {
        if let LicenseType::Commercial { usage_limits, .. } = &license.license_type {
            Some(UsageLimits {
                max_users: usage_limits.max_users,
                max_data_mb: usage_limits.max_data_mb
                    .map(|max| max.saturating_sub(license.usage_stats.data_processed_mb)),
                max_api_calls: usage_limits.max_api_calls
                    .map(|max| max.saturating_sub(license.usage_stats.api_calls)),
                geographic_restrictions: usage_limits.geographic_restrictions.clone(),
                industry_restrictions: usage_limits.industry_restrictions.clone(),
            })
        } else {
            None
        }
    }
}

/// 使用请求
#[derive(Debug, Clone)]
pub struct UsageRequest {
    /// 数据量（MB）
    pub data_mb: u64,
    /// API调用次数
    pub api_calls: u64,
    /// 用户数
    pub users: u32,
}

/// 使用更新
#[derive(Debug, Clone)]
pub struct UsageUpdate {
    /// 处理的数据量（MB）
    pub data_processed_mb: u64,
    /// API调用次数
    pub api_calls: u64,
    /// 活跃用户数
    pub active_users: u32,
}

impl Default for LicenseManager {
    fn default() -> Self {
        Self::new()
    }
}
