//! Actor de origen para DataFlare
//!
//! Implementa el actor responsable de extraer datos de las fuentes.

use std::collections::HashMap;
use actix::prelude::*;
use log::{error, info, debug, warn};
use chrono::Utc;

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecordBatch, StartExtraction, WorkflowPhase, WorkflowProgress},
    state::SourceState,
};
use dataflare_connector::source::SourceConnector;

use crate::actor::{DataFlareActor, Initialize, Finalize, Pause, Resume, GetStatus, ActorStatus, 
                  SubscribeProgress, UnsubscribeProgress, SendBatch, ReportExtractionCompletion};

/// Actor que gestiona la extracción de datos de una fuente
pub struct SourceActor {
    /// ID del actor
    id: String,

    /// Conector de origen
    connector: Box<dyn SourceConnector>,

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
}

impl SourceActor {
    /// Crea un nuevo actor de origen
    pub fn new<S: Into<String>>(id: S, connector: Box<dyn SourceConnector>) -> Self {
        Self {
            id: id.into(),
            connector,
            status: ActorStatus::Initialized,
            config: None,
            source_state: None,
            progress_recipients: HashMap::new(),
            batch_size: 1000, // Valor predeterminado
            records_processed: 0,
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
        debug!("配置内容: {:?}", msg.config);

        // 配置连接器
        info!("开始配置源连接器...");
        self.connector.configure(&msg.config)
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

    fn handle(&mut self, msg: StartExtraction, _ctx: &mut Self::Context) -> Self::Result {
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
        info!("SourceActor状态更新为Running");
        
        // 获取Actor自身的地址，用于消息传递
        let actor_addr = _ctx.address();
        let workflow_id = msg.workflow_id.clone();
        let batch_size = self.batch_size;
        let source_id = msg.source_id.clone();
        
        // 保存对连接器的引用
        let mut connector_ref = &mut self.connector;
        
        debug!("开始异步提取任务, 批处理大小: {}", batch_size);
        
        // 执行异步提取操作
        Box::pin(async move {
            info!("开始从源 {} 提取数据", source_id);
            
            // 创建数据流
            let state_option = None; // 假设这里不使用状态
            let mut stream = match connector_ref.read(state_option).await {
                Ok(stream) => {
                    info!("成功创建数据流");
                    stream
                },
                Err(e) => {
                    error!("创建数据流失败: {}", e);
                    return Err(e);
                }
            };
            
            // 跟踪进度
            let mut total_records = 0;
            let mut current_batch = Vec::new();
            
            // 处理记录流
            use futures::StreamExt;
            info!("开始处理记录流");
            while let Some(record_result) = stream.next().await {
                match record_result {
                    Ok(record) => {
                        // 添加到当前批次
                        current_batch.push(record);
                        total_records += 1;
                        
                        // 达到批处理大小时发送批次
                        if current_batch.len() >= batch_size {
                            let batch = DataRecordBatch::new(current_batch);
                            current_batch = Vec::new();
                            
                            info!("发送批次 #{}，记录数: {}", total_records / batch_size, batch.records.len());
                            if let Err(e) = actor_addr.send(SendBatch {
                                workflow_id: workflow_id.clone(),
                                batch,
                            }).await {
                                error!("发送批次失败: {}", e);
                                return Err(DataFlareError::Actor(format!("Error al enviar lote: {}", e)));
                            }
                            
                            // 报告进度 (假设总进度为100%，每个批次递增)
                            // 注意：在实际实现中，你应该有一个更好的方式来估算总记录数
                            if total_records % (batch_size * 10) == 0 {
                                info!("处理进度: {} 记录", total_records);
                            }
                        }
                    },
                    Err(e) => {
                        error!("读取记录失败: {}", e);
                        return Err(e);
                    }
                }
            }
            
            // 发送最后一个不完整的批次
            if !current_batch.is_empty() {
                let batch = DataRecordBatch::new(current_batch);
                info!("发送最后一个批次，记录数: {}", batch.records.len());
                if let Err(e) = actor_addr.send(SendBatch {
                    workflow_id: workflow_id.clone(),
                    batch,
                }).await {
                    error!("发送最后批次失败: {}", e);
                    return Err(DataFlareError::Actor(format!("Error al enviar último lote: {}", e)));
                }
            }
            
            // 发送完成通知
            info!("完成提取，总记录数: {}", total_records);
            let result: Result<()> = Ok(());
            
            // 发送消息通知Actor
            if let Err(e) = actor_addr.send(ReportExtractionCompletion {
                workflow_id: workflow_id.clone(),
                records_processed: total_records as u64,
                success: result.is_ok(),
                error: result.as_ref().err().map(|e: &DataFlareError| e.to_string()),
            }).await {
                error!("发送完成通知失败: {}", e);
            }
            
            Ok(())
        }.into_actor(self))
    }
}

/// Mensaje para reportar finalización de extracción
#[derive(Message)]
#[rtype(result = "()")]
struct ReportExtractionCompletion {
    workflow_id: String,
    records_processed: u64,
    success: bool,
    error: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;
    use std::sync::{Arc, Mutex};
    use dataflare_core::data::{DataRecord, DataRecordBatch, Schema};
    use futures::stream::BoxStream;
    
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
    
    impl dataflare_core::connector::SourceConnector for MockSourceConnector {
        fn configure(&mut self, _config: serde_json::Value) -> dataflare_core::error::Result<()> {
            Ok(())
        }
        
        fn check_connection(&self) -> dataflare_core::error::Result<()> {
            Ok(())
        }
        
        fn discover_schema(&self) -> dataflare_core::error::Result<Schema> {
            Ok(Schema::empty())
        }
        
        fn stream_records(&mut self, _state: Option<dataflare_core::connector::SourceState>) 
            -> dataflare_core::error::Result<BoxStream<'static, dataflare_core::error::Result<DataRecordBatch>>> {
            let records = self.records.lock().unwrap().clone();
            let batch = DataRecordBatch::new("test", None, records);
            Ok(Box::pin(futures::stream::once(async move { Ok(batch) })))
        }
        
        fn get_state(&self) -> dataflare_core::error::Result<dataflare_core::connector::SourceState> {
            Ok(dataflare_core::connector::SourceState::empty())
        }
    }
    
    #[test]
    fn test_source_actor_creation() {
        let connector = Box::new(MockSourceConnector::new(vec![]));
        let actor = SourceActor::new("test_source", connector);
        assert_eq!(actor.id, "test_source");
    }
}
