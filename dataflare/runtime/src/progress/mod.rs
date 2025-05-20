//! Progress reporting system for DataFlare
//!
//! This module provides a flexible progress reporting system for tracking and reporting workflow execution progress.
//! It supports multiple callback mechanisms, including function callbacks, event streams, and HTTP webhooks.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use log::{debug, info, warn, error};
use serde::{Serialize, Deserialize};
use dataflare_core::message::{WorkflowProgress, WorkflowPhase};
use dataflare_core::error::{Result, DataFlareError};
use actix::prelude::*;
use reqwest;
use futures::channel::mpsc::{channel, Sender, Receiver};
use futures::stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::StreamExt;
use std::time::Duration;
use tokio::time::sleep;
use chrono::Utc;

/// Progress callback types supported by DataFlare
#[derive(Debug, Clone)]
pub enum CallbackType {
    /// Function callback (in-memory)
    Function,
    /// Event stream (for streaming progress updates)
    EventStream,
    /// HTTP webhook (for remote notification)
    Webhook,
}

impl fmt::Display for CallbackType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallbackType::Function => write!(f, "function"),
            CallbackType::EventStream => write!(f, "event_stream"),
            CallbackType::Webhook => write!(f, "webhook"),
        }
    }
}

/// Function callback type for progress updates
pub type ProgressCallback = Box<dyn Fn(WorkflowProgress) + Send + Sync + 'static>;

/// Configuration for HTTP webhook callback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook URL
    pub url: String,
    /// HTTP method (POST, PUT)
    #[serde(default = "default_webhook_method")]
    pub method: String,
    /// Custom HTTP headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Authentication token (if required)
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Retry count for failed webhook calls
    #[serde(default = "default_retry_count")]
    pub retry_count: u32,
}

fn default_webhook_method() -> String {
    "POST".to_string()
}

fn default_retry_count() -> u32 {
    3
}

impl WebhookConfig {
    /// Create a new webhook configuration
    pub fn new(url: String) -> Self {
        Self {
            url,
            method: default_webhook_method(),
            headers: HashMap::new(),
            auth_token: None,
            retry_count: default_retry_count(),
        }
    }

    /// Add a header to the webhook configuration
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Set authentication token
    pub fn with_auth_token(mut self, token: &str) -> Self {
        self.auth_token = Some(token.to_string());
        self
    }

    /// Set HTTP method
    pub fn with_method(mut self, method: &str) -> Self {
        self.method = method.to_string();
        self
    }

    /// Set retry count
    pub fn with_retry_count(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }
}

/// Progress event stream for streaming progress updates
pub struct ProgressEventStream {
    receiver: Receiver<WorkflowProgress>,
}

impl Stream for ProgressEventStream {
    type Item = WorkflowProgress;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}

/// Progress reporter for sending progress updates through multiple channels
#[derive(Clone)]
pub struct ProgressReporter {
    /// Inner implementation wrapped in Arc<Mutex<>>
    inner: Arc<Mutex<ProgressReporterInner>>,
}

/// Inner implementation of progress reporter
struct ProgressReporterInner {
    /// Function callbacks
    function_callbacks: Vec<ProgressCallback>,
    /// Event stream senders
    event_stream_senders: Vec<Sender<WorkflowProgress>>,
    /// Webhook configurations
    webhooks: Vec<WebhookConfig>,
    /// HTTP client for webhook calls
    http_client: reqwest::Client,
}

