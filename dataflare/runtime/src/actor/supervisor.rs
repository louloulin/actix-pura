//! Actor supervisor para DataFlare
//!
//! Implementa el actor responsable de supervisar y gestionar otros actores.

use std::collections::HashMap;
use actix::prelude::*;
use actix::dev::ToEnvelope;
use log::{error, info, warn};
use chrono::{DateTime, Duration, Utc};

use crate::actor::{
    SourceActor, ProcessorActor, DestinationActor, WorkflowActor,
    DataFlareActor, Initialize, Finalize, GetStatus, ActorStatus
};

use dataflare_core::error::{DataFlareError, Result};

/// Tipo de actor supervisado
#[derive(Clone)]
enum SupervisedActor {
    Source(Addr<SourceActor>),
    Processor(Addr<ProcessorActor>),
    Destination(Addr<DestinationActor>),
    Workflow(Addr<WorkflowActor>),
}

impl SupervisedActor {
    async fn send_status(&self) -> std::result::Result<Result<ActorStatus>, MailboxError> {
        match self {
            SupervisedActor::Source(addr) => addr.send(GetStatus).await,
            SupervisedActor::Processor(addr) => addr.send(GetStatus).await,
            SupervisedActor::Destination(addr) => addr.send(GetStatus).await,
            SupervisedActor::Workflow(addr) => addr.send(GetStatus).await,
        }
    }

    async fn send_initialize(&self, workflow_id: String, config: serde_json::Value) -> std::result::Result<Result<()>, MailboxError> {
        let msg = Initialize {
            workflow_id,
            config,
        };

        match self {
            SupervisedActor::Source(addr) => addr.send(msg).await,
            SupervisedActor::Processor(addr) => addr.send(msg).await,
            SupervisedActor::Destination(addr) => addr.send(msg).await,
            SupervisedActor::Workflow(addr) => addr.send(msg).await,
        }
    }

    async fn send_finalize(&self, workflow_id: String) -> std::result::Result<Result<()>, MailboxError> {
        let msg = Finalize {
            workflow_id,
        };

        match self {
            SupervisedActor::Source(addr) => addr.send(msg).await,
            SupervisedActor::Processor(addr) => addr.send(msg).await,
            SupervisedActor::Destination(addr) => addr.send(msg).await,
            SupervisedActor::Workflow(addr) => addr.send(msg).await,
        }
    }
}

/// Actor que supervisa y gestiona otros actores
pub struct SupervisorActor {
    /// ID del actor
    id: String,

    /// Estado actual del actor
    status: ActorStatus,

    /// Configuración actual
    config: Option<serde_json::Value>,

    /// Actores supervisados
    supervised_actors: HashMap<String, SupervisedActor>,

    /// Estado de los actores supervisados
    actor_states: HashMap<String, ActorStatus>,

    /// Última vez que se verificó cada actor
    last_checked: HashMap<String, DateTime<Utc>>,

    /// Intervalo de verificación (en milisegundos)
    check_interval_ms: u64,
}

