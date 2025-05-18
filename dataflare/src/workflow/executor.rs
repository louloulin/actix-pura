//! Ejecutor de flujo de trabajo para DataFlare
//!
//! Proporciona funcionalidades para ejecutar flujos de trabajo.

use std::collections::HashMap;
use std::sync::Arc;
use actix::prelude::*;
use log::{debug, error, info, warn};
use futures::future::{self, Future, FutureExt};
use serde_json::Value;

use crate::{
    actor::{
        SourceActor, ProcessorActor, DestinationActor, WorkflowActor, SupervisorActor,
        Initialize, SubscribeToProgress,
    },
    connector::{create_connector, SourceConnector, DestinationConnector},
    error::{DataFlareError, Result},
    message::{WorkflowPhase, WorkflowProgress},
    processor::Processor,
    state::SourceState,
    workflow::Workflow,
};

/// Ejecutor de flujo de trabajo
pub struct WorkflowExecutor {
    /// Sistema de actores
    system: Option<actix::SystemRunner>,
    
    /// Actor de flujo de trabajo
    workflow_actor: Option<Addr<WorkflowActor>>,
    
    /// Actor supervisor
    supervisor_actor: Option<Addr<SupervisorActor>>,
    
    /// Actores de origen
    source_actors: HashMap<String, Addr<SourceActor>>,
    
    /// Actores de procesador
    processor_actors: HashMap<String, Addr<ProcessorActor>>,
    
    /// Actores de destino
    destination_actors: HashMap<String, Addr<DestinationActor>>,
    
    /// Estados de las fuentes
    source_states: HashMap<String, SourceState>,
    
    /// Receptor de actualizaciones de progreso
    progress_callback: Option<Box<dyn Fn(WorkflowProgress) + Send + Sync>>,
}

impl WorkflowExecutor {
    /// Crea un nuevo ejecutor de flujo de trabajo
    pub fn new() -> Self {
        Self {
            system: None,
            workflow_actor: None,
            supervisor_actor: None,
            source_actors: HashMap::new(),
            processor_actors: HashMap::new(),
            destination_actors: HashMap::new(),
            source_states: HashMap::new(),
            progress_callback: None,
        }
    }
    
