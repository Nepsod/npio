//! Enhanced error types for advanced mount operations.

use std::fmt;
use crate::error::{NpioError, IOErrorEnum};

/// Enhanced error type for advanced mount operations.
#[derive(Debug)]
pub struct AdvancedMountError {
    /// The underlying npio error.
    pub inner: NpioError,
    /// Error classification for retry decisions.
    pub class: ErrorClass,
    /// Recovery suggestions if available.
    pub recovery_suggestions: Vec<String>,
    /// Additional context specific to advanced operations.
    pub context: ErrorContext,
}

impl AdvancedMountError {
    /// Creates a new advanced mount error.
    pub fn new(
        inner: NpioError,
        class: ErrorClass,
        context: ErrorContext,
    ) -> Self {
        Self {
            inner,
            class,
            recovery_suggestions: Vec::new(),
            context,
        }
    }

    /// Creates a new advanced mount error with recovery suggestions.
    pub fn with_suggestions(
        inner: NpioError,
        class: ErrorClass,
        context: ErrorContext,
        suggestions: Vec<String>,
    ) -> Self {
        Self {
            inner,
            class,
            recovery_suggestions: suggestions,
            context,
        }
    }

    /// Creates a transient error.
    pub fn transient(inner: NpioError, context: ErrorContext) -> Self {
        Self::new(inner, ErrorClass::Transient, context)
    }

    /// Creates a permanent error.
    pub fn permanent(inner: NpioError, context: ErrorContext) -> Self {
        Self::new(inner, ErrorClass::Permanent, context)
    }

    /// Gets the error classification.
    pub fn class(&self) -> ErrorClass {
        self.class
    }

    /// Checks if this error should be retried.
    pub fn should_retry(&self) -> bool {
        matches!(self.class, ErrorClass::Transient)
    }

    /// Gets recovery suggestions.
    pub fn recovery_suggestions(&self) -> &[String] {
        &self.recovery_suggestions
    }

    /// Gets the error context.
    pub fn context(&self) -> &ErrorContext {
        &self.context
    }
}

impl fmt::Display for AdvancedMountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.inner, self.class)?;
        
        if !self.recovery_suggestions.is_empty() {
            write!(f, "\nSuggestions:")?;
            for suggestion in &self.recovery_suggestions {
                write!(f, "\n  - {}", suggestion)?;
            }
        }
        
        Ok(())
    }
}

impl std::error::Error for AdvancedMountError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner)
    }
}

impl From<NpioError> for AdvancedMountError {
    fn from(error: NpioError) -> Self {
        let class = ErrorClassifier::classify(&error);
        Self::new(error, class, ErrorContext::Unknown)
    }
}

/// Classification of errors for retry decisions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorClass {
    /// Error is transient and should be retried.
    Transient,
    /// Error is permanent and should not be retried.
    Permanent,
    /// Error classification is unknown, use default policy.
    Unknown,
}

impl fmt::Display for ErrorClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorClass::Transient => write!(f, "transient"),
            ErrorClass::Permanent => write!(f, "permanent"),
            ErrorClass::Unknown => write!(f, "unknown"),
        }
    }
}

/// Additional context for advanced mount errors.
#[derive(Debug, Clone)]
pub enum ErrorContext {
    /// Error occurred during validation phase.
    Validation {
        check_type: String,
        target: String,
    },
    /// Error occurred during mount operation.
    Mount {
        source: String,
        target: String,
        filesystem_type: Option<String>,
    },
    /// Error occurred during unmount operation.
    Unmount {
        target: String,
    },
    /// Error occurred during eject operation.
    Eject {
        device: String,
    },
    /// Error occurred during D-Bus communication.
    DBus {
        method: String,
        object_path: String,
    },
    /// Error occurred during system call.
    SystemCall {
        call: String,
        errno: Option<i32>,
    },
    /// Error occurred during cancellation.
    Cancellation {
        phase: String,
    },
    /// Unknown context.
    Unknown,
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorContext::Validation { check_type, target } => {
                write!(f, "validation {} for {}", check_type, target)
            }
            ErrorContext::Mount { source, target, filesystem_type } => {
                if let Some(fs_type) = filesystem_type {
                    write!(f, "mount {} to {} ({})", source, target, fs_type)
                } else {
                    write!(f, "mount {} to {}", source, target)
                }
            }
            ErrorContext::Unmount { target } => {
                write!(f, "unmount {}", target)
            }
            ErrorContext::Eject { device } => {
                write!(f, "eject {}", device)
            }
            ErrorContext::DBus { method, object_path } => {
                write!(f, "D-Bus {} on {}", method, object_path)
            }
            ErrorContext::SystemCall { call, errno } => {
                if let Some(errno) = errno {
                    write!(f, "system call {} (errno: {})", call, errno)
                } else {
                    write!(f, "system call {}", call)
                }
            }
            ErrorContext::Cancellation { phase } => {
                write!(f, "cancellation during {}", phase)
            }
            ErrorContext::Unknown => {
                write!(f, "unknown context")
            }
        }
    }
}

