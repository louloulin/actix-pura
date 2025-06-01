//! DataFlare CLI
//!
//! 提供命令行界面，用于验证和执行 YAML 工作流。

use std::path::PathBuf;
use std::time::Instant;
use clap::{Parser, Subcommand};
use dataflare_runtime::{
    workflow::{YamlWorkflowParser, WorkflowParser},
    WorkflowExecutor,
};
use dataflare_core::message::WorkflowProgress;
use dataflare_connector::registry;
use dataflare_processor::registry as processor_registry;
use dataflare;

#[derive(Parser)]
#[clap(name = "dataflare")]
#[clap(version = env!("CARGO_PKG_VERSION"))]
#[clap(about = "DataFlare 数据集成框架命令行工具", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 验证 YAML 工作流
    Validate {
        /// YAML 工作流文件路径
        #[clap(short, long)]
        file: PathBuf,

        /// 是否生成 DOT 图
        #[clap(short, long)]
        dot: bool,
    },

    /// 执行 YAML 工作流
    Execute {
        /// YAML 工作流文件路径
        #[clap(short, long)]
        file: PathBuf,

        /// 是否使用增量模式
        #[clap(short, long)]
        incremental: bool,

        /// 增量状态文件路径（仅在增量模式下使用）
        #[clap(short, long)]
        state: Option<PathBuf>,
    },

    /// 显示版本信息
    Version,

    /// 列出支持的连接器
    ListConnectors,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 DataFlare
    dataflare::init(dataflare::DataFlareConfig::default())?;

    // 解析命令行参数
    let cli = Cli::parse();

    // 使用 actix 系统
    let system = actix::System::new();

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
        Commands::Execute { file, incremental, state } => {
            println!("执行工作流: {:?}", file);
            if incremental {
                println!("模式: 增量");
                if let Some(state_path) = &state {
                    println!("状态文件: {:?}", state_path);
                }
            } else {
                println!("模式: 全量");
            }

            // 加载工作流
            let workflow = YamlWorkflowParser::load_from_file(&file)?;

            println!("工作流已加载: {}", workflow.id);

            // 在 actix 运行时中执行工作流
            let system = actix::System::new();
            system.block_on(async {
                // 创建工作流执行器
                let mut executor = WorkflowExecutor::new()
                    .with_progress_callback(|progress: WorkflowProgress| {
                        println!(
                            "进度: 工作流={}, 阶段={:?}, 进度={:.2}, 消息={}",
                            progress.workflow_id, progress.phase, progress.progress, progress.message
                        );
                    });

                // 加载状态（如果是增量模式）
                if incremental {
                    if let Some(ref state_path) = state {
                        // 尝试加载状态文件
                        if state_path.exists() {
                            println!("加载状态文件: {:?}", state_path);
                            // 这里应该实现状态文件的加载逻辑
                            // 例如：executor.load_state(&state_path)?;
                        } else {
                            println!("状态文件不存在，将创建新的状态文件");
                        }
                    }
                }

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

                // 保存状态（如果是增量模式）
                if incremental {
                    if let Some(ref state_path) = state {
                        println!("保存状态到: {:?}", state_path);
                        // 这里应该实现状态保存逻辑
                        // 例如：executor.save_state(&state_path)?;
                    }
                }

                // 完成执行器
                println!("完成执行器...");
                executor.finalize()?;

                Ok::<_, Box<dyn std::error::Error>>(())
            })?;
        },
        Commands::Version => {
            println!("DataFlare 版本: {}", env!("CARGO_PKG_VERSION"));
            println!("构建日期: {}", env!("CARGO_PKG_VERSION"));
            println!("Rust 版本: {}", rustc_version_runtime::version());
        },
        Commands::ListConnectors => {
            println!("支持的连接器:");

            // 获取所有注册的源连接器
            println!("\n源连接器:");
            let source_connectors = registry::get_registered_source_connectors();
            for connector in source_connectors {
                println!("  - {}", connector);
            }

            // 获取所有注册的目标连接器
            println!("\n目标连接器:");
            let destination_connectors = registry::get_registered_destination_connectors();
            for connector in destination_connectors {
                println!("  - {}", connector);
            }

            // 获取所有注册的处理器
            println!("\n处理器:");
            let processors = processor_registry::get_processor_names();
            for processor in processors {
                println!("  - {}", processor);
            }
        },
    }

    Ok(())
}
