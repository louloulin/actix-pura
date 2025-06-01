@echo off
REM JSON Transformer Plugin Build Script for Windows

echo 🔨 Building JSON Transformer WASM Plugin...

REM 检查是否安装了wasm-pack
where wasm-pack >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo ❌ wasm-pack not found. Please install it first:
    echo    cargo install wasm-pack
    exit /b 1
)

REM 构建WASM包
echo 📦 Building WASM package...
wasm-pack build --target web --out-dir pkg --release
if %ERRORLEVEL% NEQ 0 (
    echo ❌ Build failed
    exit /b 1
)

REM 创建输出目录
if not exist "..\..\..\dataflare\plugins" mkdir "..\..\..\dataflare\plugins"

REM 复制WASM文件
echo 📋 Copying WASM files...
copy pkg\json_transformer_plugin.wasm ..\..\..\dataflare\plugins\json-transformer.wasm
copy pkg\json_transformer_plugin.js ..\..\..\dataflare\plugins\json-transformer.js

REM 获取文件大小
for %%A in (..\..\..\dataflare\plugins\json-transformer.wasm) do set filesize=%%~zA

REM 创建插件元数据文件
echo 📝 Creating plugin metadata...
(
echo {
echo   "name": "json-transformer",
echo   "version": "1.0.0",
echo   "description": "Advanced JSON transformation plugin with JSONPath support",
echo   "author": "DataFlare Team",
echo   "license": "MIT",
echo   "plugin_type": "transformer",
echo   "language": "rust",
echo   "wasm_file": "json-transformer.wasm",
echo   "js_file": "json-transformer.js",
echo   "capabilities": {
echo     "supported_functions": [
echo       {
echo         "name": "transform",
echo         "description": "Transform JSON data using JSONPath rules",
echo         "input_type": "json",
echo         "output_type": "json"
echo       },
echo       {
echo         "name": "validate",
echo         "description": "Validate JSON data structure",
echo         "input_type": "json",
echo         "output_type": "boolean"
echo       }
echo     ],
echo     "supported_formats": ["json"],
echo     "max_memory_mb": 16,
echo     "timeout_seconds": 30
echo   },
echo   "dependencies": ["serde", "jsonpath"],
echo   "compatibility": ["dataflare-4.0"],
echo   "size_bytes": %filesize%,
echo   "checksum": "sha256:placeholder"
echo }
) > ..\..\..\dataflare\plugins\json-transformer.json

echo ✅ JSON Transformer plugin built successfully!
echo 📍 Plugin files:
echo    - WASM: dataflare\plugins\json-transformer.wasm
echo    - JS:   dataflare\plugins\json-transformer.js
echo    - Meta: dataflare\plugins\json-transformer.json

REM 显示文件大小
echo 📊 File sizes:
dir ..\..\..\dataflare\plugins\json-transformer.* /s