/// Classifier for determining error retry behavior.
#[derive(Debug)]
pub struct ErrorClassifier;

impl ErrorClassifier {
    /// Classifies an error for retry decisions.
    pub fn classify(error: &NpioError) -> ErrorClass {
        match error.kind() {
            // Permanent errors - don't retry
            IOErrorEnum::NotFound => ErrorClass::Permanent,
            IOErrorEnum::PermissionDenied => ErrorClass::Permanent,
            IOErrorEnum::InvalidArg => ErrorClass::Permanent,
            IOErrorEnum::NotSupported => ErrorClass::Permanent,
            IOErrorEnum::IsDirectory => ErrorClass::Permanent,
            IOErrorEnum::NotDirectory => ErrorClass::Permanent,
            
            // Transient errors - should retry
            IOErrorEnum::Busy => ErrorClass::Transient,
            IOErrorEnum::TimedOut => ErrorClass::Transient,
            IOErrorEnum::Interrupted => ErrorClass::Transient,
            IOErrorEnum::ConnectionRefused => ErrorClass::Transient,
            IOErrorEnum::ConnectionClosed => ErrorClass::Transient,
            IOErrorEnum::NetworkUnreachable => ErrorClass::Transient,
            IOErrorEnum::HostUnreachable => ErrorClass::Transient,
            IOErrorEnum::WouldBlock => ErrorClass::Transient,
            
            // Cancellation is permanent (don't retry cancelled operations)
            IOErrorEnum::Cancelled => ErrorClass::Permanent,
            
            // Unknown classification for other errors
            _ => ErrorClass::Unknown,
        }
    }

    /// Classifies an error and provides enhanced error information.
    pub fn classify_with_context(
        error: &NpioError,
        context: &ErrorContext,
    ) -> AdvancedMountError {
        let class = Self::classify(error);
        let suggestions = Self::generate_recovery_suggestions(error, context);
        
        AdvancedMountError::with_suggestions(
            error.clone(),
            class,
            context.clone(),
            suggestions,
        )
    }

    /// Generates recovery suggestions based on error and context.
    pub fn generate_recovery_suggestions(
        error: &NpioError,
        context: &ErrorContext,
    ) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Add context-specific suggestions
        match context {
            ErrorContext::Mount { source, target, filesystem_type } => {
                suggestions.push(format!("Verify source '{}' exists and is accessible", source));
                suggestions.push(format!("Check that mount point '{}' exists and is empty", target));
                
                if let Some(fs_type) = filesystem_type {
                    suggestions.push(format!("Ensure '{}' filesystem support is available", fs_type));
                }
            }
            ErrorContext::Unmount { target } => {
                suggestions.push(format!("Verify '{}' is actually mounted", target));
                suggestions.push("Check for processes using the mount point".to_string());
                suggestions.push("Use 'lsof' or 'fuser' to identify blocking processes".to_string());
            }
            ErrorContext::Eject { device } => {
                suggestions.push(format!("Ensure device '{}' is not in use", device));
                suggestions.push("Unmount all filesystems on the device first".to_string());
            }
            ErrorContext::DBus { method, .. } => {
                suggestions.push("Check if UDisks2 service is running".to_string());
                suggestions.push(format!("Verify D-Bus method '{}' is supported", method));
            }
            ErrorContext::SystemCall { call, errno } => {
                if let Some(errno_val) = errno {
                    let (_, _, errno_suggestions) = Self::map_unix_error(*errno_val);
                    suggestions.extend(errno_suggestions);
                } else {
                    suggestions.push(format!("Check system call '{}' parameters", call));
                }
            }
            _ => {}
        }

