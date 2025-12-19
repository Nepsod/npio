//! Devices Model
//!
//! Provides a unified view of drives, volumes, and mounts.
//! Currently uses mount backend for basic functionality.
//! Future: Integrate with UDisks2 for full device management.

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::backend::mount::MountBackend;
use crate::mount::Mount;
use crate::drive::Drive;
use crate::volume::Volume;
use crate::error::NpioResult;
use crate::cancellable::Cancellable;

/// Devices model that aggregates drives, volumes, and mounts
pub struct DevicesModel {
    mount_backend: Arc<MountBackend>,
    mounts: Arc<RwLock<Vec<Box<dyn Mount>>>>,
}

impl DevicesModel {
    /// Creates a new devices model
    pub fn new() -> Self {
        Self {
            mount_backend: Arc::new(MountBackend::new()),
            mounts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Loads all devices (mounts, drives, volumes)
    pub async fn load(&self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let mounts = self.mount_backend.get_mounts().await?;
        let mut mounts_guard = self.mounts.write().await;
        *mounts_guard = mounts;
        drop(mounts_guard);

        Ok(())
    }

    /// Gets all mounts
    pub async fn get_mounts(&self) -> Vec<Box<dyn Mount>> {
        // Note: We can't clone Box<dyn Mount>, so we reload from backend
        // In a real implementation, we'd maintain a cache differently
        self.mount_backend.get_mounts().await.unwrap_or_default()
    }

    /// Gets all drives
    /// Note: Currently returns empty as UDisks2 integration is pending
    pub async fn get_drives(&self) -> Vec<Box<dyn Drive>> {
        // TODO: Integrate with UDisks2 to get actual drives
        Vec::new()
    }

    /// Gets all volumes
    /// Note: Currently returns empty as UDisks2 integration is pending
    pub async fn get_volumes(&self) -> Vec<Box<dyn Volume>> {
        // TODO: Integrate with UDisks2 to get actual volumes
        Vec::new()
    }

    /// Gets a mount for a specific path
    pub async fn get_mount_for_path(
        &self,
        path: &std::path::Path,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Option<Box<dyn Mount>>> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        self.mount_backend.get_mount_for_path(path).await
    }

    /// Refreshes the devices list
    pub async fn refresh(&self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        self.load(cancellable).await
    }
}

impl Default for DevicesModel {
    fn default() -> Self {
        Self::new()
    }
}

