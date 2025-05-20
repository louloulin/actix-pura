//! # DataFlare Workflow Module
//!
//! This module defines the structures and functionality for defining and executing workflows.
//! It provides both the original workflow implementation and a new, improved workflow engine
//! that follows the dependency inversion principle and uses the new actor message system.

mod builder;
mod executor;
mod parser;
mod template;
mod yaml_parser;
mod engine;

pub use builder::WorkflowBuilder;
pub use executor::WorkflowExecutor;
pub use parser::WorkflowParser;
pub use template::{WorkflowTemplate, WorkflowTemplateManager, TemplateParameter, TemplateParameterValues};
pub use yaml_parser::YamlWorkflowParser;

// New workflow engine exports
pub use engine::{WorkflowEngine, WorkflowDefinition, ComponentConfig, ExecutionMode, WorkflowStatus,
    WorkflowStats, ComponentStats, WorkflowProgress, WorkflowOptions};

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use dataflare_core::error::{DataFlareError, Result};

/// Flujo de trabajo de integración de datos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Identificador único del flujo de trabajo
    pub id: String,

    /// Nombre del flujo de trabajo
    pub name: String,

    /// Descripción del flujo de trabajo
    pub description: Option<String>,

    /// Versión del flujo de trabajo
    pub version: String,

    /// Configuración de fuentes
    pub sources: HashMap<String, SourceConfig>,

    /// Configuración de transformaciones
    pub transformations: HashMap<String, TransformationConfig>,

    /// Configuración de destinos
    pub destinations: HashMap<String, DestinationConfig>,

    /// Configuración de programación
    pub schedule: Option<ScheduleConfig>,

    /// Metadatos adicionales
    pub metadata: HashMap<String, String>,

    /// Marca de tiempo de creación
    pub created_at: DateTime<Utc>,

    /// Marca de tiempo de última modificación
    pub updated_at: DateTime<Utc>,
}

impl Workflow {
    /// Crea un nuevo flujo de trabajo
    pub fn new<S: Into<String>>(id: S, name: S) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            version: "1.0.0".to_string(),
            sources: HashMap::new(),
            transformations: HashMap::new(),
            destinations: HashMap::new(),
            schedule: None,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Establece la descripción del flujo de trabajo
    pub fn with_description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Establece la versión del flujo de trabajo
    pub fn with_version<S: Into<String>>(mut self, version: S) -> Self {
        self.version = version.into();
        self
    }

    /// Agrega una fuente al flujo de trabajo
    pub fn add_source<S: Into<String>>(&mut self, id: S, config: SourceConfig) -> &mut Self {
        self.sources.insert(id.into(), config);
        self
    }

    /// Agrega una transformación al flujo de trabajo
    pub fn add_transformation<S: Into<String>>(&mut self, id: S, config: TransformationConfig) -> &mut Self {
        self.transformations.insert(id.into(), config);
        self
    }

    /// Agrega un destino al flujo de trabajo
    pub fn add_destination<S: Into<String>>(&mut self, id: S, config: DestinationConfig) -> &mut Self {
        self.destinations.insert(id.into(), config);
        self
    }

    /// Establece la programación del flujo de trabajo
    pub fn with_schedule(mut self, schedule: ScheduleConfig) -> Self {
        self.schedule = Some(schedule);
        self
    }

    /// Agrega un metadato al flujo de trabajo
    pub fn add_metadata<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) -> &mut Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Valida el flujo de trabajo
    pub fn validate(&self) -> Result<()> {
        // Validar que haya al menos una fuente
        if self.sources.is_empty() {
            return Err(dataflare_core::error::DataFlareError::Validation("El flujo de trabajo debe tener al menos una fuente".to_string()));
        }

        // Validar que haya al menos un destino
        if self.destinations.is_empty() {
            return Err(dataflare_core::error::DataFlareError::Validation("El flujo de trabajo debe tener al menos un destino".to_string()));
        }

        // Validar que las transformaciones tengan entradas válidas
        for (id, transformation) in &self.transformations {
            for input in &transformation.inputs {
                if !self.sources.contains_key(input) && !self.transformations.contains_key(input) {
                    return Err(dataflare_core::error::DataFlareError::Validation(
                        format!("La transformación {} tiene una entrada inválida: {}", id, input)
                    ));
                }
            }
        }

        // Validar que los destinos tengan entradas válidas
        for (id, destination) in &self.destinations {
            for input in &destination.inputs {
                if !self.sources.contains_key(input) && !self.transformations.contains_key(input) {
                    return Err(dataflare_core::error::DataFlareError::Validation(
                        format!("El destino {} tiene una entrada inválida: {}", id, input)
                    ));
                }
            }
        }

        Ok(())
    }

    /// Serializa el flujo de trabajo a YAML
    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self)
            .map_err(|e| DataFlareError::Serialization(format!("Error al serializar flujo de trabajo a YAML: {}", e)))
    }

    /// Deserializa el flujo de trabajo desde YAML
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        serde_yaml::from_str(yaml)
            .map_err(|e| DataFlareError::Serialization(format!("Error al deserializar flujo de trabajo desde YAML: {}", e)))
    }
}

/// Configuración de fuente
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Tipo de fuente
    pub r#type: String,

    /// Modo de extracción
    pub mode: Option<String>,

    /// Configuración específica de la fuente
    pub config: serde_json::Value,
}

impl SourceConfig {
    /// Crea una nueva configuración de fuente
    pub fn new<S: Into<String>>(r#type: S, config: serde_json::Value) -> Self {
        Self {
            r#type: r#type.into(),
            mode: None,
            config,
        }
    }

    /// Establece el modo de extracción
    pub fn with_mode<S: Into<String>>(mut self, mode: S) -> Self {
        self.mode = Some(mode.into());
        self
    }
}

