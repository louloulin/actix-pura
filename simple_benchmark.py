#!/usr/bin/env python3
"""
Simple standalone benchmark to simulate the DataFlare batch processing functionality.
This script doesn't rely on the Rust codebase and can be run independently.
"""

import time
import json
import datetime
import random
import os
import psutil
import gc
import threading
import multiprocessing
import statistics
from dataclasses import dataclass
from typing import List, Dict, Any, Tuple, Optional

# 获取当前进程以监控资源使用
process = psutil.Process(os.getpid())


@dataclass
class DataRecord:
    """Represents a single data record"""
    data: Dict[str, Any]
    
    def get(self, key, default=None):
        """Get a value from the data dictionary"""
        return self.data.get(key, default)


@dataclass
class DataRecordBatch:
    """Represents a batch of data records"""
    records: List[DataRecord]
    
    def __len__(self):
        return len(self.records)


@dataclass
class BenchmarkResult:
    """Benchmark result data"""
    batch_size: int
    throughput: float
    latency: float
    memory_delta: float
    cpu_percent: float
    latency_p95: Optional[float] = None
    latency_p99: Optional[float] = None


def get_memory_usage_mb():
    """获取当前内存使用(MB)"""
    return process.memory_info().rss / 1024 / 1024


def create_test_batch(size: int) -> DataRecordBatch:
    """Create a test batch of records"""
    records = []
    
    for i in range(size):
        record = DataRecord(data={
            "id": i,
            "name": f"test{i}",
            "value": i * 10,
            "timestamp": datetime.datetime.now().isoformat(),
            "data": "这是一些示例数据，足够大以模拟真实场景" * (i % 5 + 1),
        })
        
        records.append(record)
    
    return DataRecordBatch(records=records)


def test_batch_process(batch: DataRecordBatch) -> DataRecordBatch:
    """Process a batch of records"""
    processed_records = []
    
    for record in batch.records:
        # Get values from the record
        id_value = record.get("id", 0)
        value = record.get("value", 0)
        
        # Create a new processed record
        processed_record = DataRecord(data={
            "id": id_value,
            "processed_value": value * 2,
            "status": "processed",
            "timestamp": datetime.datetime.now().isoformat(),
        })
        
        processed_records.append(processed_record)
    
    return DataRecordBatch(records=processed_records)


def benchmark_throughput(batch_size: int, total_records: int) -> BenchmarkResult:
    """Benchmark throughput with no backpressure"""
    # 强制GC收集以稳定内存测量
    gc.collect()
    
    # 预热CPU测量
    process.cpu_percent()
    time.sleep(0.1)
    
    start_memory = get_memory_usage_mb()
    start_time = time.time()
    
    iterations = total_records // batch_size
    total_processed = 0
    batch_latencies = []
    
    for _ in range(iterations):
        # 记录单批次处理的延迟
        batch_start = time.time()
        
        batch = create_test_batch(batch_size)
        processed = test_batch_process(batch)
        
        batch_end = time.time()
        batch_latency = (batch_end - batch_start) * 1000  # 转换为毫秒
        batch_latencies.append(batch_latency)
        
        total_processed += len(processed)
    
    elapsed = time.time() - start_time
    cpu_percent = process.cpu_percent()
    end_memory = get_memory_usage_mb()
    
    throughput = total_processed / elapsed
    memory_delta = end_memory - start_memory
    
    # 计算延迟分位数
    if batch_latencies:
        batch_latencies.sort()
        idx_95 = int(len(batch_latencies) * 0.95)
        idx_99 = int(len(batch_latencies) * 0.99)
        latency_p95 = batch_latencies[idx_95]
        latency_p99 = batch_latencies[idx_99]
    else:
        latency_p95 = latency_p99 = 0
        
    return BenchmarkResult(
        batch_size=batch_size,
        throughput=throughput,
        latency=elapsed * 1000 / iterations,  # 平均每批次处理延迟(毫秒)
        memory_delta=memory_delta,
        cpu_percent=cpu_percent,
        latency_p95=latency_p95,
        latency_p99=latency_p99
    )