impl ProgressReporter {
    /// Create a new progress reporter
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ProgressReporterInner {
                function_callbacks: Vec::new(),
                event_stream_senders: Vec::new(),
                webhooks: Vec::new(),
                http_client: reqwest::Client::new(),
            })),
        }
    }

    /// Add a function callback
    pub fn add_function_callback<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(WorkflowProgress) + Send + Sync + 'static,
    {
        let mut inner = self.inner.lock().map_err(|e| {
            DataFlareError::Workflow(format!("Failed to lock progress reporter: {}", e))
        })?;
        
        inner.function_callbacks.push(Box::new(callback));
        Ok(())
    }

    /// Create a new event stream
    pub fn create_event_stream(&self) -> Result<ProgressEventStream> {
        let mut inner = self.inner.lock().map_err(|e| {
            DataFlareError::Workflow(format!("Failed to lock progress reporter: {}", e))
        })?;
        
        let (sender, receiver) = channel(100); // Buffer size of 100
        inner.event_stream_senders.push(sender);
        
        Ok(ProgressEventStream { receiver })
    }

    /// Add a webhook
    pub fn add_webhook(&self, config: WebhookConfig) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|e| {
            DataFlareError::Workflow(format!("Failed to lock progress reporter: {}", e))
        })?;
        
        inner.webhooks.push(config);
        Ok(())
    }

    /// Report progress
    pub async fn report(&self, progress: WorkflowProgress) -> Result<()> {
        let inner = self.inner.lock().map_err(|e| {
            DataFlareError::Workflow(format!("Failed to lock progress reporter: {}", e))
        })?;
        
        // Call function callbacks
        for callback in &inner.function_callbacks {
            callback(progress.clone());
        }
        
        // Send to event streams
        for sender in &inner.event_stream_senders {
            // Try to send but don't block if receiver is full
            if let Err(e) = sender.clone().try_send(progress.clone()) {
                debug!("Failed to send progress to event stream: {}", e);
            }
        }
        
        // Send to webhooks (drop lock before async operations)
        let webhooks = inner.webhooks.clone();
        let http_client = inner.http_client.clone();
        
        // Release the mutex lock before async operations
        drop(inner);
        
        // Send to webhooks concurrently
        for webhook in webhooks {
            if let Err(e) = self.send_webhook(&http_client, &webhook, &progress).await {
                warn!("Failed to send webhook: {}", e);
            }
        }
        
        Ok(())
    }
    
    /// Send progress to webhook
    async fn send_webhook(&self, http_client: &reqwest::Client, webhook: &WebhookConfig, progress: &WorkflowProgress) -> Result<()> {
        // 手动创建JSON，因为WorkflowProgress未实现Serialize特性
        let payload = format!(
            r#"{{
                "workflow_id": {:?},
                "phase": {:?},
                "progress": {},
                "message": {:?},
                "timestamp": {:?}
            }}"#,
            progress.workflow_id,
            format!("{:?}", progress.phase),
            progress.progress,
            progress.message,
            progress.timestamp
        );
        
        // Prepare request
        let mut request_builder = match webhook.method.to_uppercase().as_str() {
            "POST" => http_client.post(&webhook.url),
            "PUT" => http_client.put(&webhook.url),
            _ => return Err(DataFlareError::Workflow(format!("Unsupported webhook method: {}", webhook.method))),
        };
        
        // Add headers
        request_builder = request_builder.header("Content-Type", "application/json");
        
        // Add custom headers
        for (key, value) in &webhook.headers {
            request_builder = request_builder.header(key, value);
        }
        
        // Add auth token if provided
        if let Some(token) = &webhook.auth_token {
            request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
        }
        
        // Send the request
        let mut retry_count = 0;
        let mut last_error = None;
        
        while retry_count < webhook.retry_count {
            match request_builder.try_clone().unwrap().body(payload.clone()).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        // Successfully sent webhook
                        return Ok(());
                    } else {
                        // Server responded with an error
                        let status = response.status();
                        let text = response.text().await.unwrap_or_else(|_| "Could not read response body".to_string());
                        
                        // For certain status codes, don't retry
                        if status.as_u16() >= 400 && status.as_u16() < 500 && status.as_u16() != 429 {
                            return Err(DataFlareError::Workflow(format!(
                                "Webhook server responded with client error: {} - {}", 
                                status, text
                            )));
                        }
                        
                        last_error = Some(DataFlareError::Workflow(format!(
                            "Webhook server responded with error: {} - {}", 
                            status, text
                        )));
                    }
                },
                Err(e) => {
                    last_error = if e.is_timeout() {
                        Some(DataFlareError::Workflow(format!("Webhook timeout: {}", e)))
                    } else if e.is_connect() {
                        Some(DataFlareError::Connection(format!("Connection error: {}", e)))
                    } else {
                        Some(DataFlareError::Workflow(format!("Reqwest error: {}", e)))
                    };
                }
            }
            
            // Exponential backoff before retrying
            let delay = std::time::Duration::from_millis(100 * 2u64.pow(retry_count));
            tokio::time::sleep(delay).await;
            
            retry_count += 1;
        }
        
        // Return the last error if all retries failed
        Err(last_error.unwrap_or_else(|| {
            DataFlareError::Connection("Failed to send webhook after retries".to_string())
        }))
    }
    
    /// Clear all progress callbacks and webhooks
    pub fn clear(&self) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|e| {
            DataFlareError::Workflow(format!("Failed to lock progress reporter: {}", e))
        })?;
        
        // Clear function callbacks
        inner.function_callbacks.clear();
        
        // Close event stream senders
        for sender in inner.event_stream_senders.drain(..) {
            // Close the sender - this will notify all receivers that no more messages will be sent
            drop(sender);
        }
        
        // Clear webhooks
        inner.webhooks.clear();
        
        Ok(())
    }
}

