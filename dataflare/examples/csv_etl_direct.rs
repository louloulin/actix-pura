//! CSV ETL 直接示例
//!
//! 这个示例演示了如何直接使用 DataFlare 的 CSV 连接器进行 ETL 操作，
//! 而不使用工作流执行器。

use serde_json::json;
use dataflare::connector::csv::{CsvSourceConnector, CsvDestinationConnector};
use dataflare::connector::source::SourceConnector;
use dataflare::connector::destination::{DestinationConnector, WriteMode};
use dataflare::message::{DataRecord, DataRecordBatch};
use dataflare::error::Result;
use futures::StreamExt;

fn main() -> Result<()> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;

    // 创建 tokio 运行时
    let rt = tokio::runtime::Runtime::new()?;

    // 在 tokio 运行时中执行异步操作
    rt.block_on(async {
        // 1. 读取源 CSV 文件
        let source_records = read_source_csv().await?;
        println!("从源 CSV 文件读取了 {} 条记录", source_records.len());

        // 2. 转换数据
        let transformed_records = transform_data(&source_records);
        println!("转换后有 {} 条记录", transformed_records.len());

        // 3. 写入目标 CSV 文件
        write_destination_csv(&transformed_records).await?;
        println!("数据已写入目标 CSV 文件");

        Ok::<_, dataflare::error::DataFlareError>(())
    })?;

    println!("CSV ETL 操作完成！");

    Ok(())
}

/// 读取源 CSV 文件
async fn read_source_csv() -> Result<Vec<DataRecord>> {
    // 创建 CSV 源连接器配置
    let config = json!({
        "file_path": "dataflare/examples/data/sample.csv",
        "has_header": true,
        "delimiter": ",",
    });

    // 创建 CSV 源连接器
    let mut csv_connector = CsvSourceConnector::new(config.clone());

    // 配置连接器
    csv_connector.configure(&config)?;

    // 检查连接
    let connection_ok = csv_connector.check_connection().await?;
    println!("源连接状态: {}", if connection_ok { "成功" } else { "失败" });

    if !connection_ok {
        return Ok(Vec::new());
    }

    // 发现模式
    let schema = csv_connector.discover_schema().await?;
    println!("源模式: {:?}", schema);

    // 读取数据
    let mut stream = csv_connector.read(None).await?;

    // 收集数据
    let mut records = Vec::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(record) => {
                records.push(record);
            },
            Err(e) => {
                println!("读取记录时出错: {:?}", e);
            }
        }
    }

    Ok(records)
}

/// 转换数据
fn transform_data(source_records: &[DataRecord]) -> Vec<DataRecord> {
    let mut transformed_records = Vec::new();

    for record in source_records {
        // 只处理成年人（年龄 >= 18）
        if let Some(age) = record.data.get("age") {
            let age_str = age.as_str().unwrap_or("0");
            let age_num = age_str.parse::<i64>().unwrap_or(0);
            if age_num < 18 {
                println!("跳过未成年人记录: {:?}", record.data);
                continue;
            }
        }

        // 创建转换后的记录
        let mut transformed_data = serde_json::Map::new();

        // 转换 ID 为字符串类型
        if let Some(id) = record.data.get("id") {
            transformed_data.insert("user_id".to_string(), json!(id.to_string()));
        }

        // 复制姓名
        if let Some(name) = record.data.get("name") {
            transformed_data.insert("full_name".to_string(), name.clone());
        }

        // 转换邮箱为小写
        if let Some(email) = record.data.get("email") {
            if let Some(email_str) = email.as_str() {
                transformed_data.insert("email_address".to_string(), json!(email_str.to_lowercase()));
            }
        }

        // 复制年龄
        if let Some(age) = record.data.get("age") {
            transformed_data.insert("user_age".to_string(), age.clone());
        }

        // 添加转换后的记录
        transformed_records.push(DataRecord::new(serde_json::Value::Object(transformed_data)));
    }

    transformed_records
}

/// 写入目标 CSV 文件
async fn write_destination_csv(records: &[DataRecord]) -> Result<()> {
    // 创建 CSV 目标连接器配置
    let config = json!({
        "file_path": "dataflare/examples/data/transformed_output.csv",
        "delimiter": ",",
        "write_header": true
    });

    // 创建 CSV 目标连接器
    let mut csv_connector = CsvDestinationConnector::new(config.clone());

    // 配置连接器
    csv_connector.configure(&config)?;

    // 检查连接
    let connection_ok = csv_connector.check_connection().await?;
    println!("目标连接状态: {}", if connection_ok { "成功" } else { "失败" });

    if !connection_ok {
        return Ok(());
    }

    // 创建数据批次
    let batch = DataRecordBatch::new(records.to_vec());

    // 写入数据批次
    let stats = csv_connector.write_batch(&batch, WriteMode::Overwrite).await?;

    // 提交更改
    csv_connector.commit().await?;

    // 打印写入统计
    println!("写入统计:");
    println!("  记录写入数: {}", stats.records_written);
    println!("  记录失败数: {}", stats.records_failed);
    println!("  字节写入数: {}", stats.bytes_written);
    println!("  写入时间: {}ms", stats.write_time_ms);
    println!("CSV 文件已写入: {}", config["file_path"].as_str().unwrap());

    Ok(())
}
