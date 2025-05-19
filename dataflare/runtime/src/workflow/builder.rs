//! Constructor de flujo de trabajo para DataFlare
//!
//! Proporciona una API fluida para construir flujos de trabajo.

use chrono::{DateTime, Utc};
use serde_json::Value;

use dataflare_core::error::Result;
use crate::workflow::{
    Workflow, SourceConfig, TransformationConfig, DestinationConfig,
    ScheduleConfig, ScheduleType,
};

/// Constructor de flujo de trabajo
pub struct WorkflowBuilder {
    /// Flujo de trabajo en construcción
    workflow: Workflow,
}

impl WorkflowBuilder {
    /// Crea un nuevo constructor de flujo de trabajo
    pub fn new<S: Into<String>>(id: S, name: S) -> Self {
        Self {
            workflow: Workflow::new(id, name),
        }
    }

    /// Establece la descripción del flujo de trabajo
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.workflow = self.workflow.with_description(description);
        self
    }

    /// Establece la versión del flujo de trabajo
    pub fn version<S: Into<String>>(mut self, version: S) -> Self {
        self.workflow = self.workflow.with_version(version);
        self
    }

    /// Agrega una fuente al flujo de trabajo
    pub fn source<S: Into<String>>(mut self, id: S, r#type: S, config: Value) -> Self {
        let source_config = SourceConfig::new(r#type, config);
        self.workflow.add_source(id, source_config);
        self
    }

    /// Agrega una fuente con modo al flujo de trabajo
    pub fn source_with_mode<S: Into<String>>(mut self, id: S, r#type: S, mode: S, config: Value) -> Self {
        let source_config = SourceConfig::new(r#type, config).with_mode(mode);
        self.workflow.add_source(id, source_config);
        self
    }

    /// Agrega una transformación al flujo de trabajo
    pub fn transformation<S: Into<String>, I: IntoIterator<Item = S>>(
        mut self,
        id: S,
        r#type: S,
        inputs: I,
        config: Value,
    ) -> Self {
        let mut transform_config = TransformationConfig::new(r#type, config);
        for input in inputs {
            transform_config.add_input(input);
        }
        self.workflow.add_transformation(id, transform_config);
        self
    }

    /// Agrega un destino al flujo de trabajo
    pub fn destination<S: Into<String>, I: IntoIterator<Item = S>>(
        mut self,
        id: S,
        r#type: S,
        inputs: I,
        config: Value,
    ) -> Self {
        let mut dest_config = DestinationConfig::new(r#type, config);
        for input in inputs {
            dest_config.add_input(input);
        }
        self.workflow.add_destination(id, dest_config);
        self
    }

    /// Establece una programación cron
    pub fn cron_schedule<S: Into<String>>(mut self, expression: S, timezone: Option<S>) -> Self {
        let mut schedule = ScheduleConfig::new(ScheduleType::Cron, expression.into());
        if let Some(tz) = timezone {
            schedule = schedule.with_timezone(tz);
        }
        self.workflow = self.workflow.with_schedule(schedule);
        self
    }

    /// Establece una programación de intervalo
    pub fn interval_schedule<S: Into<String>>(mut self, expression: S) -> Self {
        let schedule = ScheduleConfig::new(ScheduleType::Interval, expression.into());
        self.workflow = self.workflow.with_schedule(schedule);
        self
    }

    /// Establece una programación de ejecución única
    pub fn once_schedule(mut self, datetime: DateTime<Utc>) -> Self {
        let mut schedule = ScheduleConfig::new(ScheduleType::Once, datetime.to_rfc3339());
        schedule = schedule.with_start_date(datetime);
        self.workflow = self.workflow.with_schedule(schedule);
        self
    }

    /// Agrega un metadato al flujo de trabajo
    pub fn metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.workflow.add_metadata(key, value);
        self
    }

    /// Construye el flujo de trabajo
    pub fn build(self) -> Result<Workflow> {
        // Validar el flujo de trabajo
        self.workflow.validate()?;

        Ok(self.workflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_builder() {
        let workflow = WorkflowBuilder::new("test-workflow", "Test Workflow")
            .description("A test workflow")
            .version("1.0.0")
            .source("users", "postgres", serde_json::json!({
                "host": "localhost",
                "port": 5432,
                "database": "test",
                "table": "users"
            }))
            .source_with_mode("orders", "mysql", "incremental", serde_json::json!({
                "host": "localhost",
                "port": 3306,
                "database": "test",
                "table": "orders"
            }))
            .transformation("user_transform", "mapping", vec!["users"], serde_json::json!({
                "mappings": [
                    {
                        "source": "name",
                        "destination": "user.name"
                    }
                ]
            }))
            .transformation("order_transform", "mapping", vec!["orders"], serde_json::json!({
                "mappings": [
                    {
                        "source": "id",
                        "destination": "order.id"
                    }
                ]
            }))
            .transformation("join", "join", vec!["user_transform", "order_transform"], serde_json::json!({
                "join_key": "user_id"
            }))
            .destination("es_users", "elasticsearch", vec!["join"], serde_json::json!({
                "host": "localhost",
                "port": 9200,
                "index": "users"
            }))
            .cron_schedule("0 0 * * *", Some("UTC"))
            .metadata("owner", "test-user")
            .build()
            .unwrap();

        assert_eq!(workflow.id, "test-workflow");
        assert_eq!(workflow.name, "Test Workflow");
        assert_eq!(workflow.description, Some("A test workflow".to_string()));
        assert_eq!(workflow.version, "1.0.0");

        assert_eq!(workflow.sources.len(), 2);
        assert!(workflow.sources.contains_key("users"));
        assert!(workflow.sources.contains_key("orders"));

        assert_eq!(workflow.transformations.len(), 3);
        assert!(workflow.transformations.contains_key("user_transform"));
        assert!(workflow.transformations.contains_key("order_transform"));
        assert!(workflow.transformations.contains_key("join"));

        assert_eq!(workflow.destinations.len(), 1);
        assert!(workflow.destinations.contains_key("es_users"));

        assert!(workflow.schedule.is_some());
        let schedule = workflow.schedule.unwrap();
        assert_eq!(schedule.r#type, ScheduleType::Cron);
        assert_eq!(schedule.expression, "0 0 * * *");
        assert_eq!(schedule.timezone, Some("UTC".to_string()));

        assert_eq!(workflow.metadata.get("owner"), Some(&"test-user".to_string()));
    }

    #[test]
    fn test_invalid_workflow() {
        // Flujo de trabajo sin fuentes
        let result = WorkflowBuilder::new("test", "Test")
            .build();
        assert!(result.is_err());

        // Flujo de trabajo sin destinos
        let result = WorkflowBuilder::new("test", "Test")
            .source("source", "memory", serde_json::json!({}))
            .build();
        assert!(result.is_err());

        // Flujo de trabajo con entrada inválida
        let result = WorkflowBuilder::new("test", "Test")
            .source("source", "memory", serde_json::json!({}))
            .transformation("transform", "mapping", vec!["invalid_source"], serde_json::json!({}))
            .destination("dest", "memory", vec!["transform"], serde_json::json!({}))
            .build();
        assert!(result.is_err());
    }
}
