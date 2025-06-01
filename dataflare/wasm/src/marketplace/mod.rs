//! DataFlare插件市场模块
//!
//! 提供完整的插件市场功能，包括：
//! - 插件注册表
//! - 插件搜索和发现
//! - 插件下载和安装
//! - 评分和评论系统
//! - 安全扫描和质量控制

pub mod registry;
pub mod search;
pub mod package_manager;
pub mod security_scanner;
pub mod quality_scorer;
pub mod recommendation;
pub mod types;
pub mod api;
pub mod storage;
pub mod auth;
pub mod review;
pub mod license;
pub mod payment;



pub use types::*;
pub use registry::PluginRegistry;
pub use search::PluginSearchEngine;
pub use package_manager::{PluginPackageManager, DependencyTree};
pub use security_scanner::PluginSecurityScanner;
pub use quality_scorer::PluginQualityScorer;
pub use recommendation::PluginRecommendationEngine;
pub use api::MarketplaceAPI;
pub use storage::MarketplaceStorage;
pub use auth::{AuthService, User, UserRole, ApiKey, Permission};
pub use review::{ReviewService, PluginReview, ReviewStats, ReviewFilter};
pub use license::{LicenseManager, LicenseInstance, LicenseType, LicenseValidation};
pub use payment::{PaymentService, PaymentRecord, SubscriptionRecord, RevenueShare};

use crate::{WasmError, WasmResult};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 插件市场主接口
#[async_trait]
pub trait PluginMarketplace {
    /// 搜索插件
    async fn search_plugins(&self, query: SearchQuery) -> WasmResult<SearchResult>;

    /// 获取插件详情
    async fn get_plugin_details(&self, plugin_id: &str) -> WasmResult<PluginDetails>;

    /// 获取插件版本列表
    async fn get_plugin_versions(&self, plugin_id: &str) -> WasmResult<Vec<PluginVersion>>;

    /// 下载插件
    async fn download_plugin(&self, plugin_id: &str, version: &str) -> WasmResult<PluginPackage>;

    /// 发布插件
    async fn publish_plugin(&self, package: PluginPackage, auth: AuthToken) -> WasmResult<PublishResult>;

    /// 更新插件
    async fn update_plugin(&self, plugin_id: &str, package: PluginPackage, auth: AuthToken) -> WasmResult<UpdateResult>;

    /// 删除插件版本
    async fn delete_plugin_version(&self, plugin_id: &str, version: &str, auth: AuthToken) -> WasmResult<()>;

    /// 获取用户插件
    async fn get_user_plugins(&self, user_id: &str, auth: AuthToken) -> WasmResult<Vec<PluginSummary>>;

    /// 提交评论和评分
    async fn submit_review(&self, plugin_id: &str, review: review::PluginReview, auth: AuthToken) -> WasmResult<()>;

    /// 获取插件评论
    async fn get_plugin_reviews(&self, plugin_id: &str, pagination: Pagination) -> WasmResult<Vec<review::PluginReview>>;

    /// 推荐插件
    async fn recommend_plugins(&self, user_id: &str, context: RecommendationContext) -> WasmResult<Vec<PluginRecommendation>>;
}

/// DataFlare插件市场实现
pub struct DataFlareMarketplace {
    /// 插件注册表
    registry: Arc<RwLock<PluginRegistry>>,
    /// 搜索引擎
    search_engine: Arc<PluginSearchEngine>,
    /// 包管理器
    package_manager: Arc<RwLock<PluginPackageManager>>,
    /// 安全扫描器
    security_scanner: Arc<PluginSecurityScanner>,
    /// 质量评分器
    quality_scorer: Arc<PluginQualityScorer>,
    /// 推荐引擎
    recommendation_engine: Arc<PluginRecommendationEngine>,
    /// 存储后端
    storage: Arc<dyn MarketplaceStorage>,
    /// 认证服务
    auth_service: Arc<RwLock<AuthService>>,
    /// 评论服务
    review_service: Arc<RwLock<ReviewService>>,
    /// 许可证管理器
    license_manager: Arc<RwLock<LicenseManager>>,
    /// 支付服务
    payment_service: Arc<RwLock<PaymentService>>,
    /// 配置
    config: MarketplaceConfig,
}

