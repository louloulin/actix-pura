//! Conector de destino para DataFlare
//!
//! Define la interfaz y funcionalidades para conectores de destinos de datos.

use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value;

use crate::{
    error::Result,
    message::{DataRecord, DataRecordBatch},
    model::Schema,
};

/// Modos de escritura de datos
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteMode {
    /// Agregar datos (append)
    Append,
    /// Sobrescribir datos (overwrite)
    Overwrite,
    /// Fusionar datos (merge)
    Merge,
    /// Actualizar datos (update)
    Update,
    /// Eliminar datos (delete)
    Delete,
}

/// Estadísticas de escritura
#[derive(Debug, Clone)]
pub struct WriteStats {
    /// Número de registros escritos
    pub records_written: u64,
    /// Número de registros fallidos
    pub records_failed: u64,
    /// Número de bytes escritos
    pub bytes_written: u64,
    /// Tiempo de escritura (en milisegundos)
    pub write_time_ms: u64,
}

/// Interfaz para conectores de destinos de datos
#[async_trait]
pub trait DestinationConnector: Send + Sync + 'static {
    /// Configura el conector con los parámetros proporcionados
    fn configure(&mut self, config: &Value) -> Result<()>;

    /// Verifica la conexión con el destino de datos
    async fn check_connection(&self) -> Result<bool>;

    /// Prepara el esquema en el destino
    async fn prepare_schema(&self, schema: &Schema) -> Result<()>;

    /// Escribe un lote de registros en el destino
    async fn write_batch(&mut self, batch: &DataRecordBatch, mode: WriteMode) -> Result<WriteStats>;

    /// Escribe un registro individual en el destino
    async fn write_record(&mut self, record: &DataRecord, mode: WriteMode) -> Result<WriteStats>;

    /// Finaliza la escritura (commit)
    async fn commit(&mut self) -> Result<()>;

    /// Cancela la escritura (rollback)
    async fn rollback(&mut self) -> Result<()>;

    /// Obtiene los modos de escritura soportados
    fn get_supported_write_modes(&self) -> Vec<WriteMode>;
}

/// Registra los conectores de destino predeterminados
pub fn register_default_destinations() {
    // Registrar conector de memoria
    crate::connector::register_connector::<dyn DestinationConnector>(
        "memory",
        Arc::new(|config: Value| -> Result<Box<dyn DestinationConnector>> {
            Ok(Box::new(MemoryDestinationConnector::new(config)))
        }),
    );
}

/// Conector de destino de memoria (para pruebas)
pub struct MemoryDestinationConnector {
    /// Configuración del conector
    config: Value,
    /// Datos en memoria
    data: Vec<DataRecord>,
    /// Esquema de los datos
    schema: Option<Schema>,
    /// Estadísticas de escritura
    stats: WriteStats,
}

impl MemoryDestinationConnector {
    /// Crea un nuevo conector de destino de memoria
    pub fn new(config: Value) -> Self {
        Self {
            config,
            data: Vec::new(),
            schema: None,
            stats: WriteStats {
                records_written: 0,
                records_failed: 0,
                bytes_written: 0,
                write_time_ms: 0,
            },
        }
    }

    /// Obtiene los datos almacenados
    pub fn get_data(&self) -> &[DataRecord] {
        &self.data
    }
}

