//! CSV 写入器示例
//!
//! 这个示例演示了如何使用 DataFlare 的 CSV 目标连接器写入 CSV 文件。

use serde_json::json;
use dataflare::connector::csv::CsvDestinationConnector;
use dataflare::connector::destination::{DestinationConnector, WriteMode};
use dataflare::message::{DataRecord, DataRecordBatch};
use dataflare::error::Result;

fn main() -> Result<()> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;

    // 创建 CSV 目标连接器配置
    let config = json!({
        "file_path": "dataflare/examples/data/output.csv",
        "delimiter": ",",
        "write_header": true
    });

    // 创建 CSV 目标连接器
    let mut csv_connector = CsvDestinationConnector::new(config.clone());

    // 配置连接器
    csv_connector.configure(&config)?;

    // 创建一些测试数据
    let records = vec![
        DataRecord::new(json!({
            "id": 1,
            "user_name": "张三",
            "user_email": "zhangsan@example.com",
            "user_age": 30
        })),
        DataRecord::new(json!({
            "id": 2,
            "user_name": "李四",
            "user_email": "lisi@example.com",
            "user_age": 25
        })),
        DataRecord::new(json!({
            "id": 3,
            "user_name": "王五",
            "user_email": "wangwu@example.com",
            "user_age": 35
        })),
        DataRecord::new(json!({
            "id": 4,
            "user_name": "钱七",
            "user_email": "qianqi@example.com",
            "user_age": 42
        }))
    ];

    // 创建数据批次
    let batch = DataRecordBatch::new(records);

    // 写入数据
    let rt = tokio::runtime::Runtime::new()?;
    let stats = rt.block_on(async {
        // 检查连接
        let connection_ok = csv_connector.check_connection().await?;
        println!("连接状态: {}", if connection_ok { "成功" } else { "失败" });

        if !connection_ok {
            let empty_stats = dataflare::connector::destination::WriteStats {
                records_written: 0,
                records_failed: 0,
                bytes_written: 0,
                write_time_ms: 0,
            };
            return Ok::<_, dataflare::error::DataFlareError>(empty_stats);
        }

        // 写入数据批次
        let stats = csv_connector.write_batch(&batch, WriteMode::Overwrite).await?;

        // 提交更改
        csv_connector.commit().await?;

        Ok::<_, dataflare::error::DataFlareError>(stats)
    })?;

    // 打印写入统计
    println!("写入统计:");
    println!("  记录写入数: {}", stats.records_written);
    println!("  记录失败数: {}", stats.records_failed);
    println!("  字节写入数: {}", stats.bytes_written);
    println!("  写入时间: {}ms", stats.write_time_ms);
    println!("CSV 文件已写入: {}", config["file_path"].as_str().unwrap());

    Ok(())
}
