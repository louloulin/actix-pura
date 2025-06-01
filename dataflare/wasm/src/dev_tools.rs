//! WASM插件开发工具
//!
//! 提供插件开发、调试和测试的工具集

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use log::{info, debug, warn, error};
use crate::{WasmResult, WasmError};

/// 插件开发工具集
#[derive(Debug)]
pub struct PluginDevTools {
    /// 项目根目录
    project_root: PathBuf,
    /// 构建配置
    build_config: BuildConfig,
    /// 测试配置
    test_config: TestConfig,
    /// 调试器
    debugger: PluginDebugger,
}

/// 构建配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// 目标架构
    pub target: String,
    /// 优化级别
    pub optimization: OptimizationLevel,
    /// 是否启用调试信息
    pub debug_info: bool,
    /// 输出目录
    pub output_dir: PathBuf,
    /// 源文件目录
    pub source_dir: PathBuf,
    /// 依赖项
    pub dependencies: Vec<String>,
}

/// 优化级别
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationLevel {
    /// 无优化
    None,
    /// 基本优化
    Basic,
    /// 完全优化
    Full,
    /// 大小优化
    Size,
}

/// 测试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    /// 测试数据目录
    pub test_data_dir: PathBuf,
    /// 测试超时时间
    pub timeout: Duration,
    /// 并发测试数量
    pub parallel_tests: usize,
    /// 是否生成覆盖率报告
    pub coverage: bool,
}

/// 插件调试器
#[derive(Debug)]
pub struct PluginDebugger {
    /// 断点
    breakpoints: HashMap<String, Vec<u32>>,
    /// 调试会话
    debug_session: Option<DebugSession>,
    /// 日志级别
    log_level: LogLevel,
}

/// 调试会话
#[derive(Debug)]
pub struct DebugSession {
    /// 会话ID
    pub session_id: String,
    /// 开始时间
    pub start_time: Instant,
    /// 当前状态
    pub state: DebugState,
    /// 调用栈
    pub call_stack: Vec<StackFrame>,
}

/// 调试状态
#[derive(Debug, Clone)]
pub enum DebugState {
    /// 运行中
    Running,
    /// 暂停
    Paused,
    /// 停止
    Stopped,
    /// 错误
    Error(String),
}

/// 栈帧
#[derive(Debug, Clone)]
pub struct StackFrame {
    /// 函数名
    pub function_name: String,
    /// 文件名
    pub file_name: String,
    /// 行号
    pub line_number: u32,
    /// 局部变量
    pub local_variables: HashMap<String, String>,
}

/// 日志级别
#[derive(Debug, Clone)]
pub enum LogLevel {
    /// 错误
    Error,
    /// 警告
    Warn,
    /// 信息
    Info,
    /// 调试
    Debug,
    /// 跟踪
    Trace,
}

/// 插件模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTemplate {
    /// 模板名称
    pub name: String,
    /// 模板描述
    pub description: String,
    /// 模板类型
    pub template_type: TemplateType,
    /// 文件列表
    pub files: Vec<TemplateFile>,
    /// 依赖项
    pub dependencies: Vec<String>,
}

/// 模板类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateType {
    /// 基础插件
    Basic,
    /// 数据处理器
    Processor,
    /// 数据源
    Source,
    /// 数据目标
    Destination,
    /// 转换器
    Transformer,
    /// 过滤器
    Filter,
    /// 聚合器
    Aggregator,
}

/// 模板文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateFile {
    /// 文件路径
    pub path: String,
    /// 文件内容
    pub content: String,
    /// 是否为模板
    pub is_template: bool,
}

/// 性能基准测试
#[derive(Debug)]
pub struct BenchmarkSuite {
    /// 基准测试列表
    benchmarks: Vec<Benchmark>,
    /// 结果
    results: Vec<BenchmarkResult>,
}

/// 基准测试
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Benchmark {
    /// 测试名称
    pub name: String,
    /// 测试描述
    pub description: String,
    /// 测试数据大小
    pub data_size: usize,
    /// 迭代次数
    pub iterations: usize,
}

/// 基准测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// 测试名称
    pub name: String,
    /// 平均执行时间
    pub avg_time: Duration,
    /// 最小执行时间
    pub min_time: Duration,
    /// 最大执行时间
    pub max_time: Duration,
    /// 吞吐量 (操作/秒)
    pub throughput: f64,
    /// 内存使用量
    pub memory_usage: u64,
    /// 标准差
    pub std_deviation: f64,
    /// 百分位数统计
    pub percentiles: PercentileStats,
    /// CPU使用率
    pub cpu_usage: f64,
    /// 错误率
    pub error_rate: f64,
    /// 详细统计信息
    pub detailed_stats: DetailedStats,
}

/// 百分位数统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PercentileStats {
    /// 50th百分位数 (中位数)
    pub p50: Duration,
    /// 90th百分位数
    pub p90: Duration,
    /// 95th百分位数
    pub p95: Duration,
    /// 99th百分位数
    pub p99: Duration,
    /// 99.9th百分位数
    pub p999: Duration,
}

/// 详细统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedStats {
    /// 总迭代次数
    pub total_iterations: usize,
    /// 成功次数
    pub successful_iterations: usize,
    /// 失败次数
    pub failed_iterations: usize,
    /// 平均内存分配
    pub avg_memory_allocations: u64,
    /// 峰值内存使用
    pub peak_memory_usage: u64,
    /// GC次数
    pub gc_count: u32,
    /// 缓存命中率
    pub cache_hit_rate: f64,
}

/// 高级基准测试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedBenchmarkConfig {
    /// 自定义基准测试
    pub custom_benchmarks: Vec<Benchmark>,
    /// 启用并发测试
    pub enable_concurrency_tests: bool,
    /// 启用内存压力测试
    pub enable_memory_stress_tests: bool,
    /// 并发线程数
    pub concurrency_threads: usize,
    /// 内存压力测试大小
    pub memory_stress_size: usize,
    /// 测试超时时间
    pub test_timeout: Duration,
}