impl SupervisorActor {
    /// Crea un nuevo actor supervisor
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self {
            id: id.into(),
            status: ActorStatus::Initialized,
            config: None,
            supervised_actors: HashMap::new(),
            actor_states: HashMap::new(),
            last_checked: HashMap::new(),
            check_interval_ms: 5000, // 5 segundos por defecto
        }
    }

    /// Establece el intervalo de verificación
    pub fn with_check_interval(mut self, interval_ms: u64) -> Self {
        self.check_interval_ms = interval_ms;
        self
    }

    /// Agrega un actor para supervisar
    pub fn add_supervised_actor<A>(&mut self, id: String, actor: Addr<A>)
    where
        A: Actor + DataFlareActor + Handler<GetStatus> + 'static,
        A::Context: ToEnvelope<A, GetStatus>,
    {
        // Almacenar la dirección del actor según su tipo
        let supervised_actor = match std::any::TypeId::of::<A>() {
            id if id == std::any::TypeId::of::<SourceActor>() => {
                // Es seguro porque verificamos el tipo
                let source_addr = unsafe { std::mem::transmute::<Addr<A>, Addr<SourceActor>>(actor) };
                SupervisedActor::Source(source_addr)
            },
            id if id == std::any::TypeId::of::<ProcessorActor>() => {
                // Es seguro porque verificamos el tipo
                let processor_addr = unsafe { std::mem::transmute::<Addr<A>, Addr<ProcessorActor>>(actor) };
                SupervisedActor::Processor(processor_addr)
            },
            id if id == std::any::TypeId::of::<DestinationActor>() => {
                // Es seguro porque verificamos el tipo
                let dest_addr = unsafe { std::mem::transmute::<Addr<A>, Addr<DestinationActor>>(actor) };
                SupervisedActor::Destination(dest_addr)
            },
            id if id == std::any::TypeId::of::<WorkflowActor>() => {
                // Es seguro porque verificamos el tipo
                let workflow_addr = unsafe { std::mem::transmute::<Addr<A>, Addr<WorkflowActor>>(actor) };
                SupervisedActor::Workflow(workflow_addr)
            },
            _ => {
                error!("Tipo de actor no soportado para supervisión: {}", std::any::type_name::<A>());
                return;
            }
        };

        self.supervised_actors.insert(id.clone(), supervised_actor);
        self.actor_states.insert(id.clone(), ActorStatus::Initialized);
        self.last_checked.insert(id, Utc::now());
    }

    /// Verifica el estado de los actores supervisados
    fn check_supervised_actors(&mut self, ctx: &mut <Self as Actor>::Context) {
        let now = Utc::now();

        for (id, actor) in &self.supervised_actors {
            // Verificar si es tiempo de verificar este actor
            if let Some(last_checked) = self.last_checked.get(id) {
                let elapsed = now - *last_checked;
                if elapsed < Duration::milliseconds(self.check_interval_ms as i64) {
                    continue;
                }
            }

            // Actualizar la marca de tiempo de verificación
            self.last_checked.insert(id.clone(), now);

            // Enviar mensaje para obtener el estado
            let actor_id = id.clone();
            let actor_clone = actor.clone();

            // Crear un futuro para obtener el estado
            let fut = async move {
                let status_result = actor_clone.send_status().await;
                (actor_id, status_result)
            };

            // Convertir a futuro de actor y manejar el resultado
            ctx.spawn(fut.into_actor(self).map(move |(actor_id, res), actor, _ctx| {
                match res {
                    Ok(Ok(status)) => {
                        // Actualizar el estado del actor
                        actor.actor_states.insert(actor_id.clone(), status.clone());

                        // Verificar si el actor está en error
                        if let ActorStatus::Error(ref error) = status {
                            warn!("Actor {} en estado de error: {}", actor_id, error);
                            // Aquí se implementaría la lógica de recuperación
                        }
                    },
                    Ok(Err(e)) => {
                        error!("Error al obtener estado del actor {}: {}", actor_id, e);
                        actor.actor_states.insert(actor_id.clone(), ActorStatus::Error(format!("Error al obtener estado: {}", e)));
                    },
                    Err(e) => {
                        error!("Error de comunicación con actor {}: {}", actor_id, e);
                        actor.actor_states.insert(actor_id.clone(), ActorStatus::Error(format!("Error de comunicación: {}", e)));
                    }
                }
            }));
        }
    }
}

impl Actor for SupervisorActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("SupervisorActor {} iniciado", self.id);

        // Programar verificación periódica de actores
        ctx.run_interval(std::time::Duration::from_millis(self.check_interval_ms), |act, ctx| {
            act.check_supervised_actors(ctx);
        });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!("SupervisorActor {} detenido", self.id);
    }
}

impl DataFlareActor for SupervisorActor {
    fn get_id(&self) -> &str {
        &self.id
    }

    fn get_type(&self) -> &str {
        "supervisor"
    }

    fn initialize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Inicializando SupervisorActor {}", self.id);
        self.status = ActorStatus::Initialized;
        Ok(())
    }

    fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
        info!("Finalizando SupervisorActor {}", self.id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }

    fn report_progress(&self, _workflow_id: &str, _phase: dataflare_core::message::WorkflowPhase, _progress: f64, _message: &str) {
        // El supervisor no reporta progreso
    }
}

