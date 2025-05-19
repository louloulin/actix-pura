//! DataFlare CLI
//!
//! Command-line interface for the DataFlare data integration framework.

use std::path::PathBuf;
use std::time::Instant;
use clap::{Parser, Subcommand};
use dataflare_runtime::workflow::{YamlWorkflowParser, WorkflowParser, WorkflowExecutor};
use dataflare_core::message::WorkflowProgress;
use dataflare_core::DataFlareConfig;
use dataflare_processor as processor;
use dataflare_connector::registry;

#[derive(Parser)]
#[command(name = "dataflare")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "DataFlare 数据集成框架命令行工具", long_about = None)]
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

        /// 是否使用增量模式
        #[arg(short, long)]
        incremental: bool,

        /// 增量状态文件路径（仅在增量模式下使用）
        #[arg(short, long)]
        state: Option<PathBuf>,
    },

    /// 显示版本信息
    Version,

    /// 列出支持的连接器
    ListConnectors,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 DataFlare
    dataflare_core::init(DataFlareConfig::default())?;

    // 解析命令行参数
    let cli = Cli::parse();

    // 使用 actix 系统
    let system = actix::System::new();

    // 处理命令
    match &cli.command {
        Commands::Validate { file, dot } => {
            println!("验证工作流: {:?}", file);

            // 使用 YAML 解析器加载工作流
            let workflow = YamlWorkflowParser::load_from_file(file)?;

            println!("工作流验证成功: {}", workflow.id);
            println!("名称: {}", workflow.name);
            println!("描述: {}", workflow.description.as_deref().unwrap_or("无"));
            println!("版本: {}", workflow.version);
            println!("源数量: {}", workflow.sources.len());
            println!("转换数量: {}", workflow.transformations.len());
            println!("目标数量: {}", workflow.destinations.len());

            if *dot {
                // 生成 DOT 图
                let dot_graph = workflow.to_dot_graph()?;
                let dot_file = file.with_extension("dot");
                std::fs::write(&dot_file, dot_graph)?;
                println!("DOT 图已生成: {:?}", dot_file);
            }
        },
        Commands::Execute { file, incremental, state } => {
            println!("执行工作流: {:?}", file);

            // 使用 YAML 解析器加载工作流
            let workflow = YamlWorkflowParser::load_from_file(file)?;

            // 创建工作流执行器
            let mut executor = WorkflowExecutor::new();

            // 初始化执行器
            executor.initialize()?;

            // 设置进度回调
            executor.set_progress_callback(Box::new(|progress| {
                match progress {
                    WorkflowProgress::Started { workflow_id } => {
                        println!("工作流 {} 已开始", workflow_id);
                    },
                    WorkflowProgress::SourceStarted { source_id, .. } => {
                        println!("源 {} 已开始", source_id);
                    },
                    WorkflowProgress::SourceCompleted { source_id, records_processed, .. } => {
                        println!("源 {} 已完成，处理了 {} 条记录", source_id, records_processed);
                    },
                    WorkflowProgress::TransformationStarted { transformation_id, .. } => {
                        println!("转换 {} 已开始", transformation_id);
                    },
                    WorkflowProgress::TransformationCompleted { transformation_id, records_processed, .. } => {
                        println!("转换 {} 已完成，处理了 {} 条记录", transformation_id, records_processed);
                    },
                    WorkflowProgress::DestinationStarted { destination_id, .. } => {
                        println!("目标 {} 已开始", destination_id);
                    },
                    WorkflowProgress::DestinationCompleted { destination_id, records_processed, .. } => {
                        println!("目标 {} 已完成，处理了 {} 条记录", destination_id, records_processed);
                    },
                    WorkflowProgress::Completed { workflow_id, duration_ms, .. } => {
                        println!("工作流 {} 已完成，耗时 {} 毫秒", workflow_id, duration_ms);
                    },
                    WorkflowProgress::Failed { workflow_id, error, .. } => {
                        println!("工作流 {} 失败: {}", workflow_id, error);
                    },
                }
            }));

            // 准备工作流
            executor.prepare(&workflow)?;

            // 执行工作流
            let start = Instant::now();
            system.block_on(async {
                executor.execute(&workflow).await
            })?;
            let duration = start.elapsed();

            println!("工作流执行完成，耗时 {:.2} 秒", duration.as_secs_f64());
        },
        Commands::Version => {
            println!("DataFlare 版本: {}", env!("CARGO_PKG_VERSION"));
            println!("Actix 版本: {}", actix::VERSION);
        },
        Commands::ListConnectors => {
            println!("支持的连接器:");
            println!("源连接器:");
            for connector in registry::list_source_connectors() {
                println!("  - {}", connector);
            }
            println!("目标连接器:");
            for connector in registry::list_destination_connectors() {
                println!("  - {}", connector);
            }
        },
    }

    Ok(())
}
