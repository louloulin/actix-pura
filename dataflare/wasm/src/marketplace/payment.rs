//! 付费插件支持系统

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use crate::{WasmError, WasmResult};

/// 支付方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentMethod {
    /// 信用卡
    CreditCard {
        /// 卡号（加密）
        card_number_encrypted: String,
        /// 持卡人姓名
        cardholder_name: String,
        /// 过期月份
        expiry_month: u8,
        /// 过期年份
        expiry_year: u16,
        /// 卡类型
        card_type: CardType,
    },
    /// PayPal
    PayPal {
        /// PayPal账户ID
        account_id: String,
        /// 邮箱
        email: String,
    },
    /// 银行转账
    BankTransfer {
        /// 银行名称
        bank_name: String,
        /// 账户号码
        account_number: String,
        /// 路由号码
        routing_number: String,
    },
    /// 加密货币
    Cryptocurrency {
        /// 货币类型
        currency_type: CryptoType,
        /// 钱包地址
        wallet_address: String,
    },
}

/// 卡类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CardType {
    Visa,
    MasterCard,
    AmericanExpress,
    Discover,
    Other(String),
}

/// 加密货币类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CryptoType {
    Bitcoin,
    Ethereum,
    Litecoin,
    Other(String),
}

/// 支付状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaymentStatus {
    /// 待处理
    Pending,
    /// 处理中
    Processing,
    /// 成功
    Completed,
    /// 失败
    Failed,
    /// 已退款
    Refunded,
    /// 部分退款
    PartiallyRefunded,
    /// 已取消
    Cancelled,
}

/// 支付记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRecord {
    /// 支付ID
    pub id: String,
    /// 用户ID
    pub user_id: String,
    /// 插件ID
    pub plugin_id: String,
    /// 支付金额（美分）
    pub amount_cents: u64,
    /// 货币代码
    pub currency: String,
    /// 支付方式
    pub payment_method: PaymentMethod,
    /// 支付状态
    pub status: PaymentStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 支付网关交易ID
    pub gateway_transaction_id: Option<String>,
    /// 失败原因
    pub failure_reason: Option<String>,
    /// 退款金额（美分）
    pub refunded_amount_cents: u64,
}

/// 订阅记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionRecord {
    /// 订阅ID
    pub id: String,
    /// 用户ID
    pub user_id: String,
    /// 插件ID
    pub plugin_id: String,
    /// 订阅计划
    pub plan: SubscriptionPlan,
    /// 订阅状态
    pub status: SubscriptionStatus,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 下次计费时间
    pub next_billing_at: DateTime<Utc>,
    /// 取消时间
    pub cancelled_at: Option<DateTime<Utc>>,
    /// 支付方式
    pub payment_method: PaymentMethod,
    /// 支付历史
    pub payment_history: Vec<String>, // Payment IDs
}

/// 订阅计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPlan {
    /// 计划ID
    pub id: String,
    /// 计划名称
    pub name: String,
    /// 月费（美分）
    pub monthly_price_cents: u64,
    /// 年费（美分）
    pub yearly_price_cents: Option<u64>,
    /// 计费周期
    pub billing_cycle: BillingCycle,
    /// 功能列表
    pub features: Vec<String>,
    /// 使用限制
    pub usage_limits: super::license::UsageLimits,
}

/// 计费周期
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BillingCycle {
    Monthly,
    Yearly,
    Quarterly,
}

/// 订阅状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubscriptionStatus {
    /// 活跃
    Active,
    /// 已暂停
    Paused,
    /// 已取消
    Cancelled,
    /// 已过期
    Expired,
    /// 试用中
    Trial,
}

/// 收入分成记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueShare {
    /// 记录ID
    pub id: String,
    /// 开发者ID
    pub developer_id: String,
    /// 插件ID
    pub plugin_id: String,
    /// 总收入（美分）
    pub total_revenue_cents: u64,
    /// 平台费用（美分）
    pub platform_fee_cents: u64,
    /// 开发者收入（美分）
    pub developer_revenue_cents: u64,
    /// 分成比例
    pub revenue_share_percentage: f64,
    /// 计算周期
    pub period_start: DateTime<Utc>,
    /// 计算周期结束
    pub period_end: DateTime<Utc>,
    /// 支付状态
    pub payout_status: PayoutStatus,
    /// 支付时间
    pub paid_at: Option<DateTime<Utc>>,
}

/// 支付状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PayoutStatus {
    /// 待支付
    Pending,
    /// 已支付
    Paid,
    /// 支付失败
    Failed,
    /// 已暂停
    Suspended,
}

/// 支付服务
pub struct PaymentService {
    /// 支付记录
    payments: HashMap<String, PaymentRecord>,
    /// 订阅记录
    subscriptions: HashMap<String, SubscriptionRecord>,
    /// 收入分成记录
    revenue_shares: HashMap<String, RevenueShare>,
    /// 用户支付方式
    user_payment_methods: HashMap<String, Vec<PaymentMethod>>,
    /// 订阅计划
    subscription_plans: HashMap<String, SubscriptionPlan>,
}

impl PaymentService {
    /// 创建新的支付服务
    pub fn new() -> Self {
        Self {
            payments: HashMap::new(),
            subscriptions: HashMap::new(),
            revenue_shares: HashMap::new(),
            user_payment_methods: HashMap::new(),
            subscription_plans: HashMap::new(),
        }
    }