/// 基准测试报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    /// 插件名称
    pub plugin_name: String,
    /// 测试时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 总测试时间
    pub total_duration: Duration,
    /// 所有测试结果
    pub results: Vec<BenchmarkResult>,
    /// 测试摘要
    pub summary: BenchmarkSummary,
    /// 系统信息
    pub system_info: SystemInfo,
}

/// 基准测试摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    /// 总测试数
    pub total_tests: usize,
    /// 总错误数
    pub total_errors: usize,
    /// 总体评分 (0-100)
    pub overall_score: f64,
    /// 性能等级
    pub performance_grade: PerformanceGrade,
    /// 优化建议
    pub recommendations: Vec<String>,
}

/// 性能等级
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceGrade {
    /// 优秀 (90-100分)
    Excellent,
    /// 良好 (80-89分)
    Good,
    /// 一般 (70-79分)
    Fair,
    /// 较差 (60-69分)
    Poor,
    /// 失败 (0-59分)
    Failing,
}

/// 系统信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// 操作系统
    pub os: String,
    /// CPU信息
    pub cpu: String,
    /// 内存大小
    pub memory: u64,
    /// Rust版本
    pub rust_version: String,
    /// WASM运行时版本
    pub wasm_runtime_version: String,
}

impl PluginDevTools {
    /// 创建新的开发工具
    pub fn new<P: AsRef<Path>>(project_root: P) -> WasmResult<Self> {
        let project_root = project_root.as_ref().to_path_buf();

        if !project_root.exists() {
            return Err(WasmError::runtime(format!("项目目录不存在: {:?}", project_root)));
        }

        Ok(Self {
            project_root: project_root.clone(),
            build_config: BuildConfig::default(&project_root),
            test_config: TestConfig::default(&project_root),
            debugger: PluginDebugger::new(),
        })
    }

    /// 创建新插件项目
    pub fn create_plugin_project(&self, name: &str, template: TemplateType) -> WasmResult<()> {
        let plugin_dir = self.project_root.join(name);

        if plugin_dir.exists() {
            return Err(WasmError::runtime(format!("插件目录已存在: {:?}", plugin_dir)));
        }

        std::fs::create_dir_all(&plugin_dir)
            .map_err(|e| WasmError::runtime(format!("创建插件目录失败: {}", e)))?;

        let template = self.get_template(template)?;
        self.generate_from_template(&plugin_dir, &template, name)?;

        info!("成功创建插件项目: {}", name);
        Ok(())
    }

    /// 构建插件
    pub fn build_plugin(&self, plugin_name: &str) -> WasmResult<PathBuf> {
        let plugin_dir = self.project_root.join(plugin_name);
        let output_path = self.build_config.output_dir.join(format!("{}.wasm", plugin_name));

        info!("开始构建插件: {}", plugin_name);
        let start_time = Instant::now();

        // 模拟构建过程
        self.compile_source(&plugin_dir, &output_path)?;
        self.optimize_wasm(&output_path)?;

        let build_time = start_time.elapsed();
        info!("插件构建完成: {} (耗时: {:?})", plugin_name, build_time);

        Ok(output_path)
    }

    /// 运行测试
    pub fn run_tests(&self, plugin_name: &str) -> WasmResult<TestResults> {
        let plugin_dir = self.project_root.join(plugin_name);
        let test_dir = plugin_dir.join("tests");

        // 如果测试目录不存在，创建它并添加一个基本测试文件
        if !test_dir.exists() {
            std::fs::create_dir_all(&test_dir)
                .map_err(|e| WasmError::runtime(format!("创建测试目录失败: {}", e)))?;

            // 创建一个基本的测试文件
            let test_file = test_dir.join("basic_test.rs");
            std::fs::write(&test_file, "// 基本测试文件\n")
                .map_err(|e| WasmError::runtime(format!("创建测试文件失败: {}", e)))?;
        }

        info!("开始运行测试: {}", plugin_name);
        let start_time = Instant::now();

        // 模拟测试执行
        let mut results = TestResults::new();
        results.add_test("test_basic_functionality", true, Duration::from_millis(50));
        results.add_test("test_error_handling", true, Duration::from_millis(30));
        results.add_test("test_performance", true, Duration::from_millis(100));

        let test_time = start_time.elapsed();
        results.total_time = test_time;

        info!("测试完成: {} (耗时: {:?})", plugin_name, test_time);
        Ok(results)
    }

    /// 运行基准测试
    pub fn run_benchmarks(&self, plugin_name: &str) -> WasmResult<Vec<BenchmarkResult>> {
        info!("开始运行基准测试: {}", plugin_name);

        let benchmarks = vec![
            Benchmark {
                name: "small_data_processing".to_string(),
                description: "小数据量处理".to_string(),
                data_size: 1000,
                iterations: 1000,
            },
            Benchmark {
                name: "medium_data_processing".to_string(),
                description: "中等数据量处理".to_string(),
                data_size: 10000,
                iterations: 500,
            },
            Benchmark {
                name: "large_data_processing".to_string(),
                description: "大数据量处理".to_string(),
                data_size: 100000,
                iterations: 100,
            },
            Benchmark {
                name: "stress_test".to_string(),
                description: "压力测试".to_string(),
                data_size: 1000000,
                iterations: 10,
            },
        ];

        let mut results = Vec::new();
        for benchmark in benchmarks {
            let result = self.run_single_benchmark(&benchmark)?;
            results.push(result);
        }

        info!("基准测试完成: {}", plugin_name);
        Ok(results)
    }

