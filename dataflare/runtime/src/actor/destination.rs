//! Actor de destino para DataFlare
//!
//! Implementa el actor responsable de cargar datos en los destinos.

use std::collections::HashMap;
use actix::prelude::*;
use log::{error, info};
use chrono::Utc;

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{LoadBatch, WorkflowPhase, WorkflowProgress},
};
use dataflare_connector::destination::DestinationConnector;

use crate::actor::{DataFlareActor, Initialize, Finalize, Pause, Resume, GetStatus, ActorStatus, SendBatch, SubscribeProgress, UnsubscribeProgress};

/// Actor que gestiona la carga de datos en un destino
pub struct DestinationActor {
    /// ID del actor
    id: String,

    /// Conector de destino
    connector: Box<dyn DestinationConnector>,

    /// Estado actual del actor
    status: ActorStatus,

    /// Configuración actual
    config: Option<serde_json::Value>,

    /// Destinatarios para notificaciones de progreso
    progress_recipients: HashMap<String, Vec<Recipient<WorkflowProgress>>>,

    /// Contador de registros procesados
    records_processed: u64,
}

impl DestinationActor {
    /// Crea un nuevo actor de destino
    pub fn new<S: Into<String>>(id: S, connector: Box<dyn DestinationConnector>) -> Self {
        Self {
            id: id.into(),
            connector,
            status: ActorStatus::Initialized,
            config: None,
            progress_recipients: HashMap::new(),
            records_processed: 0,
        }
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

impl Actor for DestinationActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("DestinationActor {} iniciado", self.id);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("DestinationActor {} detenido", self.id);
    }
}

impl DataFlareActor for DestinationActor {
    fn get_id(&self) -> &str {
        &self.id
    }

    fn get_type(&self) -> &str {
        "destination"
    }

    fn initialize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Inicializando DestinationActor {}", self.id);
        self.status = ActorStatus::Initialized;
        Ok(())
    }

    fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Finalizando DestinationActor {}", self.id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }

    fn report_progress(&self, workflow_id: &str, phase: WorkflowPhase, progress: f64, message: &str) {
        self.report_progress_to_subscribers(workflow_id, phase, progress, message);
    }
}

/// Implementación del handler para inicializar el actor
impl Handler<Initialize> for DestinationActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Initialize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Inicializando DestinationActor {} para workflow {}", self.id, msg.workflow_id);

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
impl Handler<Finalize> for DestinationActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Finalize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Finalizando DestinationActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }
}

/// Implementación del handler para pausar el actor
impl Handler<Pause> for DestinationActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Pause, _ctx: &mut Self::Context) -> Self::Result {
        info!("Pausando DestinationActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Paused;
        Ok(())
    }
}

/// Implementación del handler para reanudar el actor
impl Handler<Resume> for DestinationActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Resume, _ctx: &mut Self::Context) -> Self::Result {
        info!("Reanudando DestinationActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Running;
        Ok(())
    }
}

/// Implementación del handler para obtener el estado del actor
impl Handler<GetStatus> for DestinationActor {
    type Result = Result<ActorStatus>;

    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.status.clone())
    }
}

/// Implementación del handler para cargar un lote
impl Handler<LoadBatch> for DestinationActor {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: LoadBatch, _ctx: &mut Self::Context) -> Self::Result {
        info!("Cargando lote para workflow {} en destino {}", msg.workflow_id, msg.destination_id);

        // Verificar que el actor esté inicializado
        if self.status != ActorStatus::Initialized && self.status != ActorStatus::Running {
            let status = self.status.clone();
            return Box::pin(async move {
                Err(DataFlareError::Actor(format!(
                    "Actor no está en estado adecuado para carga: {:?}", status
                )))
            }.into_actor(self));
        }

        // Cambiar el estado a Running
        self.status = ActorStatus::Running;

        // Reportar inicio de carga
        self.report_progress(&msg.workflow_id, WorkflowPhase::Loading, 0.0, "Iniciando carga");

        // Crear una copia de los valores necesarios para el futuro
        let batch = msg.batch.clone();
        let _config = msg.config.clone();

        // Iniciar la carga en un futuro
        let fut = async move {
            // Aquí se implementaría la lógica real de carga
            // Por ahora, simulamos la carga

            // En una implementación real, esto llamaría a connector.write() y procesaría
            // los resultados

            // Simulamos una carga exitosa
            Ok(())
        };

        // Clonar workflow_id para el callback
        let workflow_id_for_callback = msg.workflow_id.clone();
        let batch_size = batch.records.len() as u64;

        Box::pin(fut.into_actor(self).map(move |result: Result<()>, actor, _ctx| {
            match result {
                Ok(_) => {
                    actor.report_progress(&workflow_id_for_callback, WorkflowPhase::Loading, 1.0, "Carga completada");
                    actor.status = ActorStatus::Initialized;
                    actor.records_processed += batch_size;
                    Ok(())
                },
                Err(e) => {
                    error!("Error en carga: {}", e);
                    actor.report_progress(&workflow_id_for_callback, WorkflowPhase::Error, 0.0, &format!("Error en carga: {}", e));
                    actor.status = ActorStatus::Error(e.to_string());
                    Err(e)
                }
            }
        }))
    }
}

/// Implementación del handler para recibir un lote
impl Handler<SendBatch> for DestinationActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: SendBatch, ctx: &mut Self::Context) -> Self::Result {
        info!("DestinationActor {} recibió lote para workflow {}", self.id, msg.workflow_id);

        // Verificar que el actor esté inicializado
        if self.status != ActorStatus::Initialized && self.status != ActorStatus::Running {
            return Err(DataFlareError::Actor(format!(
                "Actor no está en estado adecuado para recibir lote: {:?}", self.status
            )));
        }

        // Cargar el lote usando la configuración actual
        if let Some(config) = &self.config {
            ctx.address().do_send(LoadBatch {
                workflow_id: msg.workflow_id,
                destination_id: self.id.clone(),
                batch: msg.batch,
                config: config.clone(),
            });
        } else {
            return Err(DataFlareError::Config("Destino no configurado".to_string()));
        }

        Ok(())
    }
}

/// Implementación del handler para suscribirse a actualizaciones de progreso
impl Handler<SubscribeProgress> for DestinationActor {
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
impl Handler<UnsubscribeProgress> for DestinationActor {
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
    use crate::actor::message_bus::DataFlareMessage;
    use actix::Actor;
    
    // 简单的Mock目标连接器用于测试
    struct MockDestinationConnector {
        records_written: usize,
    }
    
    impl dataflare_core::connector::DestinationConnector for MockDestinationConnector {
        fn write(&mut self, _batch: &dataflare_core::data::DataRecordBatch) -> dataflare_core::error::Result<()> {
            self.records_written += 1;
            Ok(())
        }
        
        fn close(&mut self) -> dataflare_core::error::Result<()> {
            Ok(())
        }
    }
    
    #[test]
    fn test_destination_actor_creation() {
        let connector = Box::new(MockDestinationConnector { records_written: 0 });
        let actor = DestinationActor::new("test_dest", connector);
        assert_eq!(actor.id, "test_dest");
    }
}
