//! Actor supervisor para DataFlare
//!
//! Implementa el actor responsable de supervisar y gestionar otros actores.

use std::collections::HashMap;
use actix::prelude::*;
use log::{debug, error, info, warn};
use chrono::{DateTime, Duration, Utc};

use crate::{
    actor::{DataFlareActor, Initialize, Finalize, GetStatus, ActorStatus},
    error::{DataFlareError, Result},
};

/// Actor que supervisa y gestiona otros actores
pub struct SupervisorActor {
    /// ID del actor
    id: String,

    /// Estado actual del actor
    status: ActorStatus,

    /// Configuración actual
    config: Option<serde_json::Value>,

    /// Actores supervisados (usando Any para evitar problemas con dyn)
    supervised_actors: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,

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
        A: Actor + DataFlareActor + 'static,
        A::Context: ToEnvelope<A, GetStatus>,
    {
        // Almacenar la dirección del actor como Box<dyn Any>
        self.supervised_actors.insert(id.clone(), Box::new(actor.clone()));
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
            let fut = actor.send(GetStatus)
                .into_actor(self)
                .map(move |res, actor, _ctx| {
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
                });

            ctx.spawn(fut);
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

    fn report_progress(&self, _workflow_id: &str, _phase: crate::message::WorkflowPhase, _progress: f64, _message: &str) {
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

        // Reiniciar el actor
        let fut = async move {
            if let Some(actor) = actor {
                // Primero finalizar el actor
                actor.send(Finalize {
                    workflow_id: "supervisor".to_string(),
                }).await.map_err(|e| DataFlareError::Actor(format!("Error al finalizar actor: {}", e)))??;

                // Luego inicializar el actor
                actor.send(Initialize {
                    workflow_id: "supervisor".to_string(),
                    config: serde_json::json!({}),
                }).await.map_err(|e| DataFlareError::Actor(format!("Error al inicializar actor: {}", e)))??;

                Ok(())
            } else {
                Err(DataFlareError::Actor(format!("Actor no encontrado: {}", actor_id)))
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
    use actix::prelude::*;

    // Actor de prueba
    struct TestActor {
        id: String,
        status: ActorStatus,
    }

    impl Actor for TestActor {
        type Context = Context<Self>;
    }

    impl DataFlareActor for TestActor {
        fn get_id(&self) -> &str {
            &self.id
        }

        fn get_type(&self) -> &str {
            "test"
        }

        fn initialize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
            self.status = ActorStatus::Initialized;
            Ok(())
        }

        fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
            self.status = ActorStatus::Finalized;
            Ok(())
        }

        fn report_progress(&self, _workflow_id: &str, _phase: crate::message::WorkflowPhase, _progress: f64, _message: &str) {
            // No hace nada en la prueba
        }
    }

    impl Handler<GetStatus> for TestActor {
        type Result = Result<ActorStatus>;

        fn handle(&mut self, _msg: GetStatus, _ctx: &mut Self::Context) -> Self::Result {
            Ok(self.status.clone())
        }
    }

    impl Handler<Initialize> for TestActor {
        type Result = Result<()>;

        fn handle(&mut self, _msg: Initialize, _ctx: &mut Self::Context) -> Self::Result {
            self.status = ActorStatus::Initialized;
            Ok(())
        }
    }

    impl Handler<Finalize> for TestActor {
        type Result = Result<()>;

        fn handle(&mut self, _msg: Finalize, _ctx: &mut Self::Context) -> Self::Result {
            self.status = ActorStatus::Finalized;
            Ok(())
        }
    }

    #[actix::test]
    async fn test_supervisor_actor() {
        // Crear un actor de prueba
        let test_actor = TestActor {
            id: "test-actor".to_string(),
            status: ActorStatus::Initialized,
        };
        let test_addr = test_actor.start();

        // Crear el supervisor
        let mut supervisor = SupervisorActor::new("test-supervisor");
        supervisor.add_supervised_actor("test-actor".to_string(), test_addr.clone());
        let supervisor_addr = supervisor.start();

        // Inicializar el supervisor
        let result = supervisor_addr.send(Initialize {
            workflow_id: "test-workflow".to_string(),
            config: serde_json::json!({}),
        }).await.unwrap();

        assert!(result.is_ok());

        // Obtener el estado de los actores supervisados
        let status = supervisor_addr.send(GetSupervisedActorsStatus).await.unwrap().unwrap();
        assert!(status.contains_key("test-actor"));

        // Reiniciar el actor supervisado
        let result = supervisor_addr.send(RestartActor {
            actor_id: "test-actor".to_string(),
        }).await.unwrap();

        assert!(result.is_ok());
    }
}
