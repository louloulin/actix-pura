//! 评分和评论系统

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::{WasmError, WasmResult};

/// 插件评论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginReview {
    /// 评论ID
    pub id: String,
    /// 插件ID
    pub plugin_id: String,
    /// 用户ID
    pub user_id: String,
    /// 用户名
    pub username: String,
    /// 评分 (1-5)
    pub rating: u8,
    /// 评论标题
    pub title: String,
    /// 评论内容
    pub content: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 是否已验证购买
    pub verified_purchase: bool,
    /// 点赞数
    pub helpful_count: u32,
    /// 点踩数
    pub unhelpful_count: u32,
    /// 评论状态
    pub status: ReviewStatus,
    /// 版本信息
    pub plugin_version: Option<String>,
    /// 标签
    pub tags: Vec<ReviewTag>,
}

/// 评论状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewStatus {
    /// 已发布
    Published,
    /// 待审核
    Pending,
    /// 已隐藏
    Hidden,
    /// 已删除
    Deleted,
    /// 被举报
    Flagged,
}

/// 评论标签
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewTag {
    /// 性能相关
    Performance,
    /// 易用性
    Usability,
    /// 文档质量
    Documentation,
    /// 支持质量
    Support,
    /// 功能完整性
    Features,
    /// 稳定性
    Stability,
    /// 安全性
    Security,
}

/// 评论统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewStats {
    /// 总评论数
    pub total_reviews: u32,
    /// 平均评分
    pub average_rating: f64,
    /// 各评分的数量分布
    pub rating_distribution: HashMap<u8, u32>,
    /// 最近30天评论数
    pub reviews_last_30_days: u32,
    /// 验证购买评论数
    pub verified_reviews: u32,
}

/// 评论过滤器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFilter {
    /// 评分过滤
    pub rating: Option<u8>,
    /// 是否只显示验证购买
    pub verified_only: bool,
    /// 标签过滤
    pub tags: Vec<ReviewTag>,
    /// 时间范围
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    /// 排序方式
    pub sort_by: ReviewSortBy,
}

/// 评论排序方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewSortBy {
    /// 最新
    Newest,
    /// 最旧
    Oldest,
    /// 最有帮助
    MostHelpful,
    /// 评分最高
    HighestRating,
    /// 评分最低
    LowestRating,
}

/// 评论投票
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewVote {
    /// 投票ID
    pub id: String,
    /// 评论ID
    pub review_id: String,
    /// 用户ID
    pub user_id: String,
    /// 投票类型
    pub vote_type: VoteType,
    /// 投票时间
    pub created_at: DateTime<Utc>,
}

/// 投票类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VoteType {
    /// 有帮助
    Helpful,
    /// 无帮助
    Unhelpful,
}

/// 评论举报
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReport {
    /// 举报ID
    pub id: String,
    /// 评论ID
    pub review_id: String,
    /// 举报用户ID
    pub reporter_id: String,
    /// 举报原因
    pub reason: ReportReason,
    /// 举报描述
    pub description: String,
    /// 举报时间
    pub created_at: DateTime<Utc>,
    /// 处理状态
    pub status: ReportStatus,
}

/// 举报原因
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportReason {
    /// 垃圾信息
    Spam,
    /// 不当内容
    Inappropriate,
    /// 虚假信息
    Misinformation,
    /// 恶意攻击
    Harassment,
    /// 版权侵犯
    Copyright,
    /// 其他
    Other,
}

/// 举报处理状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportStatus {
    /// 待处理
    Pending,
    /// 已处理
    Resolved,
    /// 已忽略
    Dismissed,
}

/// 评论服务
pub struct ReviewService {
    /// 评论存储
    reviews: HashMap<String, PluginReview>,
    /// 投票存储
    votes: HashMap<String, ReviewVote>,
    /// 举报存储
    reports: HashMap<String, ReviewReport>,
    /// 插件评论统计
    plugin_stats: HashMap<String, ReviewStats>,
}

impl ReviewService {
    /// 创建新的评论服务
    pub fn new() -> Self {
        Self {
            reviews: HashMap::new(),
            votes: HashMap::new(),
            reports: HashMap::new(),
            plugin_stats: HashMap::new(),
        }
    }

