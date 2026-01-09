//! Advanced mount operations with progress reporting, cancellation, and retry logic.
//!
//! This module provides enhanced mount operation capabilities including:
//! - Progress reporting through callbacks and streams
//! - Graceful and forced cancellation
//! - Pre-operation validation
//! - Automatic retry with configurable policies
//! - Operation context management

pub mod types;
pub mod context;
pub mod progress;
pub mod cancellation;
pub mod validation;
pub mod retry;
pub mod error;
pub mod config;

pub use types::*;
pub use context::OperationContext;
pub use progress::{ProgressReporter, ProgressEvent, OperationStage};
pub use cancellation::{CancellationManager, CancellationToken};
pub use validation::{MountValidator, ValidationResult, ValidationError, ValidationWarning, ValidationMetadata};
pub use retry::{RetryPolicy, BackoffStrategy};
pub use error::{AdvancedMountError, ErrorClass};
pub use config::*;