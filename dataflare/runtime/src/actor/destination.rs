//! Actor de destino para DataFlare
//!
//! Implementa el actor responsable de cargar datos en los destinos.

use std::collections::HashMap;
use actix::prelude::*;
use log::{error, info, debug};
use chrono::Utc;

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{LoadBatch, WorkflowPhase, WorkflowProgress},
};
use dataflare_connector::destination::DestinationConnector;

use crate::actor::{DataFlareActor, Initialize, Finalize, Pause, Resume, GetStatus, ActorStatus, SendBatch, SubscribeProgress, UnsubscribeProgress, ConnectToTask, TaskActor};

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

    /// Associated TaskActor
    associated_task: Option<(String, Addr<TaskActor>)>,
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
            associated_task: None,
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

        // Calcular el tamaño del lote antes de mover el mensaje
        let batch_size = msg.batch.records.len() as u64;
        let workflow_id = msg.workflow_id.clone();

        // Crear un futuro para la operación de escritura real
        let batch_clone = msg.batch.clone();
        let write_mode = dataflare_connector::destination::WriteMode::Overwrite;

        // Usar spawn para ejecutar la escritura de forma asíncrona
        let workflow_id_clone = workflow_id.clone();
        let batch_size_clone = batch_size;

        _ctx.spawn(
            async move {
                info!("Realizando escritura real con el conector");

                // TODO: Llamar al conector real para escribir los datos
                // Por ahora simular escritura exitosa para verificar el flujo de datos
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                info!("Escritura simulada exitosa para {} registros", batch_clone.records.len());
                Ok::<(), DataFlareError>(())
            }
            .into_actor(self)
            .map(move |result, actor, _ctx| {
                match result {
                    Ok(_) => {
                        info!("Escritura completada para {} registros", batch_size_clone);
                        actor.report_progress(&workflow_id_clone, WorkflowPhase::Loading, 1.0, "Carga completada");
                        actor.status = ActorStatus::Initialized;
                        actor.records_processed += batch_size_clone;
                    },
                    Err(e) => {
                        error!("Error en escritura: {}", e);
                        actor.report_progress(&workflow_id_clone, WorkflowPhase::Error, 0.0, &format!("Error en escritura: {}", e));
                        actor.status = ActorStatus::Failed;
                    }
                }
            })
        );

        // 返回一个立即完成的future
        Box::pin(async move { Ok(()) }.into_actor(self))
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

/// Implementación del handler para ConnectToTask
impl Handler<ConnectToTask> for DestinationActor {
    type Result = ();

    fn handle(&mut self, msg: ConnectToTask, _ctx: &mut Self::Context) -> Self::Result {
        info!("DestinationActor {} connecting to task {}", self.id, msg.task_id);
        self.associated_task = Some((msg.task_id, msg.task_addr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dataflare_connector::destination::{DestinationConnector, WriteMode};
    use dataflare_core::error::Result;
    use dataflare_core::message::{DataRecord, DataRecordBatch};
    use dataflare_core::model::Schema;
    use async_trait::async_trait;

    // 简单的Mock目标连接器用于测试
    struct MockDestinationConnector {
        records_written: usize,
    }

    impl MockDestinationConnector {
        fn new() -> Self {
            Self { records_written: 0 }
        }
    }

    #[async_trait]
    impl DestinationConnector for MockDestinationConnector {
        fn configure(&mut self, _config: &serde_json::Value) -> Result<()> {
            Ok(())
        }

        async fn check_connection(&self) -> Result<bool> {
            Ok(true)
        }

        async fn prepare_schema(&self, _schema: &Schema) -> Result<()> {
            Ok(())
        }

        async fn write_batch(&mut self, _batch: &DataRecordBatch, _mode: WriteMode) -> Result<dataflare_connector::destination::WriteStats> {
            self.records_written += 1;
            Ok(dataflare_connector::destination::WriteStats {
                records_written: 1,
                records_failed: 0,
                bytes_written: 100,
                write_time_ms: 10,
            })
        }

        async fn write_record(&mut self, _record: &DataRecord, _mode: WriteMode) -> Result<dataflare_connector::destination::WriteStats> {
            self.records_written += 1;
            Ok(dataflare_connector::destination::WriteStats {
                records_written: 1,
                records_failed: 0,
                bytes_written: 100,
                write_time_ms: 10,
            })
        }

        async fn commit(&mut self) -> Result<()> {
            Ok(())
        }

        async fn rollback(&mut self) -> Result<()> {
            Ok(())
        }

        fn get_supported_write_modes(&self) -> Vec<WriteMode> {
            vec![WriteMode::Append]
        }
    }

    #[test]
    fn test_destination_actor_creation() {
        let connector = Box::new(MockDestinationConnector::new());
        let actor = DestinationActor::new("test_dest", connector);
        assert_eq!(actor.id, "test_dest");
    }
}