/// Implementación del handler para inicializar el actor
impl Handler<Initialize> for SupervisorActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Initialize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Inicializando SupervisorActor {} para workflow {}", self.id, msg.workflow_id);

        // Guardar la configuración
        self.config = Some(msg.config);
        self.status = ActorStatus::Initialized;

        Ok(())
    }
}

/// Implementación del handler para finalizar el actor
impl Handler<Finalize> for SupervisorActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: Finalize, _ctx: &mut Self::Context) -> Self::Result {
        info!("Finalizando SupervisorActor {} para workflow {}", self.id, msg.workflow_id);
        self.status = ActorStatus::Finalized;
        Ok(())
    }
}

/// Implementación del handler para obtener el estado del actor
impl Handler<GetStatus> for SupervisorActor {
    type Result = Result<ActorStatus>;

    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.status.clone())
    }
}

/// Mensaje para obtener el estado de todos los actores supervisados
#[derive(Message)]
#[rtype(result = "Result<HashMap<String, ActorStatus>>")]
pub struct GetSupervisedActorsStatus;

/// Implementación del handler para obtener el estado de todos los actores supervisados
impl Handler<GetSupervisedActorsStatus> for SupervisorActor {
    type Result = Result<HashMap<String, ActorStatus>>;

    fn handle(&mut self, _msg: GetSupervisedActorsStatus, _ctx: &mut Self::Context) -> Self::Result {
        Ok(self.actor_states.clone())
    }
}

/// Mensaje para reiniciar un actor supervisado
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct RestartActor {
    /// ID del actor a reiniciar
    pub actor_id: String,
}

/// Implementación del handler para reiniciar un actor supervisado
impl Handler<RestartActor> for SupervisorActor {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: RestartActor, _ctx: &mut Self::Context) -> Self::Result {
        info!("Reiniciando actor {}", msg.actor_id);

        // Verificar que el actor exista
        if !self.supervised_actors.contains_key(&msg.actor_id) {
            return Box::pin(async move {
                Err(DataFlareError::Actor(format!("Actor no encontrado: {}", msg.actor_id)))
            }.into_actor(self));
        }

        // Obtener el actor
        let actor = self.supervised_actors.get(&msg.actor_id).cloned();
        let actor_id = msg.actor_id.clone();
        let actor_id_for_fut = actor_id.clone();

        // Reiniciar el actor
        let fut = async move {
            if let Some(actor) = actor {
                // Primero finalizar el actor
                actor.send_finalize("supervisor".to_string()).await
                    .map_err(|e| DataFlareError::Actor(format!("Error al finalizar actor: {}", e)))??;

                // Luego inicializar el actor
                actor.send_initialize("supervisor".to_string(), serde_json::json!({})).await
                    .map_err(|e| DataFlareError::Actor(format!("Error al inicializar actor: {}", e)))??;

                Ok(())
            } else {
                Err(DataFlareError::Actor(format!("Actor no encontrado: {}", actor_id_for_fut)))
            }
        };

        Box::pin(fut.into_actor(self).map(move |result, actor, _ctx| {
            match result {
                Ok(_) => {
                    info!("Actor {} reiniciado exitosamente", actor_id);
                    actor.actor_states.insert(actor_id.clone(), ActorStatus::Initialized);
                    Ok(())
                },
                Err(e) => {
                    error!("Error al reiniciar actor {}: {}", actor_id, e);
                    actor.actor_states.insert(actor_id.clone(), ActorStatus::Error(format!("Error al reiniciar: {}", e)));
                    Err(e)
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;
    use std::time::Duration;

    #[test]
    fn test_supervisor_actor_creation() {
        let supervisor = SupervisorActor::new("test_supervisor");
        assert_eq!(supervisor.id, "test_supervisor");
        assert_eq!(supervisor.supervised_actors.len(), 0);
    }

    // 添加更多简单测试...
}