    /// 运行高级基准测试套件
    pub fn run_advanced_benchmarks(&self, plugin_name: &str, config: &AdvancedBenchmarkConfig) -> WasmResult<BenchmarkReport> {
        info!("开始运行高级基准测试套件: {}", plugin_name);
        let start_time = Instant::now();

        let mut results = Vec::new();
        let mut total_errors = 0;
        let mut total_iterations = 0;

        // 运行标准基准测试
        let standard_results = self.run_benchmarks(plugin_name)?;
        results.extend(standard_results);

        // 运行自定义基准测试
        for custom_benchmark in &config.custom_benchmarks {
            let result = self.run_single_benchmark(custom_benchmark)?;
            total_errors += result.detailed_stats.failed_iterations;
            total_iterations += result.detailed_stats.total_iterations;
            results.push(result);
        }

        // 运行并发基准测试
        if config.enable_concurrency_tests {
            let concurrency_results = self.run_concurrency_benchmarks(plugin_name)?;
            results.extend(concurrency_results);
        }

        // 运行内存压力测试
        if config.enable_memory_stress_tests {
            let memory_results = self.run_memory_stress_tests(plugin_name)?;
            results.extend(memory_results);
        }

        let total_time = start_time.elapsed();

        // 计算摘要信息（在移动results之前）
        let overall_score = self.calculate_overall_score(&results);
        let performance_grade = self.calculate_performance_grade(&results);
        let recommendations = self.generate_recommendations(&results);

        // 生成综合报告
        let report = BenchmarkReport {
            plugin_name: plugin_name.to_string(),
            timestamp: chrono::Utc::now(),
            total_duration: total_time,
            results,
            summary: BenchmarkSummary {
                total_tests: total_iterations,
                total_errors,
                overall_score,
                performance_grade,
                recommendations,
            },
            system_info: self.collect_system_info(),
        };

        info!("高级基准测试完成: {} (耗时: {:?})", plugin_name, total_time);
        Ok(report)
    }

    /// 启动调试会话
    pub fn start_debug_session(&mut self, plugin_name: &str) -> WasmResult<String> {
        let session_id = format!("debug_{}_{}", plugin_name, chrono::Utc::now().timestamp());

        let session = DebugSession {
            session_id: session_id.clone(),
            start_time: Instant::now(),
            state: DebugState::Running,
            call_stack: Vec::new(),
        };

        self.debugger.debug_session = Some(session);
        info!("启动调试会话: {}", session_id);

        Ok(session_id)
    }

    /// 设置断点
    pub fn set_breakpoint(&mut self, file: &str, line: u32) -> WasmResult<()> {
        self.debugger.breakpoints
            .entry(file.to_string())
            .or_insert_with(Vec::new)
            .push(line);

        debug!("设置断点: {}:{}", file, line);
        Ok(())
    }

    /// 生成插件文档
    pub fn generate_docs(&self, plugin_name: &str) -> WasmResult<()> {
        let plugin_dir = self.project_root.join(plugin_name);
        let docs_dir = plugin_dir.join("docs");

        std::fs::create_dir_all(&docs_dir)
            .map_err(|e| WasmError::runtime(format!("创建文档目录失败: {}", e)))?;

        // 生成API文档
        self.generate_api_docs(&docs_dir)?;

        // 生成使用指南
        self.generate_usage_guide(&docs_dir, plugin_name)?;

        info!("文档生成完成: {}", plugin_name);
        Ok(())
    }

    // 私有辅助方法
    fn get_template(&self, template_type: TemplateType) -> WasmResult<PluginTemplate> {
        // 返回预定义的模板
        match template_type {
            TemplateType::Basic => Ok(self.create_basic_template()),
            TemplateType::Processor => Ok(self.create_processor_template()),
            _ => Err(WasmError::runtime("不支持的模板类型".to_string())),
        }
    }

    fn create_basic_template(&self) -> PluginTemplate {
        PluginTemplate {
            name: "basic".to_string(),
            description: "基础插件模板".to_string(),
            template_type: TemplateType::Basic,
            files: vec![
                TemplateFile {
                    path: "src/lib.rs".to_string(),
                    content: include_str!("../templates/basic_plugin.rs").to_string(),
                    is_template: true,
                },
                TemplateFile {
                    path: "Cargo.toml".to_string(),
                    content: include_str!("../templates/Cargo.toml.template").to_string(),
                    is_template: true,
                },
            ],
            dependencies: vec!["wasmtime".to_string(), "serde".to_string()],
        }
    }

    fn create_processor_template(&self) -> PluginTemplate {
        PluginTemplate {
            name: "processor".to_string(),
            description: "数据处理器模板".to_string(),
            template_type: TemplateType::Processor,
            files: vec![
                TemplateFile {
                    path: "src/lib.rs".to_string(),
                    content: "// 数据处理器插件模板\n".to_string(),
                    is_template: true,
                },
            ],
            dependencies: vec!["dataflare-wasm".to_string()],
        }
    }

    fn generate_from_template(&self, target_dir: &Path, template: &PluginTemplate, plugin_name: &str) -> WasmResult<()> {
        for file in &template.files {
            let file_path = target_dir.join(&file.path);

            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| WasmError::runtime(format!("创建目录失败: {}", e)))?;
            }

            let content = if file.is_template {
                file.content.replace("{{plugin_name}}", plugin_name)
            } else {
                file.content.clone()
            };

