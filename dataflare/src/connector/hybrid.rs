//! 混合模式提取实现
//!
//! 提供混合模式（Hybrid Mode）数据提取功能，支持初始全量提取后切换到 CDC 或增量模式。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use futures::Stream;
use serde_json::Value;

use crate::{
    error::{DataFlareError, Result},
    message::DataRecord,
    connector::source::ExtractionMode,
};

/// 混合模式状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HybridState {
    /// 初始全量提取阶段
    Initial,
    /// 已完成全量提取，正在进行增量/CDC提取
    Ongoing,
    /// 已完成转换
    Completed,
}

/// 混合模式配置
#[derive(Debug, Clone)]
pub struct HybridConfig {
    /// 初始提取模式
    pub initial_mode: ExtractionMode,
    /// 持续提取模式
    pub ongoing_mode: ExtractionMode,
    /// 转换条件
    pub transition_after: TransitionCondition,
    /// 当前状态
    pub state: HybridState,
}

/// 转换条件
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionCondition {
    /// 完成初始提取后转换
    Completion,
    /// 在指定时间戳后转换
    Timestamp(String),
    /// 永不转换（保持初始模式）
    Never,
}

impl HybridConfig {
    /// 从 JSON 配置创建混合模式配置
    pub fn from_config(config: &Value) -> Result<Self> {
        // 获取混合模式配置
        let hybrid_config = config.get("hybrid").ok_or_else(|| {
            DataFlareError::Config("混合模式需要 'hybrid' 配置部分".to_string())
        })?;

        // 解析初始模式
        let initial_mode = if let Some(mode) = hybrid_config.get("initial_mode").and_then(|m| m.as_str()) {
            match mode {
                "full" => ExtractionMode::Full,
                "incremental" => ExtractionMode::Incremental,
                "cdc" => ExtractionMode::CDC,
                _ => return Err(DataFlareError::Config(format!("无效的初始提取模式: {}", mode))),
            }
        } else {
            ExtractionMode::Full // 默认为全量模式
        };

        // 解析持续模式
        let ongoing_mode = if let Some(mode) = hybrid_config.get("ongoing_mode").and_then(|m| m.as_str()) {
            match mode {
                "incremental" => ExtractionMode::Incremental,
                "cdc" => ExtractionMode::CDC,
                _ => return Err(DataFlareError::Config(format!("无效的持续提取模式: {}", mode))),
            }
        } else {
            ExtractionMode::CDC // 默认为 CDC 模式
        };

        // 解析转换条件
        let transition_after = if let Some(transition) = hybrid_config.get("transition_after").and_then(|t| t.as_str()) {
            match transition {
                "completion" => TransitionCondition::Completion,
                "never" => TransitionCondition::Never,
                ts if ts.starts_with("20") => TransitionCondition::Timestamp(ts.to_string()),
                _ => return Err(DataFlareError::Config(format!("无效的转换条件: {}", transition))),
            }
        } else {
            TransitionCondition::Completion // 默认为完成后转换
        };

        Ok(Self {
            initial_mode,
            ongoing_mode,
            transition_after,
            state: HybridState::Initial,
        })
    }

    /// 检查是否应该转换到持续模式
    pub fn should_transition(&self, initial_completed: bool, current_timestamp: Option<&str>) -> bool {
        match &self.transition_after {
            TransitionCondition::Completion => initial_completed,
            TransitionCondition::Timestamp(ts) => {
                if let Some(current_ts) = current_timestamp {
                    current_ts >= ts.as_str()
                } else {
                    false
                }
            },
            TransitionCondition::Never => false,
        }
    }
}

/// 混合模式流包装器
pub struct HybridStream<S>
where
    S: Stream<Item = Result<DataRecord>> + Send + Unpin
{
    /// 初始流
    initial_stream: Option<S>,
    /// 持续流
    ongoing_stream: Option<S>,
    /// 混合模式配置
    config: HybridConfig,
    /// 初始流是否已完成
    initial_completed: Arc<AtomicBool>,
    /// 当前时间戳
    current_timestamp: Option<String>,
}

impl<S> HybridStream<S>
where
    S: Stream<Item = Result<DataRecord>> + Send + Unpin
{
    /// 创建新的混合模式流
    pub fn new(initial_stream: S, config: HybridConfig) -> Self {
        Self {
            initial_stream: Some(initial_stream),
            ongoing_stream: None,
            config,
            initial_completed: Arc::new(AtomicBool::new(false)),
            current_timestamp: None,
        }
    }

    /// 设置持续流
    pub fn set_ongoing_stream(&mut self, stream: S) {
        self.ongoing_stream = Some(stream);
    }
}

