@echo off
setlocal enabledelayedexpansion

echo ========================================
echo DataFlare Extra Large Dataset Performance Test (500K records)
echo ========================================
echo.

:: Set colors
set "GREEN=[92m"
set "YELLOW=[93m"
set "RED=[91m"
set "BLUE=[94m"
set "RESET=[0m"

:: 检查是否在正确的目录
if not exist "dataflare" (
    echo %RED%错误: 请在项目根目录运行此脚本%RESET%
    echo 当前目录: %CD%
    echo 期望目录结构: 包含 dataflare 文件夹
    pause
    exit /b 1
)

:: 创建输出目录
echo %BLUE%📁 创建输出目录...%RESET%
if not exist "dataflare\output" mkdir "dataflare\output"

:: 编译测试
echo %BLUE%🔨 编译DataFlare Runtime测试...%RESET%
cargo build -p dataflare-runtime --tests
if %ERRORLEVEL% neq 0 (
    echo %RED%❌ 编译失败%RESET%
    pause
    exit /b 1
)

echo %GREEN%✅ 编译完成%RESET%

:: 执行超大数据集性能测试
echo.
echo %BLUE%🚀 开始执行超大数据集性能测试...%RESET%
echo 测试配置:
echo   - 数据量: 500,000 条记录
echo   - 测试类型: Rust单元测试
echo   - 输出目录: dataflare\output\
echo.

:: 记录开始时间
echo 开始时间: %time%

:: 执行测试
cargo test -p dataflare-runtime --test performance_test test_extra_large_dataset_performance -- --nocapture

set test_result=%ERRORLEVEL%
echo 结束时间: %time%

if %test_result% neq 0 (
    echo %RED%❌ 性能测试失败%RESET%
    echo.
    echo %YELLOW%💡 提示:%RESET%
    echo   - 50万条记录是一个很大的数据集
    echo   - 如果测试失败，可能是由于内存或时间限制
    echo   - 可以尝试运行较小的数据集测试
    echo.
    echo %BLUE%🔄 运行其他性能测试:%RESET%
    echo   - 小数据集 (1K):   cargo test test_small_dataset_performance
    echo   - 中等数据集 (10K): cargo test test_medium_dataset_performance
    echo   - 大数据集 (50K):   cargo test test_large_dataset_performance
    echo.
    pause
    exit /b 1
)

echo %GREEN%✅ 超大数据集性能测试完成%RESET%

:: 检查输出文件
echo.
echo %BLUE%📊 检查输出结果...%RESET%

if exist "dataflare\output\extra_large_output.csv" (
    echo %GREEN%✅ 输出文件已创建%RESET%

    :: 显示文件信息
    for %%F in ("dataflare\output\extra_large_output.csv") do (
        echo 📄 输出文件大小: %%~zF 字节
        echo 📍 输出文件路径: %%~fF
    )

    :: 显示文件内容预览
    echo.
    echo %BLUE%📝 文件内容预览:%RESET%
    powershell -Command "& {
        $file = 'dataflare\output\extra_large_output.csv'
        if (Test-Path $file) {
            $lines = Get-Content $file
            $lineCount = $lines.Count
            Write-Host \"总行数: $lineCount\"
            Write-Host \"\"
            Write-Host \"前5行:\"
            $lines | Select-Object -First 5 | ForEach-Object { Write-Host \"   $_\" }
            if ($lineCount -gt 10) {
                Write-Host \"   ...\"
                Write-Host \"后2行:\"
                $lines | Select-Object -Last 2 | ForEach-Object { Write-Host \"   $_\" }
            }
        }
    }"
) else (
    echo %YELLOW%⚠️  输出文件未找到，但测试可能仍然成功%RESET%
    echo 检查测试日志以获取更多信息
)

:: 运行完整性能测试套件选项
echo.
set /p run_suite="是否运行完整的性能测试套件? (y/N): "
if /i "%run_suite%"=="y" (
    echo.
    echo %BLUE%🧪 运行完整性能测试套件...%RESET%
    cargo test -p dataflare-runtime --test performance_test -- --nocapture
)

echo.
echo %GREEN%🎉 性能测试完成!%RESET%
echo.
echo %BLUE%📊 性能测试结果总结:%RESET%
echo   - 超大数据集测试已完成
echo   - 输出文件保存在: dataflare\output\extra_large_output.csv
echo   - 详细性能指标请查看测试输出
echo.
echo %BLUE%🔧 其他可用的性能测试:%RESET%
echo   cargo test -p dataflare-runtime --test performance_test test_small_dataset_performance
echo   cargo test -p dataflare-runtime --test performance_test test_medium_dataset_performance
echo   cargo test -p dataflare-runtime --test performance_test test_large_dataset_performance
echo.
pause