        // Add error-specific suggestions
        match error.kind() {
            IOErrorEnum::PermissionDenied => {
                suggestions.push("Run with elevated privileges (sudo)".to_string());
                suggestions.push("Check file and directory permissions".to_string());
            }
            IOErrorEnum::Busy => {
                suggestions.push("Wait for other operations to complete".to_string());
                suggestions.push("Check for processes using the resource".to_string());
            }
            IOErrorEnum::NotFound => {
                suggestions.push("Verify all paths exist".to_string());
                suggestions.push("Check device connectivity".to_string());
            }
            IOErrorEnum::TimedOut => {
                suggestions.push("Increase timeout values".to_string());
                suggestions.push("Check network connectivity".to_string());
            }
            _ => {}
        }

        // Remove duplicates and return
        suggestions.sort();
        suggestions.dedup();
        suggestions
    }

    /// Determines if an error should be retried based on classification and context.
    pub fn should_retry(error: &NpioError, retry_count: u32, max_retries: u32) -> bool {
        if retry_count >= max_retries {
            return false;
        }

        match Self::classify(error) {
            ErrorClass::Transient => true,
            ErrorClass::Permanent => false,
            ErrorClass::Unknown => {
                // For unknown errors, allow limited retries
                retry_count < (max_retries / 2).max(1)
            }
        }
    }

    /// Converts a validation error to an advanced mount error with enhanced suggestions.
    pub fn from_validation_error(
        validation_error: crate::mount::advanced::validation::ValidationError,
        context: ErrorContext,
    ) -> AdvancedMountError {
        use crate::mount::advanced::validation::ValidationError;
        use crate::error::IOErrorEnum;

        let (npio_error, class) = match &validation_error {
            ValidationError::MountPointNotFound { .. } => {
                (NpioError::new(IOErrorEnum::NotFound, validation_error.message()), ErrorClass::Permanent)
            }
            ValidationError::MountPointInUse { .. } => {
                (NpioError::new(IOErrorEnum::Busy, validation_error.message()), ErrorClass::Transient)
            }
            ValidationError::MountPointNotDirectory { .. } => {
                (NpioError::new(IOErrorEnum::NotDirectory, validation_error.message()), ErrorClass::Permanent)
            }
            ValidationError::MountPointNotMounted { .. } => {
                (NpioError::new(IOErrorEnum::NotFound, validation_error.message()), ErrorClass::Permanent)
            }
            ValidationError::InsufficientPermissions { .. } => {
                (NpioError::new(IOErrorEnum::PermissionDenied, validation_error.message()), ErrorClass::Permanent)
            }
            ValidationError::InvalidFilesystem { .. } => {
                (NpioError::new(IOErrorEnum::NotSupported, validation_error.message()), ErrorClass::Permanent)
            }
            ValidationError::DeviceNotFound { .. } => {
                (NpioError::new(IOErrorEnum::NotFound, validation_error.message()), ErrorClass::Permanent)
            }
            ValidationError::SystemError { .. } => {
                (NpioError::new(IOErrorEnum::Failed, validation_error.message()), ErrorClass::Unknown)
            }
        };

        let mut suggestions = validation_error.recovery_suggestions();
        
        // Add context-specific suggestions
        let context_suggestions = Self::generate_recovery_suggestions(&npio_error, &context);
        suggestions.extend(context_suggestions);
        
        // Remove duplicates
        suggestions.sort();
        suggestions.dedup();

        AdvancedMountError::with_suggestions(npio_error, class, context, suggestions)
    }

    /// Converts a validation warning to recovery suggestions.
    pub fn from_validation_warning(
        validation_warning: crate::mount::advanced::validation::ValidationWarning,
    ) -> Vec<String> {
        validation_warning.recovery_suggestions()
    }

    /// Creates an enhanced error with comprehensive recovery information.
    pub fn create_enhanced_error(
        error: NpioError,
        context: ErrorContext,
        additional_info: Option<String>,
    ) -> AdvancedMountError {
        let class = Self::classify(&error);
        let mut suggestions = Self::generate_recovery_suggestions(&error, &context);
        
        // Add additional context-specific information
        if let Some(info) = additional_info {
            suggestions.insert(0, format!("Additional context: {}", info));
        }
        
        // Add general troubleshooting suggestions based on error class
        match class {
            ErrorClass::Transient => {
                suggestions.push("This error is typically temporary - retrying may succeed".to_string());
                suggestions.push("Check system resources and try again in a moment".to_string());
            }
            ErrorClass::Permanent => {
                suggestions.push("This error requires manual intervention before retrying".to_string());
                suggestions.push("Address the underlying issue before attempting the operation again".to_string());
            }
            ErrorClass::Unknown => {
                suggestions.push("Error classification is uncertain - check system logs for more details".to_string());
                suggestions.push("Consider reporting this error if it persists".to_string());
            }
        }
        
        AdvancedMountError::with_suggestions(error, class, context, suggestions)
    }

    /// Extracts detailed error information from UDisks2 D-Bus responses.
    pub fn extract_udisks2_error(dbus_error: &str) -> (ErrorClass, Vec<String>) {
        let mut suggestions = Vec::new();
        
        let class = if dbus_error.contains("org.freedesktop.UDisks2.Error.DeviceBusy") {
            suggestions.push("Wait for other processes to finish using the device".to_string());
            suggestions.push("Use 'lsof' or 'fuser' to identify processes using the device".to_string());
            ErrorClass::Transient
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.NotAuthorized") {
            suggestions.push("Run with appropriate privileges (sudo)".to_string());
            suggestions.push("Check PolicyKit rules for mount permissions".to_string());
            ErrorClass::Permanent
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.NotSupported") {
            suggestions.push("Check if the filesystem type is supported".to_string());
            suggestions.push("Install required filesystem drivers".to_string());
            ErrorClass::Permanent
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.AlreadyMounted") {
            suggestions.push("Unmount the device first".to_string());
            suggestions.push("Check existing mount points with 'mount' command".to_string());
            ErrorClass::Permanent
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.NotMounted") {
            suggestions.push("Verify the device is actually mounted".to_string());
            suggestions.push("Check mount points with 'mount' or 'findmnt' command".to_string());
            ErrorClass::Permanent
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.MountPointNotEmpty") {
            suggestions.push("Use an empty directory as mount point".to_string());
            suggestions.push("Create a new directory for mounting".to_string());
            ErrorClass::Permanent
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.NoFilesystem") {
            suggestions.push("Format the device with a supported filesystem".to_string());
            suggestions.push("Check if the device contains a valid filesystem".to_string());
            ErrorClass::Permanent
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.WrongPassphrase") {
            suggestions.push("Provide the correct passphrase for encrypted device".to_string());
            ErrorClass::Permanent
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.Cancelled") {
            suggestions.push("Operation was cancelled by user or system".to_string());
            ErrorClass::Permanent
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.TimedOut") {
            suggestions.push("Increase timeout values".to_string());
            suggestions.push("Check device responsiveness".to_string());
            ErrorClass::Transient
        } else if dbus_error.contains("org.freedesktop.UDisks2.Error.Failed") {
            // Generic failure - could be transient, so allow retry
            suggestions.push("Check system logs for more details".to_string());
            suggestions.push("Verify device connectivity and health".to_string());
            ErrorClass::Transient
        } else if dbus_error.contains("org.freedesktop.DBus.Error.ServiceUnknown") {
            suggestions.push("Ensure UDisks2 service is running".to_string());
            suggestions.push("Install udisks2 package if missing".to_string());
            ErrorClass::Transient
        } else if dbus_error.contains("org.freedesktop.DBus.Error.NoReply") {
            suggestions.push("UDisks2 service may be unresponsive".to_string());
            suggestions.push("Restart UDisks2 service".to_string());
            ErrorClass::Transient
        } else {
            suggestions.push("Check UDisks2 logs for more information".to_string());
            ErrorClass::Unknown
        };

        (class, suggestions)
    }

    /// Maps Unix error codes to descriptive messages and classifications.
    pub fn map_unix_error(errno: i32) -> (ErrorClass, String, Vec<String>) {
        match errno {
            libc::EBUSY => (
                ErrorClass::Transient,
                "Device or resource busy".to_string(),
                vec![
                    "Wait for other processes to finish using the device".to_string(),
                    "Use 'lsof' or 'fuser' to identify processes using the device".to_string(),
                    "Try unmounting with 'lazy' option if available".to_string(),
                ],
            ),
            libc::EACCES => (
                ErrorClass::Permanent,
                "Permission denied".to_string(),
                vec![
                    "Run with appropriate privileges (sudo)".to_string(),
                    "Check file permissions on mount point and device".to_string(),
                    "Verify user is in required groups (disk, storage)".to_string(),
                ],
            ),
            libc::ENOENT => (
                ErrorClass::Permanent,
                "No such file or directory".to_string(),
                vec![
                    "Check that the device or mount point exists".to_string(),
                    "Verify the device path is correct".to_string(),
                    "Create the mount point directory if missing".to_string(),
                ],
            ),
            libc::EINVAL => (
                ErrorClass::Permanent,
                "Invalid argument".to_string(),
                vec![
                    "Check mount options and filesystem type".to_string(),
                    "Verify mount options are supported by the filesystem".to_string(),
                    "Check device format and filesystem integrity".to_string(),
                ],
            ),
            libc::ENOTDIR => (
                ErrorClass::Permanent,
                "Not a directory".to_string(),
                vec![
                    "Mount point must be a directory".to_string(),
                    "Remove existing file and create directory".to_string(),
                ],
            ),
            libc::ENOTBLK => (
                ErrorClass::Permanent,
                "Block device required".to_string(),
                vec![
                    "Source must be a block device for this filesystem type".to_string(),
                    "Use 'file' command to check device type".to_string(),
                ],
            ),
            libc::ENXIO => (
                ErrorClass::Permanent,
                "No such device or address".to_string(),
                vec![
                    "Check that the device exists and is accessible".to_string(),
                    "Verify device is properly connected".to_string(),
                    "Check if device driver is loaded".to_string(),
                ],
            ),
            libc::EROFS => (
                ErrorClass::Permanent,
                "Read-only file system".to_string(),
                vec![
                    "Use read-only mount option".to_string(),
                    "Check device write protection switch".to_string(),
                    "Verify filesystem is not corrupted".to_string(),
                ],
            ),
            libc::EMFILE => (
                ErrorClass::Transient,
                "Too many open files in process".to_string(),
                vec![
                    "Close some files in the current process".to_string(),
                    "Increase per-process file descriptor limit".to_string(),
                ],
            ),
            libc::ENFILE => (
                ErrorClass::Transient,
                "Too many open files in system".to_string(),
                vec![
                    "Wait for system resources to become available".to_string(),
                    "Increase system-wide file descriptor limit".to_string(),
                ],
            ),
            libc::ENOMEM => (
                ErrorClass::Transient,
                "Out of memory".to_string(),
                vec![
                    "Free up system memory and try again".to_string(),
                    "Close unnecessary applications".to_string(),
                    "Check for memory leaks in running processes".to_string(),
                ],
            ),
            libc::ELOOP => (
                ErrorClass::Permanent,
                "Too many symbolic links".to_string(),
                vec![
                    "Check for circular symbolic links in the path".to_string(),
                    "Use absolute paths instead of symbolic links".to_string(),
                ],
            ),
            libc::ENAMETOOLONG => (
                ErrorClass::Permanent,
                "File name too long".to_string(),
                vec![
                    "Use shorter path names".to_string(),
                    "Mount closer to filesystem root".to_string(),
                ],
            ),
            libc::ENODEV => (
                ErrorClass::Permanent,
                "No such device".to_string(),
                vec![
                    "Check if device driver is loaded".to_string(),
                    "Verify device is properly recognized by kernel".to_string(),
                    "Check 'dmesg' for device-related errors".to_string(),
                ],
            ),
            libc::ENOSPC => (
                ErrorClass::Permanent,
                "No space left on device".to_string(),
                vec![
                    "Free up space on the target filesystem".to_string(),
                    "Check available disk space with 'df'".to_string(),
                ],
            ),
            libc::ESPIPE => (
                ErrorClass::Permanent,
                "Illegal seek".to_string(),
                vec![
                    "Device does not support seeking operations".to_string(),
                    "Check if device is appropriate for filesystem".to_string(),
                ],
            ),
            libc::EEXIST => (
                ErrorClass::Permanent,
                "File exists".to_string(),
                vec![
                    "Mount point is already in use".to_string(),
                    "Use a different mount point".to_string(),
                    "Unmount existing filesystem first".to_string(),
                ],
            ),
            libc::ENOTEMPTY => (
                ErrorClass::Permanent,
                "Directory not empty".to_string(),
                vec![
                    "Use an empty directory as mount point".to_string(),
                    "Create a new directory for mounting".to_string(),
                ],
            ),
            libc::ENOTTY => (
                ErrorClass::Permanent,
                "Inappropriate ioctl for device".to_string(),
                vec![
                    "Device does not support required operations".to_string(),
                    "Check device type and capabilities".to_string(),
                ],
            ),
            libc::ETXTBSY => (
                ErrorClass::Transient,
                "Text file busy".to_string(),
                vec![
                    "Wait for processes using the file to finish".to_string(),
                    "Stop processes that have the file open for execution".to_string(),
                ],
            ),
            libc::EFAULT => (
                ErrorClass::Permanent,
                "Bad address".to_string(),
                vec![
                    "Internal error - invalid memory access".to_string(),
                    "Report this as a potential bug".to_string(),
                ],
            ),
            libc::EIO => (
                ErrorClass::Transient,
                "Input/output error".to_string(),
                vec![
                    "Check device health and connectivity".to_string(),
                    "Run filesystem check (fsck) if safe to do so".to_string(),
                    "Check system logs for hardware errors".to_string(),
                ],
            ),
            libc::EAGAIN => (
                ErrorClass::Transient,
                "Resource temporarily unavailable".to_string(),
                vec![
                    "Retry the operation after a short delay".to_string(),
                    "Check system load and resource availability".to_string(),
                ],
            ),
            libc::EINTR => (
                ErrorClass::Transient,
                "Interrupted system call".to_string(),
                vec![
                    "Retry the operation".to_string(),
                    "Operation was interrupted by signal".to_string(),
                ],
            ),
            libc::EPERM => (
                ErrorClass::Permanent,
                "Operation not permitted".to_string(),
                vec![
                    "Run with appropriate privileges (sudo)".to_string(),
                    "Check security policies and capabilities".to_string(),
                    "Verify operation is allowed by system configuration".to_string(),
                ],
            ),
            _ => (
                ErrorClass::Unknown,
                format!("System error (errno: {})", errno),
                vec![
                    "Check system logs for more details".to_string(),
                    format!("Look up errno {} in system documentation", errno),
                    "Contact system administrator if error persists".to_string(),
                ],
            ),
        }
    }
}

