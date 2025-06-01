#!/bin/bash

# JSON Transformer Plugin Build Script

set -e

echo "🔨 Building JSON Transformer WASM Plugin..."

# 检查是否安装了必要的工具
if ! command -v wasm-pack &> /dev/null; then
    echo "❌ wasm-pack not found. Installing..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# 构建WASM包
echo "📦 Building WASM package..."
wasm-pack build --target web --out-dir pkg --release

# 创建输出目录
mkdir -p ../../../dataflare/plugins

# 复制WASM文件
echo "📋 Copying WASM files..."
cp pkg/json_transformer_plugin.wasm ../../../dataflare/plugins/json-transformer.wasm
cp pkg/json_transformer_plugin.js ../../../dataflare/plugins/json-transformer.js

# 创建插件元数据文件
echo "📝 Creating plugin metadata..."
cat > ../../../dataflare/plugins/json-transformer.json << EOF
{
  "name": "json-transformer",
  "version": "1.0.0",
  "description": "Advanced JSON transformation plugin with JSONPath support",
  "author": "DataFlare Team",
  "license": "MIT",
  "plugin_type": "transformer",
  "language": "rust",
  "wasm_file": "json-transformer.wasm",
  "js_file": "json-transformer.js",
  "capabilities": {
    "supported_functions": [
      {
        "name": "transform",
        "description": "Transform JSON data using JSONPath rules",
        "input_type": "json",
        "output_type": "json"
      },
      {
        "name": "validate",
        "description": "Validate JSON data structure",
        "input_type": "json",
        "output_type": "boolean"
      }
    ],
    "supported_formats": ["json"],
    "max_memory_mb": 16,
    "timeout_seconds": 30
  },
  "dependencies": ["serde", "jsonpath"],
  "compatibility": ["dataflare-4.0"],
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "size_bytes": $(stat -c%s ../../../dataflare/plugins/json-transformer.wasm),
  "checksum": "$(sha256sum ../../../dataflare/plugins/json-transformer.wasm | cut -d' ' -f1)"
}
EOF

echo "✅ JSON Transformer plugin built successfully!"
echo "📍 Plugin files:"
echo "   - WASM: dataflare/plugins/json-transformer.wasm"
echo "   - JS:   dataflare/plugins/json-transformer.js"
echo "   - Meta: dataflare/plugins/json-transformer.json"

# 显示文件大小
echo "📊 File sizes:"
ls -lh ../../../dataflare/plugins/json-transformer.*
