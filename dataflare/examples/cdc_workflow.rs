//! Ejemplo de uso de DataFlare con un flujo de trabajo CDC

use dataflare::{
    workflow::{WorkflowBuilder, WorkflowExecutor},
    message::WorkflowProgress,
    state::SourceState,
};
use std::sync::{Arc, Mutex};

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inicializar DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;
    
    println!("Creando flujo de trabajo CDC...");
    
    // Crear flujo de trabajo
    let workflow = WorkflowBuilder::new("cdc-workflow", "CDC Workflow")
        .description("Un flujo de trabajo CDC para demostrar DataFlare")
        .version("1.0.0")
        // Fuente de datos PostgreSQL con CDC
        .source_with_mode("postgres_source", "postgres", "cdc", serde_json::json!({
            "host": "localhost",
            "port": 5432,
            "database": "example",
            "username": "postgres",
            "password": "postgres",
            "schema": "public",
            "table": "users",
            "cdc": {
                "slot_name": "dataflare_slot",
                "publication_name": "dataflare_pub",
                "plugin": "pgoutput",
                "snapshot_mode": "initial"
            }
        }))
        // Transformación para procesar eventos CDC
        .transformation("cdc_transform", "mapping", vec!["postgres_source"], serde_json::json!({
            "mappings": [
                {
                    "source": "id",
                    "destination": "user.id"
                },
                {
                    "source": "name",
                    "destination": "user.name"
                },
                {
                    "source": "email",
                    "destination": "user.email",
                    "transform": "lowercase"
                },
                {
                    "source": "op",
                    "destination": "operation"
                }
            ]
        }))
        // Destino Elasticsearch
        .destination("es_dest", "elasticsearch", vec!["cdc_transform"], serde_json::json!({
            "host": "localhost",
            "port": 9200,
            "index": "users",
            "id_field": "user.id",
            "write_mode": {
                "type": "merge",
                "merge_key": "user.id"
            }
        }))
        .build()?;
    
    println!("Flujo de trabajo CDC creado: {}", workflow.id);
    
    // Crear un contador de progreso compartido
    let progress_counter = Arc::new(Mutex::new(0));
    let progress_counter_clone = progress_counter.clone();
    
    // Crear ejecutor de flujo de trabajo
    let mut executor = WorkflowExecutor::new()
        .with_progress_callback(move |progress: WorkflowProgress| {
            println!(
                "Progreso CDC: Workflow={}, Fase={:?}, Progreso={:.2}, Mensaje={}",
                progress.workflow_id, progress.phase, progress.progress, progress.message
            );
            
            // Incrementar contador
            let mut counter = progress_counter_clone.lock().unwrap();
            *counter += 1;
        });
    
    println!("Inicializando ejecutor CDC...");
    executor.initialize()?;
    
    // Crear estado inicial para la fuente CDC
    let source_state = SourceState::new()
        .with_source_name("postgres_source")
        .with_extraction_mode("cdc")
        .with_log_position("0/0");
    
    println!("Preparando flujo de trabajo CDC...");
    executor.prepare(&workflow)?;
    
    println!("Ejecutando flujo de trabajo CDC...");
    executor.execute(&workflow).await?;
    
    // Verificar que se recibieron actualizaciones de progreso
    let counter = progress_counter.lock().unwrap();
    println!("Se recibieron {} actualizaciones de progreso CDC", *counter);
    
    println!("Finalizando ejecutor CDC...");
    executor.finalize()?;
    
    println!("¡Flujo de trabajo CDC completado con éxito!");
    
    Ok(())
}