/// Comprehensive error recovery helper for advanced mount operations.
#[derive(Debug)]
pub struct ErrorRecoveryHelper;

impl ErrorRecoveryHelper {
    /// Analyzes an error and provides comprehensive recovery guidance.
    pub fn analyze_error(
        error: &NpioError,
        context: &ErrorContext,
        operation_metadata: Option<&crate::mount::advanced::types::OperationMetadata>,
    ) -> ErrorRecoveryPlan {
        let class = ErrorClassifier::classify(error);
        let base_suggestions = ErrorClassifier::generate_recovery_suggestions(error, context);
        
        let mut recovery_plan = ErrorRecoveryPlan {
            error_class: class,
            immediate_actions: Vec::new(),
            diagnostic_steps: Vec::new(),
            prevention_measures: Vec::new(),
            alternative_approaches: Vec::new(),
            estimated_fix_time: None,
            requires_system_changes: false,
        };

        // Add immediate actions based on error type
        match error.kind() {
            IOErrorEnum::PermissionDenied => {
                recovery_plan.immediate_actions.push("Run with elevated privileges (sudo)".to_string());
                recovery_plan.immediate_actions.push("Check file and directory permissions".to_string());
                recovery_plan.requires_system_changes = true;
                recovery_plan.estimated_fix_time = Some("1-5 minutes".to_string());
            }
            IOErrorEnum::Busy => {
                recovery_plan.immediate_actions.push("Wait for other processes to finish".to_string());
                recovery_plan.immediate_actions.push("Identify processes using the resource".to_string());
                recovery_plan.estimated_fix_time = Some("30 seconds - 5 minutes".to_string());
            }
            IOErrorEnum::NotFound => {
                recovery_plan.immediate_actions.push("Verify all paths exist".to_string());
                recovery_plan.immediate_actions.push("Check device connectivity".to_string());
                recovery_plan.requires_system_changes = true;
                recovery_plan.estimated_fix_time = Some("1-10 minutes".to_string());
            }
            _ => {
                recovery_plan.immediate_actions.extend(base_suggestions);
            }
        }

        // Add diagnostic steps
        recovery_plan.diagnostic_steps = Self::generate_diagnostic_steps(error, context);
        
        // Add prevention measures
        recovery_plan.prevention_measures = Self::generate_prevention_measures(error, context);
        
        // Add alternative approaches
        recovery_plan.alternative_approaches = Self::generate_alternatives(error, context, operation_metadata);

        recovery_plan
    }

