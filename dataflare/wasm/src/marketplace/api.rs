//! 插件市场API接口

use super::types::{self as types, *};
use super::{DataFlareMarketplace, PluginMarketplace, review};
use crate::{WasmError, WasmResult};
use std::sync::Arc;
use log::{info, debug, warn, error};

/// 市场API接口
pub struct MarketplaceAPI {
    /// 市场实例
    marketplace: Arc<DataFlareMarketplace>,
    /// API配置
    config: ApiConfig,
}

/// API响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiResponse<T> {
    /// 是否成功
    pub success: bool,
    /// 响应数据
    pub data: Option<T>,
    /// 错误信息
    pub error: Option<String>,
    /// 响应时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// API错误
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiError {
    /// 错误代码
    pub code: String,
    /// 错误消息
    pub message: String,
    /// 详细信息
    pub details: Option<String>,
}

impl MarketplaceAPI {
    /// 创建新的API实例
    pub fn new(marketplace: Arc<DataFlareMarketplace>, config: ApiConfig) -> Self {
        Self {
            marketplace,
            config,
        }
    }

    /// 搜索插件
    pub async fn search_plugins(&self, query: SearchQuery) -> ApiResponse<SearchResult> {
        let start_time = std::time::Instant::now();

        match self.marketplace.search_plugins(query).await {
            Ok(result) => {
                info!("搜索完成，找到 {} 个插件", result.plugins.len());
                ApiResponse {
                    success: true,
                    data: Some(result),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                error!("搜索失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 获取插件详情
    pub async fn get_plugin_details(&self, plugin_id: &str) -> ApiResponse<PluginDetails> {
        debug!("获取插件详情: {}", plugin_id);

        match self.marketplace.get_plugin_details(plugin_id).await {
            Ok(details) => {
                ApiResponse {
                    success: true,
                    data: Some(details),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                warn!("获取插件详情失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 获取插件版本列表
    pub async fn get_plugin_versions(&self, plugin_id: &str) -> ApiResponse<Vec<PluginVersion>> {
        debug!("获取插件版本列表: {}", plugin_id);

        match self.marketplace.get_plugin_versions(plugin_id).await {
            Ok(versions) => {
                ApiResponse {
                    success: true,
                    data: Some(versions),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                warn!("获取插件版本列表失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 下载插件
    pub async fn download_plugin(&self, plugin_id: &str, version: &str) -> ApiResponse<PluginPackage> {
        info!("下载插件: {} v{}", plugin_id, version);

        match self.marketplace.download_plugin(plugin_id, version).await {
            Ok(package) => {
                info!("插件下载成功: {} v{}", plugin_id, version);
                ApiResponse {
                    success: true,
                    data: Some(package),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                error!("插件下载失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 发布插件
    pub async fn publish_plugin(&self, package: PluginPackage, auth: AuthToken) -> ApiResponse<PublishResult> {
        info!("发布插件: {}", package.metadata.name);

        // 验证认证令牌
        if let Err(e) = self.validate_auth_token(&auth).await {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(format!("认证失败: {}", e)),
                timestamp: chrono::Utc::now(),
            };
        }

        match self.marketplace.publish_plugin(package, auth).await {
            Ok(result) => {
                info!("插件发布成功: {}", result.plugin_id);
                ApiResponse {
                    success: true,
                    data: Some(result),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                error!("插件发布失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 更新插件
    pub async fn update_plugin(&self, plugin_id: &str, package: PluginPackage, auth: AuthToken) -> ApiResponse<UpdateResult> {
        info!("更新插件: {}", plugin_id);

        // 验证认证令牌
        if let Err(e) = self.validate_auth_token(&auth).await {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(format!("认证失败: {}", e)),
                timestamp: chrono::Utc::now(),
            };
        }

        match self.marketplace.update_plugin(plugin_id, package, auth).await {
            Ok(result) => {
                info!("插件更新成功: {}", plugin_id);
                ApiResponse {
                    success: true,
                    data: Some(result),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                error!("插件更新失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 删除插件版本
    pub async fn delete_plugin_version(&self, plugin_id: &str, version: &str, auth: AuthToken) -> ApiResponse<()> {
        info!("删除插件版本: {} v{}", plugin_id, version);

        // 验证认证令牌
        if let Err(e) = self.validate_auth_token(&auth).await {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(format!("认证失败: {}", e)),
                timestamp: chrono::Utc::now(),
            };
        }

        match self.marketplace.delete_plugin_version(plugin_id, version, auth).await {
            Ok(_) => {
                info!("插件版本删除成功: {} v{}", plugin_id, version);
                ApiResponse {
                    success: true,
                    data: Some(()),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                error!("插件版本删除失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 获取用户插件
    pub async fn get_user_plugins(&self, user_id: &str, auth: AuthToken) -> ApiResponse<Vec<PluginSummary>> {
        debug!("获取用户插件: {}", user_id);

        // 验证认证令牌
        if let Err(e) = self.validate_auth_token(&auth).await {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(format!("认证失败: {}", e)),
                timestamp: chrono::Utc::now(),
            };
        }

        match self.marketplace.get_user_plugins(user_id, auth).await {
            Ok(plugins) => {
                ApiResponse {
                    success: true,
                    data: Some(plugins),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                warn!("获取用户插件失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 提交评论和评分
    pub async fn submit_review(&self, plugin_id: &str, review: PluginReview, auth: AuthToken) -> ApiResponse<()> {
        info!("提交评论: 插件 {}, 评分 {}", plugin_id, review.rating);

        // 验证认证令牌
        if let Err(e) = self.validate_auth_token(&auth).await {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(format!("认证失败: {}", e)),
                timestamp: chrono::Utc::now(),
            };
        }

        // 转换为新的评论类型
        let new_review = review::PluginReview {
            id: uuid::Uuid::new_v4().to_string(),
            plugin_id: plugin_id.to_string(),
            user_id: review.user_id.clone(),
            username: review.username.clone(),
            rating: review.rating,
            title: "评论".to_string(), // 使用默认标题，因为旧类型没有title字段
            content: review.comment.clone(), // 使用comment字段作为content
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            verified_purchase: review.verified_purchase,
            helpful_count: 0,
            unhelpful_count: 0,
            status: review::ReviewStatus::Published,
            plugin_version: None, // 旧类型没有plugin_version字段
            tags: vec![], // TODO: 转换标签
        };

        match self.marketplace.submit_review(plugin_id, new_review, auth).await {
            Ok(_) => {
                info!("评论提交成功");
                ApiResponse {
                    success: true,
                    data: Some(()),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                error!("评论提交失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 获取插件评论
    pub async fn get_plugin_reviews(&self, plugin_id: &str, pagination: Pagination) -> ApiResponse<Vec<PluginReview>> {
        debug!("获取插件评论: {}", plugin_id);

        match self.marketplace.get_plugin_reviews(plugin_id, pagination).await {
            Ok(reviews) => {
                ApiResponse {
                    success: true,
                    data: Some(reviews.into_iter().map(|r| types::PluginReview {
                        id: r.id,
                        user_id: r.user_id,
                        username: r.username,
                        rating: r.rating,
                        comment: r.content, // 将content映射到comment字段
                        created_at: r.created_at,
                        verified_purchase: r.verified_purchase,
                    }).collect()),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                warn!("获取插件评论失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 推荐插件
    pub async fn recommend_plugins(&self, user_id: &str, context: RecommendationContext) -> ApiResponse<Vec<PluginRecommendation>> {
        debug!("推荐插件: 用户 {}", user_id);

        match self.marketplace.recommend_plugins(user_id, context).await {
            Ok(recommendations) => {
                info!("生成了 {} 个推荐", recommendations.len());
                ApiResponse {
                    success: true,
                    data: Some(recommendations),
                    error: None,
                    timestamp: chrono::Utc::now(),
                }
            },
            Err(e) => {
                error!("插件推荐失败: {}", e);
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    timestamp: chrono::Utc::now(),
                }
            }
        }
    }

    /// 验证认证令牌
    async fn validate_auth_token(&self, auth: &AuthToken) -> WasmResult<()> {
        // 简化实现：检查令牌是否过期
        let now = chrono::Utc::now();
        if auth.expires_at < now {
            return Err(WasmError::runtime("认证令牌已过期"));
        }

        // 检查令牌格式
        if auth.token.is_empty() || auth.user_id.is_empty() {
            return Err(WasmError::runtime("无效的认证令牌"));
        }

        Ok(())
    }

    /// 创建成功响应
    pub fn success<T>(data: T) -> ApiResponse<T> {
        ApiResponse {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 创建错误响应
    pub fn error<T>(message: &str) -> ApiResponse<T> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message.to_string()),
            timestamp: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::{MarketplaceConfig, DataFlareMarketplace};

    #[tokio::test]
    async fn test_marketplace_api() {
        let config = MarketplaceConfig::default();
        let marketplace = Arc::new(DataFlareMarketplace::new(config).await.unwrap());
        let api_config = ApiConfig::default();
        let api = MarketplaceAPI::new(marketplace, api_config);

        // 测试搜索插件
        let query = SearchQuery {
            query: Some("test".to_string()),
            categories: vec![],
            tags: vec![],
            author: None,
            license: None,
            min_rating: None,
            sort_by: SortBy::Relevance,
            pagination: Pagination { page: 0, page_size: 10 },
        };

        let response = api.search_plugins(query).await;
        assert!(response.success);
    }
}
