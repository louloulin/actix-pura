# DataFlare Performance Test - Windows PowerShell Version
param(
    [int]$Runs = 3,
    [string]$WorkflowFile = "examples\workflows\performance_test.yaml",
    [string]$InputFile = "examples\data\large_test.csv",
    [string]$OutputFile = "examples\data\performance_output.csv"
)

Write-Host "DataFlare 性能测试" -ForegroundColor Green
Write-Host "================================================" -ForegroundColor Green
Write-Host "测试日期: $(Get-Date)" -ForegroundColor Yellow
Write-Host "测试环境: $($env:COMPUTERNAME) - $($env:OS)" -ForegroundColor Yellow
Write-Host "PowerShell 版本: $($PSVersionTable.PSVersion)" -ForegroundColor Yellow
Write-Host "================================================" -ForegroundColor Green

# 获取当前目录
$CurrentPath = Get-Location
Write-Host "当前工作目录: $CurrentPath" -ForegroundColor Cyan

# 设置路径
$DataFlarePath = $CurrentPath
$ExamplesPath = Join-Path $CurrentPath "examples"
$OutputDir = Join-Path $ExamplesPath "data"

# 检查输出目录是否存在
if (-not (Test-Path $OutputDir)) {
    Write-Host "创建输出目录: $OutputDir" -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

# 检查输入文件是否存在
$InputFilePath = Join-Path $CurrentPath $InputFile
if (-not (Test-Path $InputFilePath)) {
    Write-Host "错误: 输入文件 $InputFilePath 不存在!" -ForegroundColor Red
    Write-Host "正在创建测试数据文件..." -ForegroundColor Yellow

    # 创建测试数据
    $TestData = @()
    $TestData += "id,name,age,city,salary"

    for ($i = 1; $i -le 10000; $i++) {
        $cities = @("北京", "上海", "广州", "深圳", "杭州", "南京", "成都", "武汉")
        $names = @("张三", "李四", "王五", "赵六", "钱七", "孙八", "周九", "吴十")

        $city = $cities | Get-Random
        $name = $names | Get-Random
        $age = Get-Random -Minimum 22 -Maximum 65
        $salary = Get-Random -Minimum 5000 -Maximum 50000

        $TestData += "$i,$name$i,$age,$city,$salary"
    }

    $TestData | Out-File -FilePath $InputFilePath -Encoding UTF8
    Write-Host "已创建测试数据文件: $InputFilePath ($(($TestData | Measure-Object).Count - 1) 条记录)" -ForegroundColor Green
}

# 检查输入文件大小
$InputSize = (Get-Content $InputFilePath | Measure-Object).Count - 1  # 减去标题行
Write-Host "输入文件行数: $InputSize" -ForegroundColor Cyan

# 检查工作流文件是否存在
$WorkflowPath = Join-Path $CurrentPath $WorkflowFile
if (-not (Test-Path $WorkflowPath)) {
    Write-Host "错误: 工作流文件 $WorkflowPath 不存在!" -ForegroundColor Red
    Write-Host "正在创建示例工作流文件..." -ForegroundColor Yellow

    # 创建示例工作流
    $WorkflowContent = @"
name: "性能测试工作流"
description: "用于测试DataFlare性能的工作流"

sources:
  - name: "csv_source"
    type: "csv"
    config:
      file_path: "$($InputFile.Replace('\', '/'))"
      has_header: true
      delimiter: ","

processors:
  - name: "filter_processor"
    type: "filter"
    config:
      condition: "age > 25"

  - name: "mapping_processor"
    type: "mapping"
    config:
      mappings:
        - source: "salary"
          target: "annual_salary"
          expression: "salary * 12"

destinations:
  - name: "csv_destination"
    type: "csv"
    config:
      file_path: "$($OutputFile.Replace('\', '/'))"
      has_header: true
      delimiter: ","

workflow:
  - source: "csv_source"
    processors: ["filter_processor", "mapping_processor"]
    destination: "csv_destination"
"@

    # 确保工作流目录存在
    $WorkflowDir = Split-Path $WorkflowPath -Parent
    if (-not (Test-Path $WorkflowDir)) {
        New-Item -ItemType Directory -Path $WorkflowDir -Force | Out-Null
    }

    $WorkflowContent | Out-File -FilePath $WorkflowPath -Encoding UTF8
    Write-Host "已创建示例工作流文件: $WorkflowPath" -ForegroundColor Green
}

# 记录开始时间
Write-Host "`n开始测试..." -ForegroundColor Green
Write-Host ""

Write-Host "测试工作流: $WorkflowFile ($InputSize 行数据)" -ForegroundColor Cyan
Write-Host "------------------------------------------------" -ForegroundColor Gray

$TotalTime = 0
$SuccessfulRuns = 0

for ($i = 1; $i -le $Runs; $i++) {
    Write-Host "运行 #${i}:" -ForegroundColor Yellow

    # 记录开始时间
    $StartTime = Get-Date

    # 确保输出文件不存在
    $OutputFilePath = Join-Path $CurrentPath $OutputFile
    if (Test-Path $OutputFilePath) {
        Write-Host "删除先前的输出文件" -ForegroundColor Gray
        Remove-Item $OutputFilePath -Force
    }

    # 构建执行命令
    $Command = "cargo"
    $Arguments = @("run", "--release", "-p", "dataflare-cli", "--", "execute", "-f", "`"$WorkflowPath`"")

    Write-Host "执行命令: $Command $($Arguments -join ' ')" -ForegroundColor Gray

    # 执行工作流
    try {
        $Process = Start-Process -FilePath $Command -ArgumentList $Arguments -WorkingDirectory $DataFlarePath -Wait -PassThru -NoNewWindow -RedirectStandardOutput "temp_output.log" -RedirectStandardError "temp_error.log"

        $ExitCode = $Process.ExitCode

        if ($ExitCode -eq 0) {
            Write-Host "执行成功" -ForegroundColor Green
            $SuccessfulRuns++

            # 检查输出文件是否存在
            if (Test-Path $OutputFilePath) {
                $OutputSize = (Get-Content $OutputFilePath | Measure-Object).Count - 1  # 减去标题行
                $FileSize = (Get-Item $OutputFilePath).Length

                Write-Host "输出文件已创建: $OutputFilePath" -ForegroundColor Green
                Write-Host "- 大小: $FileSize 字节" -ForegroundColor Gray
                Write-Host "- 行数: $OutputSize 行" -ForegroundColor Gray

                if ($InputSize -gt 0) {
                    $Percentage = [math]::Round(($OutputSize / $InputSize) * 100, 2)
                    Write-Host "- 比例: $Percentage% 的输入记录" -ForegroundColor Gray
                }

                # 显示输出文件前5行
                Write-Host "输出文件前5行:" -ForegroundColor Gray
                $SampleLines = Get-Content $OutputFilePath -TotalCount 5
                $SampleLines | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
            } else {
                Write-Host "警告: 输出文件未创建!" -ForegroundColor Yellow

                # 查找可能的输出文件
                Write-Host "查找可能的输出文件..." -ForegroundColor Gray
                $PossibleFiles = Get-ChildItem -Path $OutputDir -Filter "*.csv" -Recurse | Where-Object { $_.LastWriteTime -gt (Get-Date).AddMinutes(-5) }
                if ($PossibleFiles) {
                    Write-Host "找到最近创建的CSV文件:" -ForegroundColor Yellow
                    $PossibleFiles | ForEach-Object { Write-Host "  $($_.FullName)" -ForegroundColor DarkYellow }
                }
            }
        } else {
            Write-Host "执行失败，退出代码: $ExitCode" -ForegroundColor Red

            # 显示错误信息
            if (Test-Path "temp_error.log") {
                $ErrorContent = Get-Content "temp_error.log" -Raw
                if ($ErrorContent) {
                    Write-Host "错误信息:" -ForegroundColor Red
                    Write-Host $ErrorContent -ForegroundColor DarkRed
                }
            }
        }

        # 清理临时文件
        if (Test-Path "temp_output.log") { Remove-Item "temp_output.log" -Force }
        if (Test-Path "temp_error.log") { Remove-Item "temp_error.log" -Force }

    } catch {
        Write-Host "执行过程中发生异常: $($_.Exception.Message)" -ForegroundColor Red
        continue
    }

    # 记录结束时间
    $EndTime = Get-Date

    # 计算执行时间
    $ExecutionTime = ($EndTime - $StartTime).TotalSeconds
    Write-Host "执行时间: $([math]::Round($ExecutionTime, 3)) 秒" -ForegroundColor Cyan
    Write-Host ""

    # 累计总时间
    if ($ExitCode -eq 0) {
        $TotalTime += $ExecutionTime
    }
}

# 计算统计信息
if ($SuccessfulRuns -gt 0) {
    $AvgTime = [math]::Round($TotalTime / $SuccessfulRuns, 3)
    Write-Host "成功运行次数: $SuccessfulRuns / $Runs" -ForegroundColor Green
    Write-Host "平均执行时间: $AvgTime 秒" -ForegroundColor Cyan

    if ($AvgTime -gt 0 -and $InputSize -gt 0) {
        $RecordsPerSecond = [math]::Round($InputSize / $AvgTime, 0)
        Write-Host "处理速度: $RecordsPerSecond 记录/秒" -ForegroundColor Cyan
    }
} else {
    Write-Host "所有运行都失败了!" -ForegroundColor Red
}

Write-Host "================================================" -ForegroundColor Green
Write-Host "测试完成!" -ForegroundColor Green