    /// Generates diagnostic steps for troubleshooting.
    fn generate_diagnostic_steps(_error: &NpioError, context: &ErrorContext) -> Vec<String> {
        let mut steps = Vec::new();
        
        // Add context-specific diagnostics
        match context {
            ErrorContext::Mount { source, target, .. } => {
                steps.push(format!("Check if source '{}' exists: ls -l '{}'", source, target));
                steps.push(format!("Check if target '{}' exists: ls -ld '{}'", target, target));
                steps.push("Check current mounts: mount | grep -E '(source|target)'".to_string());
            }
            ErrorContext::Unmount { target } => {
                steps.push(format!("Check if '{}' is mounted: mount | grep '{}'", target, target));
                steps.push(format!("Check processes using mount point: lsof '{}'", target));
                steps.push(format!("Check processes using mount point: fuser -v '{}'", target));
            }
            ErrorContext::DBus { method, object_path } => {
                steps.push("Check if UDisks2 service is running: systemctl status udisks2".to_string());
                steps.push(format!("Test D-Bus connectivity: dbus-send --system --dest=org.freedesktop.UDisks2 {} {}", object_path, method));
            }
            _ => {}
        }

        // Add general diagnostic steps
        steps.push("Check system logs: journalctl -xe --no-pager".to_string());
        steps.push("Check dmesg for hardware issues: dmesg | tail -20".to_string());
        steps.push("Check available disk space: df -h".to_string());
        steps.push("Check system resources: free -h && uptime".to_string());

        steps
    }

