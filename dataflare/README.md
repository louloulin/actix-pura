# DataFlare

DataFlare es un framework de integración de datos basado en la arquitectura de actores Actix, diseñado para proporcionar capacidades ETL (Extract, Transform, Load) flexibles y escalables.

## Características

- **Arquitectura basada en actores**: Utiliza el modelo de actores de Actix para procesamiento distribuido y concurrente.
- **Múltiples modos de extracción**: Soporta extracción completa, incremental y CDC (Change Data Capture).
- **DSL declarativo**: Define flujos de trabajo mediante un DSL basado en YAML.
- **Conectores extensibles**: Fácil integración con diversas fuentes y destinos de datos.
- **Procesadores configurables**: Transformaciones de datos flexibles y componibles.
- **Sistema de plugins**: Soporte para extensiones mediante plugins WASM.
- **Gestión de estado**: Seguimiento y recuperación de estado para procesamientos incrementales y CDC.

## Instalación

Agrega DataFlare a tu proyecto Rust:

```toml
[dependencies]
dataflare = "0.1.0"
```

## Uso básico

### Definir un flujo de trabajo

```rust
use dataflare::workflow::WorkflowBuilder;

// Crear un flujo de trabajo
let workflow = WorkflowBuilder::new("simple-workflow", "Simple Workflow")
    .description("Un flujo de trabajo simple")
    .version("1.0.0")
    // Fuente de datos
    .source("users", "postgres", serde_json::json!({
        "host": "localhost",
        "port": 5432,
        "database": "example",
        "username": "postgres",
        "password": "postgres",
        "table": "users"
    }))
    // Transformación
    .transformation("user_transform", "mapping", vec!["users"], serde_json::json!({
        "mappings": [
            {
                "source": "first_name",
                "destination": "user.firstName"
            },
            {
                "source": "last_name",
                "destination": "user.lastName"
            }
        ]
    }))
    // Destino
    .destination("es_users", "elasticsearch", vec!["user_transform"], serde_json::json!({
        "host": "localhost",
        "port": 9200,
        "index": "users"
    }))
    .build()
    .unwrap();
```

### Ejecutar un flujo de trabajo

```rust
use dataflare::workflow::WorkflowExecutor;

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inicializar DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;
    
    // Crear ejecutor
    let mut executor = WorkflowExecutor::new()
        .with_progress_callback(|progress| {
            println!("Progreso: {:?}", progress);
        });
    
    // Inicializar ejecutor
    executor.initialize()?;
    
    // Preparar flujo de trabajo
    executor.prepare(&workflow)?;
    
    // Ejecutar flujo de trabajo
    executor.execute(&workflow).await?;
    
    // Finalizar ejecutor
    executor.finalize()?;
    
    Ok(())
}
```

## Modos de extracción

### Extracción completa

```rust
.source("users", "postgres", serde_json::json!({
    "host": "localhost",
    "port": 5432,
    "database": "example",
    "table": "users"
}))
```

### Extracción incremental

```rust
.source_with_mode("orders", "mysql", "incremental", serde_json::json!({
    "host": "localhost",
    "port": 3306,
    "database": "example",
    "table": "orders",
    "incremental": {
        "column": "updated_at",
        "cursor_field": "updated_at"
    }
}))
```

### Extracción CDC

```rust
.source_with_mode("products", "postgres", "cdc", serde_json::json!({
    "host": "localhost",
    "port": 5432,
    "database": "example",
    "table": "products",
    "cdc": {
        "slot_name": "dataflare_slot",
        "publication_name": "dataflare_pub",
        "plugin": "pgoutput"
    }
}))
```

## Transformaciones

### Mapeo

```rust
.transformation("mapping_transform", "mapping", vec!["source"], serde_json::json!({
    "mappings": [
        {
            "source": "name",
            "destination": "user.name",
            "transform": "uppercase"
        },
        {
            "source": "email",
            "destination": "user.email",
            "transform": "lowercase"
        }
    ]
}))
```

### Filtro

```rust
.transformation("filter_transform", "filter", vec!["source"], serde_json::json!({
    "condition": "user.email != null"
}))
```

## Ejemplos

Consulta la carpeta `examples/` para ver ejemplos completos:

- `simple_workflow.rs`: Flujo de trabajo básico
- `incremental_workflow.rs`: Flujo de trabajo con extracción incremental
- `cdc_workflow.rs`: Flujo de trabajo con CDC

## Licencia

Este proyecto está licenciado bajo los términos de la licencia MIT o Apache-2.0, a elección del usuario.
