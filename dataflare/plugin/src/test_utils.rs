//! Test utilities for plugin development and testing
//!
//! Provides helper functions and mock implementations for testing plugins
//! and plugin adapters.

use std::collections::HashMap;
use chrono::Utc;
use dataflare_core::message::DataRecord;
use crate::{PluginRecord, OwnedPluginRecord, SmartPlugin, PluginResult};
use crate::interface::PluginType;
use crate::core::Result;

/// Create a test DataRecord with the given data
pub fn create_test_data_record(data: serde_json::Value) -> DataRecord {
    let mut record = DataRecord::new(data);
    record.created_at = Utc::now();
    record.updated_at = Utc::now();
    record
}

/// Create a test DataRecord with string data
pub fn create_test_string_record(data: &str) -> DataRecord {
    create_test_data_record(serde_json::Value::String(data.to_string()))
}

/// Create a test DataRecord with metadata
pub fn create_test_record_with_metadata(
    data: serde_json::Value,
    metadata: HashMap<String, String>,
) -> DataRecord {
    let mut record = create_test_data_record(data);
    record.metadata = metadata;
    record
}

/// Create a test PluginRecord from bytes
pub fn create_test_plugin_record(data: &[u8]) -> OwnedPluginRecord {
    OwnedPluginRecord {
        value: data.to_vec(),
        metadata: HashMap::new(),
        timestamp: Utc::now().timestamp_millis(),
        key: None,
        partition: None,
        offset: 0,
    }
}

/// Create a test PluginRecord with metadata
pub fn create_test_plugin_record_with_metadata(
    data: &[u8],
    metadata: HashMap<String, String>,
) -> OwnedPluginRecord {
    OwnedPluginRecord {
        value: data.to_vec(),
        metadata,
        timestamp: Utc::now().timestamp_millis(),
        key: None,
        partition: None,
        offset: 0,
    }
}

/// Create a test PluginRecord with all fields
pub fn create_test_plugin_record_full(
    data: &[u8],
    metadata: HashMap<String, String>,
    key: Option<Vec<u8>>,
    offset: u64,
) -> OwnedPluginRecord {
    OwnedPluginRecord {
        value: data.to_vec(),
        metadata,
        timestamp: Utc::now().timestamp_millis(),
        key,
        partition: None,
        offset,
    }
}

/// Mock plugin for testing
pub struct MockPlugin {
    pub name: String,
    pub version: String,
    pub plugin_type: PluginType,
    pub process_fn: Box<dyn Fn(&PluginRecord) -> Result<PluginResult> + Send + Sync>,
}

impl MockPlugin {
    /// Create a new mock plugin
    pub fn new<F>(name: &str, version: &str, plugin_type: PluginType, process_fn: F) -> Self
    where
        F: Fn(&PluginRecord) -> Result<PluginResult> + Send + Sync + 'static,
    {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            plugin_type,
            process_fn: Box::new(process_fn),
        }
    }

    /// Create a simple filter plugin that filters based on a predicate
    pub fn filter<F>(name: &str, predicate: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        Self::new(name, "1.0.0", PluginType::Filter, move |record| {
            let data = std::str::from_utf8(record.value).map_err(|e| {
                crate::interface::PluginError::processing(format!("UTF-8 parsing failed: {}", e))
            })?;
            if predicate(data) {
                Ok(PluginResult::success(record.value.to_vec()))
            } else {
                Ok(PluginResult::filtered())
            }
        })
    }

    /// Create a simple map plugin that transforms strings
    pub fn map<F>(name: &str, transform: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        Self::new(name, "1.0.0", PluginType::Transformer, move |record| {
            let data = std::str::from_utf8(record.value).map_err(|e| {
                crate::interface::PluginError::processing(format!("UTF-8 parsing failed: {}", e))
            })?;
            let result = transform(data);
            Ok(PluginResult::success(result.into_bytes()))
        })
    }
}

impl SmartPlugin for MockPlugin {
    fn process(&self, record: &PluginRecord) -> Result<PluginResult> {
        (self.process_fn)(record)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn plugin_type(&self) -> crate::plugin::PluginType {
        // 转换interface::PluginType到plugin::PluginType
        match self.plugin_type {
            PluginType::Filter => crate::plugin::PluginType::Filter,
            PluginType::Transformer => crate::plugin::PluginType::Map,
            PluginType::Aggregator => crate::plugin::PluginType::Aggregator,
            PluginType::Sink => crate::plugin::PluginType::Sink,
        }
    }
}

/// Performance testing utilities
pub mod perf {
    use super::*;
    use std::time::{Duration, Instant};

    /// Benchmark a plugin's performance
    pub fn benchmark_plugin<P: crate::SmartPlugin>(
        plugin: &P,
        records: &[OwnedPluginRecord],
        iterations: usize,
    ) -> BenchmarkResult {
        let mut total_time = Duration::ZERO;
        let mut successful_runs = 0;
        let mut errors = 0;

        for _ in 0..iterations {
            let start = Instant::now();

            for record in records {
                let plugin_record = record.as_plugin_record();
                match plugin.process(&plugin_record) {
                    Ok(_) => successful_runs += 1,
                    Err(_) => errors += 1,
                }
            }

            total_time += start.elapsed();
        }

        let total_records = records.len() * iterations;
        let avg_time_per_record = if total_records > 0 {
            total_time / total_records as u32
        } else {
            Duration::ZERO
        };

        BenchmarkResult {
            total_time,
            total_records,
            successful_runs,
            errors,
            avg_time_per_record,
            records_per_second: if total_time.as_secs_f64() > 0.0 {
                total_records as f64 / total_time.as_secs_f64()
            } else {
                0.0
            },
        }
    }

    /// Benchmark result
    #[derive(Debug)]
    pub struct BenchmarkResult {
        pub total_time: Duration,
        pub total_records: usize,
        pub successful_runs: usize,
        pub errors: usize,
        pub avg_time_per_record: Duration,
        pub records_per_second: f64,
    }

    impl BenchmarkResult {
        /// Get success rate as percentage
        pub fn success_rate(&self) -> f64 {
            if self.total_records > 0 {
                (self.successful_runs as f64 / self.total_records as f64) * 100.0
            } else {
                0.0
            }
        }

        /// Get error rate as percentage
        pub fn error_rate(&self) -> f64 {
            if self.total_records > 0 {
                (self.errors as f64 / self.total_records as f64) * 100.0
            } else {
                0.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_records() {
        let record = create_test_string_record("test data");
        assert_eq!(record.data, serde_json::Value::String("test data".to_string()));

        let plugin_record = create_test_plugin_record(b"test data");
        assert_eq!(plugin_record.value, b"test data");
    }

    #[test]
    fn test_mock_plugin() {
        let plugin = MockPlugin::filter("test_filter", |data| data.contains("keep"));

        let record = create_test_plugin_record(b"keep this");
        let borrowed = record.as_plugin_record();
        let result = plugin.process(&borrowed).unwrap();

        match result {
            PluginResult::Filtered(keep) => assert!(keep),
            _ => panic!("Expected filtered result"),
        }
    }

    #[test]
    fn test_benchmark() {
        let plugin = MockPlugin::map("test_map", |data| data.to_uppercase());
        let records = vec![
            create_test_plugin_record(b"hello"),
            create_test_plugin_record(b"world"),
        ];

        let result = perf::benchmark_plugin(&plugin, &records, 10);

        assert_eq!(result.total_records, 20); // 2 records * 10 iterations
        assert_eq!(result.successful_runs, 20);
        assert_eq!(result.errors, 0);
        assert_eq!(result.success_rate(), 100.0);
    }
}