            std::fs::write(&file_path, content)
                .map_err(|e| WasmError::runtime(format!("写入文件失败: {}", e)))?;
        }

        Ok(())
    }

    fn compile_source(&self, source_dir: &Path, output_path: &Path) -> WasmResult<()> {
        debug!("编译源代码: {:?} -> {:?}", source_dir, output_path);

        // 检查源目录是否存在
        if !source_dir.exists() {
            return Err(WasmError::runtime(format!("源目录不存在: {:?}", source_dir)));
        }

        // 根据目标架构选择编译器
        match self.build_config.target.as_str() {
            "wasm32-wasi" => self.compile_rust_to_wasm(source_dir, output_path),
            "wasm32-unknown-unknown" => self.compile_rust_to_wasm_unknown(source_dir, output_path),
            _ => Err(WasmError::runtime(format!("不支持的目标架构: {}", self.build_config.target))),
        }
    }

    fn compile_rust_to_wasm(&self, source_dir: &Path, output_path: &Path) -> WasmResult<()> {
        use std::process::Command;

        debug!("使用cargo编译Rust到WASM...");

        // 构建cargo命令
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
           .arg("--target")
           .arg(&self.build_config.target)
           .current_dir(source_dir);

        // 根据优化级别设置参数
        match self.build_config.optimization {
            OptimizationLevel::None => {},
            OptimizationLevel::Basic | OptimizationLevel::Full => {
                cmd.arg("--release");
            },
            OptimizationLevel::Size => {
                cmd.arg("--release");
                cmd.env("RUSTFLAGS", "-C opt-level=s");
            },
        }

        // 执行编译
        let output = cmd.output()
            .map_err(|e| WasmError::runtime(format!("执行cargo失败: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WasmError::runtime(format!("编译失败: {}", stderr)));
        }

        // 复制编译结果到输出路径
        let target_dir = source_dir.join("target").join(&self.build_config.target);
        let profile_dir = if matches!(self.build_config.optimization, OptimizationLevel::None) {
            target_dir.join("debug")
        } else {
            target_dir.join("release")
        };

        // 查找生成的WASM文件
        let wasm_files: Vec<_> = std::fs::read_dir(&profile_dir)
            .map_err(|e| WasmError::runtime(format!("读取目标目录失败: {}", e)))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "wasm")
                    .unwrap_or(false)
            })
            .collect();

        if wasm_files.is_empty() {
            return Err(WasmError::runtime("未找到生成的WASM文件"));
        }

        // 复制第一个WASM文件到输出路径
        let source_wasm = wasm_files[0].path();
        std::fs::copy(&source_wasm, output_path)
            .map_err(|e| WasmError::runtime(format!("复制WASM文件失败: {}", e)))?;

        info!("编译完成: {:?}", output_path);
        Ok(())
    }

    fn compile_rust_to_wasm_unknown(&self, source_dir: &Path, output_path: &Path) -> WasmResult<()> {
        // 类似于compile_rust_to_wasm，但针对wasm32-unknown-unknown目标
        self.compile_rust_to_wasm(source_dir, output_path)
    }

    fn optimize_wasm(&self, wasm_path: &Path) -> WasmResult<()> {
        debug!("优化WASM模块: {:?}", wasm_path);

        if !wasm_path.exists() {
            return Err(WasmError::runtime(format!("WASM文件不存在: {:?}", wasm_path)));
        }

        // 根据优化级别进行不同的优化
        match self.build_config.optimization {
            OptimizationLevel::None => {
                debug!("跳过WASM优化");
                return Ok(());
            },
            OptimizationLevel::Basic => {
                self.basic_wasm_optimization(wasm_path)?;
            },
            OptimizationLevel::Full => {
                self.full_wasm_optimization(wasm_path)?;
            },
            OptimizationLevel::Size => {
                self.size_wasm_optimization(wasm_path)?;
            },
        }

        info!("WASM优化完成: {:?}", wasm_path);
        Ok(())
    }

    fn basic_wasm_optimization(&self, wasm_path: &Path) -> WasmResult<()> {
        // 基本优化：移除调试信息
        debug!("执行基本WASM优化");

        // 这里可以集成wasm-opt工具
        if let Err(_) = self.run_wasm_opt(wasm_path, &["-O1"]) {
            warn!("wasm-opt不可用，跳过优化");
        }

        Ok(())
    }

    fn full_wasm_optimization(&self, wasm_path: &Path) -> WasmResult<()> {
        // 完全优化：最大化性能
        debug!("执行完全WASM优化");

        if let Err(_) = self.run_wasm_opt(wasm_path, &["-O3", "--enable-bulk-memory"]) {
            warn!("wasm-opt不可用，跳过优化");
        }

        Ok(())
    }

    fn size_wasm_optimization(&self, wasm_path: &Path) -> WasmResult<()> {
        // 大小优化：最小化文件大小
        debug!("执行大小WASM优化");

        if let Err(_) = self.run_wasm_opt(wasm_path, &["-Os", "--strip-debug"]) {
            warn!("wasm-opt不可用，跳过优化");
        }

        Ok(())
    }

    fn run_wasm_opt(&self, wasm_path: &Path, args: &[&str]) -> Result<(), std::io::Error> {
        use std::process::Command;

        let mut cmd = Command::new("wasm-opt");
        cmd.args(args)
           .arg("-o")
           .arg(wasm_path)
           .arg(wasm_path);

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("wasm-opt执行失败: {}", stderr);
        }

        Ok(())
    }

    fn run_single_benchmark(&self, benchmark: &Benchmark) -> WasmResult<BenchmarkResult> {
        let start_time = Instant::now();
        let mut times = Vec::new();
        let mut successful_iterations = 0;
        let mut failed_iterations = 0;
        let mut memory_allocations = Vec::new();
        let mut peak_memory = 0u64;

        info!("运行基准测试: {} ({} 次迭代)", benchmark.name, benchmark.iterations);

        for i in 0..benchmark.iterations {
            let iter_start = Instant::now();

            // 模拟基准测试执行
            let success = self.simulate_benchmark_iteration(benchmark, i);

            let elapsed = iter_start.elapsed();
            times.push(elapsed);

            if success {
                successful_iterations += 1;
            } else {
                failed_iterations += 1;
            }

            // 模拟内存使用统计
            let memory_alloc = (1024 * (i % 100 + 1)) as u64;
            memory_allocations.push(memory_alloc);
            peak_memory = peak_memory.max(memory_alloc);
        }

        let total_time = start_time.elapsed();
        let avg_time = total_time / benchmark.iterations as u32;
        let min_time = times.iter().min().copied().unwrap_or_default();
        let max_time = times.iter().max().copied().unwrap_or_default();
        let throughput = successful_iterations as f64 / total_time.as_secs_f64();

        // 计算标准差
        let avg_nanos = avg_time.as_nanos() as f64;
        let variance: f64 = times.iter()
            .map(|t| {
                let diff = t.as_nanos() as f64 - avg_nanos;
                diff * diff
            })
            .sum::<f64>() / times.len() as f64;
        let std_deviation = variance.sqrt();

        // 计算百分位数
        let mut sorted_times = times.clone();
        sorted_times.sort();
        let percentiles = self.calculate_percentiles(&sorted_times);

        // 计算错误率
        let error_rate = failed_iterations as f64 / benchmark.iterations as f64;

        // 计算平均内存分配
        let avg_memory_allocations = memory_allocations.iter().sum::<u64>() / memory_allocations.len() as u64;

        // 模拟其他统计信息
        let cpu_usage = 45.0 + (benchmark.data_size as f64 / 10000.0); // 模拟CPU使用率
        let gc_count = (benchmark.iterations / 100) as u32; // 模拟GC次数
        let cache_hit_rate = 0.85 + (0.1 * rand::random::<f64>()); // 模拟缓存命中率

        Ok(BenchmarkResult {
            name: benchmark.name.clone(),
            avg_time,
            min_time,
            max_time,
            throughput,
            memory_usage: peak_memory,
            std_deviation,
            percentiles,
            cpu_usage,
            error_rate,
            detailed_stats: DetailedStats {
                total_iterations: benchmark.iterations,
                successful_iterations,
                failed_iterations,
                avg_memory_allocations,
                peak_memory_usage: peak_memory,
                gc_count,
                cache_hit_rate,
            },
        })
    }

    /// 模拟基准测试迭代
    fn simulate_benchmark_iteration(&self, benchmark: &Benchmark, iteration: usize) -> bool {
        // 模拟不同的执行时间和成功率
        let base_time = match benchmark.data_size {
            size if size < 1000 => 50,
            size if size < 10000 => 100,
            size if size < 100000 => 200,
            _ => 500,
        };

        // 添加一些随机性
        let jitter = (rand::random::<u64>() % 50) + 1;
        std::thread::sleep(Duration::from_micros(base_time + jitter));

        // 模拟偶尔的失败 (5%失败率)
        iteration % 20 != 0
    }

    /// 计算百分位数
    fn calculate_percentiles(&self, sorted_times: &[Duration]) -> PercentileStats {
        let len = sorted_times.len();

        let p50_idx = (len as f64 * 0.50) as usize;
        let p90_idx = (len as f64 * 0.90) as usize;
        let p95_idx = (len as f64 * 0.95) as usize;
        let p99_idx = (len as f64 * 0.99) as usize;
        let p999_idx = (len as f64 * 0.999) as usize;

        PercentileStats {
            p50: sorted_times.get(p50_idx).copied().unwrap_or_default(),
            p90: sorted_times.get(p90_idx).copied().unwrap_or_default(),
            p95: sorted_times.get(p95_idx).copied().unwrap_or_default(),
            p99: sorted_times.get(p99_idx).copied().unwrap_or_default(),
            p999: sorted_times.get(p999_idx).copied().unwrap_or_default(),
        }
    }

    /// 运行并发基准测试
    fn run_concurrency_benchmarks(&self, plugin_name: &str) -> WasmResult<Vec<BenchmarkResult>> {
        info!("运行并发基准测试: {}", plugin_name);

        let concurrency_benchmarks = vec![
            Benchmark {
                name: "concurrent_small_load".to_string(),
                description: "并发小负载测试".to_string(),
                data_size: 1000,
                iterations: 100,
            },
            Benchmark {
                name: "concurrent_medium_load".to_string(),
                description: "并发中等负载测试".to_string(),
                data_size: 10000,
                iterations: 50,
            },
        ];

        let mut results = Vec::new();
        for benchmark in concurrency_benchmarks {
            let result = self.run_concurrent_benchmark(&benchmark)?;
            results.push(result);
        }

        Ok(results)
    }

    /// 运行内存压力测试
    fn run_memory_stress_tests(&self, plugin_name: &str) -> WasmResult<Vec<BenchmarkResult>> {
        info!("运行内存压力测试: {}", plugin_name);

        let memory_benchmarks = vec![
            Benchmark {
                name: "memory_stress_1mb".to_string(),
                description: "1MB内存压力测试".to_string(),
                data_size: 1024 * 1024,
                iterations: 10,
            },
            Benchmark {
                name: "memory_stress_10mb".to_string(),
                description: "10MB内存压力测试".to_string(),
                data_size: 10 * 1024 * 1024,
                iterations: 5,
            },
        ];

        let mut results = Vec::new();
        for benchmark in memory_benchmarks {
            let result = self.run_memory_stress_benchmark(&benchmark)?;
            results.push(result);
        }

        Ok(results)
    }

    /// 运行并发基准测试
    fn run_concurrent_benchmark(&self, benchmark: &Benchmark) -> WasmResult<BenchmarkResult> {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let start_time = Instant::now();
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();

        let thread_count = 4; // 固定4个线程
        let iterations_per_thread = benchmark.iterations / thread_count;

        for thread_id in 0..thread_count {
            let results_clone = Arc::clone(&results);
            let benchmark_clone = benchmark.clone();

            let handle = thread::spawn(move || {
                let mut thread_times = Vec::new();
                let mut thread_successes = 0;

                for i in 0..iterations_per_thread {
                    let iter_start = Instant::now();

                    // 模拟并发处理
                    let success = (thread_id + i) % 20 != 0; // 5%失败率
                    let processing_time = 100 + (thread_id * 10) + (i % 50);
                    thread::sleep(Duration::from_micros(processing_time as u64));

                    thread_times.push(iter_start.elapsed());
                    if success {
                        thread_successes += 1;
                    }
                }

                let mut results_guard = results_clone.lock().unwrap();
                results_guard.push((thread_times, thread_successes));
            });

            handles.push(handle);
        }

        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }

        let total_time = start_time.elapsed();
        let results_guard = results.lock().unwrap();

        // 合并所有线程的结果
        let mut all_times = Vec::new();
        let mut total_successes = 0;

        for (thread_times, thread_successes) in results_guard.iter() {
            all_times.extend(thread_times.iter().cloned());
            total_successes += thread_successes;
        }

        let avg_time = if !all_times.is_empty() {
            all_times.iter().sum::<Duration>() / all_times.len() as u32
        } else {
            Duration::ZERO
        };

        let min_time = all_times.iter().min().copied().unwrap_or_default();
        let max_time = all_times.iter().max().copied().unwrap_or_default();
        let throughput = total_successes as f64 / total_time.as_secs_f64();

        // 计算并发特定的统计信息
        let mut sorted_times = all_times.clone();
        sorted_times.sort();
        let percentiles = self.calculate_percentiles(&sorted_times);

        let failed_iterations = benchmark.iterations - total_successes;
        let error_rate = failed_iterations as f64 / benchmark.iterations as f64;

        Ok(BenchmarkResult {
            name: format!("{}_concurrent", benchmark.name),
            avg_time,
            min_time,
            max_time,
            throughput,
            memory_usage: (benchmark.data_size * thread_count) as u64,
            std_deviation: self.calculate_std_deviation(&all_times, avg_time),
            percentiles,
            cpu_usage: 75.0, // 并发测试通常CPU使用率较高
            error_rate,
            detailed_stats: DetailedStats {
                total_iterations: benchmark.iterations,
                successful_iterations: total_successes,
                failed_iterations,
                avg_memory_allocations: (benchmark.data_size * 2) as u64,
                peak_memory_usage: (benchmark.data_size * thread_count * 2) as u64,
                gc_count: (benchmark.iterations / 50) as u32,
                cache_hit_rate: 0.75, // 并发环境下缓存命中率可能较低
            },
        })
    }

    /// 运行内存压力基准测试
    fn run_memory_stress_benchmark(&self, benchmark: &Benchmark) -> WasmResult<BenchmarkResult> {
        let start_time = Instant::now();
        let mut times = Vec::new();
        let mut successful_iterations = 0;
        let mut failed_iterations = 0;
        let mut peak_memory = 0u64;

        info!("运行内存压力测试: {} (数据大小: {} bytes)", benchmark.name, benchmark.data_size);

        for i in 0..benchmark.iterations {
            let iter_start = Instant::now();

            // 模拟大内存分配
            let memory_usage = benchmark.data_size as u64 * (i + 1) as u64;
            peak_memory = peak_memory.max(memory_usage);

            // 模拟内存密集型操作
            let processing_time = 500 + (benchmark.data_size / 1000); // 基于数据大小的处理时间
            std::thread::sleep(Duration::from_micros(processing_time as u64));

            let elapsed = iter_start.elapsed();
            times.push(elapsed);

            // 内存压力测试的成功率稍低
            if i % 15 != 0 { // 约93%成功率
                successful_iterations += 1;
            } else {
                failed_iterations += 1;
            }
        }

        let total_time = start_time.elapsed();
        let avg_time = total_time / benchmark.iterations as u32;
        let min_time = times.iter().min().copied().unwrap_or_default();
        let max_time = times.iter().max().copied().unwrap_or_default();
        let throughput = successful_iterations as f64 / total_time.as_secs_f64();

        let mut sorted_times = times.clone();
        sorted_times.sort();
        let percentiles = self.calculate_percentiles(&sorted_times);

        let error_rate = failed_iterations as f64 / benchmark.iterations as f64;

        Ok(BenchmarkResult {
            name: format!("{}_memory_stress", benchmark.name),
            avg_time,
            min_time,
            max_time,
            throughput,
            memory_usage: peak_memory,
            std_deviation: self.calculate_std_deviation(&times, avg_time),
            percentiles,
            cpu_usage: 60.0 + (benchmark.data_size as f64 / 100000.0), // 内存操作也会影响CPU
            error_rate,
            detailed_stats: DetailedStats {
                total_iterations: benchmark.iterations,
                successful_iterations,
                failed_iterations,
                avg_memory_allocations: peak_memory / benchmark.iterations as u64,
                peak_memory_usage: peak_memory,
                gc_count: (benchmark.iterations / 3) as u32, // 内存压力测试会触发更多GC
                cache_hit_rate: 0.60, // 大内存操作通常缓存命中率较低
            },
        })
    }

    /// 计算标准差
    fn calculate_std_deviation(&self, times: &[Duration], avg_time: Duration) -> f64 {
        if times.is_empty() {
            return 0.0;
        }

        let avg_nanos = avg_time.as_nanos() as f64;
        let variance: f64 = times.iter()
            .map(|t| {
                let diff = t.as_nanos() as f64 - avg_nanos;
                diff * diff
            })
            .sum::<f64>() / times.len() as f64;

        variance.sqrt()
    }

    /// 计算总体评分
    fn calculate_overall_score(&self, results: &[BenchmarkResult]) -> f64 {
        if results.is_empty() {
            return 0.0;
        }

        let mut total_score = 0.0;
        let mut weight_sum = 0.0;

        for result in results {
            // 基于吞吐量、错误率和CPU使用率计算分数
            let throughput_score = (result.throughput / 1000.0).min(1.0) * 40.0; // 最高40分
            let error_score = (1.0 - result.error_rate) * 30.0; // 最高30分
            let cpu_score = (100.0 - result.cpu_usage) / 100.0 * 20.0; // 最高20分
            let memory_score = if result.memory_usage > 0 {
                (1.0 - (result.memory_usage as f64 / (100.0 * 1024.0 * 1024.0)).min(1.0)) * 10.0 // 最高10分
            } else {
                10.0
            };

            let score = throughput_score + error_score + cpu_score + memory_score;
            total_score += score;
            weight_sum += 1.0;
        }

        total_score / weight_sum
    }

    /// 计算性能等级
    fn calculate_performance_grade(&self, results: &[BenchmarkResult]) -> PerformanceGrade {
        let score = self.calculate_overall_score(results);

        match score {
            s if s >= 90.0 => PerformanceGrade::Excellent,
            s if s >= 80.0 => PerformanceGrade::Good,
            s if s >= 70.0 => PerformanceGrade::Fair,
            s if s >= 60.0 => PerformanceGrade::Poor,
            _ => PerformanceGrade::Failing,
        }
    }

    /// 生成优化建议
    fn generate_recommendations(&self, results: &[BenchmarkResult]) -> Vec<String> {
        let mut recommendations = Vec::new();

        // 分析错误率
        let avg_error_rate = results.iter().map(|r| r.error_rate).sum::<f64>() / results.len() as f64;
        if avg_error_rate > 0.1 {
            recommendations.push("错误率较高，建议检查错误处理逻辑和输入验证".to_string());
        }

        // 分析CPU使用率
        let avg_cpu_usage = results.iter().map(|r| r.cpu_usage).sum::<f64>() / results.len() as f64;
        if avg_cpu_usage > 80.0 {
            recommendations.push("CPU使用率较高，建议优化算法复杂度或使用异步处理".to_string());
        }

        // 分析内存使用
        let max_memory = results.iter().map(|r| r.memory_usage).max().unwrap_or(0);
        if max_memory > 50 * 1024 * 1024 { // 50MB
            recommendations.push("内存使用量较大，建议优化数据结构或实现内存池".to_string());
        }

        // 分析吞吐量
        let avg_throughput = results.iter().map(|r| r.throughput).sum::<f64>() / results.len() as f64;
        if avg_throughput < 100.0 {
            recommendations.push("吞吐量较低，建议优化热点代码路径或启用编译器优化".to_string());
        }

        // 分析延迟分布
        let has_high_p99 = results.iter().any(|r| r.percentiles.p99 > Duration::from_millis(100));
        if has_high_p99 {
            recommendations.push("P99延迟较高，建议检查是否存在阻塞操作或GC压力".to_string());
        }

        if recommendations.is_empty() {
            recommendations.push("性能表现良好，继续保持当前的优化水平".to_string());
        }

        recommendations
    }

    /// 收集系统信息
    fn collect_system_info(&self) -> SystemInfo {
        SystemInfo {
            os: std::env::consts::OS.to_string(),
            cpu: "Unknown CPU".to_string(), // 在实际实现中可以使用系统API获取
            memory: 8 * 1024 * 1024 * 1024, // 模拟8GB内存
            rust_version: env!("CARGO_PKG_RUST_VERSION").to_string(),
            wasm_runtime_version: "wasmtime-33.0".to_string(),
        }
    }

    fn generate_api_docs(&self, _docs_dir: &Path) -> WasmResult<()> {
        debug!("生成API文档...");
        Ok(())
    }

    fn generate_usage_guide(&self, _docs_dir: &Path, _plugin_name: &str) -> WasmResult<()> {
        debug!("生成使用指南...");
        Ok(())
    }
}