    /// 处理一次性支付
    pub async fn process_payment(
        &mut self,
        user_id: String,
        plugin_id: String,
        amount_cents: u64,
        currency: String,
        payment_method: PaymentMethod,
    ) -> WasmResult<PaymentRecord> {
        let payment_id = Uuid::new_v4().to_string();

        let payment = PaymentRecord {
            id: payment_id.clone(),
            user_id,
            plugin_id,
            amount_cents,
            currency,
            payment_method,
            status: PaymentStatus::Processing,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            gateway_transaction_id: None,
            failure_reason: None,
            refunded_amount_cents: 0,
        };

        // 模拟支付处理
        let mut processed_payment = payment.clone();
        processed_payment.status = PaymentStatus::Completed;
        processed_payment.gateway_transaction_id = Some(format!("txn_{}", Uuid::new_v4()));
        processed_payment.updated_at = Utc::now();

        self.payments.insert(payment_id, processed_payment.clone());

        Ok(processed_payment)
    }

    /// 创建订阅
    pub async fn create_subscription(
        &mut self,
        user_id: String,
        plugin_id: String,
        plan_id: String,
        payment_method: PaymentMethod,
    ) -> WasmResult<SubscriptionRecord> {
        let plan = self.subscription_plans.get(&plan_id)
            .ok_or_else(|| WasmError::plugin_execution("订阅计划不存在".to_string()))?
            .clone();

        let subscription_id = Uuid::new_v4().to_string();

        let next_billing_at = match plan.billing_cycle {
            BillingCycle::Monthly => Utc::now() + Duration::days(30),
            BillingCycle::Quarterly => Utc::now() + Duration::days(90),
            BillingCycle::Yearly => Utc::now() + Duration::days(365),
        };

        let subscription = SubscriptionRecord {
            id: subscription_id.clone(),
            user_id,
            plugin_id,
            plan,
            status: SubscriptionStatus::Active,
            started_at: Utc::now(),
            next_billing_at,
            cancelled_at: None,
            payment_method,
            payment_history: Vec::new(),
        };

        self.subscriptions.insert(subscription_id, subscription.clone());

        Ok(subscription)
    }

    /// 取消订阅
    pub async fn cancel_subscription(&mut self, subscription_id: &str) -> WasmResult<()> {
        if let Some(subscription) = self.subscriptions.get_mut(subscription_id) {
            subscription.status = SubscriptionStatus::Cancelled;
            subscription.cancelled_at = Some(Utc::now());
        }
        Ok(())
    }

    /// 处理退款
    pub async fn process_refund(
        &mut self,
        payment_id: &str,
        refund_amount_cents: u64,
        reason: String,
    ) -> WasmResult<()> {
        if let Some(payment) = self.payments.get_mut(payment_id) {
            if payment.status != PaymentStatus::Completed {
                return Err(WasmError::plugin_execution("只能退款已完成的支付".to_string()));
            }

            if refund_amount_cents > payment.amount_cents - payment.refunded_amount_cents {
                return Err(WasmError::plugin_execution("退款金额超过可退款金额".to_string()));
            }

            payment.refunded_amount_cents += refund_amount_cents;
            payment.updated_at = Utc::now();

            if payment.refunded_amount_cents >= payment.amount_cents {
                payment.status = PaymentStatus::Refunded;
            } else {
                payment.status = PaymentStatus::PartiallyRefunded;
            }
        }

        Ok(())
    }

    /// 计算收入分成
    pub async fn calculate_revenue_share(
        &mut self,
        developer_id: String,
        plugin_id: String,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        revenue_share_percentage: f64,
    ) -> WasmResult<RevenueShare> {
        // 计算期间内的总收入
        let total_revenue_cents: u64 = self.payments.values()
            .filter(|p| {
                p.plugin_id == plugin_id &&
                p.status == PaymentStatus::Completed &&
                p.created_at >= period_start &&
                p.created_at <= period_end
            })
            .map(|p| p.amount_cents - p.refunded_amount_cents)
            .sum();

        let developer_revenue_cents = (total_revenue_cents as f64 * revenue_share_percentage / 100.0) as u64;
        let platform_fee_cents = total_revenue_cents - developer_revenue_cents;

        let revenue_share = RevenueShare {
            id: Uuid::new_v4().to_string(),
            developer_id,
            plugin_id,
            total_revenue_cents,
            platform_fee_cents,
            developer_revenue_cents,
            revenue_share_percentage,
            period_start,
            period_end,
            payout_status: PayoutStatus::Pending,
            paid_at: None,
        };

        self.revenue_shares.insert(revenue_share.id.clone(), revenue_share.clone());

        Ok(revenue_share)
    }

    /// 获取用户支付历史
    pub async fn get_user_payment_history(&self, user_id: &str) -> Vec<PaymentRecord> {
        self.payments.values()
            .filter(|p| p.user_id == user_id)
            .cloned()
            .collect()
    }

    /// 获取用户订阅
    pub async fn get_user_subscriptions(&self, user_id: &str) -> Vec<SubscriptionRecord> {
        self.subscriptions.values()
            .filter(|s| s.user_id == user_id)
            .cloned()
            .collect()
    }

    /// 添加订阅计划
    pub async fn add_subscription_plan(&mut self, plan: SubscriptionPlan) -> WasmResult<()> {
        self.subscription_plans.insert(plan.id.clone(), plan);
        Ok(())
    }

    /// 获取订阅计划
    pub async fn get_subscription_plans(&self, plugin_id: &str) -> Vec<SubscriptionPlan> {
        // 简化实现：返回所有计划
        // 实际实现中应该根据插件ID过滤
        self.subscription_plans.values().cloned().collect()
    }
}

impl Default for PaymentService {
    fn default() -> Self {
        Self::new()
    }
}
