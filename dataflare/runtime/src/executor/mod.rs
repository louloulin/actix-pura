//! Ejecutor de flujo de trabajo para DataFlare
//!
//! Proporciona funcionalidades para ejecutar flujos de trabajo.

use std::collections::HashMap;
use actix::prelude::*;
use log::{debug, info, warn};
use futures::future::{self, FutureExt};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::WorkflowProgress,
    processor::Processor,
    state::SourceState,
};
use dataflare_processor::{
    mapping::MappingProcessor,
    filter::FilterProcessor,
};
use crate::{
    actor::{
        SourceActor, ProcessorActor, DestinationActor, WorkflowActor, SupervisorActor,
        Initialize, SubscribeToProgress,
    },
    workflow::Workflow,
    progress::{ProgressReporter, ProgressActor, WebhookConfig},
};

/// Enum to specify runtime mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeMode {
    /// Standalone mode - single node execution
    Standalone,
    /// Edge mode - execution at edge location
    Edge,
    /// Cloud mode - execution in cloud environment
    Cloud,
}

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

    /// Runtime mode
    runtime_mode: RuntimeMode,

    /// Progress reporter
    progress_reporter: ProgressReporter,

    /// Progress actor
    progress_actor: Option<Addr<ProgressActor>>,

    /// Legacy progress callback (for backwards compatibility)
    legacy_progress_callback: Option<Box<dyn Fn(WorkflowProgress) + Send + Sync>>,
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
            runtime_mode: RuntimeMode::Standalone,
            progress_reporter: ProgressReporter::new(),
            progress_actor: None,
            legacy_progress_callback: None,
        }
    }

    /// Set runtime mode
    pub fn with_runtime_mode(mut self, mode: RuntimeMode) -> Self {
        self.runtime_mode = mode;
        self
    }

    /// Establece un callback para recibir actualizaciones de progreso (legacy method)
    pub fn with_progress_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(WorkflowProgress) + Send + Sync + 'static,
    {
        self.legacy_progress_callback = Some(Box::new(callback));
        self
    }

    /// Add a function callback for progress updates
    pub fn add_progress_callback<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(WorkflowProgress) + Send + Sync + 'static,
    {
        self.progress_reporter.add_function_callback(callback)
    }

    /// Add a webhook for progress updates
    pub fn add_progress_webhook(&self, config: WebhookConfig) -> Result<()> {
        self.progress_reporter.add_webhook(config)
    }

    /// Create a progress event stream
    pub fn create_progress_stream(&self) -> Result<crate::progress::ProgressEventStream> {
        self.progress_reporter.create_event_stream()
    }

    /// Inicializa el ejecutor
    pub fn initialize(&mut self) -> Result<()> {
        // Inicializar conectores
        dataflare_connector::initialize()
            .map_err(|e| DataFlareError::Registry(format!("Error al inicializar conectores: {}", e)))?;

        // No creamos un nuevo sistema de actores, usamos el existente
        // Crear actor supervisor
        let supervisor = SupervisorActor::new("supervisor");
        let supervisor_addr = supervisor.start();
        self.supervisor_actor = Some(supervisor_addr);

        // Create progress actor
        let progress_actor = ProgressActor::new(self.progress_reporter.clone());
        let progress_addr = progress_actor.start();
        self.progress_actor = Some(progress_addr);

        // Temporarily disable legacy callback support until lifecycle issues are fixed
        // TODO: Re-implement legacy callback support
        // if let Some(callback) = &self.legacy_progress_callback {
        //    // Implementation would go here
        // }

        Ok(())
    }

    /// Prepara un flujo de trabajo para su ejecución
    pub fn prepare(&mut self, workflow: &Workflow) -> Result<()> {
        info!("Preparando flujo de trabajo {}", workflow.id);

        // Crear actor de flujo de trabajo
        let workflow_actor = WorkflowActor::new(workflow.id.clone());
        let workflow_addr = workflow_actor.start();
        self.workflow_actor = Some(workflow_addr.clone());

        // Subscribe progress actor to workflow progress updates
        if let Some(progress_addr) = &self.progress_actor {
            let recipient = progress_addr.clone().recipient();
            workflow_addr.clone().do_send(SubscribeToProgress {
                workflow_id: workflow.id.clone(),
                recipient,
            });
        }

        // Crear actores de origen
        for (id, source_config) in &workflow.sources {
            let source_id = format!("{}.{}", workflow.id, id);
            debug!("Creando actor de origen {}", source_id);

            // Crear conector de origen
            let connector = dataflare_connector::registry::create_connector(&source_config.r#type, source_config.config.clone())?;

            // Crear actor de origen
            let batch_size = source_config.config.get("batch_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(1000) as usize;
            let source_actor = SourceActor::new(source_id.clone(), connector)
                .with_batch_size(batch_size);
            let source_addr = source_actor.start();
            self.source_actors.insert(source_id.clone(), source_addr.clone());

            // Subscribe progress actor to source progress updates
            if let Some(progress_addr) = &self.progress_actor {
                let progress_recipient = progress_addr.clone().recipient();
                source_addr.do_send(SubscribeToProgress {
                    workflow_id: workflow.id.clone(),
                    recipient: progress_recipient,
                });
            }
        }

        // Crear actores de procesador
        for (id, proc_config) in &workflow.transformations {
            let proc_id = format!("{}.{}", workflow.id, id);
            debug!("Creando actor de procesador {}", proc_id);

            // Crear procesador según el tipo
            let mut processor: Box<dyn Processor> = match proc_config.r#type.as_str() {
                "mapping" => Box::new(MappingProcessor::new(dataflare_processor::mapping::MappingProcessorConfig {
                    mappings: Vec::new(),
                })),
                "filter" => Box::new(FilterProcessor::new(dataflare_processor::filter::FilterProcessorConfig {
                    condition: "true".to_string(),
                })),
                _ => return Err(DataFlareError::Config(format!("Tipo de procesador desconocido: {}", proc_config.r#type))),
            };

            // Configurar el procesador con la configuración del workflow
            processor.configure(&proc_config.config)
                .map_err(|e| DataFlareError::Config(format!("Error al configurar procesador {}: {}", id, e)))?;

            // Crear actor de procesador
            let proc_actor = ProcessorActor::new(proc_id.clone(), processor);
            let proc_addr = proc_actor.start();
            self.processor_actors.insert(proc_id.clone(), proc_addr.clone());

            // Subscribe progress actor to processor progress updates
            if let Some(progress_addr) = &self.progress_actor {
                let progress_recipient = progress_addr.clone().recipient();
                proc_addr.do_send(SubscribeToProgress {
                    workflow_id: workflow.id.clone(),
                    recipient: progress_recipient,
                });
            }
        }

        // Crear actores de destino
        for (id, dest_config) in &workflow.destinations {
            let dest_id = format!("{}.{}", workflow.id, id);
            debug!("Creando actor de destino {}", dest_id);

            // Crear conector de destino
            let connector = dataflare_connector::registry::create_connector(&dest_config.r#type, dest_config.config.clone())?;

            // Crear actor de destino
            let dest_actor = DestinationActor::new(dest_id.clone(), connector);
            let dest_addr = dest_actor.start();
            self.destination_actors.insert(dest_id.clone(), dest_addr.clone());

            // Subscribe progress actor to destination progress updates
            if let Some(progress_addr) = &self.progress_actor {
                let progress_recipient = progress_addr.clone().recipient();
                dest_addr.do_send(SubscribeToProgress {
                    workflow_id: workflow.id.clone(),
                    recipient: progress_recipient,
                });
            }
        }

        // Reiniciar actores si es necesario
        if let Some(supervisor) = &self.supervisor_actor {
            supervisor.do_send(crate::actor::supervisor::RestartActor {
                actor_id: workflow.id.clone(),
            });
        }

        info!("Flujo de trabajo {} preparado con éxito", workflow.id);
        Ok(())
    }

    /// Ejecuta un flujo de trabajo
    pub async fn execute(&self, workflow: &Workflow) -> Result<()> {
        // Verify workflow actor exists
        let workflow_addr = match &self.workflow_actor {
            Some(addr) => addr,
            None => return Err(DataFlareError::Workflow("Workflow actor not initialized".to_string())),
        };

        // Initialize WorkflowActor first
        info!("Initializing WorkflowActor");
        let _ = workflow_addr.send(crate::actor::Initialize {
            workflow_id: workflow.id.clone(),
            config: serde_json::to_value(&workflow)?,
        }).await?;

        // Wait a bit to ensure WorkflowActor is fully initialized
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Initialize actors
        let mut futures = Vec::new();

        // Initialize source actors
        for (id, source_config) in &workflow.sources {
            let source_id = format!("{}.{}", workflow.id, id);
            if let Some(actor) = self.source_actors.get(&source_id) {
                let fut = actor.send(Initialize {
                    workflow_id: workflow.id.clone(),
                    config: source_config.config.clone(),
                });
                futures.push(fut.map(move |res| {
                    match res {
                        Ok(Ok(_)) => Ok(()),
                        Ok(Err(e)) => Err(e),
                        Err(e) => Err(DataFlareError::Actor(format!("Failed to initialize source actor {}: {}", source_id, e))),
                    }
                }).boxed());
            }
        }

        // Initialize processor actors
        for (id, proc_config) in &workflow.transformations {
            let proc_id = format!("{}.{}", workflow.id, id);
            if let Some(actor) = self.processor_actors.get(&proc_id) {
                let fut = actor.send(Initialize {
                    workflow_id: workflow.id.clone(),
                    config: proc_config.config.clone(),
                });
                futures.push(fut.map(move |res| {
                    match res {
                        Ok(Ok(_)) => Ok(()),
                        Ok(Err(e)) => Err(e),
                        Err(e) => Err(DataFlareError::Actor(format!("Failed to initialize processor actor {}: {}", proc_id, e))),
                    }
                }).boxed());
            }
        }

        // Initialize destination actors
        for (id, dest_config) in &workflow.destinations {
            let dest_id = format!("{}.{}", workflow.id, id);
            if let Some(actor) = self.destination_actors.get(&dest_id) {
                let fut = actor.send(Initialize {
                    workflow_id: workflow.id.clone(),
                    config: dest_config.config.clone(),
                });
                futures.push(fut.map(move |res| {
                    match res {
                        Ok(Ok(_)) => Ok(()),
                        Ok(Err(e)) => Err(e),
                        Err(e) => Err(DataFlareError::Actor(format!("Failed to initialize destination actor {}: {}", dest_id, e))),
                    }
                }).boxed());
            }
        }

        // Wait for all initializations to complete
        let results = future::join_all(futures).await;
        for result in results {
            if let Err(e) = result {
                return Err(e);
            }
        }

        // 将创建的Actor注册到WorkflowActor
        info!("Registering actors with WorkflowActor");

        // 注册源Actor
        for (id, source_config) in &workflow.sources {
            let source_id = format!("{}.{}", workflow.id, id);
            if let Some(actor) = self.source_actors.get(&source_id) {
                let _ = workflow_addr.send(crate::actor::RegisterSourceActor {
                    source_id: id.clone(),
                    source_addr: actor.clone(),
                }).await;
            }
        }

        // 注册处理器Actor
        for (id, _proc_config) in &workflow.transformations {
            let proc_id = format!("{}.{}", workflow.id, id);
            if let Some(actor) = self.processor_actors.get(&proc_id) {
                let _ = workflow_addr.send(crate::actor::RegisterProcessorActor {
                    processor_id: id.clone(),
                    processor_addr: actor.clone(),
                }).await;
            }
        }

        // 注册目标Actor
        for (id, dest_config) in &workflow.destinations {
            let dest_id = format!("{}.{}", workflow.id, id);
            if let Some(actor) = self.destination_actors.get(&dest_id) {
                let _ = workflow_addr.send(crate::actor::RegisterDestinationActor {
                    destination_id: id.clone(),
                    destination_addr: actor.clone(),
                }).await;
            }
        }

        // Execute workflow
        let result = workflow_addr.send(crate::actor::workflow::ExecuteWorkflow {
            workflow_id: workflow.id.clone(),
            parameters: Some(serde_json::to_value(workflow).map_err(|e| {
                DataFlareError::Serialization(format!("Failed to serialize workflow: {}", e))
            })?),
        }).await;

        match result {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(DataFlareError::Actor(format!("Failed to execute workflow: {}", e))),
        }
    }

    /// Finaliza el ejecutor
    pub fn finalize(&mut self) -> Result<()> {
        // Clean up progress reporting
        if let Some(progress_actor) = self.progress_actor.take() {
            // The actor will be stopped when dropped
            drop(progress_actor);
        }

        if let Err(e) = self.progress_reporter.clear() {
            warn!("Failed to clear progress reporters: {}", e);
        }

        // Detener sistema de actores
        if let Some(_system) = self.system.take() {
            // El sistema se detiene automáticamente cuando se descarta
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::WebhookConfig;
    use std::sync::{Arc, Mutex};
    use chrono::Utc;
    use dataflare_core::message::WorkflowPhase;

    // Helper function to create a test workflow
    fn create_test_workflow() -> Workflow {
        // Create a simple test workflow
        let workflow = Workflow {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            description: Some("A test workflow".to_string()),
            version: "1.0.0".to_string(),
            sources: HashMap::new(),
            transformations: HashMap::new(),
            destinations: HashMap::new(),
            schedule: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        workflow
    }

    #[actix::test]
    async fn test_progress_callback() {
        // Create a counter to track progress updates
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        // Create an executor with a progress callback
        let mut executor = WorkflowExecutor::new();
        executor.add_progress_callback(move |_progress| {
            let mut count = counter_clone.lock().unwrap();
            *count += 1;
        }).unwrap();

        // Initialize executor
        executor.initialize().unwrap();

        // Create a test workflow
        let workflow = create_test_workflow();

        // Prepare and execute workflow
        executor.prepare(&workflow).unwrap();

        // Manually send a progress update via the progress actor
        if let Some(progress_actor) = &executor.progress_actor {
            // Create test progress directly
            let progress = WorkflowProgress {
                workflow_id: workflow.id.clone(),
                phase: WorkflowPhase::Initializing,
                progress: 0.0,
                message: format!("Started workflow {}", workflow.id),
                timestamp: Utc::now(),
            };

            progress_actor.do_send(progress);

            // Allow some time for processing
            actix::clock::sleep(std::time::Duration::from_millis(100)).await;

            // Check that the callback was called
            let count = *counter.lock().unwrap();
            assert!(count > 0, "Progress callback should have been called");
        } else {
            panic!("Progress actor should be initialized");
        }

        // Finalize executor
        executor.finalize().unwrap();
    }

    #[actix::test]
    async fn test_multiple_callbacks() {
        // Create counters to track progress updates
        let counter1 = Arc::new(Mutex::new(0));
        let counter1_clone = counter1.clone();

        let counter2 = Arc::new(Mutex::new(0));
        let counter2_clone = counter2.clone();

        // Create an executor with multiple progress callbacks
        let mut executor = WorkflowExecutor::new();

        // Add first callback
        executor.add_progress_callback(move |_progress| {
            let mut count = counter1_clone.lock().unwrap();
            *count += 1;
        }).unwrap();

        // Add second callback
        executor.add_progress_callback(move |_progress| {
            let mut count = counter2_clone.lock().unwrap();
            *count += 2; // Increment by 2 to differentiate
        }).unwrap();

        // Initialize executor
        executor.initialize().unwrap();

        // Create a test workflow
        let workflow = create_test_workflow();

        // Prepare workflow
        executor.prepare(&workflow).unwrap();

        // Manually send a progress update via the progress actor
        if let Some(progress_actor) = &executor.progress_actor {
            // Create test progress directly
            let progress = WorkflowProgress {
                workflow_id: workflow.id.clone(),
                phase: WorkflowPhase::Initializing,
                progress: 0.0,
                message: format!("Started workflow {}", workflow.id),
                timestamp: Utc::now(),
            };

            progress_actor.do_send(progress);

            // Allow some time for processing
            actix::clock::sleep(std::time::Duration::from_millis(100)).await;

            // Check that both callbacks were called
            let count1 = *counter1.lock().unwrap();
            let count2 = *counter2.lock().unwrap();

            assert_eq!(count1, 1, "First progress callback should have been called once");
            assert_eq!(count2, 2, "Second progress callback should have been called once (with +2)");
        } else {
            panic!("Progress actor should be initialized");
        }

        // Finalize executor
        executor.finalize().unwrap();
    }
}

