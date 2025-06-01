//! YAML 工作流示例
//!
//! 演示如何使用 DataFlare 的 YAML 工作流解析器加载和执行工作流。

use std::path::Path;
use dataflare::{
    workflow::{YamlWorkflowParser, WorkflowExecutor},
    message::WorkflowProgress,
};
use std::sync::{Arc, Mutex};

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;
    
    // YAML 工作流文件路径
    let workflow_path = Path::new("examples/workflows/simple_workflow.yaml");
    
    println!("加载 YAML 工作流: {:?}", workflow_path);
    
    // 使用 YAML 解析器加载工作流
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
        .with_progress_callback(move |progress: WorkflowProgress| {
            println!(
                "进度: 工作流={}, 阶段={:?}, 进度={:.2}, 消息={}",
                progress.workflow_id, progress.phase, progress.progress, progress.message
            );
            
            // 增加计数器
            let mut counter = progress_counter_clone.lock().unwrap();
            *counter += 1;
        });
    
    println!("初始化执行器...");
    executor.initialize()?;
    
    println!("准备工作流...");
    executor.prepare(&workflow)?;
    
    println!("执行工作流...");
    executor.execute(&workflow).await?;
    
    // 验证收到进度更新
    let counter = progress_counter.lock().unwrap();
    println!("收到 {} 个进度更新", *counter);
    
    println!("完成执行器...");
    executor.finalize()?;
    
    println!("YAML 工作流成功完成!");
    
    Ok(())
}
