//! Módulo de mensajes para DataFlare
//!
//! Define los mensajes que se intercambian entre los actores del sistema.

pub mod progress;

use std::collections::HashMap;
use actix::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::state::{CheckpointState, SourceState};

use crate::{
    error::Result,
    model::Schema,
};

/// Registro de datos individual
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    /// Identificador único del registro
    pub id: Uuid,

    /// Datos del registro
    pub data: serde_json::Value,

    /// Metadatos del registro
    pub metadata: HashMap<String, String>,

    /// Marca de tiempo de creación del registro
    pub created_at: DateTime<Utc>,

    /// Marca de tiempo de la última modificación del registro
    pub updated_at: DateTime<Utc>,
}

impl DataRecord {
    /// Crea un nuevo registro de datos
    pub fn new(data: serde_json::Value) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            data,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 创建一个新的JSON数据记录（别名，用于可读性）
    pub fn new_json(data: serde_json::Value) -> Self {
        Self::new(data)
    }

    /// 获取数据的可变对象引用
    pub fn as_object_mut(&mut self) -> Option<&mut serde_json::Map<String, serde_json::Value>> {
        if let serde_json::Value::Object(ref mut map) = self.data {
            Some(map)
        } else {
            None
        }
    }
    
    /// 从指定路径获取字符串值
    pub fn as_str(&self, path: &str) -> Option<&str> {
        if let Some(value) = self.get_value(path) {
            value.as_str()
        } else {
            None
        }
    }
    
    /// 从指定路径获取整数值
    pub fn as_i64(&self, path: &str) -> Option<i64> {
        if let Some(value) = self.get_value(path) {
            value.as_i64()
        } else {
            None
        }
    }

    /// Agrega un metadato al registro
    pub fn add_metadata<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) -> &mut Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Obtiene un valor del registro por ruta JSON
    pub fn get_value(&self, path: &str) -> Option<&serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = &self.data;

        for part in parts {
            match current {
                serde_json::Value::Object(map) => {
                    if let Some(value) = map.get(part) {
                        current = value;
                    } else {
                        return None;
                    }
                },
                serde_json::Value::Array(arr) => {
                    if let Ok(index) = part.parse::<usize>() {
                        if index < arr.len() {
                            current = &arr[index];
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                },
                _ => return None,
            }
        }

        Some(current)
    }

    /// 获取记录中的时间戳字段
    ///
    /// 首先尝试从元数据中获取时间戳字段名，然后从数据中获取该字段的值。
    /// 如果元数据中没有指定时间戳字段，则尝试常见的时间戳字段名。
    pub fn get_timestamp(&self) -> Option<String> {
        // 首先尝试从元数据中获取时间戳字段名
        if let Some(ts_field) = self.metadata.get("timestamp_field") {
            if let Some(value) = self.get_value(ts_field) {
                if let Some(ts) = value.as_str() {
                    return Some(ts.to_string());
                }
            }
        }

        // 尝试常见的时间戳字段名
        let common_ts_fields = [
            "timestamp", "created_at", "updated_at", "date", "time",
            "event_time", "processed_at", "inserted_at", "modified_at"
        ];

        for field in &common_ts_fields {
            if let Some(value) = self.get_value(field) {
                if let Some(ts) = value.as_str() {
                    return Some(ts.to_string());
                }
            }
        }

        // 如果没有找到时间戳字段，返回记录的创建时间
        Some(self.created_at.to_rfc3339())
    }

    /// Establece un valor en el registro por ruta JSON
    pub fn set_value(&mut self, path: &str, value: serde_json::Value) -> Result<()> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return Err(crate::error::DataFlareError::Validation("Ruta vacía".to_string()));
        }

        // Si solo hay una parte, simplemente establecemos el valor en la raíz
        if parts.len() == 1 {
            if let serde_json::Value::Object(map) = &mut self.data {
                map.insert(parts[0].to_string(), value);
                self.updated_at = Utc::now();
                return Ok(());
            } else {
                return Err(crate::error::DataFlareError::Validation(
                    "La raíz del registro no es un objeto".to_string()
                ));
            }
        }

        // Para rutas más profundas, necesitamos navegar por la estructura
        let mut current = &mut self.data;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Último elemento, establecer el valor
                match current {
                    serde_json::Value::Object(map) => {
                        map.insert(part.to_string(), value);
                        self.updated_at = Utc::now();
                        return Ok(());
                    },
                    _ => return Err(crate::error::DataFlareError::Validation(
                        format!("No se puede establecer valor en la ruta {}: no es un objeto", path)
                    )),
                }
            } else {
                // Elemento intermedio, navegar más profundo
                match current {
                    serde_json::Value::Object(map) => {
                        if !map.contains_key(*part) {
                            map.insert(part.to_string(), serde_json::Value::Object(serde_json::Map::new()));
                        }
                        current = map.get_mut(*part).unwrap();
                    },
                    _ => return Err(crate::error::DataFlareError::Validation(
                        format!("No se puede navegar a través de la ruta {}: no es un objeto", path)
                    )),
                }
            }
        }

        // No deberíamos llegar aquí
        Err(crate::error::DataFlareError::Unknown("Error inesperado al establecer valor".to_string()))
    }
}

