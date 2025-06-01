//! 企业级流处理器实现
//!
//! 基于 @dataflare/plugin 构建的高性能流处理器

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::timeout;
use dataflare_plugin::{PluginRuntime, PluginManager, DataFlarePlugin, PluginRecord};
use crate::error::{EnterpriseError, EnterpriseResult};
use crate::streaming::{
    StreamProcessor, StreamProcessorConfig, StreamProcessorState,
    StreamRecord, StreamMetrics, StreamEvent, BackpressureController
};

/// 企业级流处理器
pub struct EnterpriseStreamProcessor {
    /// 配置
    config: StreamProcessorConfig,
    /// 基础插件运行时 (来自 @dataflare/plugin)
    plugin_runtime: Arc<PluginRuntime>,
    /// 插件管理器
    plugin_manager: Arc<PluginManager>,
    /// 当前状态
    state: Arc<RwLock<StreamProcessorState>>,
    /// 处理指标
    metrics: Arc<RwLock<StreamMetrics>>,
    /// 背压控制器
    backpressure_controller: Arc<Mutex<BackpressureController>>,
    /// 事件发送器
    event_sender: mpsc::UnboundedSender<StreamEvent>,
    /// 事件接收器
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<StreamEvent>>>,
    /// 检查点存储
    checkpoints: Arc<RwLock<std::collections::HashMap<String, u64>>>,
    /// 处理统计
    processing_stats: Arc<RwLock<ProcessingStats>>,
}

/// 处理统计信息
#[derive(Debug, Default)]
struct ProcessingStats {
    /// 总处理时间 (纳秒)
    total_processing_time_ns: u64,
    /// 处理次数
    processing_count: u64,
    /// 延迟样本 (最近1000个)
    latency_samples: std::collections::VecDeque<u64>,
    /// 最后处理时间
    last_processing_time: Option<Instant>,
}

impl EnterpriseStreamProcessor {
    /// 创建新的企业级流处理器
    pub async fn new(
        config: StreamProcessorConfig,
        plugin_runtime: Arc<PluginRuntime>,
    ) -> EnterpriseResult<Self> {
        // 简化实现：不使用 PluginManager，直接使用 PluginRuntime
        // let plugin_manager = Arc::new(PluginManager::new(Default::default())
        //     .map_err(|e| EnterpriseError::configuration(format!("插件管理器初始化失败: {}", e)))?);
        let plugin_manager = Arc::new(PluginManager::new(Default::default())
            .map_err(|e| EnterpriseError::configuration(format!("插件管理器初始化失败: {}", e)))?);
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let backpressure_controller = BackpressureController::new(
            config.backpressure_threshold,
            config.max_concurrent_streams,
        );

        Ok(Self {
            config,
            plugin_runtime,
            plugin_manager,
            state: Arc::new(RwLock::new(StreamProcessorState::Uninitialized)),
            metrics: Arc::new(RwLock::new(StreamMetrics::default())),
            backpressure_controller: Arc::new(Mutex::new(backpressure_controller)),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            checkpoints: Arc::new(RwLock::new(std::collections::HashMap::new())),
            processing_stats: Arc::new(RwLock::new(ProcessingStats::default())),
        })
    }

    /// 更新处理指标
    async fn update_metrics(&self, processing_time_ns: u64, record_size: usize) -> EnterpriseResult<()> {
        let mut metrics = self.metrics.write().await;
        let mut stats = self.processing_stats.write().await;

        // 更新基础指标
        metrics.records_processed += 1;
        metrics.bytes_processed += record_size as u64;

        // 更新处理统计
        stats.total_processing_time_ns += processing_time_ns;
        stats.processing_count += 1;

        // 添加延迟样本
        let latency_ms = processing_time_ns / 1_000_000;
        stats.latency_samples.push_back(latency_ms);
        if stats.latency_samples.len() > 1000 {
            stats.latency_samples.pop_front();
        }

        // 计算平均延迟
        if stats.processing_count > 0 {
            metrics.average_latency_ms = (stats.total_processing_time_ns / stats.processing_count) as f64 / 1_000_000.0;
        }

        // 计算 P99 延迟
        if !stats.latency_samples.is_empty() {
            let mut sorted_samples: Vec<u64> = stats.latency_samples.iter().cloned().collect();
            sorted_samples.sort_unstable();
            let p99_index = (sorted_samples.len() as f64 * 0.99) as usize;
            metrics.p99_latency_ms = sorted_samples.get(p99_index).copied().unwrap_or(0) as f64;
        }

        // 计算当前吞吐量
        if let Some(last_time) = stats.last_processing_time {
            let elapsed = last_time.elapsed();
            if elapsed.as_secs() >= 1 {
                metrics.current_throughput = metrics.records_processed as f64 / elapsed.as_secs_f64();
            }
        }
        stats.last_processing_time = Some(Instant::now());

        // 更新时间戳
        metrics.last_updated = chrono::Utc::now().timestamp_nanos();

        Ok(())
    }

