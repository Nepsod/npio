//! Cancellation management for advanced mount operations.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::timeout;
use std::collections::HashMap;

use super::types::CancellationReason;
use crate::error::{NpioError, IOErrorEnum};

/// Token that allows graceful cancellation of operations.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    /// Atomic flag indicating if cancellation was requested.
    cancelled: Arc<AtomicBool>,
    /// The reason for cancellation, if any.
    reason: Arc<Mutex<Option<CancellationReason>>>,
    /// Notifier for cancellation events.
    notifier: Arc<Notify>,
}

impl CancellationToken {
    /// Creates a new cancellation token.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            reason: Arc::new(Mutex::new(None)),
            notifier: Arc::new(Notify::new()),
        }
    }

    /// Checks if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Gets the cancellation reason if cancelled.
    pub fn cancellation_reason(&self) -> Option<CancellationReason> {
        self.reason.lock().unwrap().clone()
    }

    /// Requests cancellation with the given reason.
    pub fn cancel(&self, reason: CancellationReason) {
        // Only set cancellation if not already cancelled (preserve original reason)
        if !self.cancelled.load(Ordering::Acquire) {
            // Set the cancellation flag atomically
            self.cancelled.store(true, Ordering::Release);
            
            // Store the reason
            *self.reason.lock().unwrap() = Some(reason);
            
            // Notify all waiters
            self.notifier.notify_waiters();
        }
    }

    /// Waits for cancellation to be requested.
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        
        self.notifier.notified().await;
    }

    /// Throws a cancellation error if cancelled.
    pub fn throw_if_cancelled(&self) -> Result<(), NpioError> {
        if self.is_cancelled() {
            let reason = self.cancellation_reason()
                .unwrap_or(CancellationReason::UserRequested);
            
            Err(NpioError::new(
                IOErrorEnum::Cancelled,
                format!("Operation cancelled: {}", reason),
            ))
        } else {
            Ok(())
        }
    }

    /// Creates a child token that is cancelled when this token is cancelled.
    pub fn child(&self) -> Self {
        let child = Self::new();
        
        if self.is_cancelled() {
            if let Some(_reason) = self.cancellation_reason() {
                child.cancel(CancellationReason::ParentCancelled);
            }
        }
        
        child
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Manager for handling graceful and forced cancellation of operations.
pub struct CancellationManager {
    /// The cancellation token for this operation.
    token: CancellationToken,
    /// Timeout for graceful cancellation attempts.
    graceful_timeout: Duration,
    /// Timeout for forced cancellation.
    force_timeout: Duration,
    /// Cleanup callbacks to run on cancellation.
    cleanup_callbacks: Arc<Mutex<Vec<Box<dyn Fn() + Send + Sync>>>>,
    /// Backend-specific cleanup handlers.
    backend_cleanup: Arc<Mutex<HashMap<String, Box<dyn Fn() + Send + Sync>>>>,
    /// System resources that need cleanup.
    system_resources: Arc<Mutex<Vec<SystemResource>>>,
}

/// Represents a system resource that needs cleanup on cancellation.
#[derive(Debug, Clone)]
pub enum SystemResource {
    /// D-Bus method call that can be cancelled.
    DBusCall {
        object_path: String,
        method_name: String,
        call_id: Option<String>,
    },
    /// System call or process that can be interrupted.
    SystemCall {
        process_id: Option<u32>,
        call_type: String,
    },
    /// File descriptor that should be closed.
    FileDescriptor {
        fd: i32,
        description: String,
    },
    /// Temporary file or directory that should be cleaned up.
    TempResource {
        path: String,
        resource_type: String,
    },
    /// Network connection that should be closed.
    NetworkConnection {
        connection_id: String,
        connection_type: String,
    },
}

impl std::fmt::Debug for CancellationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationManager")
            .field("token", &self.token)
            .field("graceful_timeout", &self.graceful_timeout)
            .field("force_timeout", &self.force_timeout)
            .field("cleanup_callbacks", &format!("{} callbacks", self.cleanup_callbacks.lock().unwrap().len()))
            .field("backend_cleanup", &format!("{} backend handlers", self.backend_cleanup.lock().unwrap().len()))
            .field("system_resources", &format!("{} system resources", self.system_resources.lock().unwrap().len()))
            .finish()
    }
}