/// 市场配置
#[derive(Debug, Clone)]
pub struct MarketplaceConfig {
    /// 存储配置
    pub storage_config: StorageConfig,
    /// 安全配置
    pub security_config: SecurityConfig,
    /// 搜索配置
    pub search_config: SearchConfig,
    /// 推荐配置
    pub recommendation_config: RecommendationConfig,
    /// API配置
    pub api_config: ApiConfig,
}

impl Default for MarketplaceConfig {
    fn default() -> Self {
        Self {
            storage_config: StorageConfig::default(),
            security_config: SecurityConfig::default(),
            search_config: SearchConfig::default(),
            recommendation_config: RecommendationConfig::default(),
            api_config: ApiConfig::default(),
        }
    }
}

impl DataFlareMarketplace {
    /// 创建新的市场实例
    pub async fn new(config: MarketplaceConfig) -> WasmResult<Self> {
        // 创建存储后端
        let storage = storage::create_storage(&config.storage_config).await?;

        // 创建注册表
        let registry = Arc::new(RwLock::new(
            PluginRegistry::new(storage.clone()).await?
        ));

        // 创建搜索引擎
        let search_engine = Arc::new(
            PluginSearchEngine::new(config.search_config.clone()).await?
        );

        // 创建包管理器
        let package_manager = Arc::new(RwLock::new(
            PluginPackageManager::new(storage.clone(), config.storage_config.clone()).await?
        ));

        // 创建安全扫描器
        let security_scanner = Arc::new(
            PluginSecurityScanner::new(config.security_config.clone()).await?
        );

        // 创建质量评分器
        let quality_scorer = Arc::new(
            PluginQualityScorer::new().await?
        );

        // 创建推荐引擎
        let recommendation_engine = Arc::new(
            PluginRecommendationEngine::new(config.recommendation_config.clone()).await?
        );

        // 创建认证服务
        let auth_service = Arc::new(RwLock::new(
            AuthService::new("your-jwt-secret-key") // TODO: 从配置中获取
        ));

        // 创建评论服务
        let review_service = Arc::new(RwLock::new(
            ReviewService::new()
        ));

        // 创建许可证管理器
        let license_manager = Arc::new(RwLock::new(
            LicenseManager::new()
        ));

        // 创建支付服务
        let payment_service = Arc::new(RwLock::new(
            PaymentService::new()
        ));

        Ok(Self {
            registry,
            search_engine,
            package_manager,
            security_scanner,
            quality_scorer,
            recommendation_engine,
            storage,
            auth_service,
            review_service,
            license_manager,
            payment_service,
            config,
        })
    }

    /// 初始化市场
    pub async fn initialize(&self) -> WasmResult<()> {
        log::info!("初始化DataFlare插件市场");

        // 初始化存储
        self.storage.initialize().await?;

        // 初始化注册表
        self.registry.write().await.initialize().await?;

        // 初始化搜索引擎
        self.search_engine.initialize().await?;

        // 初始化推荐引擎
        self.recommendation_engine.initialize().await?;

        log::info!("DataFlare插件市场初始化完成");
        Ok(())
    }

    /// 获取市场统计信息
    pub async fn get_marketplace_stats(&self) -> WasmResult<MarketplaceStats> {
        let registry = self.registry.read().await;
        let stats = registry.get_stats().await?;

        Ok(MarketplaceStats {
            total_plugins: stats.total_plugins,
            total_downloads: stats.total_downloads,
            active_users: stats.active_users,
            new_plugins_this_month: stats.new_plugins_this_month,
            top_categories: stats.top_categories.clone(),
            average_rating: stats.average_rating,
        })
    }

    /// 验证插件包
    async fn validate_plugin_package(&self, package: &PluginPackage) -> WasmResult<ValidationResult> {
        log::debug!("验证插件包: {}", package.metadata.name);

        let mut validation_result = ValidationResult::new();

        // 安全扫描
        let security_result = self.security_scanner.scan_plugin(package).await?;
        validation_result.security_findings = security_result.findings;
        validation_result.security_rating = security_result.security_rating;

        // 质量评分
        let quality_score = self.quality_scorer.calculate_quality_score(package).await?;
        validation_result.quality_score = quality_score.overall_score;
        validation_result.quality_recommendations = quality_score.recommendations;

        // 基本验证
        if package.metadata.name.is_empty() {
            validation_result.errors.push("插件名称不能为空".to_string());
        }

        if package.metadata.description.is_empty() {
            validation_result.errors.push("插件描述不能为空".to_string());
        }

        if package.wasm_binary.is_empty() {
            validation_result.errors.push("WASM二进制文件不能为空".to_string());
        }

        // 检查版本格式
        if !is_valid_semver(&package.version.version) {
            validation_result.errors.push("版本号格式无效".to_string());
        }

        validation_result.is_valid = validation_result.errors.is_empty()
            && validation_result.security_rating != SecurityRating::Critical;

        Ok(validation_result)
    }

