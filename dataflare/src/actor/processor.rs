//! Actor de procesador para DataFlare
//!
//! Implementa el actor responsable de transformar los datos.

use std::collections::HashMap;
use actix::prelude::*;
use log::{debug, error, info, warn};
use chrono::Utc;

use crate::{
    actor::{DataFlareActor, Initialize, Finalize, Pause, Resume, GetStatus, ActorStatus, SendBatch},
    error::{DataFlareError, Result},
    message::{DataRecordBatch, ProcessBatch, WorkflowPhase, WorkflowProgress},
    processor::Processor,
};

/// Actor que gestiona la transformación de datos
pub struct ProcessorActor {
    /// ID del actor
    id: String,

    /// Procesador de datos
    processor: Box<dyn Processor>,

    /// Estado actual del actor
    status: ActorStatus,

    /// Configuración actual
    config: Option<serde_json::Value>,

    /// Destinatarios para notificaciones de progreso
    progress_recipients: HashMap<String, Vec<Recipient<WorkflowProgress>>>,

    /// Destinatario para enviar datos procesados
    next_actor: Option<Recipient<SendBatch>>,

    /// Contador de registros procesados
    records_processed: u64,
}

impl ProcessorActor {
    /// Crea un nuevo actor de procesador
    pub fn new<S: Into<String>>(id: S, processor: Box<dyn Processor>) -> Self {
        Self {
            id: id.into(),
            processor,
            status: ActorStatus::Initialized,
            config: None,
            progress_recipients: HashMap::new(),
            next_actor: None,
            records_processed: 0,
        }
    }

    /// Establece el siguiente actor en el flujo
    pub fn with_next_actor(mut self, next_actor: Recipient<SendBatch>) -> Self {
        self.next_actor = Some(next_actor);
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

impl Actor for ProcessorActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("ProcessorActor {} iniciado", self.id);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("ProcessorActor {} detenido", self.id);
    }
}

impl DataFlareActor for ProcessorActor {
    fn get_id(&self) -> &str {
        &self.id
    }

    fn get_type(&self) -> &str {
        "processor"
    }

    fn initialize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Inicializando ProcessorActor {}", self.id);
        self.status = ActorStatus::Initialized;
        Ok(())
    }

    fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Finalizando ProcessorActor {}", self.id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }

    fn report_progress(&self, workflow_id: &str, phase: WorkflowPhase, progress: f64, message: &str) {
        self.report_progress_to_subscribers(workflow_id, phase, progress, message);
    }
}

/// Implementación del handler para inicializar el actor
impl Handler<Initialize> for ProcessorActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Initialize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Inicializando ProcessorActor {} para workflow {}", self.id, msg.workflow_id);

        // Configurar el procesador
        self.processor.configure(&msg.config)
            .map_err(|e| DataFlareError::Config(format!("Error al configurar procesador: {}", e)))?;

        // Guardar la configuración
        self.config = Some(msg.config);
        self.status = ActorStatus::Initialized;

        Ok(())
    }
}

/// Implementación del handler para finalizar el actor
impl Handler<Finalize> for ProcessorActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Finalize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Finalizando ProcessorActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }
}

/// Implementación del handler para pausar el actor
impl Handler<Pause> for ProcessorActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Pause, _ctx: &mut Self::Context) -> Self::Result {
        info!("Pausando ProcessorActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Paused;
        Ok(())
    }
}

/// Implementación del handler para reanudar el actor
impl Handler<Resume> for ProcessorActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Resume, _ctx: &mut Self::Context) -> Self::Result {
        info!("Reanudando ProcessorActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Running;
        Ok(())
    }
}

/// Implementación del handler para obtener el estado del actor
impl Handler<GetStatus> for ProcessorActor {
    type Result = Result<ActorStatus>;

    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.status.clone())
    }
}

/// Implementación del handler para procesar un lote
impl Handler<ProcessBatch> for ProcessorActor {
    type Result = ResponseActFuture<Self, Result<DataRecordBatch>>;

