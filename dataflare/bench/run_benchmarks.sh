#!/bin/bash
# 批处理系统性能基准测试运行脚本

# 设置数据库连接参数（默认值，可通过环境变量覆盖）
export PG_HOST=${PG_HOST:-localhost}
export PG_PORT=${PG_PORT:-5432}
export PG_DATABASE=${PG_DATABASE:-test_db}
export PG_USERNAME=${PG_USERNAME:-postgres}
export PG_PASSWORD=${PG_PASSWORD:-postgres}
export PG_TABLE=${PG_TABLE:-test_data}

# 设置日志级别
export RUST_LOG=${RUST_LOG:-info}

# 构建项目
echo "Building DataFlare project..."
cargo build --release

# 运行内存批处理基准测试
echo "====================================="
echo "Running memory batch processing benchmark..."
echo "====================================="
cargo run --release --bin batch_benchmark
echo

# 运行PostgreSQL连接器基准测试
echo "====================================="
echo "Running PostgreSQL connector benchmark..."
echo "====================================="
cargo run --release --bin postgres_batch_benchmark
echo

# 如果指定了-g或--generate-data参数，生成测试数据
if [ "$1" = "-g" ] || [ "$1" = "--generate-data" ]; then
    echo "====================================="
    echo "Generating test data in PostgreSQL..."
    echo "====================================="
    cargo run --release --bin generate_test_data -- --count 1000000
    echo
fi

echo "====================================="
echo "All benchmarks completed!"
echo "=====================================" 