/// Lote de registros de datos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecordBatch {
    /// Identificador único del lote
    pub id: Uuid,

    /// Registros en el lote
    pub records: Vec<DataRecord>,

    /// Esquema de los registros
    pub schema: Option<Schema>,

    /// Metadatos del lote
    pub metadata: HashMap<String, String>,

    /// Marca de tiempo de creación del lote
    pub created_at: DateTime<Utc>,
}

impl DataRecordBatch {
    /// Crea un nuevo lote de registros
    pub fn new(records: Vec<DataRecord>) -> Self {
        Self {
            id: Uuid::new_v4(),
            records,
            schema: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Crea un nuevo lote con esquema
    pub fn with_schema(records: Vec<DataRecord>, schema: Schema) -> Self {
        Self {
            id: Uuid::new_v4(),
            records,
            schema: Some(schema),
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Agrega un registro al lote
    pub fn add_record(&mut self, record: DataRecord) -> &mut Self {
        self.records.push(record);
        self
    }

    /// Agrega un metadato al lote
    pub fn add_metadata<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) -> &mut Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Obtiene el número de registros en el lote
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Verifica si el lote está vacío
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Mensaje para iniciar la extracción de datos
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct StartExtraction {
    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// ID de la fuente
    pub source_id: String,

    /// Configuración de la fuente
    pub config: serde_json::Value,

    /// Estado previo de la fuente (para extracción incremental)
    pub state: Option<SourceState>,
}

/// Mensaje para procesar un lote de datos
#[derive(Message)]
#[rtype(result = "Result<DataRecordBatch>")]
pub struct ProcessBatch {
    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// ID del procesador
    pub processor_id: String,

    /// Lote de datos a procesar
    pub batch: DataRecordBatch,

    /// Configuración del procesador
    pub config: serde_json::Value,
}

/// Mensaje para cargar un lote de datos
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct LoadBatch {
    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// ID del destino
    pub destination_id: String,

    /// Lote de datos a cargar
    pub batch: DataRecordBatch,

    /// Configuración del destino
    pub config: serde_json::Value,
}

/// Mensaje para crear un punto de control
#[derive(Message)]
#[rtype(result = "Result<CheckpointState>")]
pub struct CreateCheckpoint {
    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// Estado de la fuente
    pub source_state: Option<SourceState>,

    /// Metadatos del punto de control
    pub metadata: HashMap<String, String>,
}

/// Mensaje para notificar el progreso de un flujo de trabajo
#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct WorkflowProgress {
    /// ID del flujo de trabajo
    pub workflow_id: String,

    /// Fase actual del flujo de trabajo
    pub phase: WorkflowPhase,

    /// Progreso (0.0 - 1.0)
    pub progress: f64,

    /// Mensaje de estado
    pub message: String,

    /// Marca de tiempo
    pub timestamp: DateTime<Utc>,
}

/// Fases de un flujo de trabajo
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowPhase {
    /// Inicialización
    Initializing,
    /// Extracción
    Extracting,
    /// Transformación
    Transforming,
    /// Carga
    Loading,
    /// Finalización
    Finalizing,
    /// Completado
    Completed,
    /// Error
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_record() {
        let data = serde_json::json!({
            "name": "John Doe",
            "age": 30,
            "address": {
                "street": "123 Main St",
                "city": "Anytown"
            }
        });

        let mut record = DataRecord::new(data);
        record.add_metadata("source", "test");

        assert_eq!(record.metadata.get("source"), Some(&"test".to_string()));
        assert_eq!(record.get_value("name").unwrap().as_str().unwrap(), "John Doe");
        assert_eq!(record.get_value("address.city").unwrap().as_str().unwrap(), "Anytown");
    }

    #[test]
    fn test_data_record_set_value() {
        let data = serde_json::json!({
            "name": "John Doe",
            "address": {
                "city": "Anytown"
            }
        });

        let mut record = DataRecord::new(data);

        // Establecer un valor en un campo existente
        record.set_value("name", serde_json::json!("Jane Doe")).unwrap();
        assert_eq!(record.get_value("name").unwrap().as_str().unwrap(), "Jane Doe");

        // Establecer un valor en un campo anidado existente
        record.set_value("address.city", serde_json::json!("Newtown")).unwrap();
        assert_eq!(record.get_value("address.city").unwrap().as_str().unwrap(), "Newtown");

        // Establecer un valor en un campo anidado nuevo
        record.set_value("address.street", serde_json::json!("456 Oak St")).unwrap();
        assert_eq!(record.get_value("address.street").unwrap().as_str().unwrap(), "456 Oak St");

        // Establecer un valor en un campo completamente nuevo
        record.set_value("phone", serde_json::json!("555-1234")).unwrap();
        assert_eq!(record.get_value("phone").unwrap().as_str().unwrap(), "555-1234");
    }

    #[test]
    fn test_data_record_batch() {
        let record1 = DataRecord::new(serde_json::json!({"id": 1}));
        let record2 = DataRecord::new(serde_json::json!({"id": 2}));

        let mut batch = DataRecordBatch::new(vec![record1, record2]);
        batch.add_metadata("source", "test");

        assert_eq!(batch.len(), 2);
        assert_eq!(batch.metadata.get("source"), Some(&"test".to_string()));
    }
}