    /// 用户注册
    pub async fn register_user(
        &self,
        username: String,
        email: String,
        password: String,
        display_name: String,
    ) -> WasmResult<User> {
        let mut auth_service = self.auth_service.write().await;
        auth_service.register_user(username, email, password, display_name).await
    }

    /// 用户登录
    pub async fn login_user(
        &self,
        username: String,
        password: String,
        ip_address: String,
        user_agent: String,
    ) -> WasmResult<(String, User)> {
        let mut auth_service = self.auth_service.write().await;
        auth_service.login(username, password, ip_address, user_agent).await
    }

    /// 提交插件评论
    pub async fn submit_plugin_review(
        &self,
        plugin_id: String,
        user_id: String,
        username: String,
        rating: u8,
        title: String,
        content: String,
        plugin_version: Option<String>,
        tags: Vec<review::ReviewTag>,
    ) -> WasmResult<PluginReview> {
        let mut review_service = self.review_service.write().await;
        review_service.submit_review(
            plugin_id, user_id, username, rating, title, content, plugin_version, tags
        ).await
    }

    /// 获取插件评论
    pub async fn get_plugin_reviews_with_filter(
        &self,
        plugin_id: &str,
        filter: Option<ReviewFilter>,
        page: u32,
        page_size: u32,
    ) -> WasmResult<(Vec<PluginReview>, u32)> {
        let review_service = self.review_service.read().await;
        review_service.get_plugin_reviews(plugin_id, filter, page, page_size).await
    }

    /// 激活插件许可证
    pub async fn activate_plugin_license(
        &self,
        plugin_id: String,
        user_id: String,
        license_type: license::LicenseType,
    ) -> WasmResult<LicenseInstance> {
        let mut license_manager = self.license_manager.write().await;
        license_manager.activate_license(plugin_id, user_id, license_type).await
    }

    /// 验证插件许可证
    pub async fn validate_plugin_license(
        &self,
        plugin_id: &str,
        user_id: &str,
        usage_request: &license::UsageRequest,
    ) -> WasmResult<LicenseValidation> {
        let license_manager = self.license_manager.read().await;
        license_manager.validate_license(plugin_id, user_id, usage_request).await
    }

    /// 处理插件支付
    pub async fn process_plugin_payment(
        &self,
        user_id: String,
        plugin_id: String,
        amount_cents: u64,
        currency: String,
        payment_method: payment::PaymentMethod,
    ) -> WasmResult<PaymentRecord> {
        let mut payment_service = self.payment_service.write().await;
        payment_service.process_payment(user_id, plugin_id, amount_cents, currency, payment_method).await
    }

    /// 创建插件订阅
    pub async fn create_plugin_subscription(
        &self,
        user_id: String,
        plugin_id: String,
        plan_id: String,
        payment_method: payment::PaymentMethod,
    ) -> WasmResult<SubscriptionRecord> {
        let mut payment_service = self.payment_service.write().await;
        payment_service.create_subscription(user_id, plugin_id, plan_id, payment_method).await
    }
}

/// 验证结果
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// 是否有效
    pub is_valid: bool,
    /// 错误信息
    pub errors: Vec<String>,
    /// 警告信息
    pub warnings: Vec<String>,
    /// 安全发现
    pub security_findings: Vec<SecurityFinding>,
    /// 安全评级
    pub security_rating: SecurityRating,
    /// 质量评分
    pub quality_score: f64,
    /// 质量建议
    pub quality_recommendations: Vec<QualityRecommendation>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            security_findings: Vec::new(),
            security_rating: SecurityRating::Safe,
            quality_score: 0.0,
            quality_recommendations: Vec::new(),
        }
    }
}

/// 市场统计信息
#[derive(Debug, Clone)]
pub struct MarketplaceStats {
    /// 插件总数
    pub total_plugins: u64,
    /// 总下载量
    pub total_downloads: u64,
    /// 活跃用户数
    pub active_users: u64,
    /// 本月新增插件数
    pub new_plugins_this_month: u64,
    /// 热门分类
    pub top_categories: Vec<(String, u64)>,
    /// 平均评分
    pub average_rating: f64,
}

