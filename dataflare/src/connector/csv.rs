//! CSV 文件连接器
//!
//! 提供与 CSV 文件交互的功能，支持全量和增量模式。

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;

use futures::Stream;
use serde_json::{Value, json, Map};

use crate::{
    error::{DataFlareError, Result},
    message::DataRecord,
    model::Schema,
    state::SourceState,
    connector::source::{SourceConnector, ExtractionMode},
};

/// CSV 源连接器
pub struct CsvSourceConnector {
    /// 连接器配置
    config: Value,
    /// 当前状态
    state: SourceState,
    /// 数据模式
    schema: Schema,
    /// 提取模式
    extraction_mode: ExtractionMode,
    /// 文件路径
    file_path: Option<PathBuf>,
    /// 分隔符
    delimiter: char,
    /// 是否有标题行
    has_header: bool,
    /// 列名
    columns: Vec<String>,
}

impl CsvSourceConnector {
    /// 创建新的 CSV 连接器
    pub fn new(config: Value) -> Self {
        Self {
            config,
            state: SourceState::new(),
            schema: Schema::new(),
            extraction_mode: ExtractionMode::Full,
            file_path: None,
            delimiter: ',',
            has_header: true,
            columns: Vec::new(),
        }
    }

    /// 设置提取模式
    pub fn with_extraction_mode(mut self, mode: ExtractionMode) -> Self {
        self.extraction_mode = mode;
        self
    }

    /// 从 CSV 行创建数据记录
    fn create_record_from_row(&self, row: &[String]) -> Result<DataRecord> {
        let mut record_data = Map::new();

        // 确保列名和值的数量匹配
        let columns = if self.columns.is_empty() {
            // 如果没有列名，使用索引作为列名
            (0..row.len()).map(|i| format!("column_{}", i)).collect::<Vec<_>>()
        } else {
            self.columns.clone()
        };

        // 将每个值添加到记录中
        for (i, value) in row.iter().enumerate() {
            if i < columns.len() {
                let column_name = &columns[i];
                record_data.insert(column_name.clone(), json!(value));
            }
        }

        Ok(DataRecord::new(Value::Object(record_data)))
    }

    /// 读取 CSV 文件的标题行
    fn read_header(&mut self) -> Result<Vec<String>> {
        let file_path = self.file_path.as_ref().ok_or_else(|| {
            DataFlareError::Config("No se ha configurado la ruta del archivo CSV".to_string())
        })?;

        let file = File::open(file_path).map_err(|e| {
            DataFlareError::Io(e)
        })?;

        let mut reader = csv::ReaderBuilder::new()
            .delimiter(self.delimiter as u8)
            .has_headers(self.has_header)
            .from_reader(BufReader::new(file));

        if self.has_header {
            // 读取标题行
            let headers = reader.headers().map_err(|e| {
                DataFlareError::Csv(format!("Error al leer encabezados CSV: {}", e))
            })?;

            Ok(headers.iter().map(|s| s.to_string()).collect())
        } else {
            // 如果没有标题行，返回空列表
            Ok(Vec::new())
        }
    }

    /// 发现 CSV 文件的模式
    async fn discover_csv_schema(&mut self) -> Result<Schema> {
        // 读取标题行
        let headers = self.read_header()?;
        let mut schema = Schema::new();

        // 如果有标题行，使用标题作为字段名
        if !headers.is_empty() {
            for header in headers {
                // 默认所有字段为字符串类型
                let field = crate::model::Field::new(header, crate::model::DataType::String);
                schema.add_field(field);
            }
        } else {
            // 如果没有标题行，尝试读取第一行数据来确定列数
            let file_path = self.file_path.as_ref().ok_or_else(|| {
                DataFlareError::Config("No se ha configurado la ruta del archivo CSV".to_string())
            })?;

            let file = File::open(file_path).map_err(|e| {
                DataFlareError::Io(e)
            })?;

            let mut reader = csv::ReaderBuilder::new()
                .delimiter(self.delimiter as u8)
                .has_headers(false)
                .from_reader(BufReader::new(file));

            if let Some(result) = reader.records().next() {
                let record = result.map_err(|e| {
                    DataFlareError::Csv(format!("Error al leer primera fila CSV: {}", e))
                })?;

                // 为每一列创建字段
                for i in 0..record.len() {
                    let field_name = format!("column_{}", i);
                    let field = crate::model::Field::new(field_name, crate::model::DataType::String);
                    schema.add_field(field);
                }
            }
        }

        Ok(schema)
    }
}