/// Configuración de transformación
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformationConfig {
    /// Entradas de la transformación
    pub inputs: Vec<String>,

    /// Tipo de transformación
    pub r#type: String,

    /// Configuración específica de la transformación
    pub config: serde_json::Value,
}

impl TransformationConfig {
    /// Crea una nueva configuración de transformación
    pub fn new<S: Into<String>>(r#type: S, config: serde_json::Value) -> Self {
        Self {
            inputs: Vec::new(),
            r#type: r#type.into(),
            config,
        }
    }

    /// Agrega una entrada a la transformación
    pub fn add_input<S: Into<String>>(&mut self, input: S) -> &mut Self {
        self.inputs.push(input.into());
        self
    }
}

/// Configuración de destino
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationConfig {
    /// Entradas del destino
    pub inputs: Vec<String>,

    /// Tipo de destino
    pub r#type: String,

    /// Configuración específica del destino
    pub config: serde_json::Value,
}

impl DestinationConfig {
    /// Crea una nueva configuración de destino
    pub fn new<S: Into<String>>(r#type: S, config: serde_json::Value) -> Self {
        Self {
            inputs: Vec::new(),
            r#type: r#type.into(),
            config,
        }
    }

    /// Agrega una entrada al destino
    pub fn add_input<S: Into<String>>(&mut self, input: S) -> &mut Self {
        self.inputs.push(input.into());
        self
    }
}

/// Configuración de programación
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    /// Tipo de programación
    pub r#type: ScheduleType,

    /// Expresión de programación
    pub expression: String,

    /// Zona horaria
    pub timezone: Option<String>,

    /// Fecha de inicio
    pub start_date: Option<DateTime<Utc>>,

    /// Fecha de fin
    pub end_date: Option<DateTime<Utc>>,
}

impl ScheduleConfig {
    /// Crea una nueva configuración de programación
    pub fn new(r#type: ScheduleType, expression: String) -> Self {
        Self {
            r#type,
            expression,
            timezone: None,
            start_date: None,
            end_date: None,
        }
    }

    /// Establece la zona horaria
    pub fn with_timezone<S: Into<String>>(mut self, timezone: S) -> Self {
        self.timezone = Some(timezone.into());
        self
    }

    /// Establece la fecha de inicio
    pub fn with_start_date(mut self, start_date: DateTime<Utc>) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Establece la fecha de fin
    pub fn with_end_date(mut self, end_date: DateTime<Utc>) -> Self {
        self.end_date = Some(end_date);
        self
    }
}

/// Tipos de programación
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleType {
    /// Programación basada en cron
    #[serde(rename = "cron")]
    Cron,

    /// Programación basada en intervalos
    #[serde(rename = "interval")]
    Interval,

    /// Ejecución única
    #[serde(rename = "once")]
    Once,

    /// Programación basada en eventos
    #[serde(rename = "event")]
    Event,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_creation() {
        let mut workflow = Workflow::new("test-workflow", "Test Workflow")
            .with_description("A test workflow")
            .with_version("1.0.0");

        // Agregar fuente
        let source_config = SourceConfig::new("postgres", serde_json::json!({
            "host": "localhost",
            "port": 5432,
            "database": "test",
            "table": "users"
        })).with_mode("incremental");

        workflow.add_source("users", source_config);

        // Agregar transformación
        let mut transform_config = TransformationConfig::new("mapping", serde_json::json!({
            "mappings": [
                {
                    "source": "name",
                    "destination": "user.name"
                }
            ]
        }));
        transform_config.add_input("users");

        workflow.add_transformation("user_transform", transform_config);

        // Agregar destino
        let mut dest_config = DestinationConfig::new("elasticsearch", serde_json::json!({
            "host": "localhost",
            "port": 9200,
            "index": "users"
        }));
        dest_config.add_input("user_transform");

        workflow.add_destination("es_users", dest_config);

        // Agregar programación
        let schedule = ScheduleConfig::new(
            ScheduleType::Cron,
            "0 0 * * *".to_string()
        ).with_timezone("UTC");

        workflow = workflow.with_schedule(schedule);

        // Validar flujo de trabajo
        assert!(workflow.validate().is_ok());

        // Verificar serialización
        let yaml = workflow.to_yaml().unwrap();
        let deserialized = Workflow::from_yaml(&yaml).unwrap();

        assert_eq!(deserialized.id, workflow.id);
        assert_eq!(deserialized.name, workflow.name);
        assert_eq!(deserialized.sources.len(), workflow.sources.len());
        assert_eq!(deserialized.transformations.len(), workflow.transformations.len());
        assert_eq!(deserialized.destinations.len(), workflow.destinations.len());
    }

    #[test]
    fn test_workflow_validation() {
        // Flujo de trabajo sin fuentes
        let workflow1 = Workflow::new("test1", "Test 1");
        assert!(workflow1.validate().is_err());

        // Flujo de trabajo sin destinos
        let mut workflow2 = Workflow::new("test2", "Test 2");
        workflow2.add_source("source", SourceConfig::new("memory", serde_json::json!({})));
        assert!(workflow2.validate().is_err());

        // Flujo de trabajo con entrada inválida en transformación
        let mut workflow3 = Workflow::new("test3", "Test 3");
        workflow3.add_source("source", SourceConfig::new("memory", serde_json::json!({})));

        let mut transform_config = TransformationConfig::new("mapping", serde_json::json!({}));
        transform_config.add_input("invalid_source");
        workflow3.add_transformation("transform", transform_config);

        let mut dest_config = DestinationConfig::new("memory", serde_json::json!({}));
        dest_config.add_input("transform");
        workflow3.add_destination("dest", dest_config);

        assert!(workflow3.validate().is_err());
    }
}
