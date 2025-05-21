#!/bin/bash

echo "DataFlare 性能测试"
echo "================================================"
echo "测试日期: $(date)"
echo "测试环境: $(uname -a)"
echo "================================================"

# 使用绝对路径
DATAFLARE_PATH="/Users/louloulin/mastra_docs/actix/dataflare"
EXAMPLES_PATH="/Users/louloulin/mastra_docs/actix/examples"

# 测试运行次数
RUNS=3

# 记录开始时间
echo "开始测试..."
echo ""

echo "测试工作流: performance_test.yaml (40,000行数据)"
echo "------------------------------------------------"
total_time=0

for i in $(seq 1 $RUNS); do
    echo "运行 #$i:"
    # 记录开始时间
    start_time=$(date +%s.%N)
    
    # 执行工作流（使用绝对路径）
    cd "$DATAFLARE_PATH" && cargo run --release -p dataflare-cli -- execute -f "$EXAMPLES_PATH/workflows/performance_test.yaml"
    
    # 记录结束时间
    end_time=$(date +%s.%N)
    
    # 计算执行时间
    execution_time=$(echo "$end_time - $start_time" | bc)
    echo "执行时间: $execution_time 秒"
    echo ""
    
    # 累计总时间
    total_time=$(echo "$total_time + $execution_time" | bc)
done

# 计算平均执行时间
avg_time=$(echo "scale=3; $total_time / $RUNS" | bc)
echo "平均执行时间: $avg_time 秒"

# 计算每秒处理记录数
records_per_second=$(echo "scale=0; 40000 / $avg_time" | bc)
echo "处理速度: $records_per_second 记录/秒"
echo "================================================" 