/// 测试结果
#[derive(Debug, Clone)]
pub struct TestResults {
    /// 测试列表
    pub tests: Vec<TestResult>,
    /// 总时间
    pub total_time: Duration,
}

/// 单个测试结果
#[derive(Debug, Clone)]
pub struct TestResult {
    /// 测试名称
    pub name: String,
    /// 是否通过
    pub passed: bool,
    /// 执行时间
    pub duration: Duration,
    /// 错误信息
    pub error: Option<String>,
}

impl TestResults {
    /// 创建新的测试结果
    pub fn new() -> Self {
        Self {
            tests: Vec::new(),
            total_time: Duration::ZERO,
        }
    }

    /// 添加测试结果
    pub fn add_test(&mut self, name: &str, passed: bool, duration: Duration) {
        self.tests.push(TestResult {
            name: name.to_string(),
            passed,
            duration,
            error: None,
        });
    }

    /// 获取通过的测试数量
    pub fn passed_count(&self) -> usize {
        self.tests.iter().filter(|t| t.passed).count()
    }

    /// 获取失败的测试数量
    pub fn failed_count(&self) -> usize {
        self.tests.iter().filter(|t| !t.passed).count()
    }
}

impl Default for TestResults {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildConfig {
    /// 创建默认构建配置
    pub fn default(project_root: &Path) -> Self {
        Self {
            target: "wasm32-wasi".to_string(),
            optimization: OptimizationLevel::Basic,
            debug_info: true,
            output_dir: project_root.join("target/wasm32-wasi/release"),
            source_dir: project_root.join("src"),
            dependencies: Vec::new(),
        }
    }
}

impl TestConfig {
    /// 创建默认测试配置
    pub fn default(project_root: &Path) -> Self {
        Self {
            test_data_dir: project_root.join("test_data"),
            timeout: Duration::from_secs(30),
            parallel_tests: 4,
            coverage: false,
        }
    }
}

impl PluginDebugger {
    /// 创建新的调试器
    pub fn new() -> Self {
        Self {
            breakpoints: HashMap::new(),
            debug_session: None,
            log_level: LogLevel::Info,
        }
    }
}

impl Default for PluginDebugger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_dev_tools_creation() {
        let temp_dir = TempDir::new().unwrap();
        let dev_tools = PluginDevTools::new(temp_dir.path()).unwrap();
        assert_eq!(dev_tools.project_root, temp_dir.path());
    }