    /// Generates prevention measures for future occurrences.
    fn generate_prevention_measures(error: &NpioError, context: &ErrorContext) -> Vec<String> {
        let mut measures = Vec::new();
        
        match error.kind() {
            IOErrorEnum::PermissionDenied => {
                measures.push("Set up proper user permissions and groups".to_string());
                measures.push("Consider using PolicyKit rules for mount operations".to_string());
                measures.push("Document required permissions for this operation".to_string());
            }
            IOErrorEnum::Busy => {
                measures.push("Implement proper resource locking in applications".to_string());
                measures.push("Add checks for resource availability before operations".to_string());
                measures.push("Use timeout mechanisms for resource acquisition".to_string());
            }
            IOErrorEnum::NotFound => {
                measures.push("Implement device detection and monitoring".to_string());
                measures.push("Add validation checks before operations".to_string());
                measures.push("Use device event notifications for dynamic updates".to_string());
            }
            IOErrorEnum::TimedOut => {
                measures.push("Increase timeout values for slow devices".to_string());
                measures.push("Implement retry mechanisms with backoff".to_string());
                measures.push("Monitor device performance and health".to_string());
            }
            _ => {
                measures.push("Implement comprehensive error handling".to_string());
                measures.push("Add logging and monitoring for this operation".to_string());
            }
        }

        // Add context-specific prevention measures
        match context {
            ErrorContext::Mount { .. } => {
                measures.push("Validate mount points and devices before operations".to_string());
                measures.push("Use mount operation queuing to prevent conflicts".to_string());
            }
            ErrorContext::DBus { .. } => {
                measures.push("Implement D-Bus service health monitoring".to_string());
                measures.push("Add fallback mechanisms for D-Bus failures".to_string());
            }
            _ => {}
        }

        measures
    }

