//! 工作流命令行工具
//!
//! 提供命令行界面，用于验证和执行 YAML 工作流。

use std::path::PathBuf;
use std::time::Instant;
use clap::{Parser, Subcommand};
use dataflare::{
    workflow::{YamlWorkflowParser, WorkflowParser, WorkflowExecutor},
    message::WorkflowProgress,
};

#[derive(Parser)]
#[command(name = "dataflare-cli")]
#[command(about = "DataFlare 工作流命令行工具", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 验证 YAML 工作流
    Validate {
        /// YAML 工作流文件路径
        #[arg(short, long)]
        file: PathBuf,

        /// 是否生成 DOT 图
        #[arg(short, long)]
        dot: bool,
    },

    /// 执行 YAML 工作流
    Execute {
        /// YAML 工作流文件路径
        #[arg(short, long)]
        file: PathBuf,
    },
}

#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;

    // 解析命令行参数
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { file, dot } => {
            println!("验证工作流: {:?}", file);

            // 加载工作流
            let start = Instant::now();
            let workflow = YamlWorkflowParser::load_from_file(&file)?;
            let load_duration = start.elapsed();

            println!("工作流已加载 ({:?})", load_duration);
            println!("ID: {}", workflow.id);
            println!("名称: {}", workflow.name);
            println!("描述: {}", workflow.description.as_deref().unwrap_or("无"));
            println!("版本: {}", workflow.version);
            println!("源数量: {}", workflow.sources.len());
            println!("转换数量: {}", workflow.transformations.len());
            println!("目标数量: {}", workflow.destinations.len());

            // 解析工作流
            let mut parser = WorkflowParser::new(workflow);
            parser.parse()?;

            // 检查工作流是否有循环
            if parser.has_cycles() {
                println!("警告: 工作流包含循环!");
                if let Some(cycle_path) = parser.get_cycle_path() {
                    println!("循环路径: {:?}", cycle_path);
                }
            } else {
                println!("工作流没有循环");

                // 获取拓扑排序
                let order = parser.get_topological_order()?;
                println!("拓扑排序: {:?}", order);
            }

            // 检查是否所有组件都可达
            if parser.all_components_reachable() {
                println!("所有组件都可达");
            } else {
                println!("警告: 存在不可达组件!");
                let unused = parser.find_unused_components();
                println!("未使用的组件: {:?}", unused);
            }

            // 检查孤立组件
            let isolated = parser.find_isolated_components();
            if !isolated.is_empty() {
                println!("警告: 存在孤立组件（没有输入和输出）!");
                println!("孤立组件: {:?}", isolated);
            }

            // 检查悬空组件
            let dangling = parser.find_dangling_components();
            if !dangling.is_empty() {
                println!("警告: 存在悬空组件（有输入但没有输出）!");
                println!("悬空组件: {:?}", dangling);
            }

            // 检查未连接的源
            let disconnected_sources = parser.find_disconnected_sources();
            if !disconnected_sources.is_empty() {
                println!("警告: 存在未连接的源组件（没有输出）!");
                println!("未连接的源: {:?}", disconnected_sources);
            }

            // 生成 DOT 图
            if dot {
                let dot = parser.to_dot();
                println!("\nDOT 图:");
                println!("{}", dot);
            }

            println!("工作流验证成功!");
        },
        Commands::Execute { file } => {
            println!("执行工作流: {:?}", file);

            // 加载工作流
            let workflow = YamlWorkflowParser::load_from_file(&file)?;

            println!("工作流已加载: {}", workflow.id);

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

            // 完成执行器
            println!("完成执行器...");
            executor.finalize()?;
        },
    }

    Ok(())
}
