//! Módulo de configuración para DataFlare
//!
//! Define las estructuras y funciones para configurar el sistema DataFlare.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::error::{DataFlareError, Result};

/// Configuración principal de DataFlare
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlareConfig {
    /// Directorio para plugins
    pub plugin_dir: PathBuf,
    
    /// Inicializar sistema de logging automáticamente
    pub init_logging: bool,
    
    /// Nivel de logging
    pub log_level: log::LevelFilter,
    
    /// Configuración de conectores
    pub connectors: ConnectorsConfig,
    
    /// Configuración de procesadores
    pub processors: ProcessorsConfig,
    
    /// Configuración de actores
    pub actors: ActorsConfig,
    
    /// Configuración de métricas
    pub metrics: MetricsConfig,
    
    /// Configuración de seguridad
    pub security: SecurityConfig,
}

impl Default for DataFlareConfig {
    fn default() -> Self {
        Self {
            plugin_dir: PathBuf::from("./plugins"),
            init_logging: true,
            log_level: log::LevelFilter::Info,
            connectors: ConnectorsConfig::default(),
            processors: ProcessorsConfig::default(),
            actors: ActorsConfig::default(),
            metrics: MetricsConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl DataFlareConfig {
    /// Carga la configuración desde un archivo YAML
    pub fn from_yaml_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path)
            .map_err(|e| DataFlareError::Config(format!("Failed to open config file: {}", e)))?;
        
        serde_yaml::from_reader(file)
            .map_err(|e| DataFlareError::Config(format!("Failed to parse config file: {}", e)))
    }
    
    /// Carga la configuración desde un string YAML
    pub fn from_yaml_str(yaml: &str) -> Result<Self> {
        serde_yaml::from_str(yaml)
            .map_err(|e| DataFlareError::Config(format!("Failed to parse config string: {}", e)))
    }
    
    /// Guarda la configuración a un archivo YAML
    pub fn to_yaml_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let file = std::fs::File::create(path)
            .map_err(|e| DataFlareError::Config(format!("Failed to create config file: {}", e)))?;
        
        serde_yaml::to_writer(file, self)
            .map_err(|e| DataFlareError::Config(format!("Failed to write config file: {}", e)))
    }
}

/// Configuración de conectores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorsConfig {
    /// Tamaño máximo de lote para conectores
    pub max_batch_size: usize,
    
    /// Tiempo máximo de espera para operaciones de conectores (en milisegundos)
    pub timeout_ms: u64,
    
    /// Número máximo de reintentos para operaciones de conectores
    pub max_retries: u32,
    
    /// Intervalo entre reintentos (en milisegundos)
    pub retry_interval_ms: u64,
    
    /// Número máximo de conexiones por conector
    pub max_connections: u32,
}

impl Default for ConnectorsConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            timeout_ms: 30000,
            max_retries: 3,
            retry_interval_ms: 1000,
            max_connections: 10,
        }
    }
}

/// Configuración de procesadores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorsConfig {
    /// Número máximo de procesadores concurrentes
    pub max_concurrent_processors: usize,
    
    /// Tamaño máximo de memoria para procesadores (en MB)
    pub max_memory_mb: usize,
    
    /// Tiempo máximo de ejecución para procesadores (en milisegundos)
    pub max_execution_time_ms: u64,
}

impl Default for ProcessorsConfig {
    fn default() -> Self {
        Self {
            max_concurrent_processors: 4,
            max_memory_mb: 1024,
            max_execution_time_ms: 60000,
        }
    }
}

/// Configuración de actores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorsConfig {
    /// Tamaño del buzón de actores
    pub mailbox_size: usize,
    
    /// Número de hilos para el sistema de actores
    pub thread_pool_size: usize,
    
    /// Intervalo de supervisión (en milisegundos)
    pub supervision_interval_ms: u64,
}

impl Default for ActorsConfig {
    fn default() -> Self {
        Self {
            mailbox_size: 100,
            thread_pool_size: num_cpus::get(),
            supervision_interval_ms: 5000,
        }
    }
}

/// Configuración de métricas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Habilitar recolección de métricas
    pub enabled: bool,
    
    /// Intervalo de recolección de métricas (en milisegundos)
    pub collection_interval_ms: u64,
    
    /// Puerto para exponer métricas Prometheus
    pub prometheus_port: u16,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval_ms: 10000,
            prometheus_port: 9090,
        }
    }
}

/// Configuración de seguridad
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Habilitar encriptación de datos en tránsito
    pub encrypt_in_transit: bool,
    
    /// Habilitar encriptación de datos en reposo
    pub encrypt_at_rest: bool,
    
    /// Reglas de enmascaramiento de datos sensibles
    pub masking_rules: Vec<MaskingRule>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            encrypt_in_transit: true,
            encrypt_at_rest: true,
            masking_rules: Vec::new(),
        }
    }
}

/// Regla de enmascaramiento de datos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingRule {
    /// Campo a enmascarar
    pub field: String,
    
    /// Método de enmascaramiento
    pub method: MaskingMethod,
    
    /// Caracteres a mostrar (para método parcial)
    pub show_chars: Option<usize>,
    
    /// Texto de reemplazo (para método completo)
    pub replacement: Option<String>,
}

/// Método de enmascaramiento
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MaskingMethod {
    /// Enmascaramiento completo (reemplaza todo el valor)
    #[serde(rename = "full")]
    Full,
    
    /// Enmascaramiento parcial (muestra algunos caracteres)
    #[serde(rename = "partial")]
    Partial,
    
    /// Enmascaramiento con hash
    #[serde(rename = "hash")]
    Hash,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_default_config() {
        let config = DataFlareConfig::default();
        assert_eq!(config.log_level, log::LevelFilter::Info);
        assert_eq!(config.connectors.max_batch_size, 1000);
    }
    
    #[test]
    fn test_yaml_serialization() {
        let config = DataFlareConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: DataFlareConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.log_level, config.log_level);
    }
    
    #[test]
    fn test_file_io() {
        let config = DataFlareConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        
        config.to_yaml_file(path).unwrap();
        let loaded = DataFlareConfig::from_yaml_file(path).unwrap();
        
        assert_eq!(loaded.connectors.max_batch_size, config.connectors.max_batch_size);
    }
}