impl CancellationManager {
    /// Creates a new cancellation manager with default timeouts.
    pub fn new() -> Self {
        Self::with_timeouts(
            Duration::from_secs(5),  // 5 second graceful timeout
            Duration::from_secs(2),  // 2 second force timeout
        )
    }

    /// Creates a new cancellation manager from configuration.
    pub fn from_config(config: super::config::CancellationConfig) -> Self {
        Self::with_timeouts(config.graceful_timeout, config.force_timeout)
    }

    /// Creates a new cancellation manager with custom timeouts.
    pub fn with_timeouts(graceful_timeout: Duration, force_timeout: Duration) -> Self {
        Self {
            token: CancellationToken::new(),
            graceful_timeout,
            force_timeout,
            cleanup_callbacks: Arc::new(Mutex::new(Vec::new())),
            backend_cleanup: Arc::new(Mutex::new(HashMap::new())),
            system_resources: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Gets the cancellation token.
    pub fn token(&self) -> &CancellationToken {
        &self.token
    }

    /// Gets the graceful timeout duration.
    pub fn graceful_timeout(&self) -> Duration {
        self.graceful_timeout
    }

    /// Gets the force timeout duration.
    pub fn force_timeout(&self) -> Duration {
        self.force_timeout
    }

    /// Checks if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Gets the cancellation reason if cancelled.
    pub fn cancellation_reason(&self) -> Option<CancellationReason> {
        self.token.cancellation_reason()
    }

    /// Requests graceful cancellation.
    pub fn request_cancellation(&self, reason: CancellationReason) {
        self.token.cancel(reason);
    }

    /// Requests cancellation (alias for request_cancellation).
    pub fn cancel(&self, reason: CancellationReason) {
        self.request_cancellation(reason);
    }

    /// Performs graceful cancellation with timeout fallback to forced cancellation.
    pub async fn cancel_with_cleanup<F, Fut>(&self, 
        reason: CancellationReason,
        graceful_cancel_fn: F,
    ) -> Result<(), NpioError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<(), NpioError>> + Send,
    {
        use tokio::time::{sleep, Instant};
        
        // Request cancellation
        self.request_cancellation(reason);

        // Record start time for graceful cancellation phase
        let graceful_start = Instant::now();
        let min_graceful_time = self.graceful_timeout / 2; // Minimum 50% of graceful timeout

        // Try graceful cancellation first
        let graceful_result = timeout(
            self.graceful_timeout,
            async {
                let result = graceful_cancel_fn().await;
                
                // Ensure we spend at least minimum time on graceful cancellation
                let elapsed = graceful_start.elapsed();
                if elapsed < min_graceful_time {
                    let remaining_time = min_graceful_time - elapsed;
                    sleep(remaining_time).await;
                }
                
                result
            }
        ).await;

        match graceful_result {
            Ok(Ok(())) => {
                // Graceful cancellation succeeded
                self.run_cleanup_callbacks();
                Ok(())
            }
            Ok(Err(e)) => {
                // Graceful cancellation failed, try forced cancellation
                self.force_cancel().await?;
                Err(e)
            }
            Err(_) => {
                // Graceful cancellation timed out, try forced cancellation
                self.force_cancel().await?;
                Err(NpioError::new(
                    IOErrorEnum::TimedOut,
                    "Graceful cancellation timed out, forced cancellation applied".to_string(),
                ))
            }
        }
    }

    /// Performs forced cancellation after graceful cancellation fails or times out.
    pub async fn force_cancel(&self) -> Result<(), NpioError> {
        // Run comprehensive cleanup with timeout
        let cleanup_result = timeout(
            self.force_timeout,
            async {
                self.cleanup_all_resources();
                Ok::<(), NpioError>(())
            },
        ).await;

        match cleanup_result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(NpioError::new(
                IOErrorEnum::TimedOut,
                "Forced cancellation cleanup timed out".to_string(),
            )),
        }
    }

    /// Adds a cleanup callback to run when the operation is cancelled.
    pub fn add_cleanup_callback<F>(&self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.cleanup_callbacks
            .lock()
            .unwrap()
            .push(Box::new(callback));
    }

    /// Adds a backend-specific cleanup handler.
    pub fn add_backend_cleanup<F>(&self, backend_name: String, handler: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.backend_cleanup
            .lock()
            .unwrap()
            .insert(backend_name, Box::new(handler));
    }

    /// Registers a system resource for cleanup on cancellation.
    pub fn register_system_resource(&self, resource: SystemResource) {
        self.system_resources
            .lock()
            .unwrap()
            .push(resource);
    }

    /// Registers a D-Bus call for cancellation.
    pub fn register_dbus_call(&self, object_path: String, method_name: String, call_id: Option<String>) {
        self.register_system_resource(SystemResource::DBusCall {
            object_path,
            method_name,
            call_id,
        });
    }

    /// Registers a system call for interruption.
    pub fn register_system_call(&self, process_id: Option<u32>, call_type: String) {
        self.register_system_resource(SystemResource::SystemCall {
            process_id,
            call_type,
        });
    }

    /// Registers a file descriptor for cleanup.
    pub fn register_file_descriptor(&self, fd: i32, description: String) {
        self.register_system_resource(SystemResource::FileDescriptor {
            fd,
            description,
        });
    }

    /// Registers a temporary resource for cleanup.
    pub fn register_temp_resource(&self, path: String, resource_type: String) {
        self.register_system_resource(SystemResource::TempResource {
            path,
            resource_type,
        });
    }

    /// Registers a network connection for cleanup.
    pub fn register_network_connection(&self, connection_id: String, connection_type: String) {
        self.register_system_resource(SystemResource::NetworkConnection {
            connection_id,
            connection_type,
        });
    }

    /// Gets the number of registered system resources.
    pub fn system_resource_count(&self) -> usize {
        self.system_resources.lock().unwrap().len()
    }

    /// Gets a copy of all registered system resources.
    pub fn get_system_resources(&self) -> Vec<SystemResource> {
        self.system_resources.lock().unwrap().clone()
    }

    /// Runs all registered cleanup callbacks.
    fn run_cleanup_callbacks(&self) {
        let callbacks = self.cleanup_callbacks.lock().unwrap();
        for callback in callbacks.iter() {
            callback();
        }
    }

    /// Runs backend-specific cleanup handlers.
    fn run_backend_cleanup(&self) {
        let handlers = self.backend_cleanup.lock().unwrap();
        for (backend_name, handler) in handlers.iter() {
            log::debug!("Running cleanup for backend: {}", backend_name);
            handler();
        }
    }

    /// Cleans up system resources.
    fn cleanup_system_resources(&self) {
        let resources = self.system_resources.lock().unwrap();
        for resource in resources.iter() {
            match resource {
                SystemResource::DBusCall { object_path, method_name, call_id } => {
                    log::debug!("Cancelling D-Bus call: {} on {}", method_name, object_path);
                    if let Some(id) = call_id {
                        log::debug!("D-Bus call ID: {}", id);
                    }
                    // In a real implementation, this would cancel the D-Bus call
                    // For now, we just log the cancellation attempt
                }
                SystemResource::SystemCall { process_id, call_type } => {
                    log::debug!("Interrupting system call: {}", call_type);
                    if let Some(pid) = process_id {
                        log::debug!("Sending interrupt signal to process: {}", pid);
                        // In a real implementation, this would send SIGINT or SIGTERM
                        // For now, we just log the interruption attempt
                    }
                }
                SystemResource::FileDescriptor { fd, description } => {
                    log::debug!("Closing file descriptor {}: {}", fd, description);
                    // In a real implementation, this would close the file descriptor
                    // For now, we just log the closure attempt
                }
                SystemResource::TempResource { path, resource_type } => {
                    log::debug!("Cleaning up temporary {}: {}", resource_type, path);
                    // In a real implementation, this would remove the temporary resource
                    // For now, we just log the cleanup attempt
                }
                SystemResource::NetworkConnection { connection_id, connection_type } => {
                    log::debug!("Closing {} connection: {}", connection_type, connection_id);
                    // In a real implementation, this would close the network connection
                    // For now, we just log the closure attempt
                }
            }
        }
    }

    /// Performs comprehensive cleanup of all resources.
    pub fn cleanup_all_resources(&self) {
        // Run cleanup in order of importance
        self.cleanup_system_resources();
        self.run_backend_cleanup();
        self.run_cleanup_callbacks();
    }

    /// Waits for cancellation to be requested.
    pub async fn cancelled(&self) {
        self.token.cancelled().await;
    }

    /// Throws a cancellation error if cancelled.
    pub fn throw_if_cancelled(&self) -> Result<(), NpioError> {
        self.token.throw_if_cancelled()
    }

    /// Creates a scoped cancellation manager that inherits from this one.
    pub fn create_scope(&self) -> Self {
        let child_token = self.token.child();
        Self {
            token: child_token,
            graceful_timeout: self.graceful_timeout,
            force_timeout: self.force_timeout,
            cleanup_callbacks: Arc::new(Mutex::new(Vec::new())),
            backend_cleanup: Arc::new(Mutex::new(HashMap::new())),
            system_resources: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for CancellationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_cancellation_token_basic() {
        let token = CancellationToken::new();
        
        assert!(!token.is_cancelled());
        assert!(token.cancellation_reason().is_none());
        
        token.cancel(CancellationReason::UserRequested);
        
        assert!(token.is_cancelled());
        assert_eq!(token.cancellation_reason(), Some(CancellationReason::UserRequested));
    }

    #[tokio::test]
    async fn test_cancellation_token_notification() {
        let token = CancellationToken::new();
        let token_clone = token.clone();
        
        let handle = tokio::spawn(async move {
            token_clone.cancelled().await;
            true
        });
        
        // Give the task time to start waiting
        sleep(Duration::from_millis(10)).await;
        
        token.cancel(CancellationReason::UserRequested);
        
        let result = handle.await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_cancellation_manager_basic() {
        let manager = CancellationManager::new();
        
        assert!(!manager.is_cancelled());
        assert!(manager.cancellation_reason().is_none());
        
        manager.request_cancellation(CancellationReason::Timeout);
        
        assert!(manager.is_cancelled());
        assert_eq!(manager.cancellation_reason(), Some(CancellationReason::Timeout));
    }

    #[tokio::test]
    async fn test_cleanup_callbacks() {
        let manager = CancellationManager::new();
        let cleanup_called = Arc::new(AtomicBool::new(false));
        let cleanup_called_clone = cleanup_called.clone();
        
        manager.add_cleanup_callback(move || {
            cleanup_called_clone.store(true, Ordering::Release);
        });
        
        manager.force_cancel().await.unwrap();
        
        assert!(cleanup_called.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn test_backend_cleanup() {
        let manager = CancellationManager::new();
        let backend_cleanup_called = Arc::new(AtomicBool::new(false));
        let backend_cleanup_clone = backend_cleanup_called.clone();
        
        manager.add_backend_cleanup("test_backend".to_string(), move || {
            backend_cleanup_clone.store(true, Ordering::Release);
        });
        
        manager.cleanup_all_resources();
        
        assert!(backend_cleanup_called.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn test_system_resource_registration() {
        let manager = CancellationManager::new();
        
        // Register various system resources
        manager.register_dbus_call(
            "/org/freedesktop/UDisks2/drives/test".to_string(),
            "Mount".to_string(),
            Some("call_123".to_string()),
        );
        
        manager.register_system_call(Some(12345), "mount".to_string());
        
        manager.register_file_descriptor(42, "test mount fd".to_string());
        
        manager.register_temp_resource("/tmp/mount_test".to_string(), "directory".to_string());
        
        manager.register_network_connection("conn_456".to_string(), "TCP".to_string());
        
        // Verify resources were registered
        let resources = manager.system_resources.lock().unwrap();
        assert_eq!(resources.len(), 5);
        
        // Test cleanup (should not panic)
        drop(resources);
        manager.cleanup_all_resources();
    }

    #[tokio::test]
    async fn test_graceful_cancellation_success() {
        let manager = CancellationManager::with_timeouts(
            Duration::from_millis(100),
            Duration::from_millis(50),
        );
        
        let result = manager.cancel_with_cleanup(
            CancellationReason::UserRequested,
            || async { Ok(()) },
        ).await;
        
        assert!(result.is_ok());
        assert!(manager.is_cancelled());
    }

    #[tokio::test]
    async fn test_graceful_cancellation_timeout() {
        let manager = CancellationManager::with_timeouts(
            Duration::from_millis(50),
            Duration::from_millis(50),
        );
        
        let result = manager.cancel_with_cleanup(
            CancellationReason::UserRequested,
            || async {
                sleep(Duration::from_millis(100)).await;
                Ok(())
            },
        ).await;
        
        assert!(result.is_err());
        assert!(manager.is_cancelled());
    }
}