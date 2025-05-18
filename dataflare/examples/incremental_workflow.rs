//! Ejemplo de uso de DataFlare con un flujo de trabajo incremental

use dataflare::{
    workflow::{WorkflowBuilder, WorkflowExecutor},
    message::WorkflowProgress,
    state::SourceState,
};
use std::sync::{Arc, Mutex};
use chrono::Utc;

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inicializar DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;
    
    println!("Creando flujo de trabajo incremental...");
    
    // Crear flujo de trabajo
    let workflow = WorkflowBuilder::new("incremental-workflow", "Incremental Workflow")
        .description("Un flujo de trabajo incremental para demostrar DataFlare")
        .version("1.0.0")
        // Fuente de datos MySQL con modo incremental
        .source_with_mode("mysql_source", "mysql", "incremental", serde_json::json!({
            "host": "localhost",
            "port": 3306,
            "database": "example",
            "username": "root",
            "password": "password",
            "table": "orders",
            "incremental": {
                "column": "updated_at",
                "cursor_field": "updated_at",
                "cursor_type": "timestamp"
            }
        }))
        // Transformación para procesar datos incrementales
        .transformation("order_transform", "mapping", vec!["mysql_source"], serde_json::json!({
            "mappings": [
                {
                    "source": "id",
                    "destination": "order.id"
                },
                {
                    "source": "customer_id",
                    "destination": "order.customerId"
                },
                {
                    "source": "total",
                    "destination": "order.total"
                },
                {
                    "source": "status",
                    "destination": "order.status"
                },
                {
                    "source": "updated_at",
                    "destination": "order.updatedAt"
                }
            ]
        }))
        // Destino Snowflake
        .destination("snowflake_dest", "snowflake", vec!["order_transform"], serde_json::json!({
            "account": "example",
            "warehouse": "COMPUTE_WH",
            "database": "EXAMPLE_DB",
            "schema": "PUBLIC",
            "table": "ORDERS",
            "username": "user",
            "password": "password",
            "write_mode": {
                "type": "merge",
                "merge_key": "order.id"
            }
        }))
        .build()?;
    
    println!("Flujo de trabajo incremental creado: {}", workflow.id);
    
    // Crear un contador de progreso compartido
    let progress_counter = Arc::new(Mutex::new(0));
    let progress_counter_clone = progress_counter.clone();
    
    // Crear ejecutor de flujo de trabajo
    let mut executor = WorkflowExecutor::new()
        .with_progress_callback(move |progress: WorkflowProgress| {
            println!(
                "Progreso incremental: Workflow={}, Fase={:?}, Progreso={:.2}, Mensaje={}",
                progress.workflow_id, progress.phase, progress.progress, progress.message
            );
            
            // Incrementar contador
            let mut counter = progress_counter_clone.lock().unwrap();
            *counter += 1;
        });
    
    println!("Inicializando ejecutor incremental...");
    executor.initialize()?;
    
    // Crear estado inicial para la fuente incremental
    // Usar una fecha anterior como punto de inicio
    let one_day_ago = Utc::now() - chrono::Duration::days(1);
    let cursor_value = one_day_ago.to_rfc3339();
    
    let source_state = SourceState::new()
        .with_source_name("mysql_source")
        .with_extraction_mode("incremental")
        .with_cursor("updated_at", &cursor_value);
    
    println!("Preparando flujo de trabajo incremental...");
    executor.prepare(&workflow)?;
    
    println!("Ejecutando flujo de trabajo incremental...");
    executor.execute(&workflow).await?;
    
    // Verificar que se recibieron actualizaciones de progreso
    let counter = progress_counter.lock().unwrap();
    println!("Se recibieron {} actualizaciones de progreso incremental", *counter);
    
    println!("Finalizando ejecutor incremental...");
    executor.finalize()?;
    
    println!("¡Flujo de trabajo incremental completado con éxito!");
    
    Ok(())
}
