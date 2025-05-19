//! CSV 读取器示例
//!
//! 这个示例演示了如何使用 DataFlare 的 CSV 连接器读取 CSV 文件。

use serde_json::json;
use dataflare::connector::csv::CsvSourceConnector;
use dataflare::connector::SourceConnector;
use dataflare::error::Result;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;

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
    println!("连接状态: {}", if connection_ok { "成功" } else { "失败" });

    if !connection_ok {
        return Ok(());
    }

    // 发现模式
    let schema = csv_connector.discover_schema().await?;
    println!("发现的模式: {:?}", schema);

    // 读取数据
    let mut stream = csv_connector.read(None).await?;

    // 收集并打印数据
    let mut count = 0;
    while let Some(result) = stream.next().await {
        match result {
            Ok(record) => {
                count += 1;
                println!("记录 #{}: {:?}", count, record.data);
            },
            Err(e) => {
                println!("读取记录时出错: {:?}", e);
            }
        }
    }

    println!("总共读取了 {} 条记录", count);

    Ok(())
}