#[async_trait]
impl SourceConnector for CsvSourceConnector {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = config.clone();

        // 获取文件路径
        let file_path = config.get("file_path").and_then(|p| p.as_str()).ok_or_else(|| {
            DataFlareError::Config("Se requiere el parámetro 'file_path'".to_string())
        })?;

        self.file_path = Some(PathBuf::from(file_path));

        // 获取分隔符
        if let Some(delimiter) = config.get("delimiter").and_then(|d| d.as_str()) {
            if delimiter.len() == 1 {
                self.delimiter = delimiter.chars().next().unwrap();
            } else {
                return Err(DataFlareError::Config("El delimitador debe ser un único carácter".to_string()));
            }
        }

        // 获取是否有标题行
        if let Some(has_header) = config.get("has_header").and_then(|h| h.as_bool()) {
            self.has_header = has_header;
        }

        // 获取列名（如果提供）
        if let Some(columns) = config.get("columns").and_then(|c| c.as_array()) {
            self.columns = columns.iter()
                .filter_map(|c| c.as_str().map(|s| s.to_string()))
                .collect();
        }

        // 配置提取模式
        if let Some(mode) = config.get("mode").and_then(|m| m.as_str()) {
            self.extraction_mode = match mode {
                "full" => ExtractionMode::Full,
                "incremental" => ExtractionMode::Incremental,
                _ => return Err(DataFlareError::Config(format!("Modo de extracción no válido para CSV: {}", mode))),
            };
        }

        // 配置初始状态
        let mut state = SourceState::new()
            .with_source_name("csv")
            .with_extraction_mode(match self.extraction_mode {
                ExtractionMode::Full => "full",
                ExtractionMode::Incremental => "incremental",
                _ => "full",
            });

        // 对于增量模式，配置游标
        if self.extraction_mode == ExtractionMode::Incremental {
            if let Some(incremental) = config.get("incremental") {
                // 使用行号作为游标
                state = state.with_cursor_field("row_number");

                // 如果有初始游标值，设置它
                if let Some(cursor_value) = incremental.get("cursor_value").and_then(|c| c.as_str()) {
                    state = state.with_cursor_value(cursor_value);
                } else {
                    state = state.with_cursor_value("0"); // 默认从第一行开始
                }
            }
        }

        self.state = state;

        // 如果有标题行，读取它
        if self.has_header {
            self.columns = self.read_header()?;
        }

