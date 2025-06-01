@echo off
echo ========================================
echo DataFlare 500K Records Performance Test
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

:: Create output directory
echo Creating output directory...
if not exist "dataflare\output" mkdir "dataflare\output"

:: Build tests
echo Building DataFlare Runtime tests...
cargo build -p dataflare-runtime --tests
if %ERRORLEVEL% neq 0 (
    echo Build failed
    pause
    exit /b 1
)

echo Build completed successfully

:: Run extra large dataset performance test
echo.
echo Starting extra large dataset performance test...
echo Test configuration:
echo   - Data size: 500,000 records
echo   - Test type: Rust unit test
echo   - Output directory: dataflare\output\
echo.

echo Start time: %time%

:: Execute test
cargo test -p dataflare-runtime --test performance_test test_extra_large_dataset_performance -- --nocapture

set test_result=%ERRORLEVEL%
echo End time: %time%

if %test_result% neq 0 (
    echo Performance test failed
    echo.
    echo Tips:
    echo   - 500K records is a large dataset
    echo   - If test fails, try smaller datasets first
    echo   - Check available memory and disk space
    echo.
    echo Other available performance tests:
    echo   - Small dataset (1K):   cargo test test_small_dataset_performance
    echo   - Medium dataset (10K): cargo test test_medium_dataset_performance  
    echo   - Large dataset (50K):  cargo test test_large_dataset_performance
    echo.
    pause
    exit /b 1
)

echo Extra large dataset performance test completed successfully

:: Check output file
echo.
echo Checking output results...

if exist "dataflare\output\extra_large_output.csv" (
    echo Output file created successfully
    
    :: Show file info
    for %%F in ("dataflare\output\extra_large_output.csv") do (
        echo Output file size: %%~zF bytes
        echo Output file path: %%~fF
    )
    
    :: Show file preview using PowerShell
    echo.
    echo File content preview:
    powershell -Command "& { $file = 'dataflare\output\extra_large_output.csv'; if (Test-Path $file) { $lines = Get-Content $file; $lineCount = $lines.Count; Write-Host \"Total lines: $lineCount\"; Write-Host \"\"; Write-Host \"First 5 lines:\"; $lines | Select-Object -First 5 | ForEach-Object { Write-Host \"   $_\" }; if ($lineCount -gt 10) { Write-Host \"   ...\"; Write-Host \"Last 2 lines:\"; $lines | Select-Object -Last 2 | ForEach-Object { Write-Host \"   $_\" } } } }"
) else (
    echo Warning: Output file not found, but test may have succeeded
    echo Check test logs for more information
)

:: Option to run full test suite
echo.
set /p run_suite="Run complete performance test suite? (y/N): "
if /i "%run_suite%"=="y" (
    echo.
    echo Running complete performance test suite...
    cargo test -p dataflare-runtime --test performance_test -- --nocapture
)

echo.
echo Performance test completed!
echo.
echo Performance Test Summary:
echo   - Extra large dataset test completed
echo   - Output file saved to: dataflare\output\extra_large_output.csv
echo   - Check test output for detailed performance metrics
echo.
echo Other available performance tests:
echo   cargo test -p dataflare-runtime --test performance_test test_small_dataset_performance
echo   cargo test -p dataflare-runtime --test performance_test test_medium_dataset_performance
echo   cargo test -p dataflare-runtime --test performance_test test_large_dataset_performance
echo.
pause
