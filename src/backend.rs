pub mod local;
pub mod thumbnail;

use std::collections::HashMap;

use std::sync::{Arc, RwLock};
use once_cell::sync::Lazy;

use crate::file::File;
use crate::error::{NpioError, IOErrorEnum, NpioResult};

/// Trait that all backends must implement.
/// A backend handles a specific URI scheme (e.g., "file://", "sftp://").
pub trait Backend: Send + Sync {
    fn scheme(&self) -> &'static str;
    
    fn get_file_for_uri(&self, uri: &str) -> NpioResult<Box<dyn File>>;
    
    // Future expansion:
    // fn mount(&self, ...)
    // fn unmount(&self, ...)
}

/// Global registry for backends.
pub struct BackendRegistry {
    backends: HashMap<String, Arc<dyn Backend>>,
}

impl BackendRegistry {
    fn new() -> Self {
        Self {
            backends: HashMap::new(),
        }
    }

    pub fn register(&mut self, backend: Arc<dyn Backend>) {
        self.backends.insert(backend.scheme().to_string(), backend);
    }

    pub fn get_backend(&self, scheme: &str) -> Option<Arc<dyn Backend>> {
        self.backends.get(scheme).cloned()
    }
}

static REGISTRY: Lazy<RwLock<BackendRegistry>> = Lazy::new(|| {
    RwLock::new(BackendRegistry::new())
});

pub fn register_backend(backend: Arc<dyn Backend>) {
    let mut registry = REGISTRY.write().unwrap();
    registry.register(backend);
}

pub fn get_backend_for_scheme(scheme: &str) -> Option<Arc<dyn Backend>> {
    let registry = REGISTRY.read().unwrap();
    registry.get_backend(scheme)
}

pub fn get_file_for_uri(uri: &str) -> NpioResult<Box<dyn File>> {
    // Simple URI parsing to extract scheme
    let parts: Vec<&str> = uri.split("://").collect();
    let scheme = if parts.len() > 1 {
        parts[0]
    } else {
        "file" // Default to file if no scheme present (assuming local path)
    };

    if let Some(backend) = get_backend_for_scheme(scheme) {
        backend.get_file_for_uri(uri)
    } else {
        Err(NpioError::new(IOErrorEnum::NotSupported, format!("No backend found for scheme: {}", scheme)))
    }
}
