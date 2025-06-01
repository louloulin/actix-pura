# DataFlare Performance Test - Windows PowerShell Version
param(
    [int]$Runs = 3,
    [string]$WorkflowFile = "examples\workflows\performance_test.yaml",
    [string]$InputFile = "examples\data\large_test.csv",
    [string]$OutputFile = "examples\data\performance_output.csv"
)

Write-Host "DataFlare Performance Test" -ForegroundColor Green
Write-Host "================================================" -ForegroundColor Green
Write-Host "Test Date: $(Get-Date)" -ForegroundColor Yellow
Write-Host "Test Environment: $($env:COMPUTERNAME) - $($env:OS)" -ForegroundColor Yellow
Write-Host "PowerShell Version: $($PSVersionTable.PSVersion)" -ForegroundColor Yellow
Write-Host "================================================" -ForegroundColor Green

# Get current directory
$CurrentPath = Get-Location
Write-Host "Current Working Directory: $CurrentPath" -ForegroundColor Cyan

# Set paths
$DataFlarePath = $CurrentPath
$ExamplesPath = Join-Path $CurrentPath "examples"
$OutputDir = Join-Path $ExamplesPath "data"

# Check if output directory exists
if (-not (Test-Path $OutputDir)) {
    Write-Host "Creating output directory: $OutputDir" -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

# Check if input file exists
$InputFilePath = Join-Path $CurrentPath $InputFile
if (-not (Test-Path $InputFilePath)) {
    Write-Host "Error: Input file $InputFilePath does not exist!" -ForegroundColor Red
    Write-Host "Creating test data file..." -ForegroundColor Yellow
    
    # Create test data
    $TestData = @()
    $TestData += "id,name,age,city,salary"
    
    for ($i = 1; $i -le 10000; $i++) {
        $cities = @("Beijing", "Shanghai", "Guangzhou", "Shenzhen", "Hangzhou", "Nanjing", "Chengdu", "Wuhan")
        $names = @("Zhang", "Li", "Wang", "Zhao", "Qian", "Sun", "Zhou", "Wu")
        
        $city = $cities | Get-Random
        $name = $names | Get-Random
        $age = Get-Random -Minimum 22 -Maximum 65
        $salary = Get-Random -Minimum 5000 -Maximum 50000
        
        $TestData += "$i,$name$i,$age,$city,$salary"
    }
    
    $TestData | Out-File -FilePath $InputFilePath -Encoding UTF8
    Write-Host "Created test data file: $InputFilePath ($(($TestData | Measure-Object).Count - 1) records)" -ForegroundColor Green
}

# Check input file size
$InputSize = (Get-Content $InputFilePath | Measure-Object).Count - 1  # Subtract header line
Write-Host "Input file lines: $InputSize" -ForegroundColor Cyan

# Check if workflow file exists
$WorkflowPath = Join-Path $CurrentPath $WorkflowFile
if (-not (Test-Path $WorkflowPath)) {
    Write-Host "Error: Workflow file $WorkflowPath does not exist!" -ForegroundColor Red
    Write-Host "Creating sample workflow file..." -ForegroundColor Yellow
    
    # Create sample workflow
    $WorkflowContent = @"
name: "Performance Test Workflow"
description: "Workflow for testing DataFlare performance"

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
    
    # Ensure workflow directory exists
    $WorkflowDir = Split-Path $WorkflowPath -Parent
    if (-not (Test-Path $WorkflowDir)) {
        New-Item -ItemType Directory -Path $WorkflowDir -Force | Out-Null
    }
    
    $WorkflowContent | Out-File -FilePath $WorkflowPath -Encoding UTF8
    Write-Host "Created sample workflow file: $WorkflowPath" -ForegroundColor Green
}

# Start testing
Write-Host "`nStarting tests..." -ForegroundColor Green
Write-Host ""

Write-Host "Testing workflow: $WorkflowFile ($InputSize lines of data)" -ForegroundColor Cyan
Write-Host "------------------------------------------------" -ForegroundColor Gray

$TotalTime = 0
$SuccessfulRuns = 0

for ($i = 1; $i -le $Runs; $i++) {
    Write-Host "Run #$i" -ForegroundColor Yellow
    
    # Record start time
    $StartTime = Get-Date
    
    # Ensure output file doesn't exist
    $OutputFilePath = Join-Path $CurrentPath $OutputFile
    if (Test-Path $OutputFilePath) {
        Write-Host "Removing previous output file" -ForegroundColor Gray
        Remove-Item $OutputFilePath -Force
    }
    
    # Build execution command
    $Command = "cargo"
    $Arguments = @("run", "--release", "-p", "dataflare-cli", "--", "execute", "-f", "`"$WorkflowPath`"")
    
    Write-Host "Executing command: $Command $($Arguments -join ' ')" -ForegroundColor Gray
    
    # Execute workflow
    try {
        $Process = Start-Process -FilePath $Command -ArgumentList $Arguments -WorkingDirectory $DataFlarePath -Wait -PassThru -NoNewWindow
        
        $ExitCode = $Process.ExitCode
        
        if ($ExitCode -eq 0) {
            Write-Host "Execution successful" -ForegroundColor Green
            $SuccessfulRuns++
            
            # Check if output file exists
            if (Test-Path $OutputFilePath) {
                $OutputSize = (Get-Content $OutputFilePath | Measure-Object).Count - 1  # Subtract header line
                $FileSize = (Get-Item $OutputFilePath).Length
                
                Write-Host "Output file created: $OutputFilePath" -ForegroundColor Green
                Write-Host "- Size: $FileSize bytes" -ForegroundColor Gray
                Write-Host "- Lines: $OutputSize lines" -ForegroundColor Gray
                
                if ($InputSize -gt 0) {
                    $Percentage = [math]::Round(($OutputSize / $InputSize) * 100, 2)
                    Write-Host "- Ratio: $Percentage% of input records" -ForegroundColor Gray
                }
                
                # Show first 5 lines of output file
                Write-Host "First 5 lines of output file:" -ForegroundColor Gray
                $SampleLines = Get-Content $OutputFilePath -TotalCount 5
                $SampleLines | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
            } else {
                Write-Host "Warning: Output file not created!" -ForegroundColor Yellow
                
                # Look for possible output files
                Write-Host "Looking for possible output files..." -ForegroundColor Gray
                $PossibleFiles = Get-ChildItem -Path $OutputDir -Filter "*.csv" -Recurse | Where-Object { $_.LastWriteTime -gt (Get-Date).AddMinutes(-5) }
                if ($PossibleFiles) {
                    Write-Host "Found recently created CSV files:" -ForegroundColor Yellow
                    $PossibleFiles | ForEach-Object { Write-Host "  $($_.FullName)" -ForegroundColor DarkYellow }
                }
            }
        } else {
            Write-Host "Execution failed with exit code: $ExitCode" -ForegroundColor Red
        }
        
    } catch {
        Write-Host "Exception occurred during execution: $($_.Exception.Message)" -ForegroundColor Red
        continue
    }
    
    # Record end time
    $EndTime = Get-Date
    
    # Calculate execution time
    $ExecutionTime = ($EndTime - $StartTime).TotalSeconds
    Write-Host "Execution time: $([math]::Round($ExecutionTime, 3)) seconds" -ForegroundColor Cyan
    Write-Host ""
    
    # Add to total time
    if ($ExitCode -eq 0) {
        $TotalTime += $ExecutionTime
    }
}

# Calculate statistics
if ($SuccessfulRuns -gt 0) {
    $AvgTime = [math]::Round($TotalTime / $SuccessfulRuns, 3)
    Write-Host "Successful runs: $SuccessfulRuns / $Runs" -ForegroundColor Green
    Write-Host "Average execution time: $AvgTime seconds" -ForegroundColor Cyan
    
    if ($AvgTime -gt 0 -and $InputSize -gt 0) {
        $RecordsPerSecond = [math]::Round($InputSize / $AvgTime, 0)
        Write-Host "Processing speed: $RecordsPerSecond records/second" -ForegroundColor Cyan
    }
} else {
    Write-Host "All runs failed!" -ForegroundColor Red
}

Write-Host "================================================" -ForegroundColor Green
Write-Host "Test completed!" -ForegroundColor Green