#[async_trait]
impl PluginMarketplace for DataFlareMarketplace {
    async fn search_plugins(&self, query: SearchQuery) -> WasmResult<SearchResult> {
        // 获取所有插件
        let registry = self.registry.read().await;
        let plugins = registry.list_plugins();

        // 使用搜索引擎进行搜索
        let plugin_entries: Vec<registry::PluginRegistryEntry> = Vec::new(); // 简化实现
        self.search_engine.search(&query, &plugin_entries).await
    }

    async fn get_plugin_details(&self, plugin_id: &str) -> WasmResult<PluginDetails> {
        let registry = self.registry.read().await;
        if let Some(entry) = registry.get_plugin(plugin_id).await? {
            Ok(PluginDetails {
                metadata: entry.metadata,
                latest_version: entry.versions.last()
                    .map(|v| v.version.clone())
                    .unwrap_or_else(|| "0.0.0".to_string()),
                versions: entry.versions,
                stats: entry.stats,
                quality_score: entry.quality_score,
                security_rating: entry.security_rating,
                license: entry.license,
                dependencies: entry.dependencies,
                readme: None, // TODO: 从存储获取
                changelog: None, // TODO: 从存储获取
            })
        } else {
            Err(WasmError::plugin_execution(format!("插件不存在: {}", plugin_id)))
        }
    }

    async fn get_plugin_versions(&self, plugin_id: &str) -> WasmResult<Vec<PluginVersion>> {
        let registry = self.registry.read().await;
        if let Some(entry) = registry.get_plugin(plugin_id).await? {
            Ok(entry.versions)
        } else {
            Err(WasmError::plugin_execution(format!("插件不存在: {}", plugin_id)))
        }
    }

    async fn download_plugin(&self, plugin_id: &str, version: &str) -> WasmResult<PluginPackage> {
        // 从存储获取插件包
        if let Some(package) = self.storage.get_plugin_package(plugin_id, version).await? {
            // 更新下载统计
            self.storage.increment_download_count(plugin_id).await?;
            Ok(package)
        } else {
            Err(WasmError::plugin_execution(format!("插件包不存在: {}:{}", plugin_id, version)))
        }
    }

    async fn publish_plugin(&self, package: PluginPackage, auth: AuthToken) -> WasmResult<PublishResult> {
        // 验证插件包
        let validation_result = self.validate_plugin_package(&package).await?;
        if !validation_result.is_valid {
            return Err(WasmError::plugin_execution(format!("插件验证失败: {:?}", validation_result.errors)));
        }

        // 创建注册表条目
        let entry = registry::PluginRegistryEntry {
            metadata: package.metadata.clone(),
            versions: vec![package.version.clone()],
            stats: PluginStats {
                download_count: 0,
                active_installs: 0,
                rating: 0.0,
                review_count: 0,
                last_updated: chrono::Utc::now(),
                downloads_last_30_days: 0,
            },
            quality_score: validation_result.quality_score,
            security_rating: validation_result.security_rating,
            license: LicenseInfo {
                license_type: "MIT".to_string(), // TODO: 从包中提取
                license_text: package.license_file.clone(),
                is_open_source: true,
                allows_commercial_use: true,
            },
            dependencies: Vec::new(), // TODO: 从包中提取
            status: PublishStatus::Published,
        };

        // 注册插件
        let mut registry = self.registry.write().await;
        registry.register_plugin(entry).await?;

        // 保存插件包
        let plugin_id = package.metadata.id.clone();
        let version = package.version.version.clone();
        self.storage.save_plugin_package(&plugin_id, &version, &package).await?;

        Ok(PublishResult {
            plugin_id: plugin_id.clone(),
            version: version.clone(),
            published_at: chrono::Utc::now(),
            download_url: format!("/api/plugins/{}/versions/{}/download", plugin_id, version),
        })
    }

