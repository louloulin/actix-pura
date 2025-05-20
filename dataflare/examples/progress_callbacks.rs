//! 进度回调示例
//!
//! 演示如何使用DataFlare的进度回调机制，包括函数回调、事件流和WebHook。

use std::path::Path;
use std::sync::{Arc, Mutex};
use dataflare_runtime::{
    workflow::YamlWorkflowParser, 
    executor::WorkflowExecutor,
    RuntimeMode,
    progress::WebhookConfig,
};
use futures::StreamExt;
use tokio::time::Duration;

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();
    
    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("用法: {} <工作流文件路径>", args[0]);
        return Ok(());
    }
    
    // 工作流文件路径
    let workflow_path = Path::new(&args[1]);
    
    println!("加载工作流: {:?}", workflow_path);
    
    // 使用YAML解析器加载工作流
    let workflow = YamlWorkflowParser::load_from_file(workflow_path)?;
    
    println!("工作流已加载: {}", workflow.id);
    println!("名称: {}", workflow.name);
    println!("描述: {}", workflow.description.as_deref().unwrap_or("无"));
    println!("版本: {}", workflow.version);
    println!("源数量: {}", workflow.sources.len());
    println!("转换数量: {}", workflow.transformations.len());
    println!("目标数量: {}", workflow.destinations.len());
    
    // 创建进度计数器
    let progress_counter = Arc::new(Mutex::new(0));
    let progress_counter_clone = progress_counter.clone();
    
    // 创建工作流执行器
    let mut executor = WorkflowExecutor::new()
        .with_runtime_mode(RuntimeMode::Standalone);
    
    // 1. 添加函数回调
    println!("添加函数回调...");
    executor.add_progress_callback(move |progress| {
        println!(
            "函数回调 - 进度: 工作流={}, 阶段={:?}, 进度={:.2}, 消息={}",
            progress.workflow_id, progress.phase, progress.progress, progress.message
        );
        
        // 增加计数器
        let mut counter = progress_counter_clone.lock().unwrap();
        *counter += 1;
    })?;
    
    // 2. 添加事件流
    println!("创建事件流...");
    let mut event_stream = executor.create_progress_stream()?;
    
    // 3. 添加WebHook（如果需要测试，可以使用RequestBin或类似服务获取URL）
    let webhook_url = std::env::var("DATAFLARE_WEBHOOK_URL")
        .unwrap_or_else(|_| "https://example.com/webhook".to_string());
    
    println!("添加WebHook: {}", webhook_url);
    let webhook_config = WebhookConfig::new(webhook_url)
        .with_header("X-Custom-Header", "DataFlare-Progress")
        .with_retry_count(2);
    
    executor.add_progress_webhook(webhook_config)?;
    
    // 创建任务处理事件流
    let event_stream_task = tokio::spawn(async move {
        println!("开始监听事件流...");
        while let Some(progress) = event_stream.next().await {
            println!(
                "事件流 - 进度: 工作流={}, 阶段={:?}, 进度={:.2}, 消息={}",
                progress.workflow_id, progress.phase, progress.progress, progress.message
            );
        }
        println!("事件流结束");
    });
    
    // 初始化执行器
    println!("初始化执行器...");
    executor.initialize()?;
    
    // 准备工作流
    println!("准备工作流...");
    executor.prepare(&workflow)?;
    
    // 执行工作流
    println!("执行工作流...");
    executor.execute(&workflow).await?;
    
    // 验证收到进度更新
    let counter = progress_counter.lock().unwrap();
    println!("收到 {} 个进度函数回调", *counter);
    
    // 等待事件流处理完成
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // 完成执行器
    println!("完成执行器...");
    executor.finalize()?;
    
    // 取消事件流任务
    event_stream_task.abort();
    
    println!("进度回调示例成功完成!");
    
    Ok(())
} 