impl Default for ProgressReporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Progress actor for receiving progress updates from workflow actors
pub struct ProgressActor {
    /// Progress reporter
    reporter: ProgressReporter,
}

impl ProgressActor {
    /// Create a new progress actor
    pub fn new(reporter: ProgressReporter) -> Self {
        Self { reporter }
    }
}

impl Actor for ProgressActor {
    type Context = actix::Context<Self>;
}

impl Handler<WorkflowProgress> for ProgressActor {
    type Result = ();

    fn handle(&mut self, msg: WorkflowProgress, ctx: &mut Self::Context) -> Self::Result {
        // Create a future for reporting progress
        let reporter = self.reporter.clone();
        let fut = async move {
            if let Err(e) = reporter.report(msg).await {
                error!("Failed to report progress: {}", e);
            }
        };
        
        // Spawn the future
        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);
    }
}

/// Message to subscribe to progress updates
#[derive(Message)]
#[rtype(result = "Result<ProgressEventStream>")]
pub struct SubscribeToProgressUpdates;

/// Handler for progress subscription
impl Handler<SubscribeToProgressUpdates> for ProgressActor {
    type Result = ResponseFuture<Result<ProgressEventStream>>;

    fn handle(&mut self, _: SubscribeToProgressUpdates, _: &mut Self::Context) -> Self::Result {
        let reporter = self.reporter.clone();
        Box::pin(async move {
            reporter.create_event_stream()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_function_callback() {
        let progress_counter = Arc::new(Mutex::new(0));
        let progress_counter_clone = progress_counter.clone();
        
        let reporter = ProgressReporter::new();
        reporter.add_function_callback(move |_| {
            let mut counter = progress_counter_clone.lock().unwrap();
            *counter += 1;
        }).unwrap();
        
        // Create test progress
        let progress = WorkflowProgress {
            workflow_id: "test-workflow".to_string(),
            phase: WorkflowPhase::Extracting,
            progress: 0.5,
            message: "Test progress".to_string(),
            timestamp: chrono::Utc::now()
        };
        
        reporter.report(progress).await.unwrap();
        
        let counter = progress_counter.lock().unwrap();
        assert_eq!(*counter, 1);
    }

    #[tokio::test]
    async fn test_event_stream() {
        let reporter = ProgressReporter::new();
        let mut stream = reporter.create_event_stream().unwrap();
        
        // Create test progress
        let progress = WorkflowProgress {
            workflow_id: "test-workflow".to_string(),
            phase: WorkflowPhase::Extracting,
            progress: 0.5,
            message: "Test progress".to_string(),
            timestamp: chrono::Utc::now()
        };
        
        // Spawn a task to report progress after a short delay
        let reporter_clone = reporter.clone();
        let progress_clone = progress.clone();
        
        // 创建单独的任务并执行
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                sleep(Duration::from_millis(100)).await;
                if let Err(e) = reporter_clone.report(progress_clone).await {
                    error!("Error reporting progress: {}", e);
                }
            });
        });
        
        // Wait for progress update from stream
        if let Some(received) = stream.next().await {
            assert_eq!(received.workflow_id, progress.workflow_id);
            assert_eq!(received.phase, progress.phase);
            assert_eq!(received.progress, progress.progress);
            assert_eq!(received.message, progress.message);
        } else {
            panic!("No progress received from stream");
        }
    }
} 