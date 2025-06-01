//! Performance Test Script
//! 
//! 使用直接的CSV连接器测试而不依赖Actor系统

use std::path::PathBuf;
use std::time::Instant;
use log::{info, error, debug, LevelFilter};
use serde_json::json;

use dataflare::{
    DataFlareConfig,
    connector::csv::{CsvSourceConnector, CsvDestinationConnector},
    connector::source::SourceConnector,
    connector::destination::{DestinationConnector, WriteMode},
    message::{DataRecord, DataRecordBatch},
    error::Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化 DataFlare
    let mut config = DataFlareConfig::default();
    config.log_level = LevelFilter::Debug;
    dataflare::init(config)?;

    info!("开始CSV性能测试");

    // 配置文件路径 - 使用绝对路径确保访问
    let input_path = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/data/large_test.csv");
    let output_path = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/data/performance_output.csv");
    
    info!("输入文件: {:?}", input_path);
    info!("输出文件: {:?}", output_path);

    // 创建源连接器
    let source_config = json!({
        "file_path": input_path.to_string_lossy(),
        "has_header": true,
        "delimiter": ","
    });
    
    let mut source_connector = CsvSourceConnector::new(source_config.clone());
    
    // 配置源连接器
    info!("配置CSV源连接器");
    source_connector.configure(&source_config)?;
    
    // 创建目标连接器
    let dest_config = json!({
        "file_path": output_path.to_string_lossy(),
        "write_header": true,
        "delimiter": ","
    });
    
    let mut dest_connector = CsvDestinationConnector::new(dest_config.clone());
    
    // 配置目标连接器
    info!("配置CSV目标连接器");
    dest_connector.configure(&dest_config)?;
    
    // 开始计时
    let start = Instant::now();
    
    // 读取源数据
    info!("开始读取源数据");
    let mut stream = source_connector.read(None).await?;
    
    // 处理记录
    let mut total_records = 0;
    let mut filtered_records = Vec::new();
    
    info!("开始处理记录");
    use futures::StreamExt;
    
    while let Some(record_result) = stream.next().await {
        match record_result {
            Ok(record) => {
                total_records += 1;
                
                // 调试前几条记录
                if total_records <= 5 {
                    debug!("处理记录: {:?}", record);
                }
                
                // 筛选条件：年龄 > 20
                if let Some(age_value) = record.get_value("age") {
                    // 尝试从字符串解析年龄
                    let age = if let Some(age_str) = age_value.as_str() {
                        match age_str.parse::<i64>() {
                            Ok(age_num) => Some(age_num),
                            Err(_) => None
                        }
                    } else if let Some(age_num) = age_value.as_i64() {
                        Some(age_num)
                    } else {
                        None
                    };
                    
                    // 如果年龄 > 20，转换并添加到结果
                    if let Some(age) = age {
                        if age > 20 {
                            // 创建新记录并进行转换
                            let mut transformed = DataRecord::new(json!({}));
                            
                            // 映射字段
                            if let Some(id) = record.get_value("id") {
                                transformed.set_value("user_id", id.clone());
                            }
                            
                            if let Some(name) = record.get_value("name") {
                                transformed.set_value("full_name", name.clone());
                            }
                            
                            if let Some(email) = record.get_value("email") {
                                // 转换为小写
                                if let Some(email_str) = email.as_str() {
                                    transformed.set_value("email_address", json!(email_str.to_lowercase()));
                                } else {
                                    transformed.set_value("email_address", email.clone());
                                }
                            }
                            
                            transformed.set_value("user_age", json!(age));
                            
                            // 添加到过滤结果
                            filtered_records.push(transformed);
                            
                            // 每1000条记录报告一次进度
                            if filtered_records.len() % 1000 == 0 {
                                info!("已处理 {} 条记录，筛选后 {} 条", total_records, filtered_records.len());
                            }
                        }
                    }
                }
            },
            Err(e) => {
                error!("读取记录错误: {}", e);
                return Err(e);
            }
        }
    }
    
    // 创建批次
    let batch = DataRecordBatch::new(filtered_records);
    
    // 写入目标
    info!("开始写入 {} 条记录到目标", batch.records.len());
    let write_stats = dest_connector.write_batch(&batch, WriteMode::Overwrite).await?;
    
    // 报告结果
    let elapsed = start.elapsed();
    info!("CSV性能测试完成:");
    info!("总记录数: {}", total_records);
    info!("筛选后记录数: {}", batch.records.len());
    info!("写入记录数: {}", write_stats.records_written);
    info!("写入字节数: {}", write_stats.bytes_written);
    info!("写入时间: {} ms", write_stats.write_time_ms);
    info!("总耗时: {:.2?}", elapsed);
    
    if total_records > 0 {
        let records_per_second = (total_records as f64) / (elapsed.as_secs_f64());
        info!("处理速度: {:.2} 记录/秒", records_per_second);
    }
    
    Ok(())
} 