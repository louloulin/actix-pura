//! Módulo de estado para DataFlare
//!
//! Define las estructuras para gestionar el estado de los componentes.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::{DataFlareError, Result};

/// Estado de una fuente de datos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceState {
    /// Identificador único del estado
    pub id: Uuid,

    /// Nombre de la fuente
    pub source_name: Option<String>,

    /// Modo de extracción
    pub extraction_mode: Option<String>,

    /// Valor del cursor (para extracción incremental)
    pub cursor_value: Option<String>,

    /// Campo del cursor (para extracción incremental)
    pub cursor_field: Option<String>,

    /// Posición de log (para CDC)
    pub log_position: Option<String>,

    /// Marca de tiempo de la última extracción
    pub last_extraction_time: Option<DateTime<Utc>>,

    /// Número de registros extraídos
    pub records_extracted: u64,

    /// Número de bytes extraídos
    pub bytes_extracted: u64,

    /// Metadatos adicionales
    pub metadata: HashMap<String, String>,

    /// Marca de tiempo de creación del estado
    pub created_at: DateTime<Utc>,

    /// Marca de tiempo de la última actualización del estado
    pub updated_at: DateTime<Utc>,
}

impl SourceState {
    /// Crea un nuevo estado de fuente
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            source_name: None,
            extraction_mode: None,
            cursor_value: None,
            cursor_field: None,
            log_position: None,
            last_extraction_time: None,
            records_extracted: 0,
            bytes_extracted: 0,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Establece el nombre de la fuente
    pub fn with_source_name<S: Into<String>>(mut self, name: S) -> Self {
        self.source_name = Some(name.into());
        self
    }

    /// Establece el modo de extracción
    pub fn with_extraction_mode<S: Into<String>>(mut self, mode: S) -> Self {
        self.extraction_mode = Some(mode.into());
        self
    }

    /// Establece el valor del cursor
    pub fn with_cursor<S: Into<String>, F: Into<String>>(mut self, field: F, value: S) -> Self {
        self.cursor_field = Some(field.into());
        self.cursor_value = Some(value.into());
        self
    }

    /// Establece el campo del cursor
    pub fn with_cursor_field<S: Into<String>>(mut self, field: S) -> Self {
        self.cursor_field = Some(field.into());
        self
    }

    /// Establece el valor del cursor
    pub fn with_cursor_value<S: Into<String>>(mut self, value: S) -> Self {
        self.cursor_value = Some(value.into());
        self
    }

    /// Establece la posición de log
    pub fn with_log_position<S: Into<String>>(mut self, position: S) -> Self {
        self.log_position = Some(position.into());
        self
    }

    /// Agrega un metadato
    pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Actualiza el estado con información de extracción
    pub fn update_extraction_stats(&mut self, records: u64, bytes: u64) {
        self.records_extracted += records;
        self.bytes_extracted += bytes;
        self.last_extraction_time = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Serializa el estado a JSON
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| DataFlareError::Serialization(format!("Error al serializar estado: {}", e)))
    }

    /// Deserializa el estado desde JSON
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|e| DataFlareError::Serialization(format!("Error al deserializar estado: {}", e)))
    }
}

impl Default for SourceState {
    fn default() -> Self {
        Self::new()
    }
}

/// Estado de un punto de control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointState {
    /// Identificador único del punto de control
    pub id: Uuid,

    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// Estados de las fuentes
    pub source_states: HashMap<String, SourceState>,

    /// Marca de tiempo del punto de control
    pub checkpoint_time: DateTime<Utc>,

    /// Metadatos adicionales
    pub metadata: HashMap<String, String>,
}

impl CheckpointState {
    /// Crea un nuevo punto de control
    pub fn new<S: Into<String>>(workflow_id: S) -> Self {
        Self {
            id: Uuid::new_v4(),
            workflow_id: workflow_id.into(),
            source_states: HashMap::new(),
            checkpoint_time: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Agrega el estado de una fuente
    pub fn add_source_state<S: Into<String>>(&mut self, source_id: S, state: SourceState) -> &mut Self {
        self.source_states.insert(source_id.into(), state);
        self
    }

    /// Agrega un metadato
    pub fn add_metadata<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) -> &mut Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Serializa el punto de control a JSON
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| DataFlareError::Serialization(format!("Error al serializar punto de control: {}", e)))
    }

    /// Deserializa el punto de control desde JSON
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|e| DataFlareError::Serialization(format!("Error al deserializar punto de control: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_state() {
        let mut state = SourceState::new()
            .with_source_name("test-source")
            .with_extraction_mode("incremental")
            .with_cursor("updated_at", "2023-01-01T00:00:00Z")
            .with_metadata("test-key", "test-value");

        assert_eq!(state.source_name, Some("test-source".to_string()));
        assert_eq!(state.extraction_mode, Some("incremental".to_string()));
        assert_eq!(state.cursor_field, Some("updated_at".to_string()));
        assert_eq!(state.cursor_value, Some("2023-01-01T00:00:00Z".to_string()));
        assert_eq!(state.metadata.get("test-key"), Some(&"test-value".to_string()));

        // Actualizar estadísticas
        state.update_extraction_stats(100, 1024);
        assert_eq!(state.records_extracted, 100);
        assert_eq!(state.bytes_extracted, 1024);
        assert!(state.last_extraction_time.is_some());
    }

    #[test]
    fn test_source_state_serialization() {
        let state = SourceState::new()
            .with_source_name("test-source")
            .with_extraction_mode("incremental")
            .with_cursor("updated_at", "2023-01-01T00:00:00Z");

        let json = state.to_json().unwrap();
        let deserialized = SourceState::from_json(&json).unwrap();

        assert_eq!(deserialized.source_name, state.source_name);
        assert_eq!(deserialized.extraction_mode, state.extraction_mode);
        assert_eq!(deserialized.cursor_field, state.cursor_field);
        assert_eq!(deserialized.cursor_value, state.cursor_value);
    }

    #[test]
    fn test_checkpoint_state() {
        let source_state1 = SourceState::new()
            .with_source_name("source1")
            .with_extraction_mode("full");

        let source_state2 = SourceState::new()
            .with_source_name("source2")
            .with_extraction_mode("incremental");

        let mut checkpoint = CheckpointState::new("test-workflow");
        checkpoint.add_source_state("source1", source_state1);
        checkpoint.add_source_state("source2", source_state2);
        checkpoint.add_metadata("test-key", "test-value");

        assert_eq!(checkpoint.workflow_id, "test-workflow");
        assert_eq!(checkpoint.source_states.len(), 2);
        assert!(checkpoint.source_states.contains_key("source1"));
        assert!(checkpoint.source_states.contains_key("source2"));
        assert_eq!(checkpoint.metadata.get("test-key"), Some(&"test-value".to_string()));
    }

    #[test]
    fn test_checkpoint_state_serialization() {
        let source_state = SourceState::new()
            .with_source_name("test-source")
            .with_extraction_mode("incremental");

        let mut checkpoint = CheckpointState::new("test-workflow");
        checkpoint.add_source_state("test-source", source_state);

        let json = checkpoint.to_json().unwrap();
        let deserialized = CheckpointState::from_json(&json).unwrap();

        assert_eq!(deserialized.workflow_id, checkpoint.workflow_id);
        assert_eq!(deserialized.source_states.len(), 1);
        assert!(deserialized.source_states.contains_key("test-source"));
    }
}
