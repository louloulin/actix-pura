//! Tests for the DataFlare batch processing system
//!
//! These tests validate shared batch data structures, adaptive batching,
//! and backpressure control.

use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::thread;

use dataflare_core::message::DataRecord;
use dataflare_runtime::batch::{
    SharedDataBatch, AdaptiveBatcher, AdaptiveBatchingConfig,
    BackpressureController, CreditConfig, CreditMode,
};

#[test]
fn test_shared_data_batch() {
    // Create test records
    let records = vec![
        DataRecord::new_json(serde_json::json!({"id": 1, "name": "Test 1"})),
        DataRecord::new_json(serde_json::json!({"id": 2, "name": "Test 2"})),
        DataRecord::new_json(serde_json::json!({"id": 3, "name": "Test 3"})),
    ];

    // Create a shared batch
    let batch = SharedDataBatch::new(records);

    // Test basic properties
    assert_eq!(batch.len(), 3);
    assert!(!batch.is_empty());
    assert_eq!(batch.watermark(), None);

    // Test cloning (should be cheap, not deep copy)
    let batch2 = batch.clone();
    assert_eq!(batch2.len(), 3);

    // Test slicing
    let slice = batch.slice(1, 3);
    assert_eq!(slice.len(), 2);

    // Test metadata
    let batch_with_meta = batch.clone().with_metadata("source", "test_source");
    assert_eq!(batch_with_meta.metadata().get("source").unwrap(), "test_source");

    // Test watermark
    let batch_with_watermark = batch.clone().with_watermark(123456789);
    assert_eq!(batch_with_watermark.watermark(), Some(123456789));

    // Test map operation
    let mapped_batch = batch.map(|record| {
        let mut new_record = record.clone();
        if let Some(obj) = new_record.as_object_mut() {
            if let Some(name) = obj.get_mut("name") {
                *name = format!("{}_mapped", name.as_str().unwrap()).into();
            }
        }
        new_record
    });

    assert_eq!(mapped_batch.len(), 3);
    assert!(mapped_batch.records()[0].as_str("name").unwrap().contains("_mapped"));

    // Test filter operation
    let filtered_batch = batch.filter(|record| {
        if let Some(id) = record.as_i64("id") {
            id > 1
        } else {
            false
        }
    });

    assert_eq!(filtered_batch.len(), 2);

    // Test merge operation
    let merged = SharedDataBatch::merge(&[batch.clone(), batch2.clone()]);
    assert_eq!(merged.len(), 6);
}

#[test]
fn test_adaptive_batcher() {
    // Create configuration with fast adaptation for testing
    let config = AdaptiveBatchingConfig {
        initial_size: 100,
        min_size: 10,
        max_size: 1000,
        throughput_target: 1000, // 1000 records/s
        latency_target_ms: 50,   // 50ms target latency
        adaptation_rate: 0.5,    // Faster adaptation for testing
        stability_threshold: 3,  // Only need 3 batches before adapting
    };

    let mut batcher = AdaptiveBatcher::new(config);

    // Initial size should match configuration
    assert_eq!(batcher.get_batch_size(), 100);

    // Simulate high throughput, low latency
    for _ in 0..5 {
        batcher.start_batch();
        batcher.complete_batch(100);
    }

    // Batch size should increase (high throughput, low latency)
    assert!(batcher.get_batch_size() > 100);

    // Reset
    batcher.reset();
    assert_eq!(batcher.get_batch_size(), 100);

    // Simulate low throughput, high latency
    for _ in 0..5 {
        batcher.start_batch();
        batcher.complete_batch(100);
    }

    // Batch size should decrease (high latency)
    assert!(batcher.get_batch_size() < 100);
}

#[test]
fn test_backpressure_controller() {
    let config = CreditConfig {
        base_credits: 1000,
        min_credits: 100,
        max_credits: 5000,
        credit_interval_ms: 10,
        mode: CreditMode::Adaptive,
        target_queue_size: 1000,
        target_processing_time_ms: 50,
    };

    let mode = config.mode; // 保存模式以便后续使用
    let mut controller = BackpressureController::new(config);

    // Request credits
    let granted = controller.request_credits(500);
    assert_eq!(granted, Some(500));

    // Request more than available
    let granted = controller.request_credits(600);
    assert_eq!(granted, Some(500)); // Only 500 left from initial 1000

    // Update stats
    controller.update_stats(1000, Duration::from_millis(100));

    // Wait for refill
    thread::sleep(Duration::from_millis(20));

    // Request again after refill
    let granted = controller.request_credits(100);
    assert!(granted.is_some()); // Should have some credits after refill

    // Test adaptive mode
    if mode == CreditMode::Adaptive {
        // Simulate high load
        controller.update_stats(2000, Duration::from_millis(100));

        // Verify pressure increased
        assert!(controller.current_pressure() > 0.0);
    }
}