    /// Generates alternative approaches when the primary method fails.
    fn generate_alternatives(
        error: &NpioError,
        context: &ErrorContext,
        _operation_metadata: Option<&crate::mount::advanced::types::OperationMetadata>,
    ) -> Vec<String> {
        let mut alternatives = Vec::new();
        
        match context {
            ErrorContext::Mount { source, target, filesystem_type } => {
                alternatives.push("Try mounting with different options (ro, noexec, etc.)".to_string());
                alternatives.push("Use a different mount point".to_string());
                
                if filesystem_type.is_some() {
                    alternatives.push("Try auto-detecting filesystem type instead".to_string());
                } else {
                    alternatives.push("Specify filesystem type explicitly".to_string());
                }
                
                alternatives.push(format!("Try manual mount command: mount '{}' '{}'", source, target));
                alternatives.push("Use udisksctl for user-space mounting".to_string());
            }
            ErrorContext::Unmount { target } => {
                alternatives.push("Try lazy unmount: umount -l".to_string());
                alternatives.push("Try force unmount: umount -f".to_string());
                alternatives.push(format!("Use udisksctl: udisksctl unmount -b $(findmnt -n -o SOURCE '{}')", target));
                alternatives.push("Kill processes using the mount point and retry".to_string());
            }
            ErrorContext::Eject { device: _ } => {
                alternatives.push("Try software eject: eject".to_string());
                alternatives.push("Unmount all filesystems first, then eject".to_string());
                alternatives.push("Use udisksctl power-off command".to_string());
            }
            ErrorContext::DBus { .. } => {
                alternatives.push("Use direct system calls instead of D-Bus".to_string());
                alternatives.push("Try command-line tools (mount, umount, eject)".to_string());
                alternatives.push("Restart UDisks2 service and retry".to_string());
            }
            _ => {
                alternatives.push("Try the operation with different parameters".to_string());
                alternatives.push("Use alternative tools or methods".to_string());
            }
        }

        // Add error-specific alternatives
        match error.kind() {
            IOErrorEnum::PermissionDenied => {
                alternatives.push("Use sudo or run as root".to_string());
                alternatives.push("Change file ownership or permissions".to_string());
            }
            IOErrorEnum::Busy => {
                alternatives.push("Wait and retry automatically".to_string());
                alternatives.push("Use force options if available and safe".to_string());
            }
            IOErrorEnum::TimedOut => {
                alternatives.push("Increase timeout values".to_string());
                alternatives.push("Try the operation in smaller chunks".to_string());
            }
            _ => {}
        }

        alternatives
    }
}

