//! Módulo de errores para DataFlare
//!
//! Define los tipos de errores que pueden ocurrir durante la operación de DataFlare.

use std::io;
use thiserror::Error;

#[cfg(feature = "wasm")]
use wasmtime;

/// Tipo de resultado para operaciones de DataFlare
pub type Result<T> = std::result::Result<T, DataFlareError>;

/// Errores que pueden ocurrir en DataFlare
#[derive(Error, Debug)]
pub enum DataFlareError {
    /// Error de configuración
    #[error("Error de configuración: {0}")]
    Config(String),

    /// Error de validación
    #[error("Error de validación: {0}")]
    Validation(String),

    /// Error de conexión
    #[error("Error de conexión: {0}")]
    Connection(String),

    /// Error de extracción de datos
    #[error("Error de extracción: {0}")]
    Extraction(String),

    /// Error de transformación de datos
    #[error("Error de transformación: {0}")]
    Transformation(String),

    /// Error de carga de datos
    #[error("Error de carga: {0}")]
    Loading(String),

    /// Error de esquema
    #[error("Error de esquema: {0}")]
    Schema(String),

    /// Error de serialización/deserialización
    #[error("Error de serialización: {0}")]
    Serialization(String),

    /// Error de plugin
    #[error("Error de plugin: {0}")]
    Plugin(String),

    /// Error de WASM
    #[error("Error de WASM: {0}")]
    Wasm(String),

    /// Error de actor
    #[error("Error de actor: {0}")]
    Actor(String),

    /// Error de flujo de trabajo
    #[error("Error de flujo de trabajo: {0}")]
    Workflow(String),

    /// Error de estado
    #[error("Error de estado: {0}")]
    State(String),

    /// Error de registro
    #[error("Error de registro: {0}")]
    Registry(String),

    /// Error de timeout
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Error de IO
    #[error("Error de IO: {0}")]
    Io(#[from] io::Error),

    /// Error de CSV
    #[error("Error de CSV: {0}")]
    Csv(String),

    /// Error de Serde JSON
    #[error("Error de JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// Error de Serde YAML
    #[error("Error de YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Error de Actix
    #[error("Error de Actix: {0}")]
    Actix(String),

    /// Error de base de datos
    #[error("Error de base de datos: {0}")]
    Database(String),

    /// Error de HTTP
    #[error("Error de HTTP: {0}")]
    Http(String),

    /// Error de cluster
    #[error("Error de cluster: {0}")]
    Cluster(String),

    /// Error desconocido
    #[error("Error desconocido: {0}")]
    Unknown(String),

    /// Error de consulta
    #[error("Error de consulta: {0}")]
    Query(String),

    /// Error de campo
    #[error("Error de campo: {0}")]
    Field(String),

    /// Error de procesamiento
    #[error("Error de procesamiento: {0}")]
    Processing(String),

    /// Funcionalidad no implementada
    #[error("No implementado: {0}")]
    NotImplemented(String),
}

impl From<actix::MailboxError> for DataFlareError {
    fn from(err: actix::MailboxError) -> Self {
        DataFlareError::Actix(format!("Mailbox error: {}", err))
    }
}

impl From<anyhow::Error> for DataFlareError {
    fn from(err: anyhow::Error) -> Self {
        #[cfg(feature = "wasm")]
        {
            // 检查是否是 wasmtime::Error
            if let Some(e) = err.downcast_ref::<wasmtime::Error>() {
                return DataFlareError::Wasm(format!("{}", e));
            }
        }
        DataFlareError::Unknown(format!("{}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DataFlareError::Config("Invalid configuration".to_string());
        assert_eq!(format!("{}", err), "Error de configuración: Invalid configuration");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let err: DataFlareError = io_err.into();
        match err {
            DataFlareError::Io(_) => assert!(true),
            _ => panic!("Expected Io error variant"),
        }
    }
}
