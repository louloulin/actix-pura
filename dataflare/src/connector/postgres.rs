//! Conector PostgreSQL para DataFlare
//!
//! Proporciona funcionalidades para conectar con bases de datos PostgreSQL.

use std::sync::Arc;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use serde_json::{Value, json};
use tokio_postgres::{NoTls, Row};
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};

use crate::{
    error::{DataFlareError, Result},
    message::DataRecord,
    model::Schema,
    state::SourceState,
    connector::source::{SourceConnector, ExtractionMode},
};

/// Conector de fuente PostgreSQL
pub struct PostgresSourceConnector {
    /// Configuración del conector
    config: Value,
    /// Cliente PostgreSQL
    client: Option<tokio_postgres::Client>,
    /// Estado actual
    state: SourceState,
    /// Esquema de los datos
    schema: Schema,
    /// Modo de extracción
    extraction_mode: ExtractionMode,
}

impl PostgresSourceConnector {
    /// Crea un nuevo conector PostgreSQL
    pub fn new(config: Value) -> Self {
        Self {
            config,
            client: None,
            state: SourceState::new(),
            schema: Schema::new(),
            extraction_mode: ExtractionMode::Full,
        }
    }

    /// Establece el modo de extracción
    pub fn with_extraction_mode(mut self, mode: ExtractionMode) -> Self {
        self.extraction_mode = mode;
        self
    }

    /// Conecta con la base de datos PostgreSQL
    async fn connect(&mut self) -> Result<()> {
        // Extraer parámetros de conexión
        let host = self.config.get("host").and_then(|v| v.as_str()).unwrap_or("localhost");
        let port = self.config.get("port").and_then(|v| v.as_u64()).unwrap_or(5432);
        let database = self.config.get("database").and_then(|v| v.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'database'".to_string())
        })?;
        let username = self.config.get("username").and_then(|v| v.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'username'".to_string())
        })?;
        let password = self.config.get("password").and_then(|v| v.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'password'".to_string())
        })?;

        // Construir cadena de conexión
        let connection_string = format!(
            "host={} port={} dbname={} user={} password={}",
            host, port, database, username, password
        );

        // Conectar a PostgreSQL
        let (client, connection) = tokio_postgres::connect(&connection_string, NoTls)
            .await
            .map_err(|e| DataFlareError::Connection(format!("Error al conectar a PostgreSQL: {}", e)))?;

        // Ejecutar la conexión en segundo plano
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!("Error en la conexión PostgreSQL: {}", e);
            }
        });

        self.client = Some(client);
        Ok(())
    }

    /// Convierte una fila de PostgreSQL a un registro de datos
    fn row_to_record(&self, row: Row) -> Result<DataRecord> {
        let mut record_data = serde_json::Map::new();

        // Iterar por las columnas
        for i in 0..row.len() {
            let column_name = row.columns()[i].name();
            let value: Value = match row.try_get(i) {
                Ok(Some(v)) => json!(v),
                Ok(None) => Value::Null,
                Err(_) => {
                    // Intentar convertir tipos específicos
                    if let Ok(v) = row.try_get::<_, i32>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, i64>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, f64>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, String>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, bool>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, DateTime<Utc>>(i) {
                        json!(v.to_rfc3339())
                    } else {
                        Value::Null
                    }
                }
            };

            record_data.insert(column_name.to_string(), value);
        }

        Ok(DataRecord::new(Value::Object(record_data)))
    }
}

#[async_trait]
impl SourceConnector for PostgresSourceConnector {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = config.clone();

        // Configurar modo de extracción
        if let Some(mode) = config.get("mode").and_then(|m| m.as_str()) {
            self.extraction_mode = match mode {
                "full" => ExtractionMode::Full,
                "incremental" => ExtractionMode::Incremental,
                "cdc" => ExtractionMode::CDC,
                "hybrid" => ExtractionMode::Hybrid,
                _ => return Err(DataFlareError::Config(format!("Modo de extracción no válido: {}", mode))),
            };
        }

        // Configurar estado inicial
        let mut state = SourceState::new()
            .with_source_name("postgres")
            .with_extraction_mode(match self.extraction_mode {
                ExtractionMode::Full => "full",
                ExtractionMode::Incremental => "incremental",
                ExtractionMode::CDC => "cdc",
                ExtractionMode::Hybrid => "hybrid",
            });

        // Para modo incremental, configurar cursor
        if self.extraction_mode == ExtractionMode::Incremental {
            if let Some(incremental) = config.get("incremental") {
                if let Some(cursor_field) = incremental.get("cursor_field").and_then(|c| c.as_str()) {
                    state = state.with_cursor_field(cursor_field);
                    
                    // Si hay un valor inicial del cursor, establecerlo
                    if let Some(cursor_value) = incremental.get("cursor_value").and_then(|c| c.as_str()) {
                        state = state.with_cursor_value(cursor_value);
                    }
                }
            }
        }