    /// 检查背压状态
    async fn check_backpressure(&self) -> EnterpriseResult<bool> {
        let mut controller = self.backpressure_controller.lock().await;
        let current_load = self.calculate_current_load().await?;

        if controller.should_apply_backpressure(current_load) {
            // 发送背压事件
            let event = StreamEvent::BackpressureTriggered {
                threshold: self.config.backpressure_threshold,
                current_load,
            };
            let _ = self.event_sender.send(event);

            // 更新指标
            let mut metrics = self.metrics.write().await;
            metrics.backpressure_events += 1;

            return Ok(true);
        }

        Ok(false)
    }

    /// 计算当前负载
    async fn calculate_current_load(&self) -> EnterpriseResult<f64> {
        let metrics = self.metrics.read().await;

        // 基于缓冲区使用率和活跃流数量计算负载
        let buffer_load = metrics.buffer_utilization;
        let stream_load = metrics.active_streams as f64 / self.config.max_concurrent_streams as f64;

        // 综合负载计算
        Ok((buffer_load + stream_load) / 2.0)
    }

    /// 处理单个记录的内部实现
    async fn process_record_internal(&self, record: StreamRecord) -> EnterpriseResult<StreamRecord> {
        let start_time = Instant::now();

        // 检查背压
        if self.check_backpressure().await? {
            // 应用背压延迟
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // 转换为 PluginRecord
        let plugin_record = record.to_plugin_record();

        // 使用插件运行时处理记录
        let processed_record = timeout(
            self.config.processing_timeout,
            self.process_with_plugins(&plugin_record)
        ).await
        .map_err(|_| EnterpriseError::timeout("记录处理超时", self.config.processing_timeout.as_millis() as u64))?
        .map_err(|e| EnterpriseError::stream_processing(format!("插件处理失败: {}", e), "PLUGIN_ERROR"))?;

        // 转换回 StreamRecord
        let result_record = StreamRecord::from_plugin_record(&processed_record);

        // 更新指标
        let processing_time = start_time.elapsed().as_nanos() as u64;
        self.update_metrics(processing_time, record.size()).await?;

        Ok(result_record)
    }

    /// 使用插件处理记录
    async fn process_with_plugins(&self, record: &PluginRecord) -> Result<PluginRecord, Box<dyn std::error::Error + Send + Sync>> {
        // 简化实现：直接返回原记录，实际中应该通过插件运行时处理
        // TODO: 集成真正的插件处理逻辑
        Ok(PluginRecord {
            value: record.value,
            metadata: record.metadata.clone(),
            timestamp: record.timestamp,
            partition: record.partition,
            offset: record.offset,
            key: record.key,
        })
    }

    /// 发送状态变更事件
    async fn emit_state_change(&self, old_state: StreamProcessorState, new_state: StreamProcessorState) {
        let event = StreamEvent::StateChanged { old_state, new_state };
        let _ = self.event_sender.send(event);
    }
}

#[async_trait::async_trait]
impl StreamProcessor for EnterpriseStreamProcessor {
    async fn start(&mut self) -> EnterpriseResult<()> {
        let mut state = self.state.write().await;
        let old_state = state.clone();

        if *state != StreamProcessorState::Uninitialized && *state != StreamProcessorState::Stopped {
            return Err(EnterpriseError::stream_processing(
                "流处理器已在运行或处于无效状态",
                "INVALID_STATE"
            ));
        }

        *state = StreamProcessorState::Initializing;
        self.emit_state_change(old_state.clone(), state.clone()).await;

        // 初始化插件运行时
        // 在实际实现中，这里应该加载和配置插件

        *state = StreamProcessorState::Running;
        self.emit_state_change(StreamProcessorState::Initializing, state.clone()).await;

        log::info!("企业级流处理器已启动");
        Ok(())
    }

    async fn stop(&mut self) -> EnterpriseResult<()> {
        let mut state = self.state.write().await;
        let old_state = state.clone();

        if *state == StreamProcessorState::Stopped {
            return Ok(());
        }

        *state = StreamProcessorState::Stopping;
        self.emit_state_change(old_state, state.clone()).await;

        // 停止处理逻辑
        // 等待当前处理完成
        tokio::time::sleep(Duration::from_millis(100)).await;

        *state = StreamProcessorState::Stopped;
        self.emit_state_change(StreamProcessorState::Stopping, state.clone()).await;

        log::info!("企业级流处理器已停止");
        Ok(())
    }

