//! Operation context management for advanced mount operations.

use std::sync::{Arc, Mutex};
use std::time::Instant;
use crate::mount::advanced::{
    OperationId, OperationType, OperationState, OperationResult, OperationStatusSummary,
    ProgressReporter, CancellationManager,
};
use crate::mount::advanced::config::OperationConfig;

/// Central coordinator for mount operation lifecycle and state management.
#[derive(Debug)]
pub struct OperationContext {
    id: OperationId,
    operation_type: OperationType,
    state: Arc<Mutex<OperationState>>,
    progress_reporter: ProgressReporter,
    cancellation_manager: CancellationManager,
    config: OperationConfig,
    start_time: Instant,
}

impl OperationContext {
    /// Creates a new operation context.
    pub fn new(
        operation_type: OperationType,
        config: OperationConfig,
    ) -> Self {
        let id = OperationId::new();
        let state = Arc::new(Mutex::new(OperationState::Pending));
        let progress_reporter = ProgressReporter::new(id, config.progress_config.clone());
        let cancellation_manager = CancellationManager::from_config(config.cancellation_config.clone());

        Self {
            id,
            operation_type,
            state,
            progress_reporter,
            cancellation_manager,
            config,
            start_time: Instant::now(),
        }
    }

    /// Gets the operation ID.
    pub fn id(&self) -> OperationId {
        self.id
    }

    /// Gets the operation type.
    pub fn operation_type(&self) -> &OperationType {
        &self.operation_type
    }

    /// Gets the current operation state.
    pub fn state(&self) -> OperationState {
        self.state.lock().unwrap().clone()
    }

    /// Gets the progress reporter.
    pub fn progress_reporter(&self) -> &ProgressReporter {
        &self.progress_reporter
    }

    /// Gets the cancellation manager.
    pub fn cancellation_manager(&self) -> &CancellationManager {
        &self.cancellation_manager
    }

    /// Gets the operation configuration.
    pub fn config(&self) -> &OperationConfig {
        &self.config
    }

    /// Gets the elapsed time since operation start.
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Updates the operation state atomically.
    pub fn update_state(&self, new_state: OperationState) {
        let mut state = self.state.lock().unwrap();
        *state = new_state.clone();
        drop(state); // Release the lock before calling cleanup
        
        // Automatically cleanup when reaching terminal state
        if new_state.is_terminal() {
            self.auto_cleanup_on_completion();
        }
    }

    /// Checks if the operation is cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_manager.is_cancelled()
    }

    /// Checks if the operation is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.state().is_terminal()
    }

    /// Gets the current progress if available.
    pub fn progress(&self) -> Option<f32> {
        self.state().progress()
    }

    /// Gets the current status message.
    pub fn status_message(&self) -> String {
        self.state().message()
    }

    /// Gets operation metadata if available.
    pub fn metadata(&self) -> Option<crate::mount::advanced::OperationMetadata> {
        match self.state() {
            OperationState::Completed { result } => Some(result.metadata),
            _ => None,
        }
    }

    /// Gets the operation result if completed.
    pub fn result(&self) -> Option<OperationResult> {
        match self.state() {
            OperationState::Completed { result } => Some(result),
            _ => None,
        }
    }

    /// Gets the operation error if failed.
    pub fn error(&self) -> Option<crate::error::NpioError> {
        match self.state() {
            OperationState::Failed { error, .. } => Some(error),
            _ => None,
        }
    }

    /// Gets the cancellation reason if cancelled.
    pub fn cancellation_reason(&self) -> Option<crate::mount::advanced::CancellationReason> {
        match self.state() {
            OperationState::Cancelled { reason } => Some(reason),
            _ => None,
        }
    }

    /// Gets the retry count for failed operations.
    pub fn retry_count(&self) -> u32 {
        match self.state() {
            OperationState::Failed { retry_count, .. } => retry_count,
            OperationState::Retrying { attempt, .. } => attempt,
            _ => 0,
        }
    }

    /// Gets the next retry time if operation is retrying.
    pub fn next_retry_time(&self) -> Option<Instant> {
        match self.state() {
            OperationState::Retrying { next_retry, .. } => Some(next_retry),
            _ => None,
        }
    }

    /// Gets a comprehensive status summary of the operation.
    pub fn status_summary(&self) -> OperationStatusSummary {
        let state = self.state();
        OperationStatusSummary {
            id: self.id,
            operation_type: self.operation_type.clone(),
            state: state.clone(),
            progress: state.progress(),
            message: state.message(),
            elapsed: self.elapsed(),
            is_terminal: state.is_terminal(),
            is_cancelled: self.is_cancelled(),
            retry_count: self.retry_count(),
        }
    }

    /// Checks if the operation is currently running (not pending, not terminal).
    pub fn is_running(&self) -> bool {
        matches!(self.state(), 
            OperationState::Validating |
            OperationState::InProgress { .. } |
            OperationState::Retrying { .. }
        )
    }

    /// Checks if the operation completed successfully.
    pub fn is_successful(&self) -> bool {
        matches!(self.state(), OperationState::Completed { .. })
    }

    /// Checks if the operation failed permanently.
    pub fn is_failed(&self) -> bool {
        matches!(self.state(), OperationState::Failed { .. })
    }

    /// Performs resource cleanup when operation completes.
    /// This should be called when the operation reaches a terminal state.
    pub fn cleanup(&self) {
        // Only cleanup if in terminal state
        if !self.is_terminal() {
            return;
        }

        // Cancel any remaining operations if not already cancelled
        if !self.cancellation_manager.is_cancelled() {
            self.cancellation_manager.cancel(crate::mount::advanced::CancellationReason::SystemShutdown);
        }

        // Emit final progress event if operation completed successfully
        if let OperationState::Completed { .. } = self.state() {
            let _ = self.progress_reporter.report_completion();
        }

        // Note: Progress reporter cleanup is handled automatically when dropped
        // Note: Cancellation manager cleanup is handled automatically when dropped
        // Additional cleanup can be added here as needed
    }

    /// Forces cleanup regardless of operation state.
    /// This should only be used in emergency situations or during shutdown.
    pub fn force_cleanup(&self) {
        // Force cancellation regardless of current state
        self.cancellation_manager.cancel(crate::mount::advanced::CancellationReason::SystemShutdown);
        
        // Force completion event if not already terminal
        if !self.is_terminal() {
            let _ = self.progress_reporter.report_completion();
        }
        
        // Additional forced cleanup can be added here as needed
    }

    /// Performs automatic cleanup when operation reaches terminal state.
    /// This is called internally when state changes to a terminal state.
    pub(crate) fn auto_cleanup_on_completion(&self) {
        if self.is_terminal() {
            self.cleanup();
        }
    }
}

impl Drop for OperationContext {
    fn drop(&mut self) {
        // Perform cleanup when the context is dropped
        self.cleanup();
    }
}