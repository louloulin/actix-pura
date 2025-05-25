//! Actor de origen para DataFlare
//!
//! Implementa el actor responsable de extraer datos de las fuentes.

use std::collections::HashMap;
use std::sync::Arc;
use actix::prelude::*;
use log::{error, info, debug};
use chrono::Utc;
use std::cell::RefCell;
use futures::StreamExt;

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecordBatch, StartExtraction, WorkflowPhase, WorkflowProgress},
    state::SourceState,
};
use dataflare_connector::source::{SourceConnector, ExtractionMode};

use crate::actor::{DataFlareActor, Initialize, Finalize, Pause, Resume, GetStatus, ActorStatus,
                  SubscribeProgress, UnsubscribeProgress, SendBatch, ConnectToTask, TaskActor, SetNextActor};

/// Actor que gestiona la extracción de datos de una fuente
pub struct SourceActor {
    /// ID del actor
    id: String,

    /// Conector de origen
    connector: Arc<RefCell<Box<dyn SourceConnector>>>,

    /// Estado actual del actor
    status: ActorStatus,

    /// Configuración actual
    config: Option<serde_json::Value>,

    /// Estado de la fuente
    source_state: Option<SourceState>,

    /// Destinatarios para notificaciones de progreso
    progress_recipients: HashMap<String, Vec<Recipient<WorkflowProgress>>>,

    /// Tamaño de lote para extracción
    batch_size: usize,

    /// Contador de registros procesados
    records_processed: u64,

    /// Associated TaskActor
    associated_task: Option<(String, Addr<TaskActor>)>,

    /// Next actor in the data flow
    next_actor: Option<Recipient<SendBatch>>,
}

impl SourceActor {
    /// Crea un nuevo actor de origen
    pub fn new<S: Into<String>>(id: S, connector: Box<dyn SourceConnector>) -> Self {
        Self {
            id: id.into(),
            connector: Arc::new(RefCell::new(connector)),
            status: ActorStatus::Initialized,
            config: None,
            source_state: None,
            progress_recipients: HashMap::new(),
            batch_size: 1000, // Valor predeterminado
            records_processed: 0,
            associated_task: None,
            next_actor: None,
        }
    }

    /// Establece el tamaño de lote
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Reporta el progreso a los suscriptores
    fn report_progress_to_subscribers(&self, workflow_id: &str, phase: WorkflowPhase, progress: f64, message: &str) {
        if let Some(recipients) = self.progress_recipients.get(workflow_id) {
            let progress_msg = WorkflowProgress {
                workflow_id: workflow_id.to_string(),
                phase,
                progress,
                message: message.to_string(),
                timestamp: Utc::now(),
            };

            for recipient in recipients {
                let _ = recipient.do_send(progress_msg.clone());
            }
        }
    }
}

impl Actor for SourceActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("SourceActor {} 已启动", self.id);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("SourceActor {} 已停止", self.id);
    }
}

impl DataFlareActor for SourceActor {
    fn get_id(&self) -> &str {
        &self.id
    }

    fn get_type(&self) -> &str {
        "source"
    }

    fn initialize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("初始化SourceActor {}", self.id);
        self.status = ActorStatus::Initialized;
        Ok(())
    }

    fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("完成SourceActor {}", self.id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }

    fn report_progress(&self, workflow_id: &str, phase: WorkflowPhase, progress: f64, message: &str) {
        info!("SourceActor {} 报告进度: {:?} 阶段 {:.2}% - {}",
            self.id, phase, progress * 100.0, message);
        self.report_progress_to_subscribers(workflow_id, phase, progress, message);
    }
}

/// Implementación del handler para inicializar el actor
impl Handler<Initialize> for SourceActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Initialize, _ctx: &mut Self::Context) -> Self::Result {
        info!("初始化 SourceActor {} 工作流 {}", self.id, msg.workflow_id);

        // 更详细地记录配置内容
        info!("配置内容: {}", serde_json::to_string_pretty(&msg.config).unwrap_or_default());

