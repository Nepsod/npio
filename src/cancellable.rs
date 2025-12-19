use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

use crate::error::{NpioError, IOErrorEnum};

#[derive(Clone)]
pub struct Cancellable {
    inner: Arc<CancellableInner>,
}

struct CancellableInner {
    cancelled: Mutex<bool>,
    notify: Notify,
}

impl Cancellable {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CancellableInner {
                cancelled: Mutex::new(false),
                notify: Notify::new(),
            }),
        }
    }

    pub fn cancel(&self) {
        match self.inner.cancelled.lock() {
            Ok(mut cancelled) => {
                if !*cancelled {
                    *cancelled = true;
                    self.inner.notify.notify_waiters();
                }
            }
            Err(e) => {
                eprintln!("Failed to acquire lock on cancellable state: {}", e);
                // Try to recover from poisoned lock
                let mut cancelled = e.into_inner();
                if !*cancelled {
                    *cancelled = true;
                    self.inner.notify.notify_waiters();
                }
            }
        }
    }

    pub fn is_cancelled(&self) -> bool {
        match self.inner.cancelled.lock() {
            Ok(cancelled) => *cancelled,
            Err(e) => {
                eprintln!("Failed to acquire lock on cancellable state: {}", e);
                // Try to recover from poisoned lock
                let cancelled = e.into_inner();
                *cancelled
            }
        }
    }

    pub fn check(&self) -> Result<(), NpioError> {
        if self.is_cancelled() {
            Err(NpioError::new(IOErrorEnum::Cancelled, "Operation cancelled"))
        } else {
            Ok(())
        }
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        self.inner.notify.notified().await;
    }
}

impl Default for Cancellable {
    fn default() -> Self {
        Self::new()
    }
}
