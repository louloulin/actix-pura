//! Actor de origen para DataFlare
//!
//! Implementa el actor responsable de extraer datos de las fuentes.

use std::collections::HashMap;
use actix::prelude::*;
use log::{error, info};
use chrono::Utc;

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecordBatch, StartExtraction, WorkflowPhase, WorkflowProgress},
    state::SourceState,
};
use dataflare_connector::source::SourceConnector;

use crate::actor::{DataFlareActor, Initialize, Finalize, Pause, Resume, GetStatus, ActorStatus, 
                  SubscribeProgress, UnsubscribeProgress};

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
        info!("SourceActor {} iniciado", self.id);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("SourceActor {} detenido", self.id);
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
        info!("Inicializando SourceActor {}", self.id);
        self.status = ActorStatus::Initialized;
        Ok(())
    }

    fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Finalizando SourceActor {}", self.id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }

    fn report_progress(&self, workflow_id: &str, phase: WorkflowPhase, progress: f64, message: &str) {
        self.report_progress_to_subscribers(workflow_id, phase, progress, message);
    }
}

/// Implementación del handler para inicializar el actor
impl Handler<Initialize> for SourceActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Initialize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Inicializando SourceActor {} para workflow {}", self.id, msg.workflow_id);

        // Configurar el conector
        self.connector.configure(&msg.config)
            .map_err(|e| DataFlareError::Config(format!("Error al configurar conector: {}", e)))?;

        // Guardar la configuración
        self.config = Some(msg.config);
        self.status = ActorStatus::Initialized;

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
        info!("Iniciando extracción para workflow {} en fuente {}", msg.workflow_id, msg.source_id);

        // Verificar que el actor esté inicializado
        if self.status != ActorStatus::Initialized && self.status != ActorStatus::Running {
            let status = self.status.clone();
            return Box::pin(async move {
                Err(DataFlareError::Actor(format!(
                    "Actor no está en estado adecuado para extracción: {:?}", status
                )))
            }.into_actor(self));
        }

        // Guardar el estado de la fuente
        self.source_state = msg.state.clone();

        // Configurar el conector
        if let Err(e) = self.connector.configure(&msg.config) {
            return Box::pin(async move {
                Err(DataFlareError::Config(format!("Error al configurar conector: {}", e)))
            }.into_actor(self));
        }

        // Cambiar el estado a Running
        self.status = ActorStatus::Running;

        // Reportar inicio de extracción
        self.report_progress(&msg.workflow_id, WorkflowPhase::Extracting, 0.0, "Iniciando extracción");

        // Crear una copia de los valores necesarios para el futuro
        let workflow_id = msg.workflow_id.clone();

        // Iniciar la extracción en un futuro
        let fut = async move {
            // Aquí se implementaría la lógica real de extracción
            // Por ahora, simulamos la extracción con un lote de ejemplo

            // En una implementación real, esto llamaría a connector.read() y procesaría
            // los resultados en lotes

            // Simulamos un lote de datos
            let records = (0..10).map(|i| {
                dataflare_core::message::DataRecord::new(serde_json::json!({
                    "id": i,
                    "name": format!("Record {}", i),
                    "value": i * 10
                }))
            }).collect::<Vec<_>>();

            let batch = DataRecordBatch::new(records);

            // En una implementación real, esto se enviaría al ProcessorActor
            // Por ahora, simplemente reportamos que se ha procesado el lote
            log::info!("Lote extraído con {} registros", batch.records.len());

            // Reportar finalización de extracción
            Ok(())
        };

        Box::pin(fut.into_actor(self).map(move |result: Result<()>, actor, _ctx| {
            match result {
                Ok(_) => {
                    actor.report_progress(&workflow_id, WorkflowPhase::Extracting, 1.0, "Extracción completada");
                    actor.status = ActorStatus::Initialized;
                    Ok(())
                },
                Err(e) => {
                    error!("Error en extracción: {}", e);
                    actor.report_progress(&workflow_id, WorkflowPhase::Error, 0.0, &format!("Error en extracción: {}", e));
                    actor.status = ActorStatus::Error(e.to_string());
                    Err(e)
                }
            }
        }))
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