    #[test]
    fn test_benchmark_result() {
        let result = BenchmarkResult {
            name: "test".to_string(),
            avg_time: Duration::from_millis(100),
            min_time: Duration::from_millis(50),
            max_time: Duration::from_millis(150),
            throughput: 10.0,
            memory_usage: 1024,
            std_deviation: 25.0,
            percentiles: PercentileStats {
                p50: Duration::from_millis(95),
                p90: Duration::from_millis(140),
                p95: Duration::from_millis(145),
                p99: Duration::from_millis(149),
                p999: Duration::from_millis(150),
            },
            cpu_usage: 45.5,
            error_rate: 0.05,
            detailed_stats: DetailedStats {
                total_iterations: 1000,
                successful_iterations: 950,
                failed_iterations: 50,
                avg_memory_allocations: 2048,
                peak_memory_usage: 4096,
                gc_count: 10,
                cache_hit_rate: 0.87,
            },
        };

        assert_eq!(result.name, "test");
        assert_eq!(result.avg_time, Duration::from_millis(100));
        assert_eq!(result.throughput, 10.0);
        assert_eq!(result.std_deviation, 25.0);
        assert_eq!(result.error_rate, 0.05);
        assert_eq!(result.detailed_stats.total_iterations, 1000);
        assert_eq!(result.detailed_stats.successful_iterations, 950);
    }

