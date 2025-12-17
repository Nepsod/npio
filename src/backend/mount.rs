// Basic mount backend implementation
// This is a placeholder for future implementation with libmount and UDisks2

use crate::error::{NpioError, NpioResult, IOErrorEnum};

/// Placeholder for mount backend functionality.
/// Future implementation will parse /proc/self/mountinfo and integrate with UDisks2.
pub struct MountBackend;

impl MountBackend {
    pub fn new() -> Self {
        Self
    }

    /// Placeholder: Returns an error indicating not yet implemented.
    pub async fn get_mounts(&self) -> NpioResult<Vec<Box<dyn crate::mount::Mount>>> {
        Err(NpioError::new(
            IOErrorEnum::NotSupported,
            "Mount backend not yet implemented. Requires libmount integration.",
        ))
    }
}

impl Default for MountBackend {
    fn default() -> Self {
        Self::new()
    }
}