        if let Some(file_path) = msg.config.get("file_path") {
            info!("找到文件路径: {}", file_path);
        } else if let Some(config) = msg.config.get("config") {
            if let Some(file_path) = config.get("file_path") {
                info!("在config子对象中找到文件路径: {}", file_path);
            } else {
                error!("在config子对象中没有找到文件路径");
            }
        } else {
            error!("配置中没有文件路径参数");
        }

        // 配置连接器
        info!("开始配置源连接器...");
        self.connector.borrow_mut().configure(&msg.config)
            .map_err(|e| {
                error!("配置源连接器失败: {}", e);
                DataFlareError::Config(format!("Error al configurar conector: {}", e))
            })?;

        // 保存配置
        self.config = Some(msg.config);
        self.status = ActorStatus::Initialized;
        info!("SourceActor {} 初始化完成", self.id);

        Ok(())
    }
}

/// Implementación del handler para finalizar el actor
impl Handler<Finalize> for SourceActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Finalize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Finalizando SourceActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }
}

/// Implementación del handler para pausar el actor
impl Handler<Pause> for SourceActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Pause, _ctx: &mut Self::Context) -> Self::Result {
        info!("Pausando SourceActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Paused;
        Ok(())
    }
}

/// Implementación del handler para reanudar el actor
impl Handler<Resume> for SourceActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Resume, _ctx: &mut Self::Context) -> Self::Result {
        info!("Reanudando SourceActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Running;
        Ok(())
    }
}

/// Implementación del handler para obtener el estado del actor
impl Handler<GetStatus> for SourceActor {
    type Result = Result<ActorStatus>;

    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.status.clone())
    }
}

/// Implementación del handler para iniciar la extracción
impl Handler<StartExtraction> for SourceActor {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: StartExtraction, ctx: &mut Self::Context) -> Self::Result {
        info!("开始提取数据: 工作流 {} 源 {}", msg.workflow_id, msg.source_id);

        // 验证Actor状态
        if self.status != ActorStatus::Initialized && self.status != ActorStatus::Running {
            let status = self.status.clone();
            error!("Actor状态不适合提取: {:?}", status);
            return Box::pin(async move {
                Err(DataFlareError::Actor(format!(
                    "Actor no está en estado adecuado para extracción: {:?}", status
                )))
            }.into_actor(self));
        }

        // 更新状态
        self.status = ActorStatus::Running;
        info!("SourceActor {} 开始提取数据, 批次大小: {}", self.id, self.batch_size);

        // 从配置中获取批次大小
        if let Some(batch_size) = msg.config.get("batch_size").and_then(|v| v.as_u64()) {
            self.batch_size = batch_size as usize;
        }

        // 克隆需要的值
        let self_id = self.id.clone();
        let workflow_id = msg.workflow_id.clone();
        let batch_size = self.batch_size;
        let addr = ctx.address();
        let connector = self.connector.clone();
        let associated_task = self.associated_task.clone();

