//! 插件市场第二阶段功能测试

#[cfg(test)]
mod tests {
    use crate::marketplace::*;
    use crate::marketplace::package_manager::PluginSpec;
    use chrono::Utc;

    /// 测试用户认证系统
    #[tokio::test]
    async fn test_auth_system() {
        let mut auth_service = AuthService::new("test-secret-key");

        // 测试用户注册
        let user = auth_service.register_user(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
            "Test User".to_string(),
        ).await.unwrap();

        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.role, UserRole::User);

        // 测试用户登录
        let (token, logged_in_user) = auth_service.login(
            "testuser".to_string(),
            "password123".to_string(),
            "127.0.0.1".to_string(),
            "test-agent".to_string(),
        ).await.unwrap();

        assert!(!token.is_empty());
        assert_eq!(logged_in_user.username, "testuser");

        // 测试JWT验证
        let claims = auth_service.verify_jwt_token(&token).unwrap();
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.role, UserRole::User);

        // 测试API密钥创建
        let (api_key, key_info) = auth_service.create_api_key(
            user.id.clone(),
            "Test API Key".to_string(),
            vec![Permission::ReadPlugins, Permission::PublishPlugins],
            None,
        ).await.unwrap();

        assert!(!api_key.is_empty());
        assert_eq!(key_info.name, "Test API Key");
        assert!(key_info.scopes.contains(&Permission::ReadPlugins));

        // 测试API密钥验证
        let (verified_user, permissions) = auth_service.verify_api_key(&api_key).await.unwrap();
        assert_eq!(verified_user.id, user.id);
        assert!(permissions.contains(&Permission::ReadPlugins));
    }

    /// 测试评论系统
    #[tokio::test]
    async fn test_review_system() {
        let mut review_service = ReviewService::new();

        // 测试提交评论
        let review = review_service.submit_review(
            "test-plugin".to_string(),
            "user-1".to_string(),
            "testuser".to_string(),
            5,
            "Great plugin!".to_string(),
            "This plugin works perfectly for my needs.".to_string(),
            Some("1.0.0".to_string()),
            vec![review::ReviewTag::Performance, review::ReviewTag::Usability],
        ).await.unwrap();

        assert_eq!(review.plugin_id, "test-plugin");
        assert_eq!(review.rating, 5);
        assert_eq!(review.title, "Great plugin!");

        // 测试获取评论
        let (reviews, total) = review_service.get_plugin_reviews(
            "test-plugin",
            None,
            0,
            10,
        ).await.unwrap();

        assert_eq!(reviews.len(), 1);
        assert_eq!(total, 1);
        assert_eq!(reviews[0].id, review.id);

        // 测试评论投票
        review_service.vote_review(
            review.id.clone(),
            "user-2".to_string(),
            review::VoteType::Helpful,
        ).await.unwrap();

        let (updated_reviews, _) = review_service.get_plugin_reviews(
            "test-plugin",
            None,
            0,
            10,
        ).await.unwrap();

        assert_eq!(updated_reviews[0].helpful_count, 1);

        // 测试评论统计
        let stats = review_service.get_plugin_review_stats("test-plugin").await.unwrap();
        assert_eq!(stats.total_reviews, 1);
        assert_eq!(stats.average_rating, 5.0);
    }

    /// 测试许可证管理
    #[tokio::test]
    async fn test_license_management() {
        let mut license_manager = LicenseManager::new();

        // 测试激活商业许可证
        let commercial_license = license::LicenseType::Commercial {
            name: "Pro License".to_string(),
            pricing_model: license::PricingModel::Subscription {
                monthly_price_cents: 2999,
                yearly_price_cents: Some(29999),
                currency: "USD".to_string(),
            },
            usage_limits: license::UsageLimits {
                max_users: Some(10),
                max_data_mb: Some(1000),
                max_api_calls: Some(10000),
                geographic_restrictions: vec![],
                industry_restrictions: vec![],
            },
        };

        let license = license_manager.activate_license(
            "test-plugin".to_string(),
            "user-1".to_string(),
            commercial_license,
        ).await.unwrap();

        assert_eq!(license.plugin_id, "test-plugin");
        assert_eq!(license.user_id, "user-1");
        assert_eq!(license.status, license::LicenseStatus::Active);

        // 测试许可证验证
        let usage_request = license::UsageRequest {
            data_mb: 100,
            api_calls: 500,
            users: 5,
        };

        let validation = license_manager.validate_license(
            "test-plugin",
            "user-1",
            &usage_request,
        ).await.unwrap();

        assert!(validation.is_valid);
        assert_eq!(validation.message, "许可证有效");

        // 测试使用量更新
        let usage_update = license::UsageUpdate {
            data_processed_mb: 100,
            api_calls: 500,
            active_users: 5,
        };

        license_manager.update_usage_stats(
            "test-plugin",
            "user-1",
            &usage_update,
        ).await.unwrap();

        // 测试试用许可证
        let trial_license = license::LicenseType::Trial {
            trial_days: 30,
            feature_limits: vec!["No commercial use".to_string()],
        };

        let trial = license_manager.activate_license(
            "test-plugin-2".to_string(),
            "user-2".to_string(),
            trial_license,
        ).await.unwrap();

        assert_eq!(trial.status, license::LicenseStatus::Active);
        assert!(trial.expires_at.is_some());
    }

    /// 测试支付系统
    #[tokio::test]
    async fn test_payment_system() {
        let mut payment_service = PaymentService::new();

        // 测试一次性支付
        let payment_method = payment::PaymentMethod::CreditCard {
            card_number_encrypted: "encrypted_card_number".to_string(),
            cardholder_name: "John Doe".to_string(),
            expiry_month: 12,
            expiry_year: 2025,
            card_type: payment::CardType::Visa,
        };

        let payment = payment_service.process_payment(
            "user-1".to_string(),
            "test-plugin".to_string(),
            2999, // $29.99
            "USD".to_string(),
            payment_method.clone(),
        ).await.unwrap();

        assert_eq!(payment.amount_cents, 2999);
        assert_eq!(payment.currency, "USD");
        assert_eq!(payment.status, payment::PaymentStatus::Completed);

        // 测试订阅计划
        let plan = payment::SubscriptionPlan {
            id: "pro-plan".to_string(),
            name: "Pro Plan".to_string(),
            monthly_price_cents: 2999,
            yearly_price_cents: Some(29999),
            billing_cycle: payment::BillingCycle::Monthly,
            features: vec!["Unlimited data".to_string(), "Priority support".to_string()],
            usage_limits: license::UsageLimits {
                max_users: Some(10),
                max_data_mb: None,
                max_api_calls: None,
                geographic_restrictions: vec![],
                industry_restrictions: vec![],
            },
        };

        payment_service.add_subscription_plan(plan).await.unwrap();

        // 测试创建订阅
        let subscription = payment_service.create_subscription(
            "user-1".to_string(),
            "test-plugin".to_string(),
            "pro-plan".to_string(),
            payment_method,
        ).await.unwrap();

        assert_eq!(subscription.status, payment::SubscriptionStatus::Active);
        assert_eq!(subscription.plan.id, "pro-plan");

        // 测试退款
        payment_service.process_refund(
            &payment.id,
            1000, // $10.00 partial refund
            "Customer request".to_string(),
        ).await.unwrap();

        // 测试收入分成计算
        let revenue_share = payment_service.calculate_revenue_share(
            "developer-1".to_string(),
            "test-plugin".to_string(),
            Utc::now() - chrono::Duration::days(30),
            Utc::now(),
            70.0, // 70% to developer
        ).await.unwrap();

        assert_eq!(revenue_share.revenue_share_percentage, 70.0);
        assert!(revenue_share.developer_revenue_cents > 0);
    }

    /// 测试包管理器增强功能
    #[tokio::test]
    async fn test_package_manager_enhancements() {
        use crate::marketplace::storage::MemoryStorage;
        use std::sync::Arc;

        let storage = Arc::new(MemoryStorage::new());
        let config = StorageConfig::default();
        let mut manager = PluginPackageManager::new(storage, config).await.unwrap();

        // 测试批量安装
        let specs = vec![
            PluginSpec::new("plugin-1", "1.0.0"),
            PluginSpec::new("plugin-2", "1.0.0"),
        ];

        let results = manager.batch_install(specs).await.unwrap();
        assert_eq!(results.len(), 2);

        // 测试依赖树
        let tree = manager.get_dependency_tree("plugin-1").await.unwrap();
        assert_eq!(tree.root, "plugin-1");

        // 测试插件完整性验证
        // 注意：这个测试会失败，因为插件实际上没有安装文件
        let integrity = manager.verify_plugin_integrity("plugin-1").await.unwrap();
        assert!(!integrity); // 应该返回false，因为文件不存在
    }

    /// 测试集成的市场功能
    #[tokio::test]
    async fn test_integrated_marketplace() {


        let config = MarketplaceConfig {
            storage_config: StorageConfig::default(),
            security_config: SecurityConfig::default(),
            search_config: SearchConfig::default(),
            recommendation_config: RecommendationConfig::default(),
            api_config: ApiConfig::default(),
        };

        let marketplace = DataFlareMarketplace::new(config).await.unwrap();

        // 测试用户注册
        let user = marketplace.register_user(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
            "Test User".to_string(),
        ).await.unwrap();

        // 测试许可证激活
        let license_type = license::LicenseType::Trial {
            trial_days: 30,
            feature_limits: vec![],
        };

        let license = marketplace.activate_plugin_license(
            "test-plugin".to_string(),
            user.id.clone(),
            license_type,
        ).await.unwrap();

        assert_eq!(license.plugin_id, "test-plugin");
        assert_eq!(license.user_id, user.id);

        // 测试评论提交
        let review = marketplace.submit_plugin_review(
            "test-plugin".to_string(),
            user.id.clone(),
            user.username.clone(),
            5,
            "Excellent!".to_string(),
            "This plugin is amazing!".to_string(),
            Some("1.0.0".to_string()),
            vec![review::ReviewTag::Performance],
        ).await.unwrap();

        assert_eq!(review.rating, 5);
        assert_eq!(review.user_id, user.id);
    }
}
