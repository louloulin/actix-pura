//! CSV 文件连接器
//!
//! 提供与 CSV 文件交互的功能，支持全量和增量模式。

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use log::{debug, error, info, warn};

use futures::Stream;
use serde_json::{Value, json, Map};

use dataflare_core::{
    error::{DataFlareError, Result},
    message::{DataRecord, DataRecordBatch},
    model::Schema,
    state::SourceState,
    model::Field,
    model::DataType,
};

use crate::{
    source::{SourceConnector, ExtractionMode},
    destination::DestinationConnector,
    destination::{WriteMode, WriteStats},
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
            state: SourceState::new("csv"),
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
                let field = Field::new(header, DataType::String);
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
                    let field = Field::new(field_name, DataType::String);
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
        let mut state = SourceState::new("csv");
        state.add_data("extraction_mode", match self.extraction_mode {
            ExtractionMode::Full => "full",
            ExtractionMode::Incremental => "incremental",
            _ => "full",
        });

        // 对于增量模式，配置游标
        if self.extraction_mode == ExtractionMode::Incremental {
            if let Some(incremental) = config.get("incremental") {
                // 使用行号作为游标
                state.add_data("cursor_field", "row_number");

                // 如果有初始游标值，设置它
                if let Some(cursor_value) = incremental.get("cursor_value").and_then(|c| c.as_str()) {
                    state.add_data("cursor_value", cursor_value);
                } else {
                    state.add_data("cursor_value", "0"); // 默认从第一行开始
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
            error!("CSV源连接器: 文件路径未配置");
            DataFlareError::Config("No se ha configurado la ruta del archivo CSV".to_string())
        })?;

        info!("CSV源连接器: 尝试打开文件 {:?}", file_path);
        let file = match File::open(file_path) {
            Ok(f) => {
                info!("CSV源连接器: 成功打开文件 {:?}", file_path);
                f
            },
            Err(e) => {
                error!("CSV源连接器: 打开文件 {:?} 失败: {}", file_path, e);
                return Err(DataFlareError::Io(e));
            }
        };

        // 创建 CSV 读取器
        info!("CSV源连接器: 创建CSV读取器 (分隔符: {:?}, 标题行: {})", self.delimiter, self.has_header);
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(self.delimiter as u8)
            .has_headers(self.has_header)
            .from_reader(BufReader::new(file));

        // 获取当前状态或使用提供的状态
        let current_state = state.unwrap_or_else(|| self.state.clone());
        info!("CSV源连接器: 使用提取模式: {:?}", self.extraction_mode);

        // 根据提取模式处理
        match self.extraction_mode {
            ExtractionMode::Full => {
                // 全量模式：读取所有记录
                info!("CSV源连接器: 使用全量模式读取数据");
                let records = reader.records()
                    .enumerate()
                    .map(|(i, result)| {
                        match result {
                            Ok(record) => {
                                if i == 0 || i % 1000 == 0 {
                                    debug!("CSV源连接器: 成功读取第 {} 行", i+1);
                                }
                            let row = record.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                            self.create_record_from_row(&row)
                            },
                            Err(e) => {
                                error!("CSV源连接器: 读取第 {} 行时出错: {}", i+1, e);
                                Err(DataFlareError::Csv(format!("Error al leer fila CSV {}: {}", i, e)))
                            }
                        }
                    })
                    .collect::<Vec<_>>();

                info!("CSV源连接器: 完成CSV文件读取，共 {} 条记录", records.len());
                // 创建流
                let stream = futures::stream::iter(records);
                Ok(Box::new(stream))
            },
            ExtractionMode::Incremental => {
                // 增量模式：从上次处理的行继续
                let start_row = current_state.data.get("cursor_value")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);

                info!("CSV源连接器: 使用增量模式读取数据，起始行: {}", start_row);

                // 跳过已处理的行
                let records = reader.records()
                    .enumerate()
                    .skip(start_row)
                    .map(|(i, result)| {
                        match result {
                            Ok(record) => {
                                if (i - start_row) == 0 || (i - start_row) % 1000 == 0 {
                                    debug!("CSV源连接器: 成功读取第 {} 行 (跳过 {} 行后)", i+1, start_row);
                                }
                            let row = record.iter().map(|s| s.to_string()).collect::<Vec<_>>();
                            let mut data_record = self.create_record_from_row(&row)?;

                            // 添加行号作为元数据
                            data_record.metadata.insert("row_number".to_string(), (i + 1).to_string());

                            Ok(data_record)
                            },
                            Err(e) => {
                                error!("CSV源连接器: 读取第 {} 行时出错: {}", i+1, e);
                                Err(DataFlareError::Csv(format!("Error al leer fila CSV {}: {}", i, e)))
                            }
                        }
                    })
                    .collect::<Vec<_>>();

                info!("CSV源连接器: 完成CSV文件增量读取，共 {} 条记录", records.len());
                // 创建流
                let stream = futures::stream::iter(records);
                Ok(Box::new(stream))
            },
            _ => {
                error!("CSV源连接器: 不支持的提取模式");
                Err(DataFlareError::Config("Este modo de extracción no está implementado para CSV".to_string()))
            },
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

#[async_trait]
impl DestinationConnector for CsvDestinationConnector {
    fn configure(&mut self, config: &Value) -> Result<()> {
        self.config = config.clone();

        // 获取文件路径
        if let Some(file_path) = config.get("file_path").and_then(|p| p.as_str()) {
            info!("CSV目标连接器: 配置文件路径为 {}", file_path);
            self.file_path = Some(PathBuf::from(file_path));
        } else {
            error!("CSV目标连接器: 未找到文件路径配置");
        }

        // 获取分隔符
        if let Some(delimiter) = config.get("delimiter").and_then(|d| d.as_str()) {
            if !delimiter.is_empty() {
                info!("CSV目标连接器: 配置分隔符为 '{}'", delimiter);
                self.delimiter = delimiter.chars().next().unwrap_or(',');
            }
        }

        // 获取是否写入标题行
        if let Some(write_header) = config.get("write_header").and_then(|w| w.as_bool()) {
            info!("CSV目标连接器: 配置写入标题行为 {}", write_header);
            self.write_header = write_header;
        }

        // 获取列名
        if let Some(columns) = config.get("columns").and_then(|c| c.as_array()) {
            let col_names: Vec<String> = columns.iter()
                .filter_map(|c| c.as_str())
                .map(|s| s.to_string())
                .collect();

            if !col_names.is_empty() {
                info!("CSV目标连接器: 配置列名为 {:?}", col_names);
                self.columns = col_names;
            }
        }

        info!("CSV目标连接器: 配置完成");
        Ok(())
    }

    async fn check_connection(&self) -> Result<bool> {
        // 检查文件路径是否有效
        if let Some(file_path) = &self.file_path {
            // 检查目录是否存在或可创建
            if let Some(parent) = file_path.parent() {
                if !parent.exists() {
                    // 尝试创建目录
                    match std::fs::create_dir_all(parent) {
                        Ok(_) => {
                            // 创建成功后删除，只是测试权限
                            let _ = std::fs::remove_dir(parent);
                            Ok(true)
                        },
                        Err(_) => Ok(false),
                    }
                } else {
                    // 目录存在，检查是否可写
                    Ok(true)
                }
            } else {
                // 没有父目录，检查当前目录是否可写
                Ok(true)
            }
        } else {
            // 没有配置文件路径
            Ok(false)
        }
    }

    async fn prepare_schema(&self, _schema: &Schema) -> Result<()> {
        // CSV 不需要预先创建模式
        Ok(())
    }

    async fn write_batch(&mut self, batch: &DataRecordBatch, mode: WriteMode) -> Result<WriteStats> {
        let file_path = self.file_path.as_ref().ok_or_else(|| {
            error!("CSV目标连接器: 文件路径未配置");
            DataFlareError::Config("No se ha configurado la ruta del archivo CSV".to_string())
        })?;

        info!("CSV目标连接器: 准备写入文件 {:?}, 写入模式: {:?}", file_path, mode);

        // 检查目录是否存在，不存在则创建
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                info!("CSV目标连接器: 创建目录 {:?}", parent);
            std::fs::create_dir_all(parent).map_err(|e| {
                    error!("CSV目标连接器: 创建目录失败: {}", e);
                DataFlareError::Io(e)
            })?;
        }
        }

        // 根据写入模式确定是否附加到现有文件
        let append = match mode {
            WriteMode::Append => true,
            WriteMode::Overwrite => false,
            _ => {
                error!("CSV目标连接器: 不支持的写入模式 {:?}", mode);
                return Err(DataFlareError::Config(format!(
                    "Modo de escritura no soportado para CSV: {:?}", mode
                )));
            }
        };

        // 创建写入器
        let mut writer = if append && file_path.exists() {
            info!("CSV目标连接器: 附加到现有文件 {:?}", file_path);
            // 附加到现有文件
            let file = std::fs::OpenOptions::new()
                .write(true)
                .append(true)
                .open(file_path)
                .map_err(|e| {
                    error!("CSV目标连接器: 打开文件失败: {}", e);
                    DataFlareError::Io(e)
                })?;

            // 附加模式不写标题
            csv::WriterBuilder::new()
                .delimiter(self.delimiter as u8)
                .has_headers(false)
                .from_writer(file)
        } else {
            // 创建新文件或覆盖现有文件
            info!("CSV目标连接器: 创建新文件 {:?}", file_path);
            let file = std::fs::File::create(file_path).map_err(|e| {
                error!("CSV目标连接器: 创建文件失败: {}", e);
                DataFlareError::Io(e)
            })?;

            csv::WriterBuilder::new()
            .delimiter(self.delimiter as u8)
                .has_headers(self.write_header)
                .from_writer(file)
        };

        self.file_opened = true;

        // 确定标题行
        let headers = if !self.columns.is_empty() {
            info!("CSV目标连接器: 使用配置的列名");
            self.columns.clone()
        } else if !batch.records.is_empty() {
            // 从第一条记录中获取所有字段名
            info!("CSV目标连接器: 从记录中推断列名");
            let mut fields = Vec::new();
            if let Some(first_record) = batch.records.first() {
                if let Some(obj) = first_record.data.as_object() {
                    fields = obj.keys().cloned().collect();
                }
            }
            fields
        } else {
            info!("CSV目标连接器: 记录为空，无法推断列名");
            Vec::new()
        };

        // 如果配置为写入标题且处于覆盖模式，写入标题行
        if self.write_header && !append {
            info!("CSV目标连接器: 写入标题行: {:?}", headers);
            // 写入标题行
            writer.write_record(&headers).map_err(|e| {
                error!("CSV目标连接器: 写入标题行失败: {}", e);
                DataFlareError::Csv(format!("Error al escribir la cabecera: {}", e))
                })?;
        }

        // 写入每条记录
        info!("CSV目标连接器: 开始写入记录，共 {} 条", batch.records.len());
        let mut success_count = 0;
        let mut error_count = 0;
        let mut bytes_written = 0;
        let start = std::time::Instant::now();

        for (i, record) in batch.records.iter().enumerate() {
            let mut row = Vec::new();

            // 按照标题的顺序提取每个字段的值
            for field in &headers {
                let value = record.get_value(field)
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                row.push(value);
            }

            // 估算写入的字节数
            bytes_written += row.iter().map(|s| s.len()).sum::<usize>() as u64;

            // 写入行
            match writer.write_record(&row) {
                    Ok(_) => {
                    success_count += 1;
                    if i == 0 || i % 1000 == 0 {
                        debug!("CSV目标连接器: 成功写入第 {} 条记录", i+1);
                    }
                    },
                Err(e) => {
                    error_count += 1;
                    error!("CSV目标连接器: 写入第 {} 条记录失败: {}", i+1, e);
                }
            }
        }

        // 刷新写入器
        writer.flush().map_err(|e| {
            error!("CSV目标连接器: 刷新写入器失败: {}", e);
            DataFlareError::Csv(format!("Error al finalizar la escritura: {}", e))
        })?;

        // 计算写入时间
        let write_time_ms = start.elapsed().as_millis() as u64;

        info!("CSV目标连接器: 完成写入，成功: {}，失败: {}，字节数: {}，耗时: {}ms",
            success_count, error_count, bytes_written, write_time_ms);

        // 更新记录计数
        self.records_written += success_count as u64;

        Ok(WriteStats {
            records_written: success_count as u64,
            records_failed: error_count as u64,
            bytes_written,
            write_time_ms,
        })
    }

    async fn write_record(&mut self, record: &DataRecord, mode: WriteMode) -> Result<WriteStats> {
        // 创建一个只包含单个记录的批次
        let batch = DataRecordBatch::new(vec![record.clone()]);

        // 使用 write_batch 实现
        self.write_batch(&batch, mode).await
    }

    async fn commit(&mut self) -> Result<()> {
        // CSV 不支持事务，无需特殊处理
        Ok(())
    }

    async fn rollback(&mut self) -> Result<()> {
        // CSV 不支持事务，无需特殊处理
        Ok(())
    }

    fn get_supported_write_modes(&self) -> Vec<WriteMode> {
        // CSV 只支持追加和覆盖模式
        vec![
            WriteMode::Append,
            WriteMode::Overwrite,
        ]
    }
}

/// 注册 CSV 连接器
pub fn register_csv_connectors() {
    // 注册 CSV 源连接器
    crate::registry::register_connector::<dyn SourceConnector>(
        "csv",
        Arc::new(|config: Value| -> Result<Box<dyn SourceConnector>> {
            Ok(Box::new(CsvSourceConnector::new(config)))
        }),
    );

    // 注册 CSV 目标连接器
    crate::registry::register_connector::<dyn DestinationConnector>(
        "csv",
        Arc::new(|config: Value| -> Result<Box<dyn DestinationConnector>> {
            Ok(Box::new(CsvDestinationConnector::new(config)))
        }),
    );
}
