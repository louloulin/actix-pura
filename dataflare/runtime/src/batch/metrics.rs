//! Metrics collection for batch processing
//!
//! Collects and calculates performance metrics for batch processing
//! to support adaptive batch sizing and monitoring.

use std::time::{Duration, Instant};
use std::collections::VecDeque;

/// Number of batches to keep in history for metrics calculation
const DEFAULT_HISTORY_SIZE: usize = 20;

/// Metrics for batch processing
#[derive(Debug, Clone)]
pub struct BatchingMetrics {
    /// History of batch sizes and their processing times
    history: VecDeque<(usize, Duration, usize)>, // (records_count, processing_time, batch_size)
    
    /// Maximum number of entries in history
    max_history_size: usize,
    
    /// Total number of batches processed
    total_batches: usize,
    
    /// Total number of records processed
    total_records: usize,
    
    /// Total processing time
    total_processing_time: Duration,
    
    /// System stability flag
    is_stable: bool,
    
    /// Last recording time
    last_recording: Instant,
}

impl BatchingMetrics {
    /// Create new batching metrics
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(DEFAULT_HISTORY_SIZE),
            max_history_size: DEFAULT_HISTORY_SIZE,
            total_batches: 0,
            total_records: 0,
            total_processing_time: Duration::from_secs(0),
            is_stable: false,
            last_recording: Instant::now(),
        }
    }
    
    /// Create new batching metrics with custom history size
    pub fn with_history_size(max_history_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history_size),
            max_history_size,
            total_batches: 0,
            total_records: 0,
            total_processing_time: Duration::from_secs(0),
            is_stable: false,
            last_recording: Instant::now(),
        }
    }
    
    /// Record a batch processing event
    pub fn record_batch(&mut self, records_count: usize, processing_time: Duration, batch_size: usize) {
        // Add to history
        self.history.push_back((records_count, processing_time, batch_size));
        
        // Trim history if it exceeds max size
        if self.history.len() > self.max_history_size {
            self.history.pop_front();
        }
        
        // Update totals
        self.total_batches += 1;
        self.total_records += records_count;
        self.total_processing_time += processing_time;
        
        // Update last recording time
        self.last_recording = Instant::now();
    }
    
    /// Get average batch size
    pub fn average_batch_size(&self) -> Option<usize> {
        if self.history.is_empty() {
            return None;
        }
        
        let sum: usize = self.history.iter().map(|(_, _, size)| size).sum();
        Some(sum / self.history.len())
    }
    
    /// Get average records per batch
    pub fn average_records_per_batch(&self) -> Option<usize> {
        if self.history.is_empty() {
            return None;
        }
        
        let sum: usize = self.history.iter().map(|(records, _, _)| records).sum();
        Some(sum / self.history.len())
    }
    
    /// Get average latency (processing time) per batch
    pub fn average_latency(&self) -> Option<Duration> {
        if self.history.is_empty() {
            return None;
        }
        
        let sum: Duration = self.history.iter()
            .fold(Duration::from_secs(0), |sum, (_, duration, _)| sum + *duration);
            
        Some(sum / self.history.len() as u32)
    }
    
    /// Get average latency per record in milliseconds
    pub fn average_latency_per_record_ms(&self) -> Option<f64> {
        if self.history.is_empty() {
            return None;
        }
        
        let latency_sum_ms: u128 = self.history.iter()
            .fold(0, |sum, (records, duration, _)| {
                sum + if *records > 0 {
                    duration.as_millis() / *records as u128
                } else {
                    0
                }
            });
            
        Some(latency_sum_ms as f64 / self.history.len() as f64)
    }
    
    /// Get throughput (records per second)
    pub fn throughput(&self) -> Option<f64> {
        if self.history.is_empty() {
            return None;
        }
        
        let mut total_records = 0;
        let mut total_seconds = 0.0;
        
        for (records, duration, _) in &self.history {
            total_records += records;
            total_seconds += duration.as_secs_f64();
        }
        
        if total_seconds > 0.0 {
            Some(total_records as f64 / total_seconds)
        } else {
            None
        }
    }
    
    /// Check if the system is stable
    pub fn is_system_stable(&self) -> bool {
        self.is_stable
    }
    
    /// Set the system stability status
    pub fn set_system_stable(&mut self, stable: bool) {
        self.is_stable = stable;
    }
    
    /// Get total batches processed
    pub fn total_batches(&self) -> usize {
        self.total_batches
    }
    
    /// Get total records processed
    pub fn total_records(&self) -> usize {
        self.total_records
    }
    
    /// Get total processing time
    pub fn total_processing_time(&self) -> Duration {
        self.total_processing_time
    }
    
    /// Reset metrics
    pub fn reset(&mut self) {
        self.history.clear();
        self.total_batches = 0;
        self.total_records = 0;
        self.total_processing_time = Duration::from_secs(0);
        self.is_stable = false;
        self.last_recording = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics_initialization() {
        let metrics = BatchingMetrics::new();
        assert_eq!(metrics.total_batches, 0);
        assert_eq!(metrics.total_records, 0);
        assert!(metrics.history.is_empty());
    }
    
    #[test]
    fn test_metrics_recording() {
        let mut metrics = BatchingMetrics::new();
        
        // Record some batches
        metrics.record_batch(100, Duration::from_millis(50), 100);
        metrics.record_batch(200, Duration::from_millis(100), 200);
        
        // Check totals
        assert_eq!(metrics.total_batches, 2);
        assert_eq!(metrics.total_records, 300);
        assert_eq!(metrics.total_processing_time, Duration::from_millis(150));
    }
    
    #[test]
    fn test_metrics_averages() {
        let mut metrics = BatchingMetrics::new();
        
        // Record batches with identical batch sizes but different record counts
        metrics.record_batch(100, Duration::from_millis(50), 200);
        metrics.record_batch(200, Duration::from_millis(100), 200);
        metrics.record_batch(300, Duration::from_millis(150), 200);
        
        // Check averages
        assert_eq!(metrics.average_batch_size(), Some(200));
        assert_eq!(metrics.average_records_per_batch(), Some(200));
        assert_eq!(metrics.average_latency(), Some(Duration::from_millis(100)));
    }
    
    #[test]
    fn test_metrics_throughput() {
        let mut metrics = BatchingMetrics::new();
        
        // Record batches with 1000 records/sec throughput
        metrics.record_batch(1000, Duration::from_secs(1), 1000);
        metrics.record_batch(2000, Duration::from_secs(2), 2000);
        
        // Check throughput
        assert!(metrics.throughput().unwrap() > 990.0);
        assert!(metrics.throughput().unwrap() < 1010.0);
    }
    
    #[test]
    fn test_metrics_reset() {
        let mut metrics = BatchingMetrics::new();
        
        // Record some data
        metrics.record_batch(100, Duration::from_millis(50), 100);
        
        // Reset
        metrics.reset();
        
        // Check reset state
        assert_eq!(metrics.total_batches, 0);
        assert_eq!(metrics.total_records, 0);
        assert!(metrics.history.is_empty());
        assert!(!metrics.is_system_stable());
    }
} 