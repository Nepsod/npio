//! Progress reporting for mount operations.

use std::time::Instant;
use tokio::sync::broadcast;
use crate::mount::advanced::{OperationId, config::ProgressConfig};

/// Handles progress reporting through multiple channels.
pub struct ProgressReporter {
    operation_id: OperationId,
    progress_tx: broadcast::Sender<ProgressEvent>,
    callback: Option<Box<dyn Fn(ProgressEvent) + Send + Sync>>,
    config: ProgressConfig,
}

impl std::fmt::Debug for ProgressReporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressReporter")
            .field("operation_id", &self.operation_id)
            .field("progress_tx", &self.progress_tx)
            .field("callback", &self.callback.as_ref().map(|_| "Some(callback)"))
            .field("config", &self.config)
            .finish()
    }
}

impl ProgressReporter {
    /// Creates a new progress reporter.
    pub fn new(operation_id: OperationId, config: ProgressConfig) -> Self {
        let (progress_tx, _) = broadcast::channel(config.buffer_size);
        
        Self {
            operation_id,
            progress_tx,
            callback: None,
            config,
        }
    }

    /// Sets a progress callback.
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(ProgressEvent) + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(callback));
    }

    /// Gets a progress event receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<ProgressEvent> {
        self.progress_tx.subscribe()
    }

    /// Reports progress.
    pub fn report_progress(&self, progress: f32, message: String, stage: OperationStage) {
        let event = ProgressEvent {
            operation_id: self.operation_id,
            progress,
            message,
            timestamp: Instant::now(),
            stage,
        };

        // Send to callback if enabled
        if self.config.enable_callbacks {
            if let Some(callback) = &self.callback {
                callback(event.clone());
            }
        }

        // Send to stream if enabled
        if self.config.enable_streams {
            let _ = self.progress_tx.send(event);
        }
    }

    /// Reports operation completion.
    pub fn report_completion(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.report_progress(1.0, "Operation completed".to_string(), OperationStage::Completion);
        Ok(())
    }
}

/// Progress event emitted during operations.
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub operation_id: OperationId,
    pub progress: f32, // 0.0 to 1.0
    pub message: String,
    pub timestamp: Instant,
    pub stage: OperationStage,
}

/// Stage of the operation for progress reporting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OperationStage {
    Validation,
    Preparation,
    Execution,
    Cleanup,
    Completion,
}