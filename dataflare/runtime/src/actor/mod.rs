//! Módulo de actores para DataFlare
//!
//! Define los actores que componen el sistema DataFlare.

mod source;
mod processor;
mod destination;
pub mod workflow;
pub mod supervisor;

pub use source::SourceActor;
pub use processor::ProcessorActor;
pub use destination::DestinationActor;
pub use workflow::WorkflowActor;
pub use supervisor::SupervisorActor;

use actix::prelude::*;
use dataflare_core::error::Result;
use dataflare_core::message::{DataRecordBatch, WorkflowProgress};

/// Trait para actores de DataFlare
pub trait DataFlareActor: Actor {
    /// Obtiene el ID del actor
    fn get_id(&self) -> &str;

    /// Obtiene el tipo del actor
    fn get_type(&self) -> &str;

    /// Inicializa el actor
    fn initialize(&mut self, ctx: &mut Self::Context) -> Result<()>;

    /// Finaliza el actor
    fn finalize(&mut self, ctx: &mut Self::Context) -> Result<()>;

    /// Reporta progreso
    fn report_progress(&self, workflow_id: &str, phase: dataflare_core::message::WorkflowPhase, progress: f64, message: &str);
}

/// Mensaje para inicializar un actor
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Initialize {
    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// Configuración del actor
    pub config: serde_json::Value,
}

/// Mensaje para finalizar un actor
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Finalize {
    /// ID del flujo de trabajo
    pub workflow_id: String,
}

/// Mensaje para pausar un actor
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Pause {
    /// ID del flujo de trabajo
    pub workflow_id: String,
}

/// Mensaje para reanudar un actor
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct Resume {
    /// ID del flujo de trabajo
    pub workflow_id: String,
}

/// Mensaje para obtener el estado de un actor
#[derive(Message)]
#[rtype(result = "Result<ActorStatus>")]
pub struct GetStatus;

/// Estado de un actor
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorStatus {
    /// Actor inicializado
    Initialized,
    /// Actor en ejecución
    Running,
    /// Actor pausado
    Paused,
    /// Actor finalizado
    Finalized,
    /// Actor en error
    Error(String),
}

/// Mensaje para enviar un lote de datos
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct SendBatch {
    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// Lote de datos
    pub batch: DataRecordBatch,
}

/// Mensaje para suscribirse a actualizaciones de progreso
#[derive(Message)]
#[rtype(result = "()")]
pub struct SubscribeToProgress {
    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// Receptor de actualizaciones
    pub recipient: Recipient<WorkflowProgress>,
}

/// Mensaje para cancelar la suscripción a actualizaciones de progreso
#[derive(Message)]
#[rtype(result = "()")]
pub struct UnsubscribeFromProgress {
    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// Receptor a cancelar
    pub recipient: Recipient<WorkflowProgress>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Implementación de prueba de un actor DataFlare
    struct TestActor {
        id: String,
        actor_type: String,
    }

    impl Actor for TestActor {
        type Context = Context<Self>;
    }

    impl DataFlareActor for TestActor {
        fn get_id(&self) -> &str {
            &self.id
        }

        fn get_type(&self) -> &str {
            &self.actor_type
        }

        fn initialize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
            Ok(())
        }

        fn finalize(&mut self, _ctx: &mut Self::Context) -> Result<()> {
            Ok(())
        }

        fn report_progress(&self, _workflow_id: &str, _phase: dataflare_core::message::WorkflowPhase, _progress: f64, _message: &str) {
            // No hace nada en la prueba
        }
    }

    #[test]
    fn test_actor_trait() {
        let actor = TestActor {
            id: "test-actor".to_string(),
            actor_type: "test".to_string(),
        };

        assert_eq!(actor.get_id(), "test-actor");
        assert_eq!(actor.get_type(), "test");
    }
}
