//! DataFlare CLI
//!
//! Command-line interface for the DataFlare data integration framework.

use std::path::PathBuf;
use std::time::Instant;
use clap::{Parser, Subcommand};
use dataflare_runtime::workflow::YamlWorkflowParser;
use dataflare_runtime::WorkflowExecutor;
use log::{info, debug, error, warn, LevelFilter};

#[derive(Parser)]
#[command(name = "dataflare")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "DataFlare 数据集成框架命令行工具", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// 日志级别 (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand, Debug, Clone)]
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

    /// WASM插件管理
    Plugin {
        #[command(subcommand)]
        command: dataflare_enterprise_cli::WasmCommands,

        /// Enable verbose logging
        #[arg(short, long, global = true)]
        verbose: bool,

        /// Disable colored output
        #[arg(long, global = true)]
        no_color: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 解析命令行参数
    let cli = Cli::parse();

    // 设置环境变量，让env_logger使用对应的日志级别
    std::env::set_var("RUST_LOG", cli.log_level.clone());

    // 初始化 DataFlare (包含了日志初始化)
    dataflare_cli::init()?;

    info!("DataFlare CLI 启动");
    debug!("日志级别: {}", cli.log_level);

    // 处理命令
    match &cli.command {
        Commands::Validate { file, dot } => {
            info!("验证工作流: {:?}", file);

            // 使用 YAML 解析器加载工作流
            let workflow = YamlWorkflowParser::load_from_file(file)?;
            info!("工作流验证成功: {}", workflow.id);
            info!("名称: {}", workflow.name);
            info!("描述: {}", workflow.description.as_deref().unwrap_or("无"));
            info!("版本: {}", workflow.version);
            debug!("源数量: {}", workflow.sources.len());
            debug!("转换数量: {}", workflow.transformations.len());
            debug!("目标数量: {}", workflow.destinations.len());

            // 打印目标配置的详细信息
            for (name, dest) in &workflow.destinations {
                debug!("目标 '{}' 配置:", name);
                if let Some(file_path) = dest.config.get("file_path") {
                    debug!("  文件路径: {:?}", file_path);
                }
            }

            if *dot {
                // 生成 DOT 图
                info!("DOT 图功能暂未实现");
            }
        },
        Commands::Execute { file, incremental: _, state: _ } => {
            info!("执行工作流: {:?}", file);
            // 验证文件路径
            if !file.exists() {
                error!("工作流文件不存在: {:?}", file);
                return Err(format!("工作流文件不存在: {:?}", file).into());
            }
            info!("工作流文件路径: {}", file.canonicalize()?.display());

            // 使用 YAML 解析器加载工作流
            let workflow = YamlWorkflowParser::load_from_file(file)?;
            info!("工作流已加载: {}", workflow.id);

            // 输出CSV源配置信息
            for (name, source) in &workflow.sources {
                info!("源配置 '{}' 信息:", name);
                if let Some(file_path) = source.config.get("file_path") {
                    info!("  文件路径: {:?}", file_path);
                    if let Some(path_str) = file_path.as_str() {
                        let path = std::path::Path::new(path_str);
                        if path.exists() {
                            let metadata = std::fs::metadata(path)?;
                            info!("  文件大小: {} 字节", metadata.len());
                            debug!("  读取文件前10行:");
                            if let Ok(file) = std::fs::File::open(path) {
                                use std::io::{BufRead, BufReader};
                                let reader = BufReader::new(file);
                                for (i, line) in reader.lines().take(10).enumerate() {
                                    if let Ok(line) = line {
                                        debug!("    行 {}: {}", i + 1, line);
                                    }
                                }
                            }
                        } else {
                            warn!("  文件不存在: {}", path.display());
                        }
                    }
                }
            }

            // 输出目标配置信息
            for (name, dest) in &workflow.destinations {
                info!("目标配置 '{}' 信息:", name);
                if let Some(file_path) = dest.config.get("file_path") {
                    info!("  文件路径: {:?}", file_path);
                    if let Some(path_str) = file_path.as_str() {
                        let path = std::path::Path::new(path_str);
                        if path.exists() {
                            info!("  文件已存在，将被覆盖或追加");
                        } else {
                            if let Some(parent) = path.parent() {
                                if parent.exists() {
                                    info!("  父目录存在: {}", parent.display());
                                } else {
                                    warn!("  父目录不存在: {}", parent.display());
                                    debug!("  尝试创建目录: {}", parent.display());
                                    std::fs::create_dir_all(parent)?;
                                    info!("  已创建目录: {}", parent.display());
                                }
                            }
                        }

                        // 尝试创建一个空文件以验证写入权限
                        let test_path = path.with_file_name(format!("test_{}.tmp", workflow.id));
                        match std::fs::File::create(&test_path) {
                            Ok(_) => {
                                info!("  有写入权限");
                                // 删除测试文件
                                let _ = std::fs::remove_file(&test_path);
                            },
                            Err(e) => {
                                warn!("  可能没有写入权限: {}", e);
                            }
                        }
                    }
                }
            }

            // 创建工作流执行器
            info!("创建工作流执行器");
            let mut executor = WorkflowExecutor::new();

            // 创建并使用本地 Tokio 运行时，同时创建 LocalSet 来支持 spawn_local
            info!("创建 Tokio 运行时");
            let runtime = tokio::runtime::Runtime::new()?;
            let local = tokio::task::LocalSet::new();
            local.block_on(&runtime, async {
            // 初始化执行器
                info!("初始化执行器");
            executor.initialize()?;

            // 设置进度回调
            let mut executor = executor.with_progress_callback(Box::new(|progress| {
                    info!("工作流进度更新: {:?}", progress);
            }));

            // 准备工作流
                info!("准备工作流");
            executor.prepare(&workflow)?;

            // 执行工作流
                info!("开始执行工作流");
            let start = Instant::now();
                match executor.execute(&workflow).await {
                    Ok(_) => {
                        info!("工作流执行成功");
                    },
                    Err(e) => {
                        error!("工作流执行失败: {}", e);
                        return Err(Box::new(e) as Box<dyn std::error::Error>);
                    }
                }
            let duration = start.elapsed();

                info!("工作流执行完成，耗时 {:.2} 秒", duration.as_secs_f64());

                // 检查目标文件是否生成
                for (name, dest) in &workflow.destinations {
                    if let Some(file_path) = dest.config.get("file_path").and_then(|v| v.as_str()) {
                        let path = std::path::Path::new(file_path);
                        if path.exists() {
                            let metadata = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                            info!("目标文件已创建: {} (大小: {} 字节)", file_path, metadata);

                            // 读取文件前几行作为示例
                            if metadata > 0 {
                                debug!("文件内容预览:");
                                if let Ok(file) = std::fs::File::open(path) {
                                    use std::io::{BufRead, BufReader};
                                    let reader = BufReader::new(file);
                                    for (i, line) in reader.lines().take(5).enumerate() {
                                        if let Ok(line) = line {
                                            debug!("  行 {}: {}", i + 1, line);
                                        }
                                    }
                                }
                            } else {
                                warn!("目标文件存在但为空");
                            }
                        } else {
                            warn!("目标文件未创建: {}", file_path);

                            // 检查文件父目录是否存在
                            if let Some(parent) = path.parent() {
                                if !parent.exists() {
                                    warn!("目标文件的父目录不存在: {}", parent.display());
                                } else {
                                    // 检查目录下的其他文件
                                    debug!("检查目录 {}:", parent.display());
                                    if let Ok(entries) = std::fs::read_dir(parent) {
                                        for entry in entries {
                                            if let Ok(entry) = entry {
                                                debug!("  {}", entry.path().display());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Ok::<(), Box<dyn std::error::Error>>(())
            })?;
        },
        Commands::Version => {
            info!("DataFlare 版本: {}", env!("CARGO_PKG_VERSION"));
            info!("使用 Tokio 运行时");
        },
        Commands::ListConnectors => {
            info!("支持的连接器:");
            info!("源连接器:");
            for connector in dataflare_cli::list_source_connectors() {
                info!("  - {}", connector);
            }
            info!("目标连接器:");
            for connector in dataflare_cli::list_destination_connectors() {
                info!("  - {}", connector);
            }
        },
        Commands::Plugin { command, verbose, no_color } => {
            // 创建WASM CLI实例
            let wasm_cli = dataflare_enterprise_cli::WasmCli {
                command: command.clone(),
                verbose: *verbose,
                no_color: *no_color,
            };

            // 执行WASM命令
            let rt = tokio::runtime::Runtime::new()?;
            if let Err(e) = rt.block_on(dataflare_enterprise_cli::execute_wasm_command(wasm_cli)) {
                error!("WASM插件命令执行失败: {}", e);
                return Err(e.into());
            }
        },
    }

    info!("DataFlare CLI 执行完成");
    Ok(())
}
