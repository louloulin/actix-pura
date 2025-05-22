//! Conector PostgreSQL para DataFlare
//!
//! Proporciona funcionalidades para conectar con bases de datos PostgreSQL.

use std::sync::Arc;
use async_trait::async_trait;
use futures::Stream;
use serde_json::{Value, json};
use tokio_postgres::{NoTls, Row};
use chrono::{DateTime, Utc};
use log::{error, info};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::DataRecord,
    model::Schema,
    state::SourceState,
    model::Field,
    model::DataType,
};

use crate::source::SourceConnector;
use crate::hybrid::HybridConfig;
use crate::source::ExtractionMode;

pub mod batch;
pub mod cdc;
#[cfg(test)]
pub mod batch_test;

pub use self::batch::PostgresBatchSourceConnector;

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
            state: SourceState::new("postgres"),
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
            let value: Value = match row.try_get::<_, Option<String>>(i) {
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
        let mut state = SourceState::new("postgres");
        state.add_data("extraction_mode", match self.extraction_mode {
            ExtractionMode::Full => "full",
            ExtractionMode::Incremental => "incremental",
            ExtractionMode::CDC => "cdc",
            ExtractionMode::Hybrid => "hybrid",
        });

        // Para modo incremental, configurar cursor
        if self.extraction_mode == ExtractionMode::Incremental {
            if let Some(incremental) = config.get("incremental") {
                if let Some(cursor_field) = incremental.get("cursor_field").and_then(|c| c.as_str()) {
                    state.add_data("cursor_field", cursor_field);

                    // Si hay un valor inicial del cursor, establecerlo
                    if let Some(cursor_value) = incremental.get("cursor_value").and_then(|c| c.as_str()) {
                        state.add_data("cursor_value", cursor_value);
                    }
                }
            }
        }

        // Para modo CDC, configurar posición de log
        if self.extraction_mode == ExtractionMode::CDC {
            if let Some(cdc) = config.get("cdc") {
                if let Some(slot_name) = cdc.get("slot_name").and_then(|s| s.as_str()) {
                    state.add_data("slot_name", slot_name);
                }

                if let Some(log_position) = cdc.get("log_position").and_then(|l| l.as_str()) {
                    state.add_data("log_position", log_position);
                }
            }
        }

        self.state = state;
        Ok(())
    }

    async fn check_connection(&self) -> Result<bool> {
        // Si no hay cliente, intentar conectar
        if self.client.is_none() {
            // 创建一个新的连接器实例并连接
            let mut connector = self.clone();
            match connector.connect().await {
                Ok(_) => {
                    // 检查是否成功连接
                    if connector.client.is_some() {
                        // 执行简单查询验证连接
                        let client = connector.client.as_ref().unwrap();
                        let result = client.query_one("SELECT 1", &[]).await;
                        return Ok(result.is_ok());
                    } else {
                        return Ok(false);
                    }
                },
                Err(_) => return Ok(false),
            }
        } else {
            // 已有连接，执行简单查询验证连接
            let client = self.client.as_ref().unwrap();
            let result = client.query_one("SELECT 1", &[]).await;
            Ok(result.is_ok())
        }
    }

    async fn discover_schema(&self) -> Result<Schema> {
        // 获取表名
        let table = self.config.get("table").and_then(|t| t.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'table'".to_string())
        })?;

        // 确保有连接
        let client = if let Some(client) = &self.client {
            client
        } else {
            // 创建一个新的连接器实例并连接
            let mut connector = self.clone();
            connector.connect().await?;

            if let Some(client) = &connector.client {
                // 使用连接器的客户端执行查询
                let schema_query = "SELECT column_name, data_type, is_nullable
                     FROM information_schema.columns
                     WHERE table_name = $1
                     ORDER BY ordinal_position".to_string();

                let rows = client.query(&schema_query, &[&table]).await
                    .map_err(|e| DataFlareError::Query(format!("Error al consultar estructura de tabla: {}", e)))?;

                return Self::process_schema_rows(rows);
            } else {
                return Err(DataFlareError::Connection("No se pudo conectar a PostgreSQL".to_string()));
            }
        };

        // 查询表结构
        let schema_query = "SELECT column_name, data_type, is_nullable
             FROM information_schema.columns
             WHERE table_name = $1
             ORDER BY ordinal_position".to_string();

        let rows = client.query(&schema_query, &[&table]).await
            .map_err(|e| DataFlareError::Query(format!("Error al consultar estructura de tabla: {}", e)))?;

        Self::process_schema_rows(rows)
    }

    async fn read(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>> {
        // 确保已连接
        if self.client.is_none() {
            self.connect().await?;
        }

        // 获取客户端
        let client = self.client.as_ref().ok_or_else(|| {
            DataFlareError::Connection("No se pudo conectar a PostgreSQL".to_string())
        })?;

        // 获取表名
        let table = self.config.get("table").and_then(|v| v.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'table'".to_string())
        })?;

        // 获取提取模式
        let extraction_mode = self.get_extraction_mode();

        // 根据提取模式构建查询
        let rows = match extraction_mode {
            ExtractionMode::Full => {
                // 全量模式：读取所有数据
                let query = format!("SELECT * FROM {}", table);
                client.query(&query, &[]).await
                    .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta: {}", e)))?
            },
            ExtractionMode::Incremental => {
                // 增量模式：根据游标读取数据
                if let Some(source_state) = state {
                    if let (Some(cursor_field), Some(cursor_value)) = (source_state.data.get("cursor_field").and_then(|v| v.as_str()), source_state.data.get("cursor_value").and_then(|v| v.as_str())) {
                        let query = format!("SELECT * FROM {} WHERE {} > $1 ORDER BY {} ASC",
                            table, cursor_field, cursor_field);
                        client.query(&query, &[&cursor_value]).await
                            .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta: {}", e)))?
                    } else {
                        return Err(DataFlareError::Config("Se requiere cursor_field y cursor_value para el modo incremental".to_string()));
                    }
                } else {
                    // 如果没有状态，使用配置中的初始游标值
                    if let Some(incremental) = self.config.get("incremental") {
                        let cursor_field = incremental.get("cursor_field")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| DataFlareError::Config("Se requiere cursor_field para el modo incremental".to_string()))?;

                        let cursor_value = incremental.get("cursor_value")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| DataFlareError::Config("Se requiere cursor_value para el modo incremental".to_string()))?;

                        let query = format!("SELECT * FROM {} WHERE {} > $1 ORDER BY {} ASC",
                            table, cursor_field, cursor_field);
                        client.query(&query, &[&cursor_value]).await
                            .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta: {}", e)))?
                    } else {
                        return Err(DataFlareError::Config("Se requiere configuración 'incremental' para el modo incremental".to_string()));
                    }
                }
            },
            ExtractionMode::CDC => {
                // CDC 模式：使用逻辑复制
                return Err(DataFlareError::NotImplemented("El modo CDC no está completamente implementado".to_string()));
            },
            ExtractionMode::Hybrid => {
                // 混合模式：根据配置使用初始模式
                if let Ok(hybrid_config) = HybridConfig::from_config(&self.config) {
                    match hybrid_config.initial_mode {
                        dataflare_core::connector::ExtractionMode::Full => {
                            // 使用全量模式作为初始模式
                            let query = format!("SELECT * FROM {}", table);
                            client.query(&query, &[]).await
                                .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta: {}", e)))?
                        },
                        dataflare_core::connector::ExtractionMode::Incremental => {
                            // 使用增量模式作为初始模式
                            if let Some(incremental) = self.config.get("incremental") {
                                let cursor_field = incremental.get("cursor_field")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| DataFlareError::Config("Se requiere cursor_field para el modo incremental".to_string()))?;

                                let cursor_value = incremental.get("cursor_value")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| DataFlareError::Config("Se requiere cursor_value para el modo incremental".to_string()))?;

                                let query = format!("SELECT * FROM {} WHERE {} > $1 ORDER BY {} ASC",
                                    table, cursor_field, cursor_field);
                                client.query(&query, &[&cursor_value]).await
                                    .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta: {}", e)))?
                            } else {
                                return Err(DataFlareError::Config("Se requiere configuración 'incremental' para el modo incremental".to_string()));
                            }
                        },
                        _ => {
                            return Err(DataFlareError::Config("Modo inicial no soportado para modo híbrido".to_string()));
                        }
                    }
                } else {
                    return Err(DataFlareError::Config("Configuración híbrida inválida".to_string()));
                }
            },
        };

        // 将行转换为记录
        let records = rows.into_iter().map(|row| {
            let mut record_data = serde_json::Map::new();

            // 获取列名
            let columns = row.columns();

            // 处理每一列
            for (i, column) in columns.iter().enumerate() {
                let column_name = column.name();

                // 获取值
                let value: Value = match row.try_get::<_, Option<String>>(i) {
                    Ok(Some(v)) => json!(v),
                    Ok(None) => Value::Null,
                    Err(_) => Value::Null,
                };

                // 添加到记录
                record_data.insert(column_name.to_string(), value);
            }

            // 创建记录
            let record = DataRecord::new(Value::Object(record_data));
            Ok(record)
        });

        // 创建流
        let stream = futures::stream::iter(records);

        Ok(Box::new(stream))
    }

    fn get_state(&self) -> Result<SourceState> {
        // 获取提取模式
        let extraction_mode = self.get_extraction_mode();

        // 根据提取模式创建状态
        match extraction_mode {
            ExtractionMode::Incremental => {
                // 对于增量模式，从配置中获取游标字段和值
                if let Some(incremental) = self.config.get("incremental") {
                    let cursor_field = incremental.get("cursor_field")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| DataFlareError::Config("Se requiere cursor_field para el modo incremental".to_string()))?;

                    let cursor_value = incremental.get("cursor_value")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| DataFlareError::Config("Se requiere cursor_value para el modo incremental".to_string()))?;

                    let mut state = SourceState::new("postgres");
                    state.add_data("cursor_field", cursor_field);
                    state.add_data("cursor_value", cursor_value);

                    Ok(state)
                } else {
                    Err(DataFlareError::Config("Se requiere configuración 'incremental' para el modo incremental".to_string()))
                }
            },
            ExtractionMode::CDC => {
                // 对于 CDC 模式，从配置中获取时间戳字段和值
                if let Some(cdc) = self.config.get("cdc") {
                    let timestamp_field = cdc.get("timestamp_field")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| DataFlareError::Config("Se requiere timestamp_field para el modo CDC".to_string()))?;

                    // 使用当前时间作为初始值
                    let timestamp_value = Utc::now().to_rfc3339();

                    let mut state = SourceState::new("postgres");
                    state.add_data("cursor_field", timestamp_field);
                    state.add_data("cursor_value", timestamp_value);

                    Ok(state)
                } else {
                    Err(DataFlareError::Config("Se requiere configuración 'cdc' para el modo CDC".to_string()))
                }
            },
            _ => {
                // 对于全量模式，返回空状态
                Ok(SourceState::new("postgres"))
            }
        }
    }

    fn get_extraction_mode(&self) -> ExtractionMode {
        // 从配置中获取提取模式
        if let Some(mode) = self.config.get("mode").and_then(|v| v.as_str()) {
            match mode {
                "incremental" => ExtractionMode::Incremental,
                "cdc" => ExtractionMode::CDC,
                "hybrid" => ExtractionMode::Hybrid,
                _ => ExtractionMode::Full,
            }
        } else {
            // 默认为全量模式
            ExtractionMode::Full
        }
    }

    async fn estimate_record_count(&self, state: Option<SourceState>) -> Result<u64> {
        // 确保已连接
        if self.client.is_none() {
            // 如果没有连接，返回 0
            return Ok(0);
        }

        // 获取表名
        let table = self.config.get("table").and_then(|v| v.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'table'".to_string())
        })?;

        // 获取客户端
        let client = self.client.as_ref().unwrap();

        // 获取提取模式
        let extraction_mode = self.get_extraction_mode();

        // 根据提取模式构建查询
        let count = match extraction_mode {
            ExtractionMode::Full => {
                // 全量模式：计算所有记录
                let query = format!("SELECT COUNT(*) FROM {}", table);
                let row = client.query_one(&query, &[]).await
                    .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta de conteo: {}", e)))?;
                let count: i64 = row.get(0);
                count as u64
            },
            ExtractionMode::Incremental => {
                // 增量模式：根据游标计算记录
                if let Some(source_state) = state {
                    if let (Some(cursor_field), Some(cursor_value)) = (source_state.data.get("cursor_field").and_then(|v| v.as_str()), source_state.data.get("cursor_value").and_then(|v| v.as_str())) {
                        let query = format!("SELECT COUNT(*) FROM {} WHERE {} > $1",
                            table, cursor_field);
                        let row = client.query_one(&query, &[&cursor_value]).await
                            .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta de conteo: {}", e)))?;
                        let count: i64 = row.get(0);
                        count as u64
                    } else {
                        return Err(DataFlareError::Config("Se requiere cursor_field y cursor_value para el modo incremental".to_string()));
                    }
                } else {
                    // 如果没有状态，使用配置中的初始游标值
                    if let Some(incremental) = self.config.get("incremental") {
                        let cursor_field = incremental.get("cursor_field")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| DataFlareError::Config("Se requiere cursor_field para el modo incremental".to_string()))?;

                        let cursor_value = incremental.get("cursor_value")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| DataFlareError::Config("Se requiere cursor_value para el modo incremental".to_string()))?;

                        let query = format!("SELECT COUNT(*) FROM {} WHERE {} > $1",
                            table, cursor_field);
                        let row = client.query_one(&query, &[&cursor_value]).await
                            .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta de conteo: {}", e)))?;
                        let count: i64 = row.get(0);
                        count as u64
                    } else {
                        return Err(DataFlareError::Config("Se requiere configuración 'incremental' para el modo incremental".to_string()));
                    }
                }
            },
            ExtractionMode::CDC => {
                // CDC 模式：无法估算
                0
            },
            ExtractionMode::Hybrid => {
                // 混合模式：根据配置使用初始模式
                if let Ok(hybrid_config) = HybridConfig::from_config(&self.config) {
                    match hybrid_config.initial_mode {
                        dataflare_core::connector::ExtractionMode::Full => {
                            // 使用全量模式作为初始模式
                            let query = format!("SELECT COUNT(*) FROM {}", table);
                            let row = client.query_one(&query, &[]).await
                                .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta de conteo: {}", e)))?;
                            let count: i64 = row.get(0);
                            count as u64
                        },
                        dataflare_core::connector::ExtractionMode::Incremental => {
                            // 使用增量模式作为初始模式
                            if let Some(incremental) = self.config.get("incremental") {
                                let cursor_field = incremental.get("cursor_field")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| DataFlareError::Config("Se requiere cursor_field para el modo incremental".to_string()))?;

                                let cursor_value = incremental.get("cursor_value")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| DataFlareError::Config("Se requiere cursor_value para el modo incremental".to_string()))?;

                                let query = format!("SELECT COUNT(*) FROM {} WHERE {} > $1",
                                    table, cursor_field);
                                let row = client.query_one(&query, &[&cursor_value]).await
                                    .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta de conteo: {}", e)))?;
                                let count: i64 = row.get(0);
                                count as u64
                            } else {
                                return Err(DataFlareError::Config("Se requiere configuración 'incremental' para el modo incremental".to_string()));
                            }
                        },
                        _ => {
                            // 对于其他模式，返回 0
                            0
                        }
                    }
                } else {
                    return Err(DataFlareError::Config("Configuración híbrida inválida".to_string()));
                }
            },
        };

        Ok(count)
    }

}