    /// Establece un callback para recibir actualizaciones de progreso
    pub fn with_progress_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(WorkflowProgress) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }
    
    /// Inicializa el ejecutor
    pub fn initialize(&mut self) -> Result<()> {
        // Crear sistema de actores si no existe
        if self.system.is_none() {
            let system = actix::System::new();
            self.system = Some(system);
        }
        
        // Crear actor supervisor
        let supervisor = SupervisorActor::new("supervisor");
        let supervisor_addr = supervisor.start();
        self.supervisor_actor = Some(supervisor_addr);
        
        Ok(())
    }
    
    /// Prepara un flujo de trabajo para su ejecución
    pub fn prepare(&mut self, workflow: &Workflow) -> Result<()> {
        // Validar el flujo de trabajo
        workflow.validate()?;
        
        // Crear actores de origen
        for (id, source_config) in &workflow.sources {
            // Crear conector
            let connector = create_connector::<dyn SourceConnector>(
                &source_config.r#type,
                source_config.config.clone(),
            )?;
            
            // Crear actor
            let source_actor = SourceActor::new(id.clone(), connector);
            let source_addr = source_actor.start();
            
            // Registrar actor
            self.source_actors.insert(id.clone(), source_addr.clone());
            
            // Supervisar actor
            if let Some(supervisor) = &self.supervisor_actor {
                supervisor.do_send(crate::actor::supervisor::RestartActor {
                    actor_id: id.clone(),
                });
            }
        }
        
        // Crear actores de procesador
        for (id, transform_config) in &workflow.transformations {
            // Crear procesador según el tipo
            let processor: Box<dyn Processor> = match transform_config.r#type.as_str() {
                "mapping" => Box::new(crate::processor::MappingProcessor::new()),
                "filter" => Box::new(crate::processor::FilterProcessor::new()),
                _ => return Err(DataFlareError::Config(format!(
                    "Tipo de procesador no soportado: {}", transform_config.r#type
                ))),
            };
            
            // Crear actor
            let processor_actor = ProcessorActor::new(id.clone(), processor);
            let processor_addr = processor_actor.start();
            
            // Registrar actor
            self.processor_actors.insert(id.clone(), processor_addr.clone());
            
            // Supervisar actor
            if let Some(supervisor) = &self.supervisor_actor {
                supervisor.do_send(crate::actor::supervisor::RestartActor {
                    actor_id: id.clone(),
                });
            }
        }
        
        // Crear actores de destino
        for (id, dest_config) in &workflow.destinations {
            // Crear conector
            let connector = create_connector::<dyn DestinationConnector>(
                &dest_config.r#type,
                dest_config.config.clone(),
            )?;
            
            // Crear actor
            let dest_actor = DestinationActor::new(id.clone(), connector);
            let dest_addr = dest_actor.start();
            
            // Registrar actor
            self.destination_actors.insert(id.clone(), dest_addr.clone());
            
            // Supervisar actor
            if let Some(supervisor) = &self.supervisor_actor {
                supervisor.do_send(crate::actor::supervisor::RestartActor {
                    actor_id: id.clone(),
                });
            }
        }
        
        // Crear actor de flujo de trabajo
        let mut workflow_actor = WorkflowActor::new(workflow.id.clone());
        
        // Agregar actores al flujo de trabajo
        for (id, addr) in &self.source_actors {
            workflow_actor.add_source_actor(id.clone(), addr.clone());
        }
        
        for (id, addr) in &self.processor_actors {
            workflow_actor.add_processor_actor(id.clone(), addr.clone());
        }
        
        for (id, addr) in &self.destination_actors {
            workflow_actor.add_destination_actor(id.clone(), addr.clone());
        }
        
        // Iniciar actor de flujo de trabajo
        let workflow_addr = workflow_actor.start();
        self.workflow_actor = Some(workflow_addr.clone());
        
        // Supervisar actor de flujo de trabajo
        if let Some(supervisor) = &self.supervisor_actor {
            supervisor.do_send(crate::actor::supervisor::RestartActor {
                actor_id: workflow.id.clone(),
            });
        }
        
        // Suscribirse a actualizaciones de progreso si hay callback
        if self.progress_callback.is_some() {
            let callback = self.progress_callback.clone();
            let recipient = actix::spawn(async move {
                let mut ctx = actix::Context::new();
                
                // Crear actor para recibir actualizaciones
                let progress_actor = ProgressActor { callback };
                let addr = progress_actor.start();
                
                // Mantener el actor vivo
                actix::fut::ready(addr).await;
            });
            
            if let Some(workflow_addr) = &self.workflow_actor {
                workflow_addr.do_send(SubscribeToProgress {
                    workflow_id: workflow.id.clone(),
                    recipient: recipient.recipient(),
                });
            }
        }
        
        Ok(())
    }
    
    /// Ejecuta un flujo de trabajo
    pub async fn execute(&self, workflow: &Workflow) -> Result<()> {
        // Verificar que el actor de flujo de trabajo exista
        let workflow_addr = match &self.workflow_actor {
            Some(addr) => addr,
            None => return Err(DataFlareError::Workflow("Actor de flujo de trabajo no inicializado".to_string())),
        };
        
        // Inicializar actores
        let mut futures = Vec::new();
        
        // Inicializar actores de origen
        for (id, source_config) in &workflow.sources {
            if let Some(actor) = self.source_actors.get(id) {
                let fut = actor.send(Initialize {
                    workflow_id: workflow.id.clone(),
                    config: source_config.config.clone(),
                });
                futures.push(fut.map(|res| {
                    match res {
                        Ok(Ok(_)) => Ok(()),
                        Ok(Err(e)) => Err(e),
                        Err(e) => Err(DataFlareError::Actor(format!("Error al inicializar actor de origen {}: {}", id, e))),
                    }
                }).boxed());
            }
        }
        
        // Inicializar actores de procesador
        for (id, transform_config) in &workflow.transformations {
            if let Some(actor) = self.processor_actors.get(id) {
                let fut = actor.send(Initialize {
                    workflow_id: workflow.id.clone(),
                    config: transform_config.config.clone(),
                });
                futures.push(fut.map(|res| {
                    match res {
                        Ok(Ok(_)) => Ok(()),
                        Ok(Err(e)) => Err(e),
                        Err(e) => Err(DataFlareError::Actor(format!("Error al inicializar actor de procesador {}: {}", id, e))),
                    }
                }).boxed());
            }
        }
        
        // Inicializar actores de destino
        for (id, dest_config) in &workflow.destinations {
            if let Some(actor) = self.destination_actors.get(id) {
                let fut = actor.send(Initialize {
                    workflow_id: workflow.id.clone(),
                    config: dest_config.config.clone(),
                });
                futures.push(fut.map(|res| {
                    match res {
                        Ok(Ok(_)) => Ok(()),
                        Ok(Err(e)) => Err(e),
                        Err(e) => Err(DataFlareError::Actor(format!("Error al inicializar actor de destino {}: {}", id, e))),
                    }
                }).boxed());
            }
        }
        
        // Esperar a que todos los actores se inicialicen
        let results = future::join_all(futures).await;
        for result in results {
            result?;
        }
        
        // Ejecutar flujo de trabajo
        let result = workflow_addr.send(crate::actor::workflow::ExecuteWorkflow {
            workflow_id: workflow.id.clone(),
            config: serde_json::to_value(workflow).map_err(|e| {
                DataFlareError::Serialization(format!("Error al serializar flujo de trabajo: {}", e))
            })?,
        }).await;
        
        match result {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(DataFlareError::Actor(format!("Error al ejecutar flujo de trabajo: {}", e))),
        }
    }
    
    /// Finaliza el ejecutor
    pub fn finalize(&mut self) -> Result<()> {
        // Detener sistema de actores
        if let Some(system) = self.system.take() {
            system.stop();
        }
        
        // Limpiar actores
        self.workflow_actor = None;
        self.supervisor_actor = None;
        self.source_actors.clear();
        self.processor_actors.clear();
        self.destination_actors.clear();
        
        Ok(())
    }
}