    /// 提交评论
    pub async fn submit_review(
        &mut self,
        plugin_id: String,
        user_id: String,
        username: String,
        rating: u8,
        title: String,
        content: String,
        plugin_version: Option<String>,
        tags: Vec<ReviewTag>,
    ) -> WasmResult<PluginReview> {
        // 验证评分范围
        if rating < 1 || rating > 5 {
            return Err(WasmError::plugin_execution("评分必须在1-5之间".to_string()));
        }

        // 检查用户是否已经评论过该插件
        let existing_review = self.reviews.values()
            .find(|r| r.plugin_id == plugin_id && r.user_id == user_id);

        if existing_review.is_some() {
            return Err(WasmError::plugin_execution("您已经评论过该插件".to_string()));
        }

        // 创建评论
        let review = PluginReview {
            id: Uuid::new_v4().to_string(),
            plugin_id: plugin_id.clone(),
            user_id,
            username,
            rating,
            title,
            content,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            verified_purchase: false, // TODO: 实际验证购买状态
            helpful_count: 0,
            unhelpful_count: 0,
            status: ReviewStatus::Published,
            plugin_version,
            tags,
        };

        // 存储评论
        self.reviews.insert(review.id.clone(), review.clone());

        // 更新插件统计
        self.update_plugin_stats(&plugin_id).await?;

        Ok(review)
    }

    /// 获取插件评论
    pub async fn get_plugin_reviews(
        &self,
        plugin_id: &str,
        filter: Option<ReviewFilter>,
        page: u32,
        page_size: u32,
    ) -> WasmResult<(Vec<PluginReview>, u32)> {
        let mut reviews: Vec<PluginReview> = self.reviews.values()
            .filter(|r| r.plugin_id == plugin_id && r.status == ReviewStatus::Published)
            .cloned()
            .collect();

        // 应用过滤器和排序
        let sort_by = if let Some(ref filter) = filter {
            reviews = self.apply_review_filter(reviews, filter);
            &filter.sort_by
        } else {
            &ReviewSortBy::Newest
        };

        self.sort_reviews(&mut reviews, sort_by);

        // 分页
        let total_count = reviews.len() as u32;
        let start = (page * page_size) as usize;
        let end = std::cmp::min(start + page_size as usize, reviews.len());

        let page_reviews = if start < reviews.len() {
            reviews[start..end].to_vec()
        } else {
            Vec::new()
        };

        Ok((page_reviews, total_count))
    }

    /// 投票评论
    pub async fn vote_review(
        &mut self,
        review_id: String,
        user_id: String,
        vote_type: VoteType,
    ) -> WasmResult<()> {
        // 检查评论是否存在
        let review = self.reviews.get_mut(&review_id)
            .ok_or_else(|| WasmError::plugin_execution("评论不存在".to_string()))?;

        // 检查用户是否已经投票
        let existing_vote_info = self.votes.values()
            .find(|v| v.review_id == review_id && v.user_id == user_id)
            .map(|v| (v.id.clone(), v.vote_type.clone()));

        if let Some((existing_vote_id, existing_vote_type)) = existing_vote_info {
            // 如果投票类型相同，取消投票
            if existing_vote_type == vote_type {
                self.votes.remove(&existing_vote_id);
                match vote_type {
                    VoteType::Helpful => review.helpful_count = review.helpful_count.saturating_sub(1),
                    VoteType::Unhelpful => review.unhelpful_count = review.unhelpful_count.saturating_sub(1),
                }
                return Ok(());
            } else {
                // 更改投票类型
                self.votes.remove(&existing_vote_id);
                match existing_vote_type {
                    VoteType::Helpful => review.helpful_count = review.helpful_count.saturating_sub(1),
                    VoteType::Unhelpful => review.unhelpful_count = review.unhelpful_count.saturating_sub(1),
                }
            }
        }

        // 创建新投票
        let vote = ReviewVote {
            id: Uuid::new_v4().to_string(),
            review_id,
            user_id,
            vote_type: vote_type.clone(),
            created_at: Utc::now(),
        };

        self.votes.insert(vote.id.clone(), vote);

        // 更新评论计数
        match vote_type {
            VoteType::Helpful => review.helpful_count += 1,
            VoteType::Unhelpful => review.unhelpful_count += 1,
        }

        Ok(())
    }