    async fn update_plugin(&self, plugin_id: &str, package: PluginPackage, auth: AuthToken) -> WasmResult<UpdateResult> {
        // 验证插件包
        let validation_result = self.validate_plugin_package(&package).await?;
        if !validation_result.is_valid {
            return Err(WasmError::plugin_execution(format!("插件验证失败: {:?}", validation_result.errors)));
        }

        // 更新注册表
        let mut registry = self.registry.write().await;
        if let Some(mut entry) = registry.get_plugin(plugin_id).await? {
            entry.versions.push(package.version.clone());
            entry.metadata.updated_at = chrono::Utc::now();
            entry.quality_score = validation_result.quality_score;
            entry.security_rating = validation_result.security_rating;

            registry.update_plugin(plugin_id, entry).await?;
        } else {
            return Err(WasmError::plugin_execution(format!("插件不存在: {}", plugin_id)));
        }

        // 保存插件包
        self.storage.save_plugin_package(plugin_id, &package.version.version, &package).await?;

        Ok(UpdateResult {
            plugin_id: plugin_id.to_string(),
            new_version: package.version.version,
            updated_at: chrono::Utc::now(),
        })
    }

    async fn delete_plugin_version(&self, plugin_id: &str, version: &str, auth: AuthToken) -> WasmResult<()> {
        // 从存储删除插件包
        self.storage.delete_plugin_package(plugin_id, version).await?;

        // 更新注册表（移除版本）
        let mut registry = self.registry.write().await;
        if let Some(mut entry) = registry.get_plugin(plugin_id).await? {
            entry.versions.retain(|v| v.version != version);

            if entry.versions.is_empty() {
                // 如果没有版本了，删除整个插件
                registry.delete_plugin(plugin_id).await?;
            } else {
                registry.update_plugin(plugin_id, entry).await?;
            }
        }

        Ok(())
    }

    async fn get_user_plugins(&self, user_id: &str, auth: AuthToken) -> WasmResult<Vec<PluginSummary>> {
        let plugin_ids = self.storage.get_user_plugins(user_id).await?;
        let mut summaries = Vec::new();

        let registry = self.registry.read().await;
        for plugin_id in plugin_ids {
            if let Some(entry) = registry.get_plugin(&plugin_id).await? {
                summaries.push(PluginSummary {
                    id: entry.metadata.id,
                    name: entry.metadata.name,
                    description: entry.metadata.description,
                    author: entry.metadata.author,
                    latest_version: entry.versions.last()
                        .map(|v| v.version.clone())
                        .unwrap_or_else(|| "0.0.0".to_string()),
                    download_count: entry.stats.download_count,
                    rating: entry.stats.rating,
                    updated_at: entry.metadata.updated_at,
                    keywords: entry.metadata.keywords,
                    security_rating: entry.security_rating,
                });
            }
        }

        Ok(summaries)
    }

    async fn submit_review(&self, plugin_id: &str, review: review::PluginReview, auth: AuthToken) -> WasmResult<()> {
        // 使用新的评论服务
        let mut review_service = self.review_service.write().await;
        review_service.submit_review(
            review.plugin_id.clone(),
            review.user_id.clone(),
            review.username.clone(),
            review.rating,
            review.title.clone(),
            review.content.clone(),
            review.plugin_version.clone(),
            review.tags.clone(),
        ).await?;

        Ok(())
    }

    async fn get_plugin_reviews(&self, plugin_id: &str, pagination: Pagination) -> WasmResult<Vec<review::PluginReview>> {
        let review_service = self.review_service.read().await;
        let (reviews, _total) = review_service.get_plugin_reviews(
            plugin_id,
            None,
            pagination.page,
            pagination.page_size,
        ).await?;
        Ok(reviews)
    }

    async fn recommend_plugins(&self, user_id: &str, context: RecommendationContext) -> WasmResult<Vec<PluginRecommendation>> {
        self.recommendation_engine.recommend_plugins(user_id, context).await
    }
}