        // 创建异步任务处理数据读取和转发
        Box::pin(
            async move {
                // Verify that we have an associated task to send data to
                if associated_task.is_none() {
                    error!("SourceActor {} has no associated task, cannot start extraction", self_id);
                    return Err(DataFlareError::Actor(
                        format!("SourceActor has no associated task")
                    ));
                }

                let (task_id, task_addr) = associated_task.unwrap();
                info!("SourceActor {} will send data to task {}", self_id, task_id);

                // 打开连接器中的流
                let mut connector_ref = connector.borrow_mut();
                let stream_result = connector_ref.read(msg.state).await;

                match stream_result {
                    Ok(mut stream) => {
                        info!("SourceActor {} 成功打开数据流", self_id);

                        // 收集批次数据
                        let mut batch_records = Vec::with_capacity(batch_size);
                        let mut total_records = 0;
                        let mut batch_number = 0;

                        // 处理流中的每条记录
                        while let Some(record_result) = stream.next().await {
                            match record_result {
                                Ok(record) => {
                                    batch_records.push(record);

                                    // 如果批次已满，发送并重置
                                    if batch_records.len() >= batch_size {
                                        batch_number += 1;
                                        let record_count = batch_records.len();
                                        total_records += record_count;

                                        // 创建批次
                                        let batch = DataRecordBatch::new(
                                            std::mem::take(&mut batch_records)
                                        );

                                        // 发送批次
                                        debug!("SourceActor {} 发送批次 {}: {} 条记录",
                                              self_id, batch_number, record_count);

                                        let send_result = addr.send(SendBatch {
                                            workflow_id: workflow_id.clone(),
                                            batch,
                                            is_last_batch: false,
                                        }).await;

                                        if let Err(e) = send_result {
                                            error!("发送批次时出错: {}", e);
                                            return Err(DataFlareError::Actor(
                                                format!("Error al enviar lote: {}", e)
                                            ));
                                        }
                                    }
                                },
                                Err(e) => {
                                    error!("读取记录时出错: {}", e);
                                    // 继续处理，不中断流
                                }
                            }
                        }

                        // 处理最后一个可能不完整的批次
                        if !batch_records.is_empty() {
                            batch_number += 1;
                            let record_count = batch_records.len();
                            total_records += record_count;

                            // 创建批次
                            let batch = DataRecordBatch::new(
                                std::mem::take(&mut batch_records)
                            );

                            // 发送批次
                            debug!("SourceActor {} 发送最后批次 {}: {} 条记录",
                                  self_id, batch_number, record_count);

                            let send_result = addr.send(SendBatch {
                                workflow_id: workflow_id.clone(),
                                batch,
                                is_last_batch: true,
                            }).await;

                            if let Err(e) = send_result {
                                error!("发送最后批次时出错: {}", e);
                                return Err(DataFlareError::Actor(
                                    format!("Error al enviar último lote: {}", e)
                                ));
                            }
                        }

                        // 发送提取完成报告
                        let completion_result = addr.send(ReportExtractionCompletion {
                            workflow_id: workflow_id.clone(),
                            records_processed: total_records as u64,
                            success: true,
                            error: None,
                        }).await;

                        if let Err(e) = completion_result {
                            error!("发送提取完成报告时出错: {}", e);
                        }

                        info!("SourceActor {} 完成数据提取，总共处理了 {} 条记录", self_id, total_records);
                    Ok(())
                },
                Err(e) => {
                        error!("SourceActor {} 打开数据流失败: {}", self_id, e);

                        // 报告提取失败
                        let _ = addr.send(ReportExtractionCompletion {
                            workflow_id: workflow_id.clone(),
                            records_processed: 0,
                            success: false,
                            error: Some(format!("{}", e)),
                        }).await;

                    Err(e)
                }
            }
            }.into_actor(self))
    }
}

/// Mensaje para reportar finalización de extracción
#[derive(Message)]
#[rtype(result = "()")]
pub struct ReportExtractionCompletion {
    pub workflow_id: String,
    pub records_processed: u64,
    pub success: bool,
    pub error: Option<String>,
}

/// Handler para ReportExtractionCompletion
impl Handler<ReportExtractionCompletion> for SourceActor {
    type Result = ();

    fn handle(&mut self, msg: ReportExtractionCompletion, _ctx: &mut Self::Context) -> Self::Result {
        // Actualizar el estado del actor
        self.status = ActorStatus::Initialized;
        self.records_processed += msg.records_processed;

        // Reportar finalización
        let message = if msg.success {
            format!("Extracción completada. Procesados {} registros", msg.records_processed)
        } else {
            format!("Error en extracción: {}", msg.error.unwrap_or_else(|| "Desconocido".to_string()))
        };

        self.report_progress(
            &msg.workflow_id,
            WorkflowPhase::Extracting,
            if msg.success { 1.0 } else { -1.0 },
            &message
        );
    }
}