/// Comprehensive error recovery plan with actionable guidance.
#[derive(Debug)]
pub struct ErrorRecoveryPlan {
    pub error_class: ErrorClass,
    pub immediate_actions: Vec<String>,
    pub diagnostic_steps: Vec<String>,
    pub prevention_measures: Vec<String>,
    pub alternative_approaches: Vec<String>,
    pub estimated_fix_time: Option<String>,
    pub requires_system_changes: bool,
}

impl ErrorRecoveryPlan {
    /// Gets a formatted recovery plan as a string.
    pub fn format_plan(&self) -> String {
        let mut plan = String::new();
        
        plan.push_str(&format!("Error Classification: {}\n", self.error_class));
        
        if let Some(ref time) = self.estimated_fix_time {
            plan.push_str(&format!("Estimated Fix Time: {}\n", time));
        }
        
        if self.requires_system_changes {
            plan.push_str("⚠️  This error may require system-level changes\n");
        }
        
        if !self.immediate_actions.is_empty() {
            plan.push_str("\n🔧 Immediate Actions:\n");
            for (i, action) in self.immediate_actions.iter().enumerate() {
                plan.push_str(&format!("  {}. {}\n", i + 1, action));
            }
        }
        
        if !self.diagnostic_steps.is_empty() {
            plan.push_str("\n🔍 Diagnostic Steps:\n");
            for (i, step) in self.diagnostic_steps.iter().enumerate() {
                plan.push_str(&format!("  {}. {}\n", i + 1, step));
            }
        }
        
        if !self.alternative_approaches.is_empty() {
            plan.push_str("\n🔄 Alternative Approaches:\n");
            for (i, approach) in self.alternative_approaches.iter().enumerate() {
                plan.push_str(&format!("  {}. {}\n", i + 1, approach));
            }
        }
        
        if !self.prevention_measures.is_empty() {
            plan.push_str("\n🛡️  Prevention Measures:\n");
            for (i, measure) in self.prevention_measures.iter().enumerate() {
                plan.push_str(&format!("  {}. {}\n", i + 1, measure));
            }
        }
        
        plan
    }
}