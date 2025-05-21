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

# 检查输出目录是否存在
OUTPUT_DIR="/Users/louloulin/mastra_docs/actix/examples/data"
if [ ! -d "$OUTPUT_DIR" ]; then
    echo "创建输出目录: $OUTPUT_DIR"
    mkdir -p "$OUTPUT_DIR"
fi

# 确保大型测试文件存在
INPUT_FILE="$OUTPUT_DIR/large_test.csv"
if [ ! -f "$INPUT_FILE" ]; then
    echo "错误: 输入文件 $INPUT_FILE 不存在!"
    exit 1
fi

# 检查输入文件大小
INPUT_SIZE=$(wc -l < "$INPUT_FILE")
echo "输入文件行数: $INPUT_SIZE"

# 记录开始时间
echo "开始测试..."
echo ""

echo "测试工作流: performance_test.yaml (${INPUT_SIZE}行数据)"
echo "------------------------------------------------"
total_time=0

for i in $(seq 1 $RUNS); do
    echo "运行 #$i:"
    # 记录开始时间
    start_time=$(date +%s.%N)
    
    # 确保输出文件不存在（如果有的话）
    OUTPUT_FILE="/Users/louloulin/mastra_docs/actix/examples/data/performance_output.csv"
    if [ -f "$OUTPUT_FILE" ]; then
        echo "删除先前的输出文件"
        rm "$OUTPUT_FILE"
    fi
    
    # 执行工作流（使用绝对路径）
    echo "执行命令: cd $DATAFLARE_PATH && RUST_LOG=debug cargo run --release -p dataflare-cli -- execute -f \"$EXAMPLES_PATH/workflows/performance_test.yaml\""
    cd "$DATAFLARE_PATH" && RUST_LOG=debug cargo run --release -p dataflare-cli -- execute -f "$EXAMPLES_PATH/workflows/performance_test.yaml"
    
    # 检查执行结果
    RESULT=$?
    if [ $RESULT -ne 0 ]; then
        echo "执行失败，退出代码: $RESULT"
        exit $RESULT
    fi
    
    # 检查输出文件是否存在
    if [ -f "$OUTPUT_FILE" ]; then
        OUTPUT_SIZE=$(wc -l < "$OUTPUT_FILE")
        echo "输出文件已创建: $OUTPUT_FILE"
        echo "- 大小: $(wc -c < "$OUTPUT_FILE") 字节"
        echo "- 行数: $OUTPUT_SIZE 行"
        echo "- 比例: $(echo "scale=2; $OUTPUT_SIZE/$INPUT_SIZE*100" | bc)% 的输入记录"
        
        # 查看文件内容样本
        echo "输出文件前5行:"
        head -n 5 "$OUTPUT_FILE"
    else
        echo "错误: 输出文件未创建!"
        # 检查可能的临时文件或其他位置
        echo "查找可能的输出文件..."
        find "$DATAFLARE_PATH" -name "performance_output.csv" -type f
        find "$OUTPUT_DIR" -name "*.csv" -type f -newer "$INPUT_FILE"
    fi
    
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
records_per_second=$(echo "scale=0; $INPUT_SIZE / $avg_time" | bc)
echo "处理速度: $records_per_second 记录/秒"
echo "================================================" 