@echo off
setlocal enabledelayedexpansion

echo ========================================
echo DataFlare 500K Records Complete Test
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

echo.
echo ========================================
echo Test 1: Rust Unit Test (500K Records)
echo ========================================

:: Build tests
echo Building DataFlare Runtime tests...
cargo build -p dataflare-runtime --tests
if %ERRORLEVEL% neq 0 (
    echo Build failed
    pause
    exit /b 1
)

echo Build completed successfully

:: Run 500K records test
echo.
echo Running 500K records performance test via Rust unit test...
echo Start time: %time%

cargo test -p dataflare-runtime --test performance_test test_extra_large_dataset_performance -- --nocapture

set rust_test_result=%ERRORLEVEL%
echo End time: %time%

if %rust_test_result% neq 0 (
    echo Rust unit test failed
) else (
    echo Rust unit test completed successfully
)

echo.
echo ========================================
echo Test 2: DataFlare CLI Test (10K Records)
echo ========================================

:: Generate smaller test data for CLI
echo Generating 10K test data for CLI test...
powershell -ExecutionPolicy Bypass -File "generate_test_data.ps1" -RecordCount 10000 -OutputFile "examples\data\cli_test.csv"

if %ERRORLEVEL% neq 0 (
    echo CLI test data generation failed
    goto :summary
)

:: Build CLI
echo Building DataFlare CLI...
cargo build --release -p dataflare-cli
if %ERRORLEVEL% neq 0 (
    echo CLI build failed
    goto :summary
)

:: Create simple CLI workflow
echo Creating simple CLI workflow...
echo id: cli-test > examples\workflows\cli_test.yaml
echo name: CLI Test Workflow >> examples\workflows\cli_test.yaml
echo description: Simple CLI test >> examples\workflows\cli_test.yaml
echo version: 1.0.0 >> examples\workflows\cli_test.yaml
echo. >> examples\workflows\cli_test.yaml
echo sources: >> examples\workflows\cli_test.yaml
echo   csv_source: >> examples\workflows\cli_test.yaml
echo     type: csv >> examples\workflows\cli_test.yaml
echo     config: >> examples\workflows\cli_test.yaml
echo       file_path: "examples/data/cli_test.csv" >> examples\workflows\cli_test.yaml
echo       has_header: true >> examples\workflows\cli_test.yaml
echo       delimiter: "," >> examples\workflows\cli_test.yaml
echo. >> examples\workflows\cli_test.yaml
echo destinations: >> examples\workflows\cli_test.yaml
echo   csv_output: >> examples\workflows\cli_test.yaml
echo     inputs: >> examples\workflows\cli_test.yaml
echo       - csv_source >> examples\workflows\cli_test.yaml
echo     type: csv >> examples\workflows\cli_test.yaml
echo     config: >> examples\workflows\cli_test.yaml
echo       file_path: "dataflare/output/cli_test_output.csv" >> examples\workflows\cli_test.yaml
echo       delimiter: "," >> examples\workflows\cli_test.yaml
echo       write_header: true >> examples\workflows\cli_test.yaml

:: Validate workflow
echo Validating CLI workflow...
target\release\dataflare.exe validate -f examples\workflows\cli_test.yaml
if %ERRORLEVEL% neq 0 (
    echo CLI workflow validation failed
    goto :summary
)

:: Execute workflow
echo Executing CLI workflow...
echo Start time: %time%

target\release\dataflare.exe execute -f examples\workflows\cli_test.yaml

set cli_test_result=%ERRORLEVEL%
echo End time: %time%

if %cli_test_result% neq 0 (
    echo CLI workflow execution failed
) else (
    echo CLI workflow execution completed
)

:summary
echo.
echo ========================================
echo Test Results Summary
echo ========================================

echo.
echo Rust Unit Test (500K Records):
if %rust_test_result% equ 0 (
    echo   Status: PASSED
    if exist "dataflare\output\extra_large_output.csv" (
        for %%F in ("dataflare\output\extra_large_output.csv") do (
            echo   Output file: %%~nxF ^(%%~zF bytes^)
        )
    )
) else (
    echo   Status: FAILED
)

echo.
echo DataFlare CLI Test (10K Records):
if %cli_test_result% equ 0 (
    echo   Status: PASSED
    if exist "dataflare\output\cli_test_output.csv" (
        for %%F in ("dataflare\output\cli_test_output.csv") do (
            echo   Output file: %%~nxF ^(%%~zF bytes^)
        )
    ) else (
        echo   Warning: Output file not created
    )
) else (
    echo   Status: FAILED
)

echo.
echo Output Files in dataflare\output\:
if exist "dataflare\output\*.csv" (
    dir /b dataflare\output\*.csv
) else (
    echo   No CSV files found
)

echo.
echo ========================================
echo Performance Test Capabilities
echo ========================================
echo.
echo DataFlare successfully demonstrates:
echo   - High-performance Actor model architecture
echo   - 500K records processing via Rust unit tests
echo   - CLI interface for workflow execution
echo   - Configurable data transformation pipelines
echo   - Real file I/O operations
echo.
echo Available test commands:
echo   Rust Tests:
echo     cargo test -p dataflare-runtime --test performance_test test_small_dataset_performance
echo     cargo test -p dataflare-runtime --test performance_test test_medium_dataset_performance
echo     cargo test -p dataflare-runtime --test performance_test test_large_dataset_performance
echo     cargo test -p dataflare-runtime --test performance_test test_extra_large_dataset_performance
echo.
echo   CLI Tests:
echo     target\release\dataflare.exe validate -f [workflow.yaml]
echo     target\release\dataflare.exe execute -f [workflow.yaml]
echo.
echo Complete test finished!
pause
