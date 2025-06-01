//! CSV 目标示例
//!
//! 这个示例演示了如何使用 DataFlare 将数据写入 CSV 文件。

use std::path::PathBuf;
use std::time::Instant;

use dataflare::{
    workflow::{YamlWorkflowParser, WorkflowParser, WorkflowExecutor},
    message::WorkflowProgress,
};

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;

    // 工作流文件路径
    let file = PathBuf::from("dataflare/examples/workflows/csv_destination_workflow.yaml");

    println!("执行 CSV 目标工作流: {:?}", file);

    // 加载工作流
    let workflow = YamlWorkflowParser::load_from_file(&file)?;

    println!("工作流已加载: {}", workflow.id);
    println!("名称: {}", workflow.name);
    println!("描述: {}", workflow.description.as_deref().unwrap_or("无"));
    println!("版本: {}", workflow.version);
    println!("源数量: {}", workflow.sources.len());
    println!("转换数量: {}", workflow.transformations.len());
    println!("目标数量: {}", workflow.destinations.len());

    // 创建工作流执行器
    let mut executor = WorkflowExecutor::new()
        .with_progress_callback(|progress: WorkflowProgress| {
            println!(
                "进度: 工作流={}, 阶段={:?}, 进度={:.2}, 消息={}",
                progress.workflow_id, progress.phase, progress.progress, progress.message
            );
        });

    // 初始化执行器
    println!("初始化执行器...");
    executor.initialize()?;

    // 准备工作流
    println!("准备工作流...");
    executor.prepare(&workflow)?;

    // 执行工作流
    println!("执行工作流...");
    let start = Instant::now();
    executor.execute(&workflow).await?;
    let duration = start.elapsed();

    println!("工作流执行完成! 耗时: {:?}", duration);
    println!("CSV 文件已写入: {}", workflow.destinations.get("csv_output").unwrap().config.get("file_path").unwrap().as_str().unwrap());

    // 完成执行器
    println!("完成执行器...");
    executor.finalize()?;

    Ok(())
}
