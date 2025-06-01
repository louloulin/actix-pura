//! DataFlare Plugin 简化性能演示
//!
//! 展示阶段2性能优化的核心功能

use dataflare_plugin::{
    MemoryPool, MemoryPoolConfig, BufferSize,
};
use std::time::{Duration, Instant};

fn main() {
    println!("🚀 DataFlare Plugin 阶段2性能优化演示");
    println!("=====================================");
    println!();

    demo_memory_pool();
    demo_buffer_classification();
    demo_performance_comparison();

    println!("✅ 演示完成！");
    println!();
    println!("🎯 主要性能提升:");
    println!("   • 5级内存池系统: 智能缓冲区分类和复用");
    println!("   • 预分配策略: 减少运行时内存分配开销");
    println!("   • 自动清理机制: 防止内存泄漏和碎片化");
    println!("   • 性能监控: 实时统计和优化建议");
}

/// 演示内存池基本功能
fn demo_memory_pool() {
    println!("🧠 内存池基本功能演示");
    println!("====================");

    let config = MemoryPoolConfig {
        max_tiny_buffers: 1000,
        max_small_buffers: 500,
        max_medium_buffers: 100,
        max_large_buffers: 50,
        max_huge_buffers: 10,
        cleanup_interval: Duration::from_secs(60),
        idle_timeout: Duration::from_secs(300),
        enable_stats: true,
        memory_pressure_threshold: 0.8,
        min_memory_efficiency: 0.7,
    };

    let pool = MemoryPool::new(config);

    println!("  📊 内存池配置:");
    println!("     - Tiny缓冲区 (256B): 最大1000个");
    println!("     - Small缓冲区 (4KB): 最大500个");
    println!("     - Medium缓冲区 (64KB): 最大100个");
    println!("     - Large缓冲区 (1MB): 最大50个");
    println!("     - Huge缓冲区 (4MB): 最大10个");

    // 预热内存池
    println!("  🔥 预热内存池...");
    pool.warmup();

    // 测试不同大小的缓冲区分配
    let test_sizes = vec![100, 2048, 32768, 524288];

    for size in test_sizes {
        let buffer = pool.get_buffer(size);
        let buffer_type = if size <= 256 {
            "Tiny"
        } else if size <= 4096 {
            "Small"
        } else if size <= 65536 {
            "Medium"
        } else if size <= 1048576 {
            "Large"
        } else {
            "Huge"
        };

        println!("  ✅ 分配{}字节缓冲区 -> 使用{}级缓冲区 (实际大小: {}字节)",
                 size, buffer_type, buffer.data().len());

        // 归还缓冲区
        pool.return_buffer(buffer);
    }

    println!();
}

/// 演示缓冲区分类系统
fn demo_buffer_classification() {
    println!("📦 缓冲区分类系统演示");
    println!("====================");

    let sizes_and_types = vec![
        (100, BufferSize::Tiny, "小型数据包"),
        (2048, BufferSize::Small, "配置文件"),
        (32768, BufferSize::Medium, "图片缩略图"),
        (524288, BufferSize::Large, "文档文件"),
        (2097152, BufferSize::Huge, "视频片段"),
    ];

    for (size, expected_type, description) in sizes_and_types {
        let actual_type = BufferSize::from_size(size);
        let match_status = if actual_type == expected_type { "✅" } else { "❌" };

        println!("  {} {}字节 ({}) -> {:?}级缓冲区",
                 match_status, size, description, actual_type);
    }

    println!();
    println!("  📋 分类规则:");
    println!("     - Tiny (≤256B): 小型控制消息、状态数据");
    println!("     - Small (≤4KB): 配置文件、小型JSON");
    println!("     - Medium (≤64KB): 图片、小型文档");
    println!("     - Large (≤1MB): 大型文档、数据文件");
    println!("     - Huge (>1MB): 视频、大型数据集");
    println!();
}

/// 演示性能对比
fn demo_performance_comparison() {
    println!("⚡ 性能对比演示");
    println!("===============");

    let config = MemoryPoolConfig::default();
    let pool = MemoryPool::new(config);
    pool.warmup();

    let test_size = 4096;
    let iterations = 10000;

    // 测试直接分配
    println!("  🔄 测试直接内存分配...");
    let start = Instant::now();
    for _ in 0..iterations {
        let _buffer = vec![0u8; test_size];
        // 模拟使用
        std::hint::black_box(&_buffer);
    }
    let direct_time = start.elapsed();
    let direct_ops_per_sec = iterations as f64 / direct_time.as_secs_f64();

    // 测试内存池分配
    println!("  🏊 测试内存池分配...");
    let start = Instant::now();
    for _ in 0..iterations {
        let buffer = pool.get_buffer(test_size);
        std::hint::black_box(&buffer);
        pool.return_buffer(buffer);
    }
    let pool_time = start.elapsed();
    let pool_ops_per_sec = iterations as f64 / pool_time.as_secs_f64();

    // 显示结果
    println!();
    println!("  📊 性能对比结果 ({}次{}字节分配):", iterations, test_size);
    println!("     直接分配:");
    println!("       - 总时间: {:.2}ms", direct_time.as_millis());
    println!("       - 平均延迟: {:.2}μs", direct_time.as_micros() as f64 / iterations as f64);
    println!("       - 吞吐量: {:.0} ops/sec", direct_ops_per_sec);

    println!("     内存池分配:");
    println!("       - 总时间: {:.2}ms", pool_time.as_millis());
    println!("       - 平均延迟: {:.2}μs", pool_time.as_micros() as f64 / iterations as f64);
    println!("       - 吞吐量: {:.0} ops/sec", pool_ops_per_sec);

    let speedup = pool_ops_per_sec / direct_ops_per_sec;
    let latency_reduction = (1.0 - pool_time.as_secs_f64() / direct_time.as_secs_f64()) * 100.0;

    println!();
    println!("  🎯 性能提升:");
    println!("     - 速度提升: {:.1}x", speedup);
    println!("     - 延迟降低: {:.1}%", latency_reduction);

    // 显示内存池统计
    let stats = pool.get_stats();
    println!();
    println!("  📈 内存池统计:");
    println!("     - 总分配次数: {}", stats.total_allocations);
    println!("     - 总释放次数: {}", stats.total_deallocations);
    println!("     - 缓存命中次数: {}", stats.cache_hits);
    println!("     - 缓存未命中次数: {}", stats.cache_misses);

    let hit_rate = if stats.cache_hits + stats.cache_misses > 0 {
        stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64 * 100.0
    } else {
        0.0
    };
    println!("     - 缓存命中率: {:.1}%", hit_rate);

    println!("     - 当前缓存的缓冲区数量: {}", stats.cached_buffers);
    println!("     - 总内存使用量: {:.2}MB", stats.total_memory_usage as f64 / 1024.0 / 1024.0);
    println!("     - 当前活跃缓冲区: {}", stats.cached_buffers);

    println!();
}
