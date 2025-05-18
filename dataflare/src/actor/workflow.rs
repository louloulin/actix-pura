//! Actor de flujo de trabajo para DataFlare
//!
//! Implementa el actor responsable de coordinar la ejecución de flujos de trabajo.

use std::collections::HashMap;
use actix::prelude::*;
use log::{debug, error, info, warn};
use chrono::Utc;

use crate::{
    actor::{
        DataFlareActor, Initialize, Finalize, Pause, Resume, GetStatus, ActorStatus,
        SubscribeToProgress, UnsubscribeFromProgress
    },
    error::{DataFlareError, Result},
    message::{StartExtraction, WorkflowPhase, WorkflowProgress},
    state::SourceState,
};

/// Actor que gestiona la ejecución de flujos de trabajo
pub struct WorkflowActor {
    /// ID del actor
    id: String,
    
    /// Estado actual del actor
    status: ActorStatus,
    
    /// Configuración actual
    config: Option<serde_json::Value>,
    
    /// Actores de origen
    source_actors: HashMap<String, Addr<crate::actor::SourceActor>>,
    
    /// Actores de procesador
    processor_actors: HashMap<String, Addr<crate::actor::ProcessorActor>>,
    
    /// Actores de destino
    destination_actors: HashMap<String, Addr<crate::actor::DestinationActor>>,
    
    /// Destinatarios para notificaciones de progreso
    progress_recipients: HashMap<String, Vec<Recipient<WorkflowProgress>>>,
    
    /// Estado de las fuentes
    source_states: HashMap<String, SourceState>,
}

impl WorkflowActor {
    /// Crea un nuevo actor de flujo de trabajo
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self {
            id: id.into(),
            status: ActorStatus::Initialized,
            config: None,
            source_actors: HashMap::new(),
            processor_actors: HashMap::new(),
            destination_actors: HashMap::new(),
            progress_recipients: HashMap::new(),
            source_states: HashMap::new(),
        }
    }
    
    /// Agrega un actor de origen
    pub fn add_source_actor(&mut self, id: String, actor: Addr<crate::actor::SourceActor>) {
        self.source_actors.insert(id, actor);
    }
    
    /// Agrega un actor de procesador
    pub fn add_processor_actor(&mut self, id: String, actor: Addr<crate::actor::ProcessorActor>) {
        self.processor_actors.insert(id, actor);
    }
    
    /// Agrega un actor de destino
    pub fn add_destination_actor(&mut self, id: String, actor: Addr<crate::actor::DestinationActor>) {
        self.destination_actors.insert(id, actor);
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

impl Actor for WorkflowActor {
    type Context = Context<Self>;
    
    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("WorkflowActor {} iniciado", self.id);
    }
    
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("WorkflowActor {} detenido", self.id);
    }
}

impl DataFlareActor for WorkflowActor {
    fn get_id(&self) -> &str {
        &self.id
    }
    
    fn get_type(&self) -> &str {
        "workflow"
    }
    
    fn initialize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Inicializando WorkflowActor {}", self.id);
        self.status = ActorStatus::Initialized;
        Ok(())
    }
    
    fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Finalizando WorkflowActor {}", self.id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }
    
    fn report_progress(&self, workflow_id: &str, phase: WorkflowPhase, progress: f64, message: &str) {
        self.report_progress_to_subscribers(workflow_id, phase, progress, message);
    }
}

/// Mensaje para ejecutar un flujo de trabajo
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct ExecuteWorkflow {
    /// ID del flujo de trabajo
    pub workflow_id: String,
    
    /// Configuración del flujo de trabajo
    pub config: serde_json::Value,
}

/// Implementación del handler para inicializar el actor
impl Handler<Initialize> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: Initialize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Inicializando WorkflowActor {} para workflow {}", self.id, msg.workflow_id);
        
        // Guardar la configuración
        self.config = Some(msg.config);
        self.status = ActorStatus::Initialized;
        
        Ok(())
    }
}

/// Implementación del handler para finalizar el actor
impl Handler<Finalize> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: Finalize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Finalizando WorkflowActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }
}

/// Implementación del handler para pausar el actor
impl Handler<Pause> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: Pause, _ctx: &mut Self::Context) -> Self::Result {
        info!("Pausando WorkflowActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Paused;
        Ok(())
    }
}

/// Implementación del handler para reanudar el actor
impl Handler<Resume> for WorkflowActor {
    type Result = Result<()>;
    
    fn handle(&mut self, msg: Resume, _ctx: &mut Self::Context) -> Self::Result {
        info!("Reanudando WorkflowActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Running;
        Ok(())
    }
}

/// Implementación del handler para obtener el estado del actor
impl Handler<GetStatus> for WorkflowActor {
    type Result = Result<ActorStatus>;
    
    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.status.clone())
    }
}

/// Implementación del handler para ejecutar un flujo de trabajo
impl Handler<ExecuteWorkflow> for WorkflowActor {
    type Result = ResponseActFuture<Self, Result<()>>;
    