def benchmark_parallel(batch_size: int, total_records: int, num_threads: int) -> BenchmarkResult:
    """使用多线程进行批处理测试"""
    gc.collect()
    process.cpu_percent()
    time.sleep(0.1)
    
    start_memory = get_memory_usage_mb()
    start_time = time.time()
    
    records_per_thread = total_records // num_threads
    threads = []
    all_latencies = []
    latency_locks = threading.Lock()
    
    def worker():
        local_latencies = []
        iterations = records_per_thread // batch_size
        for _ in range(iterations):
            batch_start = time.time()
            
            batch = create_test_batch(batch_size)
            processed = test_batch_process(batch)
            
            batch_end = time.time()
            batch_latency = (batch_end - batch_start) * 1000
            local_latencies.append(batch_latency)
        
        with latency_locks:
            all_latencies.extend(local_latencies)
    
    for _ in range(num_threads):
        t = threading.Thread(target=worker)
        threads.append(t)
        t.start()
    
    for t in threads:
        t.join()
    
    elapsed = time.time() - start_time
    cpu_percent = process.cpu_percent()
    end_memory = get_memory_usage_mb()
    
    # 每个线程处理 (records_per_thread // batch_size) 批次
    # 总共处理的批次数
    total_batches = num_threads * (records_per_thread // batch_size)
    total_processed = total_batches * batch_size
    
    throughput = total_processed / elapsed
    memory_delta = end_memory - start_memory
    
    # 计算延迟分位数
    if all_latencies:
        all_latencies.sort()
        idx_95 = int(len(all_latencies) * 0.95)
        idx_99 = int(len(all_latencies) * 0.99)
        latency_p95 = all_latencies[idx_95]
        latency_p99 = all_latencies[idx_99]
    else:
        latency_p95 = latency_p99 = 0
    
    return BenchmarkResult(
        batch_size=batch_size,
        throughput=throughput,
        latency=sum(all_latencies) / len(all_latencies) if all_latencies else 0,
        memory_delta=memory_delta,
        cpu_percent=cpu_percent,
        latency_p95=latency_p95,
        latency_p99=latency_p99
    )


def main():
    """Main function to run benchmarks"""
    print("========== DataFlare 批处理性能基准测试 ==========")
    
    batch_sizes = [100, 500, 1000, 5000, 10000, 50000]
    total_records = 500000
    
    # 1. 单线程吞吐量测试
    print("\n1. 单线程批处理吞吐量测试")
    print("-" * 100)
    print("批大小\t\t吞吐量(记录/秒)\t平均延迟(ms)\t95%延迟(ms)\t99%延迟(ms)\t内存增长(MB)\tCPU使用率(%)")
    print("-" * 100)
    
    results = []
    for batch_size in batch_sizes:
        result = benchmark_throughput(batch_size, total_records)
        results.append(result)
        print(f"{result.batch_size}\t\t{result.throughput:.2f}\t\t{result.latency:.2f}\t\t{result.latency_p95:.2f}\t\t{result.latency_p99:.2f}\t\t{result.memory_delta:.2f}\t\t{result.cpu_percent:.2f}")
    
    # 找出最佳批大小（基于吞吐量）
    best_result = max(results, key=lambda r: r.throughput)
    print("-" * 100)
    print(f"最佳批大小: {best_result.batch_size}，吞吐量: {best_result.throughput:.2f} 记录/秒")
    print()
    
    # 2. 多线程吞吐量测试
    thread_counts = [2, 4, 8]
    best_batch_size = best_result.batch_size
    
    print("\n2. 多线程批处理吞吐量测试 (使用最佳批大小)")
    print("-" * 100)
    print("线程数\t\t吞吐量(记录/秒)\t平均延迟(ms)\t95%延迟(ms)\t99%延迟(ms)\t内存增长(MB)\tCPU使用率(%)")
    print("-" * 100)
    
    # 存储单线程最佳结果，用于计算多线程提升比例
    single_thread_best = best_result.throughput
    
    for thread_count in thread_counts:
        result = benchmark_parallel(best_batch_size, total_records, thread_count)
        speedup = result.throughput / single_thread_best
        print(f"{thread_count}\t\t{result.throughput:.2f}\t\t{result.latency:.2f}\t\t{result.latency_p95:.2f}\t\t{result.latency_p99:.2f}\t\t{result.memory_delta:.2f}\t\t{result.cpu_percent:.2f}\t(提升: {speedup:.2f}x)")
    
    print("-" * 100)
    
    # 找出最佳多线程配置
    parallel_results = []
    for thread_count in thread_counts:
        result = benchmark_parallel(best_batch_size, total_records, thread_count)
        parallel_results.append((thread_count, result))
    
    best_thread_count, best_parallel = max(parallel_results, key=lambda x: x[1].throughput)
    
    print("\n性能分析总结:")
    print(f"1. 单线程最佳批大小: {best_result.batch_size}，提供最高吞吐量: {best_result.throughput:.2f} 记录/秒")
    print(f"2. 内存使用与批大小呈正相关关系，批大小为 {best_result.batch_size} 时内存增长 {best_result.memory_delta:.2f} MB")
    print(f"3. 批大小对CPU使用率影响相对较小，所有测试CPU使用率都在 {min(r.cpu_percent for r in results):.1f}% - {max(r.cpu_percent for r in results):.1f}% 之间")
    
    # 检查是否有多线程提升
    if best_parallel.throughput > best_result.throughput:
        print(f"4. 多线程处理可将吞吐量提高到 {best_parallel.throughput:.2f} 记录/秒，使用 {best_thread_count} 线程 (相比单线程提升 {best_parallel.throughput / single_thread_best:.2f}x)")
    else:
        print(f"4. 多线程处理未提供性能提升，最佳配置仍为单线程 (多线程最高吞吐量: {best_parallel.throughput:.2f} 记录/秒)")
        print("   备注: 这可能是因为Python的全局解释器锁(GIL)限制了并行执行效率")
    
    print("\n测试完成！")


if __name__ == "__main__":
    main() 