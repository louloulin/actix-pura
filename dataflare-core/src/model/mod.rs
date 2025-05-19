//! Módulo de modelo de datos para DataFlare
//!
//! Define las estructuras fundamentales para representar datos en el sistema.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::error::{DataFlareError, Result};

/// Tipos de datos soportados por DataFlare
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// Valor nulo
    Null,
    /// Valor booleano
    Boolean,
    /// Entero de 32 bits con signo
    Int32,
    /// Entero de 64 bits con signo
    Int64,
    /// Número de punto flotante de 32 bits
    Float32,
    /// Número de punto flotante de 64 bits
    Float64,
    /// Cadena de texto
    String,
    /// Fecha (sin hora)
    Date,
    /// Hora (sin fecha)
    Time,
    /// Fecha y hora
    Timestamp,
    /// Arreglo de valores
    Array(Box<DataType>),
    /// Objeto (mapa de clave-valor)
    Object,
    /// Datos binarios
    Binary,
    /// Tipo personalizado
    Custom(String),
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::Null => write!(f, "null"),
            DataType::Boolean => write!(f, "boolean"),
            DataType::Int32 => write!(f, "int32"),
            DataType::Int64 => write!(f, "int64"),
            DataType::Float32 => write!(f, "float32"),
            DataType::Float64 => write!(f, "float64"),
            DataType::String => write!(f, "string"),
            DataType::Date => write!(f, "date"),
            DataType::Time => write!(f, "time"),
            DataType::Timestamp => write!(f, "timestamp"),
            DataType::Array(item_type) => write!(f, "array<{}>", item_type),
            DataType::Object => write!(f, "object"),
            DataType::Binary => write!(f, "binary"),
            DataType::Custom(name) => write!(f, "custom<{}>", name),
        }
    }
}

/// Campo de un esquema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    /// Nombre del campo
    pub name: String,
    /// Tipo de datos del campo
    pub data_type: DataType,
    /// Indica si el campo puede ser nulo
    pub nullable: bool,
    /// Descripción del campo
    pub description: Option<String>,
    /// Metadatos adicionales del campo
    pub metadata: HashMap<String, String>,
}

impl Field {
    /// Crea un nuevo campo
    pub fn new<S: Into<String>>(name: S, data_type: DataType) -> Self {
        Self {
            name: name.into(),
            data_type,
            nullable: false,
            description: None,
            metadata: HashMap::new(),
        }
    }
    
    /// Establece si el campo puede ser nulo
    pub fn nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }
    
    /// Establece la descripción del campo
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }
    
    /// Agrega un metadato al campo
    pub fn metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Esquema que define la estructura de los datos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    /// Campos del esquema
    pub fields: Vec<Field>,
    /// Metadatos adicionales del esquema
    pub metadata: HashMap<String, String>,
}

impl Schema {
    /// Crea un nuevo esquema vacío
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            metadata: HashMap::new(),
        }
    }
    
    /// Crea un esquema a partir de una lista de campos
    pub fn from_fields(fields: Vec<Field>) -> Self {
        Self {
            fields,
            metadata: HashMap::new(),
        }
    }
    
    /// Agrega un campo al esquema
    pub fn add_field(&mut self, field: Field) -> &mut Self {
        self.fields.push(field);
        self
    }
    
    /// Agrega un metadato al esquema
    pub fn add_metadata<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) -> &mut Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    
    /// Obtiene un campo por nombre
    pub fn get_field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
    
    /// Verifica si el esquema contiene un campo con el nombre dado
    pub fn has_field(&self, name: &str) -> bool {
        self.fields.iter().any(|f| f.name == name)
    }
    
    /// Fusiona este esquema con otro
    pub fn merge(&self, other: &Schema) -> Result<Schema> {
        let mut fields = self.fields.clone();
        
        // Verificar campos duplicados
        for field in &other.fields {
            if self.has_field(&field.name) {
                return Err(DataFlareError::Schema(format!(
                    "Campo duplicado en la fusión de esquemas: {}", field.name
                )));
            }
            fields.push(field.clone());
        }
        
        // Fusionar metadatos
        let mut metadata = self.metadata.clone();
        for (key, value) in &other.metadata {
            metadata.insert(key.clone(), value.clone());
        }
        
        Ok(Schema { fields, metadata })
    }
    
    /// Convierte el esquema a formato JSON
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| DataFlareError::Serialization(format!("Error al serializar esquema a JSON: {}", e)))
    }
    
    /// Crea un esquema a partir de una cadena JSON
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|e| DataFlareError::Serialization(format!("Error al deserializar esquema desde JSON: {}", e)))
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_data_type_display() {
        assert_eq!(format!("{}", DataType::String), "string");
        assert_eq!(format!("{}", DataType::Array(Box::new(DataType::Int32))), "array<int32>");
        assert_eq!(format!("{}", DataType::Custom("decimal".to_string())), "custom<decimal>");
    }
    
    #[test]
    fn test_field_builder() {
        let field = Field::new("name", DataType::String)
            .nullable(true)
            .description("Person's name")
            .metadata("max_length", "100");
        
        assert_eq!(field.name, "name");
        assert_eq!(field.data_type, DataType::String);
        assert!(field.nullable);
        assert_eq!(field.description, Some("Person's name".to_string()));
        assert_eq!(field.metadata.get("max_length"), Some(&"100".to_string()));
    }
    
    #[test]
    fn test_schema_operations() {
        let mut schema = Schema::new();
        schema.add_field(Field::new("id", DataType::Int64));
        schema.add_field(Field::new("name", DataType::String));
        
        assert!(schema.has_field("id"));
        assert!(schema.has_field("name"));
        assert!(!schema.has_field("age"));
        
        let field = schema.get_field("name").unwrap();
        assert_eq!(field.data_type, DataType::String);
    }
    
    #[test]
    fn test_schema_merge() {
        let mut schema1 = Schema::new();
        schema1.add_field(Field::new("id", DataType::Int64));
        schema1.add_field(Field::new("name", DataType::String));
        
        let mut schema2 = Schema::new();
        schema2.add_field(Field::new("age", DataType::Int32));
        schema2.add_field(Field::new("email", DataType::String));
        
        let merged = schema1.merge(&schema2).unwrap();
        assert_eq!(merged.fields.len(), 4);
        assert!(merged.has_field("id"));
        assert!(merged.has_field("name"));
        assert!(merged.has_field("age"));
        assert!(merged.has_field("email"));
    }
    
    #[test]
    fn test_schema_merge_conflict() {
        let mut schema1 = Schema::new();
        schema1.add_field(Field::new("id", DataType::Int64));
        schema1.add_field(Field::new("name", DataType::String));
        
        let mut schema2 = Schema::new();
        schema2.add_field(Field::new("name", DataType::String)); // Campo duplicado
        
        let result = schema1.merge(&schema2);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_schema_serialization() {
        let mut schema = Schema::new();
        schema.add_field(Field::new("id", DataType::Int64));
        schema.add_field(Field::new("name", DataType::String).nullable(true));
        
        let json = schema.to_json().unwrap();
        let deserialized = Schema::from_json(&json).unwrap();
        
        assert_eq!(deserialized.fields.len(), 2);
        assert!(deserialized.has_field("id"));
        assert!(deserialized.has_field("name"));
        assert_eq!(deserialized.get_field("name").unwrap().nullable, true);
    }
}
