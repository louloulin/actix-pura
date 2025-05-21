//! Metrics collection for batch processing
//!
//! Collects and calculates performance metrics for batch processing
//! to support adaptive batch sizing and monitoring.

use std::time::{Duration, Instant};
use std::collections::VecDeque;

const MAX_HISTORY_SIZE: usize = 100; // Keep metrics for last 100 batches

/// Structure to track and calculate batching metrics
#[derive(Debug, Clone)]
pub struct BatchingMetrics {
    /// History of batch sizes
    batch_sizes: VecDeque<usize>,
    
    /// History of processing times
    processing_times: VecDeque<Duration>,
    
    /// Total records processed
    total_records: usize,
    
    /// Total processing time
    total_time: Duration,
    
    /// First batch timestamp
    first_batch_time: Option<Instant>,
    
    /// Last batch timestamp
    last_batch_time: Option<Instant>,
}

impl BatchingMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            batch_sizes: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            processing_times: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            total_records: 0,
            total_time: Duration::from_secs(0),
            first_batch_time: None,
            last_batch_time: None,
        }
    }
    
    /// Record a batch processing event
    pub fn record_batch(&mut self, size: usize, processing_time: Duration) {
        // Update history, removing oldest if we exceed max size
        self.batch_sizes.push_back(size);
        self.processing_times.push_back(processing_time);
        
        if self.batch_sizes.len() > MAX_HISTORY_SIZE {
            self.batch_sizes.pop_front();
            self.processing_times.pop_front();
        }
        
        // Update totals
        self.total_records += size;
        self.total_time += processing_time;
        
        // Update timestamps
        let now = Instant::now();
        if self.first_batch_time.is_none() {
            self.first_batch_time = Some(now);
        }
        self.last_batch_time = Some(now);
    }
    
    /// Calculate current throughput (records per second)
    /// Returns None if not enough data is available
    pub fn throughput(&self) -> Option<f64> {
        if self.total_records == 0 || self.total_time.as_secs_f64() == 0.0 {
            return None;
        }
        
        Some(self.total_records as f64 / self.total_time.as_secs_f64())
    }
    
    /// Get average batch size
    /// Returns None if no batches have been processed
    pub fn average_batch_size(&self) -> Option<f64> {
        if self.batch_sizes.is_empty() {
            return None;
        }
        
        let sum: usize = self.batch_sizes.iter().sum();
        Some(sum as f64 / self.batch_sizes.len() as f64)
    }
    
    /// Get average processing time per batch
    /// Returns None if no batches have been processed
    pub fn average_processing_time(&self) -> Option<Duration> {
        if self.processing_times.is_empty() {
            return None;
        }
        
        let sum: u128 = self.processing_times.iter().map(|d| d.as_nanos()).sum();
        Some(Duration::from_nanos((sum / self.processing_times.len() as u128) as u64))
    }
    
    /// Get average latency per record
    /// Returns None if no batches have been processed
    pub fn average_latency(&self) -> Option<Duration> {
        if self.batch_sizes.is_empty() || self.processing_times.is_empty() {
            return None;
        }
        
        let mut total_ns = 0u128;
        let mut total_records = 0usize;
        
        for (size, time) in self.batch_sizes.iter().zip(self.processing_times.iter()) {
            total_ns += time.as_nanos();
            total_records += size;
        }
        
        if total_records == 0 {
            return None;
        }
        
        Some(Duration::from_nanos((total_ns / total_records as u128) as u64))
    }
    
    /// Get total elapsed time since first batch
    /// Returns None if no batches have been processed
    pub fn total_elapsed(&self) -> Option<Duration> {
        match (self.first_batch_time, self.last_batch_time) {
            (Some(first), Some(last)) => Some(last.duration_since(first)),
            _ => None,
        }
    }
    
    /// Get total records processed
    pub fn total_records(&self) -> usize {
        self.total_records
    }
    
    /// Get total processing time
    pub fn total_processing_time(&self) -> Duration {
        self.total_time
    }
    
    /// Get number of batches processed
    pub fn batch_count(&self) -> usize {
        self.batch_sizes.len()
    }
    
    /// Reset all metrics
    pub fn reset(&mut self) {
        self.batch_sizes.clear();
        self.processing_times.clear();
        self.total_records = 0;
        self.total_time = Duration::from_secs(0);
        self.first_batch_time = None;
        self.last_batch_time = None;
    }
    
    /// Get recent batch sizes (up to MAX_HISTORY_SIZE)
    pub fn recent_batch_sizes(&self) -> &VecDeque<usize> {
        &self.batch_sizes
    }
    
    /// Get recent processing times (up to MAX_HISTORY_SIZE)
    pub fn recent_processing_times(&self) -> &VecDeque<Duration> {
        &self.processing_times
    }
} 