    fn handle(&mut self, msg: ProcessBatch, _ctx: &mut Self::Context) -> Self::Result {
        info!("Procesando lote para workflow {} con procesador {}", msg.workflow_id, msg.processor_id);

        // Verificar que el actor esté inicializado
        if self.status != ActorStatus::Initialized && self.status != ActorStatus::Running {
            let status = self.status.clone();
            return Box::pin(async move {
                Err(DataFlareError::Actor(format!(
                    "Actor no está en estado adecuado para procesamiento: {:?}", status
                )))
            }.into_actor(self));
        }

        // Cambiar el estado a Running
        self.status = ActorStatus::Running;

        // Reportar inicio de procesamiento
        self.report_progress(&msg.workflow_id, WorkflowPhase::Transforming, 0.0, "Iniciando procesamiento");

        // Crear una copia de los valores necesarios para el futuro
        let workflow_id_clone = msg.workflow_id.clone();
        let batch = msg.batch.clone();
        let _config = msg.config.clone();
        let next_actor = self.next_actor.clone();

        // Iniciar el procesamiento en un futuro
        let fut = async move {
            // Aquí se implementaría la lógica real de procesamiento
            // Por ahora, simulamos el procesamiento

            // En una implementación real, esto llamaría a processor.process() y procesaría
            // los resultados

            // Simulamos procesamiento modificando los registros
            let processed_records = batch.records.into_iter().map(|mut record| {
                // Simulamos una transformación simple
                if let serde_json::Value::Object(ref mut map) = record.data {
                    map.insert("processed".to_string(), serde_json::Value::Bool(true));
                    map.insert("processor_id".to_string(), serde_json::Value::String(msg.processor_id.clone()));
                }
                record
            }).collect::<Vec<_>>();

            let processed_batch = DataRecordBatch::new(processed_records);

            // Si hay un actor siguiente, enviar el lote procesado
            if let Some(next) = next_actor {
                next.send(SendBatch {
                    workflow_id: workflow_id_clone.clone(),
                    batch: processed_batch.clone(),
                }).await.map_err(|e| DataFlareError::Actor(format!("Error al enviar lote procesado: {}", e)))?;
            }

            Ok(processed_batch)
        };

        // Clonar workflow_id para el callback
        let workflow_id_for_callback = msg.workflow_id.clone();

        Box::pin(fut.into_actor(self).map(move |result: Result<DataRecordBatch>, actor, _ctx| {
            match result {
                Ok(batch) => {
                    actor.report_progress(&workflow_id_for_callback, WorkflowPhase::Transforming, 1.0, "Procesamiento completado");
                    actor.status = ActorStatus::Initialized;
                    actor.records_processed += batch.records.len() as u64;
                    Ok(batch)
                },
                Err(e) => {
                    error!("Error en procesamiento: {}", e);
                    actor.report_progress(&workflow_id_for_callback, WorkflowPhase::Error, 0.0, &format!("Error en procesamiento: {}", e));
                    actor.status = ActorStatus::Error(e.to_string());
                    Err(e)
                }
            }
        }))
    }
}

/// Implementación del handler para recibir un lote
impl Handler<SendBatch> for ProcessorActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: SendBatch, ctx: &mut Self::Context) -> Self::Result {
        info!("ProcessorActor {} recibió lote para workflow {}", self.id, msg.workflow_id);

        // Verificar que el actor esté inicializado
        if self.status != ActorStatus::Initialized && self.status != ActorStatus::Running {
            return Err(DataFlareError::Actor(format!(
                "Actor no está en estado adecuado para recibir lote: {:?}", self.status
            )));
        }

        // Procesar el lote usando la configuración actual
        if let Some(config) = &self.config {
            ctx.address().do_send(ProcessBatch {
                workflow_id: msg.workflow_id,
                processor_id: self.id.clone(),
                batch: msg.batch,
                config: config.clone(),
            });
        } else {
            return Err(DataFlareError::Config("Procesador no configurado".to_string()));
        }

        Ok(())
    }
}

/// Implementación del handler para suscribirse a actualizaciones de progreso
impl Handler<crate::actor::SubscribeToProgress> for ProcessorActor {
    type Result = ();

    fn handle(&mut self, msg: crate::actor::SubscribeToProgress, _ctx: &mut Self::Context) -> Self::Result {
        let recipients = self.progress_recipients
            .entry(msg.workflow_id.clone())
            .or_insert_with(Vec::new);

        recipients.push(msg.recipient);
    }
}

/// Implementación del handler para cancelar la suscripción a actualizaciones de progreso
impl Handler<crate::actor::UnsubscribeFromProgress> for ProcessorActor {
    type Result = ();

    fn handle(&mut self, msg: crate::actor::UnsubscribeFromProgress, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(recipients) = self.progress_recipients.get_mut(&msg.workflow_id) {
            recipients.retain(|r| r != &msg.recipient);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::MockProcessor;

    #[actix::test]
    async fn test_processor_actor_initialization() {
        let mut mock_processor = MockProcessor::new();
        mock_processor.expect_configure()
            .returning(|_| Ok(()));

        let processor_actor = ProcessorActor::new("test-processor", Box::new(mock_processor));
        let addr = processor_actor.start();

        let result = addr.send(Initialize {
            workflow_id: "test-workflow".to_string(),
            config: serde_json::json!({}),
        }).await.unwrap();

        assert!(result.is_ok());

        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert_eq!(status, ActorStatus::Initialized);
    }
}
