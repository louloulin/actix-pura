//! Direct CSV Test
//! CSV文件直接读写测试，不依赖Actor系统
//! 用于验证性能测试工作流中的路径问题

use std::path::PathBuf;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use serde_json::json;
use dataflare::connector::csv::{CsvSourceConnector, CsvDestinationConnector};
use dataflare::connector::source::SourceConnector;
use dataflare::connector::destination::{DestinationConnector, WriteMode};
use dataflare::message::{DataRecord, DataRecordBatch};
use dataflare::error::Result;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;
    
    // 配置文件路径
    let input_path = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/data/large_test.csv");
    let output_path = PathBuf::from("/Users/louloulin/mastra_docs/actix/examples/data/performance_output.csv");
    
    println!("开始直接CSV读写测试");
    println!("输入文件: {}", input_path.display());
    println!("输出文件: {}", output_path.display());
    
    // 验证输入文件
    if !input_path.exists() {
        return Err(dataflare::error::DataFlareError::Config(
            format!("输入文件不存在: {}", input_path.display())
        ));
    }
    
    // 预先清理输出文件
    if output_path.exists() {
        println!("删除已存在的输出文件");
        std::fs::remove_file(&output_path)?;
    }
    
    // 确保输出目录存在
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            println!("创建输出目录: {}", parent.display());
            std::fs::create_dir_all(parent)?;
        }
    }
    
    // 测试输出目录写入权限
    let test_file_path = output_path.with_file_name("test_write.tmp");
    match File::create(&test_file_path).await {
        Ok(mut file) => {
            file.write_all(b"test").await?;
            println!("输出目录写入测试成功");
            tokio::fs::remove_file(&test_file_path).await?;
        },
        Err(e) => {
            return Err(dataflare::error::DataFlareError::Io(e));
        }
    }
    
    // 创建CSV源连接器配置
    let source_config = json!({
        "file_path": input_path.to_str().unwrap(),
        "has_header": true,
        "delimiter": ","
    });
    
    // 创建CSV目标连接器配置
    let dest_config = json!({
        "file_path": output_path.to_str().unwrap(),
        "write_header": true,
        "delimiter": ","
    });
    
    // 创建连接器实例
    let mut source = CsvSourceConnector::new(source_config.clone());
    let mut dest = CsvDestinationConnector::new(dest_config.clone());
    
    // 配置连接器
    source.configure(&source_config)?;
    dest.configure(&dest_config)?;
    
    // 验证连接
    let source_ok = source.check_connection().await?;
    println!("源连接器检查: {}", if source_ok { "成功" } else { "失败" });
    
    // 开始数据处理
    println!("\n开始数据处理...");
    let start = Instant::now();
    
    // 读取源数据
    let mut stream = source.read(None).await?;
    
    // 筛选和转换规则（模拟性能测试工作流）
    let mut filtered_records = Vec::new();
    let mut total_records = 0;
    let mut filtered_count = 0;
    
    // 处理记录
    while let Some(record_result) = stream.next().await {
        match record_result {
            Ok(record) => {
                total_records += 1;
                
                // 调试: 打印记录内容
                println!("处理记录: {:?}", record);
                
                // 筛选条件：年龄 > 20
                if let Some(age_value) = record.get_value("age") {
                    println!("  找到age字段: {:?}", age_value);
                    
                    // 尝试从字符串解析年龄
                    let age = if let Some(age_str) = age_value.as_str() {
                        match age_str.parse::<i64>() {
                            Ok(age_num) => {
                                println!("  解析字符串为整数: {}", age_num);
                                Some(age_num)
                            },
                            Err(_) => {
                                println!("  无法解析字符串为整数: {}", age_str);
                                None
                            }
                        }
                    } else if let Some(age_num) = age_value.as_i64() {
                        println!("  获取为整数: {}", age_num);
                        Some(age_num)
                    } else {
                        println!("  无法获取或解析为整数");
                        None
                    };
                    
                    // 如果解析成功，应用筛选条件
                    if let Some(age_num) = age {
                        if age_num > 20 {
                            println!("  通过筛选条件 (age > 20)");
                            // 创建新记录并进行转换
                            let mut transformed = DataRecord::new(json!({}));
                            
                            // 字段映射
                            if let Some(id) = record.get_value("id") {
                                transformed.set_value("user_id", id.clone())?;
                            }
                            
                            if let Some(name) = record.get_value("name") {
                                transformed.set_value("full_name", name.clone())?;
                            }
                            
                            if let Some(email) = record.get_value("email") {
                                if let Some(email_str) = email.as_str() {
                                    transformed.set_value("email_address", 
                                        json!(email_str.to_lowercase()))?;
                                }
                            }
                            
                            transformed.set_value("user_age", age_value.clone())?;
                            
                            filtered_records.push(transformed);
                            filtered_count += 1;
                        } else {
                            println!("  未通过筛选条件 (age <= 20)");
                        }
                    }
                } else {
                    println!("  未找到age字段");
                }
                
                // 每1000条记录输出一次进度
                if total_records % 1000 == 0 {
                    println!("已处理: {}条记录，已筛选: {}条记录", total_records, filtered_count);
                }
                
                // 每5000条记录，写入一个批次
                if filtered_records.len() >= 5000 {
                    let batch = DataRecordBatch::new(filtered_records);
                    dest.write_batch(&batch, WriteMode::Append).await?;
                    filtered_records = Vec::new();
                }
            },
            Err(e) => {
                println!("处理记录出错: {}", e);
            }
        }
    }
    
    // 写入剩余记录
    if !filtered_records.is_empty() {
        let batch = DataRecordBatch::new(filtered_records);
        dest.write_batch(&batch, WriteMode::Append).await?;
    }
    
    // 提交更改
    dest.commit().await?;
    
    // 计算性能
    let duration = start.elapsed();
    println!("\n数据处理完成");
    println!("总记录数: {}", total_records);
    println!("筛选后记录数: {}", filtered_count);
    println!("处理时间: {:.3}秒", duration.as_secs_f64());
    println!("每秒处理记录数: {:.0}", total_records as f64 / duration.as_secs_f64());
    
    // 验证输出文件
    if output_path.exists() {
        let metadata = std::fs::metadata(&output_path)?;
        println!("\n输出文件已创建");
        println!("文件大小: {}字节", metadata.len());
        
        // 读取前几行作为示例
        if metadata.len() > 0 {
            println!("\n输出文件前5行:");
            let file = std::fs::File::open(&output_path)?;
            let reader = std::io::BufReader::new(file);
            use std::io::BufRead;
            for (i, line) in reader.lines().take(5).enumerate() {
                if let Ok(line) = line {
                    println!("  {}: {}", i + 1, line);
                }
            }
        } else {
            println!("警告: 输出文件为空");
        }
    } else {
        println!("错误: 输出文件未创建");
    }
    
    Ok(())
} 