    #[test]
    fn test_test_results() {
        let mut results = TestResults::new();
        results.add_test("test1", true, Duration::from_millis(50));
        results.add_test("test2", false, Duration::from_millis(30));
        results.add_test("test3", true, Duration::from_millis(70));

        assert_eq!(results.passed_count(), 2);
        assert_eq!(results.failed_count(), 1);
        assert_eq!(results.tests.len(), 3);
    }

    #[test]
    fn test_advanced_benchmark_config() {
        let config = AdvancedBenchmarkConfig {
            custom_benchmarks: vec![
                Benchmark {
                    name: "custom_test".to_string(),
                    description: "自定义测试".to_string(),
                    data_size: 5000,
                    iterations: 200,
                }
            ],
            enable_concurrency_tests: true,
            enable_memory_stress_tests: true,
            concurrency_threads: 8,
            memory_stress_size: 1024 * 1024,
            test_timeout: Duration::from_secs(60),
        };

        assert_eq!(config.custom_benchmarks.len(), 1);
        assert!(config.enable_concurrency_tests);
        assert!(config.enable_memory_stress_tests);
        assert_eq!(config.concurrency_threads, 8);
    }

    #[test]
    fn test_performance_grade_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let dev_tools = PluginDevTools::new(temp_dir.path()).unwrap();