        Ok(())
    }

    async fn check_connection(&self) -> Result<bool> {
        // 检查文件是否存在且可读
        let file_path = self.file_path.as_ref().ok_or_else(|| {
            DataFlareError::Config("No se ha configurado la ruta del archivo CSV".to_string())
        })?;

        let file_exists = Path::new(file_path).exists();
        if !file_exists {
            return Ok(false);
        }

        // 尝试打开文件
        match File::open(file_path) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn discover_schema(&self) -> Result<Schema> {
        // 克隆 self 以便修改
        let mut this = self.clone();

        // 发现 CSV 模式
        this.discover_csv_schema().await
    }

    async fn read(&mut self, state: Option<SourceState>) -> Result<Box<dyn Stream<Item = Result<DataRecord>> + Send + Unpin>> {
        let file_path = self.file_path.as_ref().ok_or_else(|| {
            DataFlareError::Config("No se ha configurado la ruta del archivo CSV".to_string())
        })?;

        let file = File::open(file_path).map_err(|e| {
            DataFlareError::Io(e)
        })?;

        // 创建 CSV 读取器
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(self.delimiter as u8)
            .has_headers(self.has_header)
            .from_reader(BufReader::new(file));

        // 获取当前状态或使用提供的状态
        let current_state = state.unwrap_or_else(|| self.state.clone());

        // 根据提取模式处理
        match self.extraction_mode {
            ExtractionMode::Full => {
                // 全量模式：读取所有记录
                let records = reader.records()
                    .enumerate()
                    .map(|(i, result)| {
                        result.map_err(|e| {
                            DataFlareError::Csv(format!("Error al leer fila CSV {}: {}", i, e))
                        }).and_then(|record| {
                            let row = record.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                            self.create_record_from_row(&row)
                        })
                    })
                    .collect::<Vec<_>>();

                // 创建流
                let stream = futures::stream::iter(records);
                Ok(Box::new(stream))
            },
            ExtractionMode::Incremental => {
                // 增量模式：从上次处理的行继续
                let start_row = current_state.cursor_value
                    .as_deref()
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);

                // 跳过已处理的行
                let records = reader.records()
                    .enumerate()
                    .skip(start_row)
                    .map(|(i, result)| {
                        result.map_err(|e| {
                            DataFlareError::Csv(format!("Error al leer fila CSV {}: {}", i, e))
                        }).and_then(|record| {
                            let row = record.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                            let mut data_record = self.create_record_from_row(&row)?;

                            // 添加行号作为元数据
                            data_record.metadata.insert("row_number".to_string(), (i + 1).to_string());

                            Ok(data_record)
                        })
                    })
                    .collect::<Vec<_>>();

                // 创建流
                let stream = futures::stream::iter(records);
                Ok(Box::new(stream))
            },
            _ => Err(DataFlareError::Config("Este modo de extracción no está implementado para CSV".to_string())),
        }
    }

    fn get_state(&self) -> Result<SourceState> {
        Ok(self.state.clone())
    }

    fn get_extraction_mode(&self) -> ExtractionMode {
        self.extraction_mode.clone()
    }

    async fn estimate_record_count(&self, _state: Option<SourceState>) -> Result<u64> {
        let file_path = self.file_path.as_ref().ok_or_else(|| {
            DataFlareError::Config("No se ha configurado la ruta del archivo CSV".to_string())
        })?;

        let file = File::open(file_path).map_err(|e| {
            DataFlareError::Io(e)
        })?;

        let mut reader = csv::ReaderBuilder::new()
            .delimiter(self.delimiter as u8)
            .has_headers(self.has_header)
            .from_reader(BufReader::new(file));

        // 计算记录数
        let count = reader.records().count() as u64;
        Ok(count)
    }
}

// 手动实现 Clone
impl Clone for CsvSourceConnector {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: self.state.clone(),
            schema: self.schema.clone(),
            extraction_mode: self.extraction_mode.clone(),
            file_path: self.file_path.clone(),
            delimiter: self.delimiter,
            has_header: self.has_header,
            columns: self.columns.clone(),
        }
    }
}

/// CSV 目标连接器
pub struct CsvDestinationConnector {
    /// 连接器配置
    config: Value,
    /// 文件路径
    file_path: Option<PathBuf>,
    /// 分隔符
    delimiter: char,
    /// 是否写入标题行
    write_header: bool,
    /// 列名
    columns: Vec<String>,
    /// 文件是否已打开
    file_opened: bool,
    /// 写入的记录数
    records_written: u64,
}

impl CsvDestinationConnector {
    /// 创建新的 CSV 目标连接器
    pub fn new(config: Value) -> Self {
        Self {
            config,
            file_path: None,
            delimiter: ',',
            write_header: true,
            columns: Vec::new(),
            file_opened: false,
            records_written: 0,
        }
    }
}

/// 注册 CSV 连接器
pub fn register_csv_connectors() {
    // 注册 CSV 源连接器
    crate::connector::register_connector::<dyn SourceConnector>(
        "csv",
        Arc::new(|config: Value| -> Result<Box<dyn SourceConnector>> {
            Ok(Box::new(CsvSourceConnector::new(config)))
        }),
    );

    // 注册 CSV 目标连接器
    crate::connector::register_connector::<dyn crate::connector::DestinationConnector>(
        "csv",
        Arc::new(|config: Value| -> Result<Box<dyn crate::connector::DestinationConnector>> {
            Ok(Box::new(CsvDestinationConnector::new(config)))
        }),
    );
}