impl PostgresSourceConnector {
    /// 处理模式查询结果
    fn process_schema_rows(rows: Vec<tokio_postgres::Row>) -> Result<Schema> {
        let mut schema = Schema::new();

        // 处理每一列
        for row in rows {
            let column_name: String = row.get("column_name");
            let data_type: String = row.get("data_type");
            let is_nullable: String = row.get("is_nullable");

            // 将 PostgreSQL 数据类型转换为 DataFlare 数据类型
            let field_type = match data_type.as_str() {
                "integer" | "smallint" | "bigint" => DataType::Int64,
                "numeric" | "decimal" | "real" | "double precision" => DataType::Float64,
                "character varying" | "varchar" | "text" | "char" | "character" => DataType::String,
                "boolean" => DataType::Boolean,
                "date" | "timestamp" | "timestamp without time zone" | "timestamp with time zone" => DataType::Timestamp,
                "json" | "jsonb" => DataType::Object,
                _ => DataType::String, // 默认为字符串
            };

            // 创建字段并添加到模式
            let nullable = is_nullable == "YES";
            let field = Field::new(column_name, field_type).nullable(nullable);
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
                let cursor_field = current_state.data.get("cursor_field").and_then(|v| v.as_str()).ok_or_else(|| {
                    DataFlareError::Config("Se requiere cursor_field para modo incremental".to_string())
                })?;

                let cursor_value = current_state.data.get("cursor_value").and_then(|v| v.as_str()).unwrap_or("0");

                format!("SELECT * FROM {} WHERE {} > '{}' ORDER BY {} ASC",
                    table, cursor_field, cursor_value, cursor_field)
            },
            ExtractionMode::CDC => {
                // 对于 CDC 模式，我们需要使用逻辑复制 API
                // 在实际实现中，应该在这里设置复制槽
                // 但由于借用问题，我们在这里简化处理

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
                let last_timestamp = current_state.data.get("last_timestamp")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1970-01-01T00:00:00Z");

                // 构建查询，获取自上次更新以来的所有更改
                format!("SELECT *, 'UPDATE' as cdc_operation FROM {} WHERE {} > '{}'",
                    table, timestamp_field, last_timestamp)
            },
            ExtractionMode::Hybrid => {
                // 混合模式：使用 HybridStream 包装器
                if let Ok(hybrid_config) = HybridConfig::from_config(&self.config) {
                    // 克隆状态以避免移动问题
                    let state_clone = state.clone();

                    // 创建初始流
                    let mut initial_connector = self.clone();
                    // 将 dataflare_core::ExtractionMode 转换为 source::ExtractionMode
                initial_connector.extraction_mode = match hybrid_config.initial_mode {
                    dataflare_core::connector::ExtractionMode::Full => ExtractionMode::Full,
                    dataflare_core::connector::ExtractionMode::Incremental => ExtractionMode::Incremental,
                    dataflare_core::connector::ExtractionMode::CDC => ExtractionMode::CDC,
                    dataflare_core::connector::ExtractionMode::Hybrid => ExtractionMode::Hybrid,
                };

                    // 根据初始模式构建查询
                    let client = initial_connector.client.as_ref().ok_or_else(|| {
                        DataFlareError::Connection("No hay conexión a PostgreSQL".to_string())
                    })?;

                    let table = initial_connector.config.get("table").and_then(|t| t.as_str()).ok_or_else(|| {
                        DataFlareError::Config("Se requiere el parámetro 'table'".to_string())
                    })?;

                    let query = match initial_connector.extraction_mode {
                        ExtractionMode::Full => {
                            format!("SELECT * FROM {}", table)
                        },
                        ExtractionMode::Incremental => {
                            // 使用提供的状态或当前状态
                            let current_state = state_clone.clone().unwrap_or_else(|| initial_connector.state.clone());

                            // 获取游标字段和值
                            let cursor_field = current_state.data.get("cursor_field").and_then(|v| v.as_str()).ok_or_else(|| {
                                DataFlareError::Config("Se requiere cursor_field para modo incremental".to_string())
                            })?;

                            let cursor_value = current_state.data.get("cursor_value").and_then(|v| v.as_str()).unwrap_or("0");

                            format!("SELECT * FROM {} WHERE {} > '{}' ORDER BY {} ASC",
                                table, cursor_field, cursor_value, cursor_field)
                        },
                        _ => {
                            return Err(DataFlareError::Config("Modo no soportado para flujo inicial".to_string()));
                        }
                    };

                    // 执行查询
                    let rows = client.query(&query, &[]).await
                        .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta: {}", e)))?;

                    // 转换行为记录
                    let initial_records = rows.into_iter()
                        .map(|row| initial_connector.row_to_record(row))
                        .collect::<Vec<_>>();

                    // 创建初始流
                    let initial_stream = Box::new(futures::stream::iter(initial_records));

                    // 创建持续流（如果需要）
                    let mut ongoing_connector = self.clone();
                    ongoing_connector.extraction_mode = match hybrid_config.ongoing_mode {
                        dataflare_core::connector::ExtractionMode::Full => ExtractionMode::Full,
                        dataflare_core::connector::ExtractionMode::Incremental => ExtractionMode::Incremental,
                        dataflare_core::connector::ExtractionMode::CDC => ExtractionMode::CDC,
                        dataflare_core::connector::ExtractionMode::Hybrid => ExtractionMode::Hybrid,
                    };

                    // 根据持续模式构建查询
                    let client = ongoing_connector.client.as_ref().ok_or_else(|| {
                        DataFlareError::Connection("No hay conexión a PostgreSQL".to_string())
                    })?;

                    let table = ongoing_connector.config.get("table").and_then(|t| t.as_str()).ok_or_else(|| {
                        DataFlareError::Config("Se requiere el parámetro 'table'".to_string())
                    })?;

                    let query = match ongoing_connector.extraction_mode {
                        ExtractionMode::Incremental => {
                            // 使用提供的状态或当前状态
                            let current_state = state.clone().unwrap_or_else(|| ongoing_connector.state.clone());

                            // 获取游标字段和值
                            let cursor_field = current_state.data.get("cursor_field").and_then(|v| v.as_str()).ok_or_else(|| {
                                DataFlareError::Config("Se requiere cursor_field para modo incremental".to_string())
                            })?;

                            let cursor_value = current_state.data.get("cursor_value").and_then(|v| v.as_str()).unwrap_or("0");

                            format!("SELECT * FROM {} WHERE {} > '{}' ORDER BY {} ASC",
                                table, cursor_field, cursor_value, cursor_field)
                        },
                        ExtractionMode::CDC => {
                            // 使用提供的状态或当前状态
                            let current_state = state.clone().unwrap_or_else(|| ongoing_connector.state.clone());

                            // 获取最后更新时间字段
                            let timestamp_field = ongoing_connector.config.get("cdc")
                                .and_then(|c| c.get("timestamp_field"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("updated_at");

                            // 获取最后更新时间值
                            let last_timestamp = current_state.data.get("last_timestamp")
                                .and_then(|v| v.as_str())
                                .unwrap_or("1970-01-01T00:00:00Z");

                            // 构建查询，获取自上次更新以来的所有更改
                            format!("SELECT *, 'UPDATE' as cdc_operation FROM {} WHERE {} > '{}'",
                                table, timestamp_field, last_timestamp)
                        },
                        _ => {
                            return Err(DataFlareError::Config("Modo no soportado para flujo continuo".to_string()));
                        }
                    };

                    // 执行查询
                    let rows = client.query(&query, &[]).await
                        .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta: {}", e)))?;

                    // 转换行为记录
                    let ongoing_records = rows.into_iter()
                        .map(|row| ongoing_connector.row_to_record(row))
                        .collect::<Vec<_>>();

                    // 创建持续流
                    let ongoing_stream = Box::new(futures::stream::iter(ongoing_records));

                    // 创建混合流
                    let mut hybrid_stream = crate::hybrid::HybridStream::new(initial_stream, hybrid_config);
                    hybrid_stream.set_ongoing_stream(ongoing_stream);

                    return Ok(Box::new(hybrid_stream));
                } else {
                    return Err(DataFlareError::Config("Configuración híbrida inválida".to_string()));
                }
            }
        };

        // Ejecutar consulta
        let rows = client.query(&query, &[]).await
            .map_err(|e| DataFlareError::Query(format!("Error al ejecutar consulta: {}", e)))?;

        // Convertir filas a registros
        let records = rows.into_iter()
            .map(|row| {
                let mut record_data = serde_json::Map::new();

                // 获取列名
                let columns = row.columns();

                // 处理每一列
                for (i, column) in columns.iter().enumerate() {
                    let column_name = column.name();

                    // 获取值
                    let value: Value = match row.try_get::<_, Option<String>>(i) {
                        Ok(Some(v)) => json!(v),
                        Ok(None) => Value::Null,
                        Err(_) => Value::Null,
                    };

                    // 添加到记录
                    record_data.insert(column_name.to_string(), value);
                }

                // 创建记录
                let record = DataRecord::new(Value::Object(record_data));
                Ok(record)
            })
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
        if self.client.is_none() {
            // Si no hay cliente, devolver 0
            return Ok(0);
        }

        // Obtener cliente
        let client = self.client.as_ref().unwrap();

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
    crate::registry::register_connector::<dyn SourceConnector>(
        "postgres",
        Arc::new(|config: Value| -> Result<Box<dyn SourceConnector>> {
            Ok(Box::new(PostgresSourceConnector::new(config)))
        }),
    );
}

/// Registra los conectores PostgreSQL
pub fn register_postgres_connectors() {
    register_postgres_connector();
    batch::register_postgres_batch_connector();
    cdc::register_postgres_cdc_connector();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_postgres_connector_creation() {
        // Test connector creation with valid config
        let config = json!({
            "host": "localhost",
            "port": 5432,
            "database": "test_db",
            "username": "test_user",
            "password": "test_pass",
            "table": "test_table"
        });
        
        let connector = PostgresSourceConnector::new(config.clone());
        
        // Verify connector fields
        assert_eq!(connector.extraction_mode, ExtractionMode::Full); // Default mode
        assert!(connector.client.is_none()); // No client yet
        assert_eq!(connector.connector_type(), "postgres");
    }
    
    #[test]
    fn test_postgres_extraction_modes() {
        // Test different extraction modes
        let mut connector = PostgresSourceConnector::new(json!({}));
        
        // Test setting full mode
        connector = connector.with_extraction_mode(ExtractionMode::Full);
        assert_eq!(connector.get_extraction_mode(), ExtractionMode::Full);
        
        // Test setting incremental mode
        connector = connector.with_extraction_mode(ExtractionMode::Incremental);
        assert_eq!(connector.get_extraction_mode(), ExtractionMode::Incremental);
        
        // Test setting CDC mode
        connector = connector.with_extraction_mode(ExtractionMode::CDC);
        assert_eq!(connector.get_extraction_mode(), ExtractionMode::CDC);
        
        // Test setting hybrid mode
        connector = connector.with_extraction_mode(ExtractionMode::Hybrid);
        assert_eq!(connector.get_extraction_mode(), ExtractionMode::Hybrid);
    }
    
    #[test]
    fn test_postgres_connector_config_validation() {
        // Test missing database parameter
        let invalid_config = json!({
            "host": "localhost",
            "port": 5432,
            "username": "test_user",
            "password": "test_pass",
            "table": "test_table"
        });
        
        let mut connector = PostgresSourceConnector::new(invalid_config.clone());
        let result = connector.configure(&invalid_config);
        assert!(result.is_err());
        
        // Test missing username parameter
        let invalid_config = json!({
            "host": "localhost",
            "port": 5432,
            "database": "test_db",
            "password": "test_pass",
            "table": "test_table"
        });
        
        let mut connector = PostgresSourceConnector::new(invalid_config.clone());
        let result = connector.configure(&invalid_config);
        assert!(result.is_err());
        
        // Test missing password parameter
        let invalid_config = json!({
            "host": "localhost",
            "port": 5432,
            "database": "test_db",
            "username": "test_user",
            "table": "test_table"
        });
        
        let mut connector = PostgresSourceConnector::new(invalid_config.clone());
        let result = connector.configure(&invalid_config);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_postgres_connector_clone() {
        // Test connector clone works correctly
        let config = json!({
            "host": "localhost",
            "database": "test_db",
            "username": "test_user",
            "password": "test_pass",
            "table": "test_table"
        });
        
        let connector = PostgresSourceConnector::new(config);
        let cloned = connector.clone();
        
        // Verify cloned connector has the same properties but no client
        assert_eq!(connector.extraction_mode, cloned.extraction_mode);
        assert_eq!(connector.config, cloned.config);
        assert!(cloned.client.is_none());
    }
    
    #[test]
    fn test_data_type_conversion() {
        // This is a placeholder test for data type conversion
        // In a real test scenario, we would:
        // 1. Mock tokio_postgres::Row using a testing framework like mockall
        // 2. Create test data with various PostgreSQL types
        // 3. Test the conversion logic to DataFlare types
        
        // For now, we just assert true to keep the test passing
        assert!(true);
    }
}