    fn handle(&mut self, msg: ExecuteWorkflow, ctx: &mut Self::Context) -> Self::Result {
        info!("Ejecutando flujo de trabajo {}", msg.workflow_id);
        
        // Verificar que el actor esté inicializado
        if self.status != ActorStatus::Initialized && self.status != ActorStatus::Running {
            return Box::pin(async move {
                Err(DataFlareError::Actor(format!(
                    "Actor no está en estado adecuado para ejecución: {:?}", self.status
                )))
            }.into_actor(self));
        }
        
        // Cambiar el estado a Running
        self.status = ActorStatus::Running;
        
        // Reportar inicio de flujo de trabajo
        self.report_progress(&msg.workflow_id, WorkflowPhase::Initializing, 0.0, "Iniciando flujo de trabajo");
        
        // Crear una copia de los valores necesarios para el futuro
        let workflow_id = msg.workflow_id.clone();
        let config = msg.config.clone();
        let source_actors = self.source_actors.clone();
        let source_states = self.source_states.clone();
        
        // Iniciar la ejecución del flujo de trabajo en un futuro
        let fut = async move {
            // Aquí se implementaría la lógica real de ejecución del flujo de trabajo
            // Por ahora, simulamos la ejecución
            
            // Iniciar la extracción en cada fuente
            for (source_id, source_actor) in source_actors {
                // Obtener la configuración de la fuente desde la configuración del flujo de trabajo
                let source_config = if let Some(sources) = config.get("sources").and_then(|s| s.as_object()) {
                    if let Some(src_config) = sources.get(&source_id) {
                        src_config.clone()
                    } else {
                        return Err(DataFlareError::Config(format!("Configuración no encontrada para fuente {}", source_id)));
                    }
                } else {
                    return Err(DataFlareError::Config("Configuración de fuentes no encontrada".to_string()));
                };
                
                // Obtener el estado previo de la fuente
                let source_state = source_states.get(&source_id).cloned();
                
                // Iniciar la extracción
                source_actor.send(StartExtraction {
                    workflow_id: workflow_id.clone(),
                    source_id: source_id.clone(),
                    config: source_config,
                    state: source_state,
                }).await.map_err(|e| DataFlareError::Actor(format!("Error al enviar mensaje a fuente: {}", e)))??;
            }
            
            Ok(())
        };
        
        Box::pin(fut.into_actor(self).map(move |result, actor, _ctx| {
            match result {
                Ok(_) => {
                    actor.report_progress(&workflow_id, WorkflowPhase::Completed, 1.0, "Flujo de trabajo completado");
                    actor.status = ActorStatus::Initialized;
                    Ok(())
                },
                Err(e) => {
                    error!("Error en flujo de trabajo: {}", e);
                    actor.report_progress(&workflow_id, WorkflowPhase::Error, 0.0, &format!("Error en flujo de trabajo: {}", e));
                    actor.status = ActorStatus::Error(e.to_string());
                    Err(e)
                }
            }
        }))
    }
}

/// Implementación del handler para suscribirse a actualizaciones de progreso
impl Handler<SubscribeToProgress> for WorkflowActor {
    type Result = ();
    
    fn handle(&mut self, msg: SubscribeToProgress, _ctx: &mut Self::Context) -> Self::Result {
        let recipients = self.progress_recipients
            .entry(msg.workflow_id.clone())
            .or_insert_with(Vec::new);
        
        recipients.push(msg.recipient);
    }
}

/// Implementación del handler para cancelar la suscripción a actualizaciones de progreso
impl Handler<UnsubscribeFromProgress> for WorkflowActor {
    type Result = ();
    
    fn handle(&mut self, msg: UnsubscribeFromProgress, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(recipients) = self.progress_recipients.get_mut(&msg.workflow_id) {
            recipients.retain(|r| r != &msg.recipient);
        }
    }
}

/// Implementación del handler para recibir actualizaciones de progreso
impl Handler<WorkflowProgress> for WorkflowActor {
    type Result = ();
    
    fn handle(&mut self, msg: WorkflowProgress, _ctx: &mut Self::Context) -> Self::Result {
        // Reenviar la actualización de progreso a los suscriptores
        self.report_progress_to_subscribers(&msg.workflow_id, msg.phase.clone(), msg.progress, &msg.message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::prelude::*;
    
    #[actix::test]
    async fn test_workflow_actor_initialization() {
        let workflow_actor = WorkflowActor::new("test-workflow");
        let addr = workflow_actor.start();
        
        let result = addr.send(Initialize {
            workflow_id: "test-workflow".to_string(),
            config: serde_json::json!({}),
        }).await.unwrap();
        
        assert!(result.is_ok());
        
        let status = addr.send(GetStatus).await.unwrap().unwrap();
        assert_eq!(status, ActorStatus::Initialized);
    }
}
