@echo off
setlocal enabledelayedexpansion

echo ========================================
echo DataFlare 超大数据集性能测试 (50万条记录)
echo ========================================
echo.

:: 设置颜色
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

:: 创建必要的目录
echo %BLUE%📁 创建必要的目录...%RESET%
if not exist "examples\data" mkdir "examples\data"
if not exist "dataflare\output" mkdir "dataflare\output"

:: 生成测试数据
echo %BLUE%📊 生成50万条测试数据...%RESET%
echo 这可能需要几分钟时间，请耐心等待...

:: 使用PowerShell生成CSV数据
powershell -Command "& {
    $outputFile = 'examples\data\extra_large_test.csv'
    $recordCount = 500000
    
    Write-Host '正在生成CSV文件头...'
    'id,name,email,age' | Out-File -FilePath $outputFile -Encoding UTF8
    
    Write-Host '正在生成数据记录...'
    $batchSize = 10000
    $batches = [math]::Ceiling($recordCount / $batchSize)
    
    for ($batch = 0; $batch -lt $batches; $batch++) {
        $startId = $batch * $batchSize + 1
        $endId = [math]::Min(($batch + 1) * $batchSize, $recordCount)
        
        $records = @()
        for ($i = $startId; $i -le $endId; $i++) {
            $name = 'User' + $i.ToString('D6')
            $email = 'user' + $i + '@example.com'
            $age = Get-Random -Minimum 18 -Maximum 80
            $records += \"$i,$name,$email,$age\"
        }
        
        $records | Out-File -FilePath $outputFile -Append -Encoding UTF8
        
        $progress = [math]::Round(($batch + 1) / $batches * 100, 1)
        Write-Host \"进度: $progress%% (批次 $($batch + 1)/$batches)\"
    }
    
    Write-Host '数据生成完成!'
    $fileSize = (Get-Item $outputFile).Length / 1MB
    Write-Host \"文件大小: $([math]::Round($fileSize, 2)) MB\"
}"

if %ERRORLEVEL% neq 0 (
    echo %RED%❌ 数据生成失败%RESET%
    pause
    exit /b 1
)

echo %GREEN%✅ 测试数据生成完成%RESET%

:: 检查数据文件
if not exist "examples\data\extra_large_test.csv" (
    echo %RED%❌ 测试数据文件未找到%RESET%
    pause
    exit /b 1
)

:: 显示文件信息
for %%F in ("examples\data\extra_large_test.csv") do (
    echo 📄 数据文件大小: %%~zF 字节
)

:: 编译DataFlare CLI
echo.
echo %BLUE%🔨 编译DataFlare CLI...%RESET%
cargo build --release -p dataflare-cli
if %ERRORLEVEL% neq 0 (
    echo %RED%❌ 编译失败%RESET%
    pause
    exit /b 1
)

echo %GREEN%✅ 编译完成%RESET%

:: 执行性能测试
echo.
echo %BLUE%🚀 开始执行超大数据集性能测试...%RESET%
echo 测试配置:
echo   - 数据量: 500,000 条记录
echo   - 工作流: examples\workflows\extra_large_performance_test.yaml
echo   - 输出: dataflare\output\extra_large_performance_output.csv
echo.

:: 记录开始时间
set start_time=%time%
echo 开始时间: %start_time%

:: 执行工作流
target\release\dataflare-cli.exe execute examples\workflows\extra_large_performance_test.yaml

set end_time=%time%
echo 结束时间: %end_time%

if %ERRORLEVEL% neq 0 (
    echo %RED%❌ 工作流执行失败%RESET%
    pause
    exit /b 1
)

echo %GREEN%✅ 工作流执行成功%RESET%

:: 验证输出文件
echo.
echo %BLUE%📊 验证输出结果...%RESET%

if exist "dataflare\output\extra_large_performance_output.csv" (
    echo %GREEN%✅ 输出文件已创建%RESET%
    
    :: 统计输出文件行数
    powershell -Command "& {
        $outputFile = 'dataflare\output\extra_large_performance_output.csv'
        $lineCount = (Get-Content $outputFile | Measure-Object -Line).Lines
        $fileSize = (Get-Item $outputFile).Length / 1MB
        
        Write-Host \"📄 输出文件行数: $lineCount\"
        Write-Host \"📁 输出文件大小: $([math]::Round($fileSize, 2)) MB\"
        Write-Host \"📍 输出文件路径: $((Get-Item $outputFile).FullName)\"
        
        # 显示前几行内容
        Write-Host \"\"
        Write-Host \"📝 文件内容预览:\"
        Get-Content $outputFile -Head 5 | ForEach-Object { Write-Host \"   $_\" }
        
        if ($lineCount -gt 3) {
            Write-Host \"   ...\"
            Get-Content $outputFile -Tail 2 | ForEach-Object { Write-Host \"   $_\" }
        }
    }"
    
    echo %GREEN%✅ 性能测试完成%RESET%
) else (
    echo %RED%❌ 输出文件未找到%RESET%
)

:: 计算执行时间（简化版本）
echo.
echo %BLUE%⏱️  性能统计:%RESET%
echo 开始时间: %start_time%
echo 结束时间: %end_time%

:: 清理临时文件（可选）
echo.
set /p cleanup="是否删除测试数据文件? (y/N): "
if /i "%cleanup%"=="y" (
    del "examples\data\extra_large_test.csv"
    echo %YELLOW%🗑️  测试数据文件已删除%RESET%
)

echo.
echo %GREEN%🎉 超大数据集性能测试完成!%RESET%
echo 输出文件保存在: dataflare\output\extra_large_performance_output.csv
echo.
pause