/// 验证语义化版本号
fn is_valid_semver(version: &str) -> bool {
    // 简单的语义化版本验证
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }

    for part in parts {
        if part.parse::<u32>().is_err() {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::license::{LicenseType, PricingModel, UsageLimits, UsageRequest};
    use crate::marketplace::payment::{PaymentMethod, CardType, BillingCycle, SubscriptionPlan};

    #[test]
    fn test_semver_validation() {
        assert!(is_valid_semver("1.0.0"));
        assert!(is_valid_semver("0.1.0"));
        assert!(is_valid_semver("10.20.30"));
        assert!(!is_valid_semver("1.0"));
        assert!(!is_valid_semver("1.0.0.0"));
        assert!(!is_valid_semver("1.0.a"));
    }

    #[tokio::test]
    async fn test_license_management() {
        let mut license_manager = LicenseManager::new();

        // 测试激活商业许可证
        let license_type = LicenseType::Commercial {
            name: "Professional".to_string(),
            pricing_model: PricingModel::Subscription {
                monthly_price_cents: 2999,
                yearly_price_cents: Some(29999),
                currency: "USD".to_string(),
            },
            usage_limits: UsageLimits {
                max_users: Some(10),
                max_data_mb: Some(1000),
                max_api_calls: Some(10000),
                geographic_restrictions: vec![],
                industry_restrictions: vec![],
            },
        };

        let license = license_manager.activate_license(
            "test-plugin".to_string(),
            "user-123".to_string(),
            license_type,
        ).await.unwrap();

        assert_eq!(license.plugin_id, "test-plugin");
        assert_eq!(license.user_id, "user-123");
        assert!(license.license_key.starts_with("DF-"));

        // 测试许可证验证
        let usage_request = UsageRequest {
            data_mb: 100,
            api_calls: 1000,
            users: 5,
        };

        let validation = license_manager.validate_license(
            "test-plugin",
            "user-123",
            &usage_request,
        ).await.unwrap();

        assert!(validation.is_valid);
        assert_eq!(validation.message, "许可证有效");
    }

    #[tokio::test]
    async fn test_payment_processing() {
        let mut payment_service = PaymentService::new();

        // 测试一次性支付
        let payment_method = PaymentMethod::CreditCard {
            card_number_encrypted: "encrypted_card_number".to_string(),
            cardholder_name: "John Doe".to_string(),
            expiry_month: 12,
            expiry_year: 2025,
            card_type: CardType::Visa,
        };

        let payment = payment_service.process_payment(
            "user-123".to_string(),
            "test-plugin".to_string(),
            2999, // $29.99
            "USD".to_string(),
            payment_method.clone(),
        ).await.unwrap();

        assert_eq!(payment.user_id, "user-123");
        assert_eq!(payment.plugin_id, "test-plugin");
        assert_eq!(payment.amount_cents, 2999);
        assert_eq!(payment.currency, "USD");

        // 测试订阅创建
        let subscription_plan = SubscriptionPlan {
            id: "pro-monthly".to_string(),
            name: "Professional Monthly".to_string(),
            monthly_price_cents: 2999,
            yearly_price_cents: Some(29999),
            billing_cycle: BillingCycle::Monthly,
            features: vec!["Advanced Analytics".to_string(), "Priority Support".to_string()],
            usage_limits: UsageLimits {
                max_users: Some(10),
                max_data_mb: Some(1000),
                max_api_calls: Some(10000),
                geographic_restrictions: vec![],
                industry_restrictions: vec![],
            },
        };

        payment_service.add_subscription_plan(subscription_plan).await.unwrap();

        let subscription = payment_service.create_subscription(
            "user-123".to_string(),
            "test-plugin".to_string(),
            "pro-monthly".to_string(),
            payment_method,
        ).await.unwrap();

        assert_eq!(subscription.user_id, "user-123");
        assert_eq!(subscription.plugin_id, "test-plugin");
        assert_eq!(subscription.plan.id, "pro-monthly");
    }

    #[tokio::test]
    async fn test_revenue_sharing() {
        let mut payment_service = PaymentService::new();

        // 模拟一些支付记录
        let payment_method = PaymentMethod::CreditCard {
            card_number_encrypted: "encrypted_card_number".to_string(),
            cardholder_name: "John Doe".to_string(),
            expiry_month: 12,
            expiry_year: 2025,
            card_type: CardType::Visa,
        };

        // 创建多个支付记录
        for i in 0..5 {
            payment_service.process_payment(
                format!("user-{}", i),
                "test-plugin".to_string(),
                1000, // $10.00
                "USD".to_string(),
                payment_method.clone(),
            ).await.unwrap();
        }

        // 计算收入分成
        let period_start = chrono::Utc::now() - chrono::Duration::hours(1);
        let period_end = chrono::Utc::now();

        let revenue_share = payment_service.calculate_revenue_share(
            "developer-123".to_string(),
            "test-plugin".to_string(),
            period_start,
            period_end,
            70.0, // 70% 给开发者
        ).await.unwrap();

        assert_eq!(revenue_share.total_revenue_cents, 5000); // 5 * $10.00
        assert_eq!(revenue_share.developer_revenue_cents, 3500); // 70% of $50.00
        assert_eq!(revenue_share.platform_fee_cents, 1500); // 30% of $50.00
        assert_eq!(revenue_share.revenue_share_percentage, 70.0);
    }
}