    /// 举报评论
    pub async fn report_review(
        &mut self,
        review_id: String,
        reporter_id: String,
        reason: ReportReason,
        description: String,
    ) -> WasmResult<ReviewReport> {
        // 检查评论是否存在
        if !self.reviews.contains_key(&review_id) {
            return Err(WasmError::plugin_execution("评论不存在".to_string()));
        }

        // 创建举报
        let report = ReviewReport {
            id: Uuid::new_v4().to_string(),
            review_id,
            reporter_id,
            reason,
            description,
            created_at: Utc::now(),
            status: ReportStatus::Pending,
        };

        self.reports.insert(report.id.clone(), report.clone());

        Ok(report)
    }

    /// 获取插件评论统计
    pub async fn get_plugin_review_stats(&self, plugin_id: &str) -> WasmResult<ReviewStats> {
        if let Some(stats) = self.plugin_stats.get(plugin_id) {
            Ok(stats.clone())
        } else {
            // 如果没有缓存统计，计算并返回
            self.calculate_plugin_stats(plugin_id)
        }
    }

    /// 更新插件统计
    async fn update_plugin_stats(&mut self, plugin_id: &str) -> WasmResult<()> {
        let stats = self.calculate_plugin_stats(plugin_id)?;
        self.plugin_stats.insert(plugin_id.to_string(), stats);
        Ok(())
    }

    /// 计算插件统计
    fn calculate_plugin_stats(&self, plugin_id: &str) -> WasmResult<ReviewStats> {
        let reviews: Vec<&PluginReview> = self.reviews.values()
            .filter(|r| r.plugin_id == plugin_id && r.status == ReviewStatus::Published)
            .collect();

        let total_reviews = reviews.len() as u32;

        let average_rating = if total_reviews > 0 {
            reviews.iter().map(|r| r.rating as f64).sum::<f64>() / total_reviews as f64
        } else {
            0.0
        };

        let mut rating_distribution = HashMap::new();
        for rating in 1..=5 {
            let count = reviews.iter().filter(|r| r.rating == rating).count() as u32;
            rating_distribution.insert(rating, count);
        }

        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        let reviews_last_30_days = reviews.iter()
            .filter(|r| r.created_at > thirty_days_ago)
            .count() as u32;

        let verified_reviews = reviews.iter()
            .filter(|r| r.verified_purchase)
            .count() as u32;

        Ok(ReviewStats {
            total_reviews,
            average_rating,
            rating_distribution,
            reviews_last_30_days,
            verified_reviews,
        })
    }

    /// 应用评论过滤器
    fn apply_review_filter(&self, mut reviews: Vec<PluginReview>, filter: &ReviewFilter) -> Vec<PluginReview> {
        if let Some(rating) = filter.rating {
            reviews.retain(|r| r.rating == rating);
        }

        if filter.verified_only {
            reviews.retain(|r| r.verified_purchase);
        }

        if !filter.tags.is_empty() {
            reviews.retain(|r| filter.tags.iter().any(|tag| r.tags.contains(tag)));
        }

        if let Some((start, end)) = filter.date_range {
            reviews.retain(|r| r.created_at >= start && r.created_at <= end);
        }

        reviews
    }

    /// 排序评论
    fn sort_reviews(&self, reviews: &mut Vec<PluginReview>, sort_by: &ReviewSortBy) {
        match sort_by {
            ReviewSortBy::Newest => {
                reviews.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            }
            ReviewSortBy::Oldest => {
                reviews.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            }
            ReviewSortBy::MostHelpful => {
                reviews.sort_by(|a, b| b.helpful_count.cmp(&a.helpful_count));
            }
            ReviewSortBy::HighestRating => {
                reviews.sort_by(|a, b| b.rating.cmp(&a.rating));
            }
            ReviewSortBy::LowestRating => {
                reviews.sort_by(|a, b| a.rating.cmp(&b.rating));
            }
        }
    }
}

impl Default for ReviewService {
    fn default() -> Self {
        Self::new()
    }
}