#[async_trait]
impl DestinationConnector for MemoryDestinationConnector {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = config.clone();
        Ok(())
    }

    async fn check_connection(&self) -> Result<bool> {
        // Siempre está conectado
        Ok(true)
    }

    async fn prepare_schema(&self, schema: &Schema) -> Result<()> {
        // No hace nada, siempre exitoso
        Ok(())
    }

    async fn write_batch(&mut self, batch: &DataRecordBatch, mode: WriteMode) -> Result<WriteStats> {
        let start = std::time::Instant::now();

        match mode {
            WriteMode::Append => {
                // Agregar todos los registros
                self.data.extend_from_slice(&batch.records);
                self.stats.records_written += batch.records.len() as u64;
            },
            WriteMode::Overwrite => {
                // Limpiar datos existentes y agregar nuevos
                self.data.clear();
                self.data.extend_from_slice(&batch.records);
                self.stats.records_written += batch.records.len() as u64;
            },
            WriteMode::Merge => {
                // Fusionar registros por ID
                for record in &batch.records {
                    if let Some(pos) = self.data.iter().position(|r| r.id == record.id) {
                        self.data[pos] = record.clone();
                    } else {
                        self.data.push(record.clone());
                    }
                    self.stats.records_written += 1;
                }
            },
            WriteMode::Update => {
                // Actualizar registros existentes
                for record in &batch.records {
                    if let Some(pos) = self.data.iter().position(|r| r.id == record.id) {
                        self.data[pos] = record.clone();
                        self.stats.records_written += 1;
                    } else {
                        self.stats.records_failed += 1;
                    }
                }
            },
            WriteMode::Delete => {
                // Eliminar registros por ID
                for record in &batch.records {
                    if let Some(pos) = self.data.iter().position(|r| r.id == record.id) {
                        self.data.remove(pos);
                        self.stats.records_written += 1;
                    } else {
                        self.stats.records_failed += 1;
                    }
                }
            },
        }

        // Calcular bytes escritos (aproximado)
        let bytes_written = batch.records.iter()
            .map(|r| serde_json::to_string(&r.data).unwrap_or_default().len() as u64)
            .sum::<u64>();

        self.stats.bytes_written += bytes_written;
        self.stats.write_time_ms += start.elapsed().as_millis() as u64;

        Ok(self.stats.clone())
    }

    async fn write_record(&mut self, record: &DataRecord, mode: WriteMode) -> Result<WriteStats> {
        let batch = DataRecordBatch::new(vec![record.clone()]);
        self.write_batch(&batch, mode).await
    }

    async fn commit(&mut self) -> Result<()> {
        // No hace nada, siempre exitoso
        Ok(())
    }

    async fn rollback(&mut self) -> Result<()> {
        // No hace nada, siempre exitoso
        Ok(())
    }

    fn get_supported_write_modes(&self) -> Vec<WriteMode> {
        vec![
            WriteMode::Append,
            WriteMode::Overwrite,
            WriteMode::Merge,
            WriteMode::Update,
            WriteMode::Delete,
        ]
    }
}

#[cfg(test)]
use mockall::{mock, predicate};

#[cfg(test)]
mock! {
    pub DestinationConnector {}

    #[async_trait]
    impl DestinationConnector for DestinationConnector {
        fn configure(&mut self, config: &Value) -> Result<()>;
        async fn check_connection(&self) -> Result<bool>;
        async fn prepare_schema(&self, schema: &Schema) -> Result<()>;
        async fn write_batch(&mut self, batch: &DataRecordBatch, mode: WriteMode) -> Result<WriteStats>;
        async fn write_record(&mut self, record: &DataRecord, mode: WriteMode) -> Result<WriteStats>;
        async fn commit(&mut self) -> Result<()>;
        async fn rollback(&mut self) -> Result<()>;
        fn get_supported_write_modes(&self) -> Vec<WriteMode>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_destination_connector() {
        // Crear conector
        let mut connector = MemoryDestinationConnector::new(serde_json::json!({}));

        // Verificar conexión
        let connected = connector.check_connection().await.unwrap();
        assert!(connected);

        // Crear registros de prueba
        let record1 = DataRecord::new(serde_json::json!({"id": 1, "name": "Test 1"}));
        let record2 = DataRecord::new(serde_json::json!({"id": 2, "name": "Test 2"}));

        // Crear lote
        let batch = DataRecordBatch::new(vec![record1, record2]);

        // Escribir lote
        let stats = connector.write_batch(&batch, WriteMode::Append).await.unwrap();

        // Verificar estadísticas
        assert_eq!(stats.records_written, 2);
        assert_eq!(stats.records_failed, 0);

        // Verificar datos almacenados
        assert_eq!(connector.get_data().len(), 2);
        assert_eq!(connector.get_data()[0].data.get("name").unwrap().as_str().unwrap(), "Test 1");
        assert_eq!(connector.get_data()[1].data.get("name").unwrap().as_str().unwrap(), "Test 2");
    }
}
