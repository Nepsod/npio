//! Core types and enums for advanced mount operations.

use std::fmt;
use std::time::Instant;
use uuid::Uuid;
use crate::error::NpioError;

/// Unique identifier for mount operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationId(Uuid);

impl OperationId {
    /// Creates a new unique operation ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Gets the inner UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for OperationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Type of mount operation being performed.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    /// Mount a volume to a mount point.
    Mount {
        volume_path: String,
        mount_point: Option<String>,
    },
    /// Unmount from a mount point.
    Unmount {
        mount_point: String,
    },
    /// Eject a device.
    Eject {
        device_path: String,
    },
    /// Remount with new options.
    Remount {
        mount_point: String,
        options: MountOptions,
    },
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperationType::Mount { volume_path, mount_point } => {
                write!(f, "Mount {} to {}", volume_path, 
                       mount_point.as_deref().unwrap_or("auto"))
            }
            OperationType::Unmount { mount_point } => {
                write!(f, "Unmount {}", mount_point)
            }
            OperationType::Eject { device_path } => {
                write!(f, "Eject {}", device_path)
            }
            OperationType::Remount { mount_point, .. } => {
                write!(f, "Remount {}", mount_point)
            }
        }
    }
}

/// Mount options for remount operations.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MountOptions {
    pub read_only: Option<bool>,
    pub no_exec: Option<bool>,
    pub no_suid: Option<bool>,
    pub no_dev: Option<bool>,
    pub sync: Option<bool>,
    pub custom_options: Vec<String>,
}

impl MountOptions {
    /// Creates new empty mount options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets read-only flag.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = Some(read_only);
        self
    }

    /// Sets no-exec flag.
    pub fn no_exec(mut self, no_exec: bool) -> Self {
        self.no_exec = Some(no_exec);
        self
    }

    /// Sets no-suid flag.
    pub fn no_suid(mut self, no_suid: bool) -> Self {
        self.no_suid = Some(no_suid);
        self
    }

    /// Sets no-dev flag.
    pub fn no_dev(mut self, no_dev: bool) -> Self {
        self.no_dev = Some(no_dev);
        self
    }

    /// Sets sync flag.
    pub fn sync(mut self, sync: bool) -> Self {
        self.sync = Some(sync);
        self
    }

    /// Adds a custom option.
    pub fn add_custom_option(mut self, option: String) -> Self {
        self.custom_options.push(option);
        self
    }
}

/// Current state of a mount operation.
#[derive(Debug, Clone)]
pub enum OperationState {
    /// Operation is pending and hasn't started yet.
    Pending,
    /// Operation is validating prerequisites.
    Validating,
    /// Operation is in progress.
    InProgress {
        progress: f32,
        message: String,
    },
    /// Operation is retrying after a failure.
    Retrying {
        attempt: u32,
        next_retry: Instant,
    },
    /// Operation completed successfully.
    Completed {
        result: OperationResult,
    },
    /// Operation was cancelled.
    Cancelled {
        reason: CancellationReason,
    },
    /// Operation failed permanently.
    Failed {
        error: NpioError,
        retry_count: u32,
    },
}

impl OperationState {
    /// Checks if the operation is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, 
            OperationState::Completed { .. } |
            OperationState::Cancelled { .. } |
            OperationState::Failed { .. }
        )
    }

    /// Gets the current progress if available.
    pub fn progress(&self) -> Option<f32> {
        match self {
            OperationState::InProgress { progress, .. } => Some(*progress),
            OperationState::Completed { .. } => Some(1.0),
            _ => None,
        }
    }

    /// Gets the current status message.
    pub fn message(&self) -> String {
        match self {
            OperationState::Pending => "Pending".to_string(),
            OperationState::Validating => "Validating".to_string(),
            OperationState::InProgress { message, .. } => message.clone(),
            OperationState::Retrying { attempt, .. } => {
                format!("Retrying (attempt {})", attempt)
            }
            OperationState::Completed { .. } => "Completed".to_string(),
            OperationState::Cancelled { reason } => {
                format!("Cancelled: {:?}", reason)
            }
            OperationState::Failed { error, .. } => {
                format!("Failed: {}", error)
            }
        }
    }
}

/// Result of a completed operation.
#[derive(Debug, Clone)]
pub struct OperationResult {
    pub operation_id: OperationId,
    pub operation_type: OperationType,
    pub duration: std::time::Duration,
    pub metadata: OperationMetadata,
}

/// Metadata collected during operation execution.
#[derive(Debug, Clone, Default)]
pub struct OperationMetadata {
    pub filesystem_type: Option<String>,
    pub device_info: Option<DeviceInfo>,
    pub mount_point: Option<String>,
    pub bytes_processed: Option<u64>,
    pub retry_count: u32,
    pub validation_warnings: Vec<String>,
}

/// Comprehensive status summary for an operation.
#[derive(Debug, Clone)]
pub struct OperationStatusSummary {
    pub id: OperationId,
    pub operation_type: OperationType,
    pub state: OperationState,
    pub progress: Option<f32>,
    pub message: String,
    pub elapsed: std::time::Duration,
    pub is_terminal: bool,
    pub is_cancelled: bool,
    pub retry_count: u32,
}

/// Device information collected during operations.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_path: String,
    pub device_name: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub size: Option<u64>,
    pub removable: bool,
}

/// Cancellation reason for cancelled operations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CancellationReason {
    /// User requested cancellation.
    UserRequested,
    /// Operation timed out.
    Timeout,
    /// System is shutting down.
    SystemShutdown,
    /// Parent operation was cancelled.
    ParentCancelled,
}

impl fmt::Display for CancellationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CancellationReason::UserRequested => write!(f, "User requested"),
            CancellationReason::Timeout => write!(f, "Timeout"),
            CancellationReason::SystemShutdown => write!(f, "System shutdown"),
            CancellationReason::ParentCancelled => write!(f, "Parent cancelled"),
        }
    }
}