impl Drop for WorkflowExecutor {
    fn drop(&mut self) {
        let _ = self.finalize();
    }
}

/// Actor para recibir actualizaciones de progreso
struct ProgressActor {
    /// Callback para notificar progreso
    callback: Option<Box<dyn Fn(WorkflowProgress) + Send + Sync>>,
}

impl Actor for ProgressActor {
    type Context = Context<Self>;
}

impl Handler<WorkflowProgress> for ProgressActor {
    type Result = ();
    
    fn handle(&mut self, msg: WorkflowProgress, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(callback) = &self.callback {
            callback(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::WorkflowBuilder;
    use std::sync::{Arc, Mutex};
    
    #[actix::test]
    async fn test_workflow_executor() {
        // Crear flujo de trabajo
        let workflow = WorkflowBuilder::new("test-workflow", "Test Workflow")
            .source("source", "memory", serde_json::json!({
                "data": [
                    {"id": 1, "name": "Test 1"},
                    {"id": 2, "name": "Test 2"}
                ]
            }))
            .transformation("transform", "mapping", vec!["source"], serde_json::json!({
                "mappings": [
                    {
                        "source": "name",
                        "destination": "user.name",
                        "transform": "uppercase"
                    }
                ]
            }))
            .destination("dest", "memory", vec!["transform"], serde_json::json!({}))
            .build()
            .unwrap();
        
        // Crear ejecutor
        let progress_updates = Arc::new(Mutex::new(Vec::new()));
        let progress_updates_clone = progress_updates.clone();
        
        let mut executor = WorkflowExecutor::new()
            .with_progress_callback(move |progress| {
                let mut updates = progress_updates_clone.lock().unwrap();
                updates.push(progress);
            });
        
        // Inicializar ejecutor
        executor.initialize().unwrap();
        
        // Preparar flujo de trabajo
        executor.prepare(&workflow).unwrap();
        
        // Ejecutar flujo de trabajo
        executor.execute(&workflow).await.unwrap();
        
        // Verificar actualizaciones de progreso
        let updates = progress_updates.lock().unwrap();
        assert!(!updates.is_empty());
        
        // Finalizar ejecutor
        executor.finalize().unwrap();
    }
}
