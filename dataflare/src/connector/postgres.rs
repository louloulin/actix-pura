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

    /// 设置 CDC 复制槽
    async fn setup_cdc_slot(&mut self) -> Result<()> {
        // 确保客户端已连接
        if self.client.is_none() {
            self.connect().await?;
        }

        let client = self.client.as_ref().ok_or_else(|| {
            DataFlareError::Connection("No se pudo conectar a PostgreSQL".to_string())
        })?;

        // 从配置中获取 CDC 参数
        let cdc_config = self.config.get("cdc").ok_or_else(|| {
            DataFlareError::Config("El modo CDC requiere configuración 'cdc'".to_string())
        })?;

        let slot_name = cdc_config.get("slot_name").and_then(|s| s.as_str()).ok_or_else(|| {
            DataFlareError::Config("El modo CDC requiere parámetro 'slot_name'".to_string())
        })?;

        let publication_name = cdc_config.get("publication_name").and_then(|p| p.as_str()).ok_or_else(|| {
            DataFlareError::Config("El modo CDC requiere parámetro 'publication_name'".to_string())
        })?;

        let plugin = cdc_config.get("plugin").and_then(|p| p.as_str()).unwrap_or("pgoutput");

        // 检查复制槽是否存在
        let slot_exists_query = "SELECT 1 FROM pg_replication_slots WHERE slot_name = $1";
        let slot_exists = client.query_opt(slot_exists_query, &[&slot_name]).await
            .map_err(|e| DataFlareError::Query(format!("Error al verificar slot de replicación: {}", e)))?
            .is_some();

        // 如果复制槽不存在，创建它
        if !slot_exists {
            info!("Creando slot de replicación '{}'", slot_name);
            let create_slot_query = format!(
                "SELECT pg_create_logical_replication_slot('{}', '{}')",
                slot_name, plugin
            );
            client.execute(&create_slot_query, &[]).await
                .map_err(|e| DataFlareError::Query(format!("Error al crear slot de replicación: {}", e)))?;
        }

        // 检查发布是否存在
        let publication_exists_query = "SELECT 1 FROM pg_publication WHERE pubname = $1";
        let publication_exists = client.query_opt(publication_exists_query, &[&publication_name]).await
            .map_err(|e| DataFlareError::Query(format!("Error al verificar publicación: {}", e)))?
            .is_some();

        // 如果发布不存在，创建它
        if !publication_exists {
            info!("Creando publicación '{}'", publication_name);
            let table = self.config.get("table").and_then(|t| t.as_str()).ok_or_else(|| {
                DataFlareError::Config("Se requiere el parámetro 'table'".to_string())
            })?;

            let create_publication_query = format!(
                "CREATE PUBLICATION {} FOR TABLE {}",
                publication_name, table
            );
            client.execute(&create_publication_query, &[]).await
                .map_err(|e| DataFlareError::Query(format!("Error al crear publicación: {}", e)))?;
        }

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
        // 克隆 self 以便修改
        let mut this = self.clone();

        // 获取表名
        let table = this.config.get("table").and_then(|t| t.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'table'".to_string())
        })?;

        // 发现表模式
        this.discover_table_schema(table).await
    }

    /// 发现表的模式
    async fn discover_table_schema(&mut self, table: &str) -> Result<Schema> {
        let client = match &self.client {
            Some(client) => client,
            None => {
                self.connect().await?;
                self.client.as_ref().ok_or_else(|| {
                    DataFlareError::Connection("No se pudo conectar a PostgreSQL".to_string())
                })?
            }
        };

        // 查询表结构
        let schema_query = format!(
            "SELECT column_name, data_type, is_nullable
             FROM information_schema.columns
             WHERE table_name = $1
             ORDER BY ordinal_position",
        );

        let rows = client.query(&schema_query, &[&table]).await
            .map_err(|e| DataFlareError::Query(format!("Error al consultar estructura de tabla: {}", e)))?;

        let mut schema = Schema::new();

        // 处理每一列
        for row in rows {
            let column_name: String = row.get("column_name");
            let data_type: String = row.get("data_type");
            let is_nullable: String = row.get("is_nullable");

            // 将 PostgreSQL 数据类型转换为 DataFlare 数据类型
            let field_type = match data_type.as_str() {
                "integer" | "smallint" | "bigint" => crate::model::DataType::Int64,
                "numeric" | "decimal" | "real" | "double precision" => crate::model::DataType::Float64,
                "character varying" | "varchar" | "text" | "char" | "character" => crate::model::DataType::String,
                "boolean" => crate::model::DataType::Boolean,
                "date" | "timestamp" | "timestamp without time zone" | "timestamp with time zone" => crate::model::DataType::Timestamp,
                "json" | "jsonb" => crate::model::DataType::Object,
                _ => crate::model::DataType::String, // 默认为字符串
            };

            // 创建字段并添加到模式
            let nullable = is_nullable == "YES";
            let field = crate::model::Field::new(column_name, field_type).nullable(nullable);
            schema.add_field(field);
        }

        Ok(schema)
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
                // 设置复制槽
                self.setup_cdc_slot().await?;

                // 对于 CDC 模式，我们需要使用逻辑复制 API
                // 这里实现一个简化版本，使用查询模拟 CDC
                // 在实际生产环境中，应该使用 PostgreSQL 的逻辑复制 API

                // 使用提供的状态或当前状态
                let current_state = state.unwrap_or_else(|| self.state.clone());

                // 获取最后更新时间字段
                let timestamp_field = self.config.get("cdc")
                    .and_then(|c| c.get("timestamp_field"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("updated_at");

                // 获取最后更新时间值
                let last_timestamp = current_state.metadata.get("last_timestamp")
                    .map(|s| s.as_str())
                    .unwrap_or("1970-01-01T00:00:00Z");

                // 构建查询，获取自上次更新以来的所有更改
                format!("SELECT *, 'UPDATE' as cdc_operation FROM {} WHERE {} > '{}'",
                    table, timestamp_field, last_timestamp)
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