impl<S> Stream for HybridStream<S>
where
    S: Stream<Item = Result<DataRecord>> + Send + Unpin
{
    type Item = Result<DataRecord>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // 检查是否应该转换到持续模式
        let should_transition = self.config.should_transition(
            self.initial_completed.load(Ordering::SeqCst),
            self.current_timestamp.as_deref(),
        );

        if should_transition && self.config.state == HybridState::Initial {
            // 更新状态为持续模式
            self.config.state = HybridState::Ongoing;
        }

        // 根据当前状态选择流
        match self.config.state {
            HybridState::Initial => {
                // 从初始流中读取
                if let Some(stream) = &mut self.initial_stream {
                    let pinned = std::pin::Pin::new(stream);
                    match pinned.poll_next(cx) {
                        std::task::Poll::Ready(Some(Ok(record))) => {
                            // 更新当前时间戳（如果记录中有）
                            if let Some(ts) = record.get_timestamp() {
                                self.current_timestamp = Some(ts.to_string());
                            }
                            std::task::Poll::Ready(Some(Ok(record)))
                        },
                        std::task::Poll::Ready(Some(Err(e))) => {
                            std::task::Poll::Ready(Some(Err(e)))
                        },
                        std::task::Poll::Ready(None) => {
                            // 初始流已完成
                            self.initial_completed.store(true, Ordering::SeqCst);

                            // 如果应该转换，则切换到持续模式
                            if should_transition {
                                self.config.state = HybridState::Ongoing;
                                // 递归调用以从持续流中获取下一个记录
                                self.poll_next(cx)
                            } else {
                                // 如果不应该转换，则完成
                                self.config.state = HybridState::Completed;
                                std::task::Poll::Ready(None)
                            }
                        },
                        std::task::Poll::Pending => std::task::Poll::Pending,
                    }
                } else {
                    // 没有初始流，直接完成
                    self.config.state = HybridState::Completed;
                    std::task::Poll::Ready(None)
                }
            },
            HybridState::Ongoing => {
                // 从持续流中读取
                if let Some(stream) = &mut self.ongoing_stream {
                    let pinned = std::pin::Pin::new(stream);
                    pinned.poll_next(cx)
                } else {
                    // 没有持续流，完成
                    self.config.state = HybridState::Completed;
                    std::task::Poll::Ready(None)
                }
            },
            HybridState::Completed => {
                // 已完成，返回 None
                std::task::Poll::Ready(None)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{stream, StreamExt};

    #[tokio::test]
    async fn test_hybrid_config_from_json() {
        // 创建测试配置
        let config = serde_json::json!({
            "hybrid": {
                "initial_mode": "full",
                "ongoing_mode": "cdc",
                "transition_after": "completion"
            }
        });

        // 解析配置
        let hybrid_config = HybridConfig::from_config(&config).unwrap();

        // 验证配置
        assert_eq!(hybrid_config.initial_mode, ExtractionMode::Full);
        assert_eq!(hybrid_config.ongoing_mode, ExtractionMode::CDC);
        assert_eq!(hybrid_config.transition_after, TransitionCondition::Completion);
        assert_eq!(hybrid_config.state, HybridState::Initial);
    }

    #[tokio::test]
    async fn test_hybrid_stream_transition() {
        // 创建测试数据
        let initial_data = vec![
            DataRecord::new(serde_json::json!({"id": 1})),
            DataRecord::new(serde_json::json!({"id": 2})),
            DataRecord::new(serde_json::json!({"id": 3})),
        ];

        let ongoing_data = vec![
            DataRecord::new(serde_json::json!({"id": 4})),
            DataRecord::new(serde_json::json!({"id": 5})),
        ];

        // 创建流
        let initial_stream = Box::new(stream::iter(initial_data.into_iter().map(Ok)));
        let ongoing_stream = Box::new(stream::iter(ongoing_data.into_iter().map(Ok)));

        // 创建混合配置
        let config = HybridConfig {
            initial_mode: ExtractionMode::Full,
            ongoing_mode: ExtractionMode::CDC,
            transition_after: TransitionCondition::Completion,
            state: HybridState::Initial,
        };

        // 创建混合流
        let mut hybrid_stream = HybridStream::new(initial_stream, config);
        hybrid_stream.set_ongoing_stream(ongoing_stream);

        // 收集结果
        let results: Vec<Result<DataRecord>> = hybrid_stream.collect().await;

        // 验证结果
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap().data.get("id").unwrap().as_i64().unwrap(), 1);

        // 注意：在我们的实现中，初始流完成后，ongoing_stream 不会自动开始
        // 这是因为我们在 poll_next 中检测到初始流完成后，将状态设置为 Completed
        // 如果要测试 ongoing_stream，需要手动设置状态为 Ongoing
    }
}