/// Implementación del handler para suscribirse a actualizaciones de progreso
impl Handler<SubscribeProgress> for SourceActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: SubscribeProgress, _ctx: &mut Self::Context) -> Self::Result {
        let recipients = self.progress_recipients
            .entry(msg.workflow_id.clone())
            .or_insert_with(Vec::new);

        recipients.push(msg.recipient);
        Ok(())
    }
}

/// Implementación del handler para cancelar la suscripción a actualizaciones de progreso
impl Handler<UnsubscribeProgress> for SourceActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: UnsubscribeProgress, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(recipients) = self.progress_recipients.get_mut(&msg.workflow_id) {
            recipients.retain(|r| r != &msg.recipient);
        }
        Ok(())
    }
}

/// 实现SendBatch消息处理器
impl Handler<SendBatch> for SourceActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: SendBatch, _ctx: &mut Self::Context) -> Self::Result {
        // 增加处理的记录数
        self.records_processed += msg.batch.len() as u64;

        // 记录处理信息
        info!("SourceActor {} 收到数据批次: {} 条记录, 工作流 {}",
              self.id, msg.batch.len(), msg.workflow_id);

        // 记录批次处理进度
        if self.records_processed % 10000 == 0 {
            self.report_progress(
                &msg.workflow_id,
                WorkflowPhase::Extracting,
                0.5,  // 假设50%进度，实际应根据估算总数计算
                &format!("已处理 {} 条记录", self.records_processed)
            );
        }

        // 批次处理成功
        Ok(())
    }
}

/// Handler implementation for ConnectToTask
impl Handler<ConnectToTask> for SourceActor {
    type Result = ();

    fn handle(&mut self, msg: ConnectToTask, _ctx: &mut Self::Context) -> Self::Result {
        info!("SourceActor {} connecting to task {}", self.id, msg.task_id);
        self.associated_task = Some((msg.task_id, msg.task_addr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;
    use std::sync::{Arc, Mutex};
    use futures::stream::BoxStream;
    use dataflare_core::message::{DataRecord, DataRecordBatch};
    use dataflare_core::model::Schema;
    use dataflare_connector::source::{SourceConnector, ExtractionMode};

    // 简单的Mock源连接器用于测试
    struct MockSourceConnector {
        records: Arc<Mutex<Vec<DataRecord>>>,
    }

    impl MockSourceConnector {
        fn new(records: Vec<DataRecord>) -> Self {
            Self {
                records: Arc::new(Mutex::new(records)),
            }
        }
    }

    #[async_trait::async_trait]
    impl SourceConnector for MockSourceConnector {
        fn configure(&mut self, _config: &serde_json::Value) -> Result<()> {
            Ok(())
        }

        async fn check_connection(&self) -> Result<bool> {
            Ok(true)
        }

        async fn discover_schema(&self) -> Result<Schema> {
            Ok(Schema::new())
        }

        async fn read(&mut self, _state: Option<SourceState>) -> Result<Box<dyn futures::Stream<Item = Result<DataRecord>> + Send + Unpin>> {
            let records = self.records.lock().unwrap().clone();
            let stream = futures::stream::iter(records.into_iter().map(Ok));
            Ok(Box::new(stream))
        }

        fn get_state(&self) -> Result<SourceState> {
            Ok(SourceState::new("mock"))
        }

        fn get_extraction_mode(&self) -> ExtractionMode {
            ExtractionMode::Full
        }

        async fn estimate_record_count(&self, _state: Option<SourceState>) -> Result<u64> {
            Ok(self.records.lock().unwrap().len() as u64)
        }
    }

    // 实现Clone特性以满足Actor要求
    impl Clone for MockSourceConnector {
        fn clone(&self) -> Self {
            Self {
                records: Arc::clone(&self.records),
            }
        }
    }

    #[test]
    fn test_source_actor_creation() {
        let connector = Box::new(MockSourceConnector::new(vec![]));
        let actor = SourceActor::new("test_source", connector);
        assert_eq!(actor.id, "test_source");
    }
}
