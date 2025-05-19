//! 工具模块
//!
//! 提供各种实用功能。

use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::{DataFlareError, Result};

/// 生成唯一ID
pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

/// 获取当前时间戳
pub fn current_timestamp() -> DateTime<Utc> {
    Utc::now()
}

/// 格式化持续时间
pub fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let millis = duration.subsec_millis();
    
    if hours > 0 {
        format!("{}h {}m {}s {}ms", hours, minutes, seconds, millis)
    } else if minutes > 0 {
        format!("{}m {}s {}ms", minutes, seconds, millis)
    } else if seconds > 0 {
        format!("{}s {}ms", seconds, millis)
    } else {
        format!("{}ms", millis)
    }
}

/// 计时器
pub struct Timer {
    /// 开始时间
    start: Instant,
    /// 标签
    label: String,
}

impl Timer {
    /// 创建新的计时器
    pub fn new<S: Into<String>>(label: S) -> Self {
        Self {
            start: Instant::now(),
            label: label.into(),
        }
    }
    
    /// 获取经过的时间
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
    
    /// 获取格式化的经过时间
    pub fn elapsed_formatted(&self) -> String {
        format_duration(self.elapsed())
    }
    
    /// 重置计时器
    pub fn reset(&mut self) {
        self.start = Instant::now();
    }
    
    /// 打印经过的时间
    pub fn log(&self) {
        log::info!("{}: {}", self.label, self.elapsed_formatted());
    }
}

/// 重试函数执行
pub async fn retry<F, Fut, T>(
    f: F,
    max_retries: u32,
    retry_interval: Duration,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut retries = 0;
    let mut last_error = None;
    
    while retries < max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                last_error = Some(err);
                retries += 1;
                
                if retries < max_retries {
                    log::warn!("重试 {}/{}: {}", retries, max_retries, last_error.as_ref().unwrap());
                    tokio::time::sleep(retry_interval).await;
                }
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| DataFlareError::Unknown("未知错误".to_string())))
}

/// 批处理迭代器
pub struct BatchIterator<I, T>
where
    I: Iterator<Item = T>,
{
    /// 源迭代器
    iter: I,
    /// 批处理大小
    batch_size: usize,
}

impl<I, T> BatchIterator<I, T>
where
    I: Iterator<Item = T>,
{
    /// 创建新的批处理迭代器
    pub fn new(iter: I, batch_size: usize) -> Self {
        Self { iter, batch_size }
    }
}

impl<I, T> Iterator for BatchIterator<I, T>
where
    I: Iterator<Item = T>,
{
    type Item = Vec<T>;
    
    fn next(&mut self) -> Option<Self::Item> {
        let mut batch = Vec::with_capacity(self.batch_size);
        
        for _ in 0..self.batch_size {
            match self.iter.next() {
                Some(item) => batch.push(item),
                None => break,
            }
        }
        
        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_id() {
        let id1 = generate_id();
        let id2 = generate_id();
        
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36); // UUID 格式
    }
    
    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_millis(100)), "100ms");
        assert_eq!(format_duration(Duration::from_secs(5)), "5s 0ms");
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 5s 0ms");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m 5s 0ms");
    }
    
    #[test]
    fn test_timer() {
        let timer = Timer::new("测试计时器");
        std::thread::sleep(Duration::from_millis(10));
        assert!(timer.elapsed().as_millis() >= 10);
    }
    
    #[test]
    fn test_batch_iterator() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let batches: Vec<Vec<i32>> = BatchIterator::new(data.into_iter(), 3).collect();
        
        assert_eq!(batches.len(), 4);
        assert_eq!(batches[0], vec![1, 2, 3]);
        assert_eq!(batches[1], vec![4, 5, 6]);
        assert_eq!(batches[2], vec![7, 8, 9]);
        assert_eq!(batches[3], vec![10]);
    }
}
