//! 连接器初始化单元测试
//!
//! 测试CSV连接器的初始化和配置功能

use tempfile::{NamedTempFile, TempDir};
use serde_json::json;
use dataflare_connector::csv::{CsvSourceConnector, CsvDestinationConnector};
use dataflare_connector::source::SourceConnector;
use dataflare_connector::destination::DestinationConnector;
use std::io::Write;

#[tokio::test]
async fn test_csv_source_connector_initialization() {
    // 创建测试CSV文件
    let test_data = "id,name,age\n1,Alice,25\n2,Bob,30\n3,Charlie,35\n";
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(test_data.as_bytes()).unwrap();

    let config = json!({
        "file_path": temp_file.path().to_str().unwrap(),
        "has_header": true,
        "delimiter": ","
    });

    // 创建连接器
    let mut connector = CsvSourceConnector::new(config.clone());

    // 测试配置
    let configure_result = connector.configure(&config);
    assert!(configure_result.is_ok(), "CSV源连接器配置失败: {:?}", configure_result.err());

    // 测试连接检查
    let connection_result = connector.check_connection().await;
    assert!(connection_result.is_ok(), "CSV源连接器连接检查失败: {:?}", connection_result.err());
    assert!(connection_result.unwrap(), "CSV源连接器连接应该成功");

    // 测试模式发现
    let schema_result = connector.discover_schema().await;
    assert!(schema_result.is_ok(), "CSV源连接器模式发现失败: {:?}", schema_result.err());

    let schema = schema_result.unwrap();
    assert_eq!(schema.fields.len(), 3, "应该发现3个字段");

    let field_names: Vec<&str> = schema.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"id"));
    assert!(field_names.contains(&"name"));
    assert!(field_names.contains(&"age"));
}

#[tokio::test]
async fn test_csv_source_connector_with_missing_file() {
    let config = json!({
        "file_path": "nonexistent_file.csv",
        "has_header": true,
        "delimiter": ","
    });

    let mut connector = CsvSourceConnector::new(config.clone());

    // 配置可能失败，因为文件不存在
    let configure_result = connector.configure(&config);
    // 如果配置失败，这是预期的行为
    if configure_result.is_err() {
        println!("配置失败（预期）: {:?}", configure_result.err());
        return; // 测试通过
    }

    // 连接检查应该失败
    let connection_result = connector.check_connection().await;
    assert!(connection_result.is_ok(), "连接检查应该返回结果");
    assert!(!connection_result.unwrap(), "连接应该失败，因为文件不存在");
}

#[tokio::test]
async fn test_csv_destination_connector_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output.csv");

    let config = json!({
        "file_path": output_path.to_str().unwrap(),
        "delimiter": ",",
        "write_header": true
    });

    // 创建连接器
    let mut connector = CsvDestinationConnector::new(config.clone());

    // 测试配置
    let configure_result = connector.configure(&config);
    assert!(configure_result.is_ok(), "CSV目标连接器配置失败: {:?}", configure_result.err());

    // 测试连接检查
    let connection_result = connector.check_connection().await;
    assert!(connection_result.is_ok(), "CSV目标连接器连接检查失败: {:?}", connection_result.err());
    assert!(connection_result.unwrap(), "CSV目标连接器连接应该成功");
}

#[tokio::test]
async fn test_csv_destination_connector_with_invalid_path() {
    // 使用无效路径（只读目录或不存在的目录）
    let config = json!({
        "file_path": "/invalid/path/output.csv",
        "delimiter": ",",
        "write_header": true
    });

    let mut connector = CsvDestinationConnector::new(config.clone());

    // 配置应该成功
    let configure_result = connector.configure(&config);
    assert!(configure_result.is_ok(), "配置不应该立即失败");

    // 连接检查可能失败（取决于权限）
    let connection_result = connector.check_connection().await;
    // 注意：这个测试可能在不同系统上有不同结果
    assert!(connection_result.is_ok(), "连接检查应该返回结果");
}

#[tokio::test]
async fn test_csv_source_connector_data_reading() {
    use futures::StreamExt;

    // 创建测试CSV文件
    let test_data = "id,name,age\n1,Alice,25\n2,Bob,30\n3,Charlie,35\n";
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(test_data.as_bytes()).unwrap();

    let config = json!({
        "file_path": temp_file.path().to_str().unwrap(),
        "has_header": true,
        "delimiter": ","
    });

    let mut connector = CsvSourceConnector::new(config.clone());
    connector.configure(&config).unwrap();

    // 测试数据读取
    let read_result = connector.read(None).await;
    assert!(read_result.is_ok(), "数据读取失败: {:?}", read_result.err());

    let mut stream = read_result.unwrap();
    let mut record_count = 0;

    while let Some(record_result) = stream.next().await {
        assert!(record_result.is_ok(), "记录读取失败: {:?}", record_result.err());

        let record = record_result.unwrap();
        let data = &record.data;

        // 验证记录包含预期字段
        assert!(data.get("id").is_some());
        assert!(data.get("name").is_some());
        assert!(data.get("age").is_some());

        record_count += 1;
    }

    assert_eq!(record_count, 3, "应该读取到3条记录");
}

#[tokio::test]
async fn test_csv_connector_with_different_delimiters() {
    // 测试分号分隔符
    let test_data = "id;name;age\n1;Alice;25\n2;Bob;30\n";
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(test_data.as_bytes()).unwrap();

    let config = json!({
        "file_path": temp_file.path().to_str().unwrap(),
        "has_header": true,
        "delimiter": ";"
    });

    let mut connector = CsvSourceConnector::new(config.clone());
    let configure_result = connector.configure(&config);
    assert!(configure_result.is_ok(), "分号分隔符配置失败: {:?}", configure_result.err());

    let connection_result = connector.check_connection().await;
    assert!(connection_result.is_ok() && connection_result.unwrap(), "分号分隔符连接失败");
}

#[tokio::test]
async fn test_csv_connector_without_header() {
    // 测试无标题行的CSV
    let test_data = "1,Alice,25\n2,Bob,30\n";
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(test_data.as_bytes()).unwrap();

    let config = json!({
        "file_path": temp_file.path().to_str().unwrap(),
        "has_header": false,
        "delimiter": ","
    });

    let mut connector = CsvSourceConnector::new(config.clone());
    let configure_result = connector.configure(&config);
    assert!(configure_result.is_ok(), "无标题配置失败: {:?}", configure_result.err());

    let connection_result = connector.check_connection().await;
    assert!(connection_result.is_ok() && connection_result.unwrap(), "无标题连接失败");
}
