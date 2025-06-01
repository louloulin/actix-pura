//! CSV 连接器测试
//! 直接测试CSV文件的读取和写入功能

use dataflare::connector::csv::{CsvSourceConnector, CsvDestinationConnector};
use dataflare::connector::source::SourceConnector;
use dataflare::connector::destination::{DestinationConnector, WriteMode};
use dataflare::error::Result;
use dataflare::message::DataRecordBatch;
use std::path::PathBuf;
use serde_json::json;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;

    println!("===== CSV 连接器测试 =====");

    // 测试文件路径
    let input_path = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/data/large_test.csv");
    let output_path = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/data/test_output.csv");

    println!("输入文件: {}", input_path.display());
    println!("输出文件: {}", output_path.display());

    // 检查输入文件是否存在
    if !input_path.exists() {
        println!("错误: 输入文件不存在!");
        return Ok(());
    }

    // 创建源连接器配置
    let source_config = json!({
        "file_path": input_path.to_str().unwrap(),
        "has_header": true,
        "delimiter": ","
    });

    println!("创建CSV源连接器...");
    let mut source = CsvSourceConnector::new(source_config.clone());
    source.configure(&source_config)?;

    println!("检查连接...");
    let connected = source.check_connection().await?;
    println!("连接状态: {}", if connected { "成功" } else { "失败" });

    if !connected {
        return Ok(());
    }

    println!("读取数据...");
    let mut stream = source.read(None).await?;
    let mut records = Vec::new();
    let mut count = 0;

    println!("处理记录...");
    while let Some(record_result) = stream.next().await {
        match record_result {
            Ok(record) => {
                count += 1;
                if count <= 5 {
                    println!("记录 {}: {:?}", count, record.data);
                }
                records.push(record);
                if count % 1000 == 0 {
                    println!("已处理 {} 条记录", count);
                }
            },
            Err(e) => {
                println!("错误: 读取记录失败: {}", e);
            }
        }
    }

    println!("读取完成，总记录数: {}", count);

    // 创建目标连接器配置
    let dest_config = json!({
        "file_path": output_path.to_str().unwrap(),
        "delimiter": ",",
        "write_header": true
    });

    println!("创建CSV目标连接器...");
    let mut dest = CsvDestinationConnector::new(dest_config.clone());
    dest.configure(&dest_config)?;

    // 将数据分为小批次
    let batch_size = 1000;
    let total_batches = (records.len() + batch_size - 1) / batch_size;
    
    println!("写入数据 ({} 批次)...", total_batches);
    
    for (i, chunk) in records.chunks(batch_size).enumerate() {
        let batch = DataRecordBatch::new(chunk.to_vec());
        let stats = dest.write_batch(&batch, WriteMode::Overwrite).await?;
        println!("批次 {}/{}: 写入 {} 条记录 ({} 字节), 耗时 {}ms", 
                 i+1, total_batches, stats.records_written, stats.bytes_written, stats.write_time_ms);
    }

    // 提交更改
    println!("提交更改...");
    dest.commit().await?;

    // 检查输出文件
    if output_path.exists() {
        let metadata = std::fs::metadata(&output_path)?;
        println!("输出文件已创建: {} (大小: {} 字节)", output_path.display(), metadata.len());
    } else {
        println!("错误: 输出文件未创建!");
    }

    println!("===== 测试完成 =====");
    Ok(())
} 