        // Para modo CDC, configurar posición de log
        if self.extraction_mode == ExtractionMode::CDC {
            if let Some(cdc) = config.get("cdc") {
                if let Some(slot_name) = cdc.get("slot_name").and_then(|s| s.as_str()) {
                    state = state.with_metadata("slot_name", slot_name);
                }
                
                if let Some(log_position) = cdc.get("log_position").and_then(|l| l.as_str()) {
                    state = state.with_log_position(log_position);
                }
            }
        }

        self.state = state;
        Ok(())
    }

    async fn check_connection(&self) -> Result<bool> {
        // Si no hay cliente, intentar conectar
        let client = match &self.client {
            Some(client) => client,
            None => {
                // Clonar self para poder modificarlo
                let mut this = self.clone();
                this.connect().await?;
                match &this.client {
                    Some(client) => client,
                    None => return Err(DataFlareError::Connection("No se pudo conectar a PostgreSQL".to_string())),
                }
            }
        };

        // Ejecutar consulta simple para verificar conexión
        let result = client.query_one("SELECT 1", &[]).await;
        Ok(result.is_ok())
    }

    async fn discover_schema(&self) -> Result<Schema> {
        // Implementación pendiente
        Ok(self.schema.clone())
    }

    async fn read(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>> {
        // Si no hay cliente, conectar
        if self.client.is_none() {
            self.connect().await?;
        }

        let client = self.client.as_ref().ok_or_else(|| {
            DataFlareError::Connection("No hay conexión a PostgreSQL".to_string())
        })?;

        // Obtener tabla
        let table = self.config.get("table").and_then(|t| t.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'table'".to_string())
        })?;

        // Construir consulta según el modo de extracción
        let query = match self.extraction_mode {
            ExtractionMode::Full => {
                format!("SELECT * FROM {}", table)
            },
            ExtractionMode::Incremental => {
                // Usar estado proporcionado o estado actual
                let current_state = state.unwrap_or_else(|| self.state.clone());
                
                // Obtener campo y valor del cursor
                let cursor_field = current_state.cursor_field.as_deref().ok_or_else(|| {
                    DataFlareError::Config("Se requiere cursor_field para modo incremental".to_string())
                })?;
                
                let cursor_value = current_state.cursor_value.as_deref().unwrap_or("0");
                
                format!("SELECT * FROM {} WHERE {} > '{}' ORDER BY {} ASC", 
                    table, cursor_field, cursor_value, cursor_field)
            },
            ExtractionMode::CDC => {
                // CDC requiere configuración adicional
                return Err(DataFlareError::NotImplemented("Modo CDC no implementado completamente".to_string()));
            },
            ExtractionMode::Hybrid => {
                return Err(DataFlareError::NotImplemented("Modo Hybrid no implementado".to_string()));
            }
        };

        // Ejecutar consulta
        let rows = client.query(&query, &[]).await
            .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta: {}", e)))?;

        // Convertir filas a registros
        let records = rows.into_iter()
            .map(|row| self.row_to_record(row))
            .collect::<Vec<_>>();

        // Crear stream
        let stream = futures::stream::iter(records);

        Ok(Box::new(stream))
    }

    fn get_state(&self) -> Result<SourceState> {
        Ok(self.state.clone())
    }

    fn get_extraction_mode(&self) -> ExtractionMode {
        self.extraction_mode.clone()
    }

    async fn estimate_record_count(&self, _state: Option<SourceState>) -> Result<u64> {
        // Si no hay cliente, conectar
        let client = match &self.client {
            Some(client) => client,
            None => {
                // Clonar self para poder modificarlo
                let mut this = self.clone();
                this.connect().await?;
                match &this.client {
                    Some(client) => client,
                    None => return Err(DataFlareError::Connection("No se pudo conectar a PostgreSQL".to_string())),
                }
            }
        };

        // Obtener tabla
        let table = self.config.get("table").and_then(|t| t.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'table'".to_string())
        })?;

        // Ejecutar consulta para contar registros
        let row = client.query_one(&format!("SELECT COUNT(*) FROM {}", table), &[]).await
            .map_err(|e| DataFlareError::Query(format!("Error al contar registros: {}", e)))?;

        let count: i64 = row.get(0);
        Ok(count as u64)
    }
}

// Implementar Clone manualmente
impl Clone for PostgresSourceConnector {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            client: None, // No clonar el cliente, se creará uno nuevo cuando sea necesario
            state: self.state.clone(),
            schema: self.schema.clone(),
            extraction_mode: self.extraction_mode.clone(),
        }
    }
}

/// Registra el conector PostgreSQL
pub fn register_postgres_connector() {
    crate::connector::register_connector::<dyn SourceConnector>(
        "postgres",
        Arc::new(|config: Value| -> Result<Box<dyn SourceConnector>> {
            Ok(Box::new(PostgresSourceConnector::new(config)))
        }),
    );
}
