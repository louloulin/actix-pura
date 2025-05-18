//! Ejemplo de uso de DataFlare con un flujo de trabajo simple

use dataflare::{
    workflow::{WorkflowBuilder, WorkflowExecutor},
    message::WorkflowProgress,
};
use std::sync::{Arc, Mutex};

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inicializar DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;
    
    println!("Creando flujo de trabajo...");
    
    // Crear flujo de trabajo
    let workflow = WorkflowBuilder::new("simple-workflow", "Simple Workflow")
        .description("Un flujo de trabajo simple para demostrar DataFlare")
        .version("1.0.0")
        // Fuente de datos en memoria
        .source("users", "memory", serde_json::json!({
            "data": [
                {"id": 1, "first_name": "John", "last_name": "Doe", "email": "john.doe@example.com"},
                {"id": 2, "first_name": "Jane", "last_name": "Smith", "email": "jane.smith@example.com"},
                {"id": 3, "first_name": "Bob", "last_name": "Johnson", "email": "bob.johnson@example.com"}
            ]
        }))
        // Transformación de mapeo
        .transformation("user_transform", "mapping", vec!["users"], serde_json::json!({
            "mappings": [
                {
                    "source": "first_name",
                    "destination": "user.firstName",
                    "transform": "uppercase"
                },
                {
                    "source": "last_name",
                    "destination": "user.lastName",
                    "transform": "uppercase"
                },
                {
                    "source": "email",
                    "destination": "user.email",
                    "transform": "lowercase"
                }
            ]
        }))
        // Transformación de filtro
        .transformation("email_filter", "filter", vec!["user_transform"], serde_json::json!({
            "condition": "user.email != null"
        }))
        // Destino en memoria
        .destination("memory_dest", "memory", vec!["email_filter"], serde_json::json!({}))
        .build()?;
    
    println!("Flujo de trabajo creado: {}", workflow.id);
    
    // Crear un contador de progreso compartido
    let progress_counter = Arc::new(Mutex::new(0));
    let progress_counter_clone = progress_counter.clone();
    
    // Crear ejecutor de flujo de trabajo
    let mut executor = WorkflowExecutor::new()
        .with_progress_callback(move |progress: WorkflowProgress| {
            println!(
                "Progreso: Workflow={}, Fase={:?}, Progreso={:.2}, Mensaje={}",
                progress.workflow_id, progress.phase, progress.progress, progress.message
            );
            
            // Incrementar contador
            let mut counter = progress_counter_clone.lock().unwrap();
            *counter += 1;
        });
    
    println!("Inicializando ejecutor...");
    executor.initialize()?;
    
    println!("Preparando flujo de trabajo...");
    executor.prepare(&workflow)?;
    
    println!("Ejecutando flujo de trabajo...");
    executor.execute(&workflow).await?;
    
    // Verificar que se recibieron actualizaciones de progreso
    let counter = progress_counter.lock().unwrap();
    println!("Se recibieron {} actualizaciones de progreso", *counter);
    
    println!("Finalizando ejecutor...");
    executor.finalize()?;
    
    println!("¡Flujo de trabajo completado con éxito!");
    
    Ok(())
}