    async fn pause(&mut self) -> EnterpriseResult<()> {
        let mut state = self.state.write().await;
        let old_state = state.clone();

        if *state != StreamProcessorState::Running {
            return Err(EnterpriseError::stream_processing(
                "只能暂停运行中的流处理器",
                "INVALID_STATE"
            ));
        }

        *state = StreamProcessorState::Paused;
        self.emit_state_change(old_state, state.clone()).await;

        log::info!("企业级流处理器已暂停");
        Ok(())
    }

    async fn resume(&mut self) -> EnterpriseResult<()> {
        let mut state = self.state.write().await;
        let old_state = state.clone();

        if *state != StreamProcessorState::Paused {
            return Err(EnterpriseError::stream_processing(
                "只能恢复暂停中的流处理器",
                "INVALID_STATE"
            ));
        }

        *state = StreamProcessorState::Running;
        self.emit_state_change(old_state, state.clone()).await;

        log::info!("企业级流处理器已恢复");
        Ok(())
    }

    async fn process_record(&mut self, record: StreamRecord) -> EnterpriseResult<StreamRecord> {
        let state = self.state.read().await;
        if *state != StreamProcessorState::Running {
            return Err(EnterpriseError::stream_processing(
                "流处理器未在运行状态",
                "NOT_RUNNING"
            ));
        }
        drop(state);

        self.process_record_internal(record).await
    }

    async fn process_batch(&mut self, records: Vec<StreamRecord>) -> EnterpriseResult<Vec<StreamRecord>> {
        let state = self.state.read().await;
        if *state != StreamProcessorState::Running {
            return Err(EnterpriseError::stream_processing(
                "流处理器未在运行状态",
                "NOT_RUNNING"
            ));
        }
        drop(state);

        let batch_id = uuid::Uuid::new_v4().to_string();
        let mut results = Vec::with_capacity(records.len());

        for record in records {
            match self.process_record_internal(record).await {
                Ok(processed) => results.push(processed),
                Err(e) => {
                    // 记录错误但继续处理其他记录
                    let mut metrics = self.metrics.write().await;
                    metrics.error_count += 1;

                    log::warn!("批处理中记录处理失败: {}", e);
                    // 根据错误策略决定是否继续
                    continue;
                }
            }
        }

        // 发送批次完成事件
        let event = StreamEvent::BatchCompleted {
            batch_id,
            record_count: results.len(),
        };
        let _ = self.event_sender.send(event);

        Ok(results)
    }

    fn get_state(&self) -> StreamProcessorState {
        // 使用 try_read 避免阻塞
        self.state.try_read()
            .map(|state| state.clone())
            .unwrap_or(StreamProcessorState::Error)
    }

    async fn get_metrics(&self) -> EnterpriseResult<StreamMetrics> {
        let metrics = self.metrics.read().await;
        Ok(metrics.clone())
    }

    async fn create_checkpoint(&mut self) -> EnterpriseResult<String> {
        let checkpoint_id = uuid::Uuid::new_v4().to_string();
        let metrics = self.metrics.read().await;
        let current_offset = metrics.records_processed;
        drop(metrics);

        // 存储检查点
        let mut checkpoints = self.checkpoints.write().await;
        checkpoints.insert(checkpoint_id.clone(), current_offset);

        // 发送检查点事件
        let event = StreamEvent::CheckpointCreated {
            checkpoint_id: checkpoint_id.clone(),
            offset: current_offset,
        };
        let _ = self.event_sender.send(event);

        log::info!("创建检查点: {} (offset: {})", checkpoint_id, current_offset);
        Ok(checkpoint_id)
    }

    async fn restore_from_checkpoint(&mut self, checkpoint_id: &str) -> EnterpriseResult<()> {
        let checkpoints = self.checkpoints.read().await;
        let offset = checkpoints.get(checkpoint_id)
            .ok_or_else(|| EnterpriseError::stream_processing(
                format!("检查点不存在: {}", checkpoint_id),
                "CHECKPOINT_NOT_FOUND"
            ))?;

        // 恢复状态
        let mut metrics = self.metrics.write().await;
        metrics.records_processed = *offset;

        log::info!("从检查点恢复: {} (offset: {})", checkpoint_id, offset);
        Ok(())
    }
}