        // 测试优秀等级
        let excellent_results = vec![
            BenchmarkResult {
                name: "excellent_test".to_string(),
                avg_time: Duration::from_millis(10),
                min_time: Duration::from_millis(5),
                max_time: Duration::from_millis(15),
                throughput: 1000.0,
                memory_usage: 1024,
                std_deviation: 2.0,
                percentiles: PercentileStats {
                    p50: Duration::from_millis(10),
                    p90: Duration::from_millis(12),
                    p95: Duration::from_millis(13),
                    p99: Duration::from_millis(14),
                    p999: Duration::from_millis(15),
                },
                cpu_usage: 30.0,
                error_rate: 0.01,
                detailed_stats: DetailedStats {
                    total_iterations: 1000,
                    successful_iterations: 990,
                    failed_iterations: 10,
                    avg_memory_allocations: 1024,
                    peak_memory_usage: 2048,
                    gc_count: 5,
                    cache_hit_rate: 0.95,
                },
            }
        ];

        let grade = dev_tools.calculate_performance_grade(&excellent_results);
        assert!(matches!(grade, PerformanceGrade::Excellent));

        // 测试评分计算
        let score = dev_tools.calculate_overall_score(&excellent_results);
        assert!(score >= 90.0);
    }

    #[test]
    fn test_recommendations_generation() {
        let temp_dir = TempDir::new().unwrap();
        let dev_tools = PluginDevTools::new(temp_dir.path()).unwrap();

        // 创建有问题的基准测试结果
        let problematic_results = vec![
            BenchmarkResult {
                name: "problematic_test".to_string(),
                avg_time: Duration::from_millis(200),
                min_time: Duration::from_millis(100),
                max_time: Duration::from_millis(500),
                throughput: 50.0, // 低吞吐量
                memory_usage: 100 * 1024 * 1024, // 100MB - 高内存使用
                std_deviation: 50.0,
                percentiles: PercentileStats {
                    p50: Duration::from_millis(180),
                    p90: Duration::from_millis(300),
                    p95: Duration::from_millis(400),
                    p99: Duration::from_millis(450), // 高P99延迟
                    p999: Duration::from_millis(500),
                },
                cpu_usage: 85.0, // 高CPU使用率
                error_rate: 0.15, // 高错误率
                detailed_stats: DetailedStats {
                    total_iterations: 100,
                    successful_iterations: 85,
                    failed_iterations: 15,
                    avg_memory_allocations: 50 * 1024 * 1024,
                    peak_memory_usage: 100 * 1024 * 1024,
                    gc_count: 20,
                    cache_hit_rate: 0.60,
                },
            }
        ];

        let recommendations = dev_tools.generate_recommendations(&problematic_results);

        // 应该包含多个建议
        assert!(recommendations.len() >= 4);

        // 检查是否包含预期的建议
        let recommendations_text = recommendations.join(" ");
        assert!(recommendations_text.contains("错误率较高"));
        assert!(recommendations_text.contains("CPU使用率较高"));
        assert!(recommendations_text.contains("内存使用量较大"));
        assert!(recommendations_text.contains("吞吐量较低"));
    }
}
