//! CSV 写入测试
//! 用于测试CSV文件写入功能

use dataflare::connector::csv::CsvDestinationConnector;
use dataflare::connector::destination::{DestinationConnector, WriteMode};
use dataflare::message::{DataRecord, DataRecordBatch};
use dataflare::error::Result;
use serde_json::json;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    // 测试路径
    let abs_path = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/data/test_output.csv");
    println!("尝试写入CSV文件到: {}", abs_path.display());

    // 检查父目录
    if let Some(parent) = abs_path.parent() {
        println!("父目录: {}", parent.display());
        println!("父目录是否存在: {}", parent.exists());
        
        // 尝试创建目录
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
            println!("已创建父目录");
        }
    }

    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;

    // 创建 CSV 目标连接器配置
    let config = json!({
        "file_path": abs_path.to_str().unwrap(),
        "delimiter": ",",
        "write_header": true
    });
    println!("配置: {:?}", config);

    // 创建 CSV 目标连接器
    let mut csv_connector = CsvDestinationConnector::new(config.clone());

    // 配置连接器
    csv_connector.configure(&config)?;
    println!("连接器已配置");

    // 检查连接
    let connection_ok = csv_connector.check_connection().await?;
    println!("连接状态: {}", if connection_ok { "成功" } else { "失败" });

    if !connection_ok {
        return Ok(());
    }

    // 创建一些测试数据
    let records = vec![
        DataRecord::new(json!({
            "id": 1,
            "name": "测试1",
            "value": 100
        })),
        DataRecord::new(json!({
            "id": 2, 
            "name": "测试2",
            "value": 200
        }))
    ];
    println!("已创建测试数据记录");

    // 创建数据批次
    let batch = DataRecordBatch::new(records);

    // 写入数据批次
    println!("开始写入数据...");
    let stats = csv_connector.write_batch(&batch, WriteMode::Overwrite).await?;
    println!("数据写入完成");

    // 提交更改
    csv_connector.commit().await?;
    println!("已提交更改");

    // 打印写入统计
    println!("写入统计:");
    println!("  记录写入数: {}", stats.records_written);
    println!("  记录失败数: {}", stats.records_failed);
    println!("  字节写入数: {}", stats.bytes_written);
    println!("  写入时间: {}ms", stats.write_time_ms);

    // 检查文件是否存在
    if abs_path.exists() {
        let metadata = std::fs::metadata(&abs_path)?;
        println!("CSV 文件已写入: {} (大小: {} 字节)", abs_path.display(), metadata.len());
        
        // 显示文件内容
        let content = std::fs::read_to_string(&abs_path)?;
        println!("文件内容:\n{}", content);
    } else {
        println!("错误: 文件未创建!");
    }

    Ok(())
} 