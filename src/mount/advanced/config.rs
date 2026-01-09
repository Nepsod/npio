//! Configuration structures for advanced mount operations.

use std::time::Duration;
use crate::error::IOErrorEnum;
use crate::mount::advanced::BackoffStrategy;

/// Main configuration for advanced mount operations.
#[derive(Debug, Clone)]
pub struct OperationConfig {
    pub progress_config: ProgressConfig,
    pub cancellation_config: CancellationConfig,
    pub validation_config: ValidationConfig,
    pub retry_config: RetryConfig,
}

impl Default for OperationConfig {
    fn default() -> Self {
        Self {
            progress_config: ProgressConfig::default(),
            cancellation_config: CancellationConfig::default(),
            validation_config: ValidationConfig::default(),
            retry_config: RetryConfig::default(),
        }
    }
}

/// Configuration for progress reporting.
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    pub report_interval: Duration,
    pub enable_callbacks: bool,
    pub enable_streams: bool,
    pub buffer_size: usize,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            report_interval: Duration::from_millis(100),
            enable_callbacks: true,
            enable_streams: true,
            buffer_size: 100,
        }
    }
}

/// Configuration for cancellation behavior.
#[derive(Debug, Clone)]
pub struct CancellationConfig {
    pub graceful_timeout: Duration,
    pub force_timeout: Duration,
    pub cleanup_timeout: Duration,
}

impl Default for CancellationConfig {
    fn default() -> Self {
        Self {
            graceful_timeout: Duration::from_secs(5),
            force_timeout: Duration::from_secs(10),
            cleanup_timeout: Duration::from_secs(3),
        }
    }
}

/// Configuration for validation checks.
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub check_mount_point_exists: bool,
    pub check_mount_point_available: bool,
    pub check_permissions: bool,
    pub check_filesystem: bool,
    pub check_device_availability: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            check_mount_point_exists: true,
            check_mount_point_available: true,
            check_permissions: true,
            check_filesystem: true,
            check_device_availability: true,
        }
    }
}

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_strategy: BackoffStrategy,
    pub transient_errors: Vec<IOErrorEnum>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::default(),
            transient_errors: vec![
                IOErrorEnum::Busy,
                IOErrorEnum::TimedOut,
                IOErrorEnum::Interrupted,
                IOErrorEnum::ConnectionRefused,
                IOErrorEnum::ConnectionClosed,
                IOErrorEnum::NetworkUnreachable,
                IOErrorEnum::HostUnreachable,
                IOErrorEnum::WouldBlock,
            ],
        }
    }
}