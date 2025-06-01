@echo off
setlocal enabledelayedexpansion

echo ========================================
echo DataFlare CLI 500K Records Performance Test
echo ========================================
echo.

:: Check if we're in the right directory
if not exist "dataflare" (
    echo Error: Please run this script from the project root directory
    echo Current directory: %CD%
    echo Expected: directory containing 'dataflare' folder
    pause
    exit /b 1
)

:: Create necessary directories
echo Creating necessary directories...
if not exist "examples\data" mkdir "examples\data"
if not exist "dataflare\output" mkdir "dataflare\output"

:: Step 1: Generate test data
echo.
echo Step 1: Generating 500K test records...
echo This may take a few minutes, please wait...

powershell -ExecutionPolicy Bypass -File "generate_test_data.ps1" -RecordCount 500000 -OutputFile "examples\data\extra_large_test.csv"

if %ERRORLEVEL% neq 0 (
    echo Error: Test data generation failed
    pause
    exit /b 1
)

echo Test data generation completed successfully

:: Check if test data file exists
if not exist "examples\data\extra_large_test.csv" (
    echo Error: Test data file not found
    pause
    exit /b 1
)

:: Display file info
for %%F in ("examples\data\extra_large_test.csv") do (
    echo Test data file size: %%~zF bytes
)

:: Step 2: Build DataFlare CLI
echo.
echo Step 2: Building DataFlare CLI...
cargo build --release -p dataflare-cli

if %ERRORLEVEL% neq 0 (
    echo Error: DataFlare CLI build failed
    pause
    exit /b 1
)

echo DataFlare CLI build completed successfully

:: Step 3: Validate workflow
echo.
echo Step 3: Validating workflow configuration...
target\release\dataflare.exe validate -f examples\workflows\extra_large_performance_test.yaml

if %ERRORLEVEL% neq 0 (
    echo Error: Workflow validation failed
    pause
    exit /b 1
)

echo Workflow validation completed successfully

:: Step 4: Execute workflow
echo.
echo Step 4: Executing 500K records performance test via DataFlare CLI...
echo Configuration:
echo   - Data size: 500,000 records
echo   - Input file: examples\data\extra_large_test.csv
echo   - Output file: dataflare\output\extra_large_performance_output.csv
echo   - Workflow: examples\workflows\extra_large_performance_test.yaml
echo.

echo Start time: %time%

:: Execute the workflow
target\release\dataflare.exe execute -f examples\workflows\extra_large_performance_test.yaml

set test_result=%ERRORLEVEL%
echo End time: %time%

if %test_result% neq 0 (
    echo Error: Workflow execution failed
    echo.
    echo Troubleshooting tips:
    echo   - Check if input file exists and is readable
    echo   - Verify output directory permissions
    echo   - Check DataFlare CLI logs for detailed error information
    echo.
    pause
    exit /b 1
)

echo Workflow execution completed successfully

:: Step 5: Verify results
echo.
echo Step 5: Verifying results...

if exist "dataflare\output\extra_large_performance_output.csv" (
    echo Output file created successfully

    :: Display file information
    for %%F in ("dataflare\output\extra_large_performance_output.csv") do (
        echo Output file size: %%~zF bytes
        echo Output file path: %%~fF
    )

    :: Display file content preview using PowerShell
    echo.
    echo File content preview:
    powershell -Command "& { $file = 'dataflare\output\extra_large_performance_output.csv'; if (Test-Path $file) { $lines = Get-Content $file; $lineCount = $lines.Count; Write-Host \"Total lines: $lineCount\" -ForegroundColor Green; Write-Host \"\"; Write-Host \"First 5 lines:\" -ForegroundColor Yellow; $lines | Select-Object -First 5 | ForEach-Object { Write-Host \"   $_\" -ForegroundColor Gray }; if ($lineCount -gt 10) { Write-Host \"   ...\" -ForegroundColor Gray; Write-Host \"Last 2 lines:\" -ForegroundColor Yellow; $lines | Select-Object -Last 2 | ForEach-Object { Write-Host \"   $_\" -ForegroundColor Gray } } } }"

    echo.
    echo Performance test completed successfully!
) else (
    echo Warning: Output file not found
    echo Expected: dataflare\output\extra_large_performance_output.csv
    echo.
    echo Possible issues:
    echo   - Workflow execution may have failed silently
    echo   - Output path configuration issue
    echo   - Insufficient disk space or permissions
)

:: Optional cleanup
echo.
set /p cleanup="Delete test data file to save space? (y/N): "
if /i "%cleanup%"=="y" (
    del "examples\data\extra_large_test.csv"
    echo Test data file deleted
)

:: Summary
echo.
echo ========================================
echo Performance Test Summary
echo ========================================
echo Test Type: DataFlare CLI Execution
echo Data Size: 500,000 records
echo Input File: examples\data\extra_large_test.csv
echo Output File: dataflare\output\extra_large_performance_output.csv
echo Workflow: examples\workflows\extra_large_performance_test.yaml
echo.
echo Next steps:
echo   - Analyze output file for data quality
echo   - Compare performance with unit test results
echo   - Test with different data sizes
echo.
echo Other available tests:
echo   - Unit tests: cargo test -p dataflare-runtime --test performance_test
echo   - CLI validation: target\release\dataflare-cli.exe validate -f [workflow.yaml]
echo   - CLI execution: target\release\dataflare-cli.exe execute -f [workflow.yaml]
echo.
pause
