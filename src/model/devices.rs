//! Devices Model
//!
//! Provides a unified view of drives, volumes, and mounts.
//! Currently uses mount backend for basic functionality.
//! Future: Integrate with UDisks2 for full device management.

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::backend::mount::MountBackend;
use crate::backend::udisks2::UDisks2Backend;
use crate::mount::Mount;
use crate::drive::Drive;
use crate::volume::Volume;
use crate::error::NpioResult;
use crate::cancellable::Cancellable;

/// Devices model that aggregates drives, volumes, and mounts
pub struct DevicesModel {
    mount_backend: Arc<MountBackend>,
    udisks2_backend: Arc<tokio::sync::Mutex<UDisks2Backend>>,
    mounts: Arc<RwLock<Vec<Box<dyn Mount>>>>,
    drives: Arc<RwLock<Vec<Box<dyn Drive>>>>,
    volumes: Arc<RwLock<Vec<Box<dyn Volume>>>>,
}

impl DevicesModel {
    /// Creates a new devices model
    pub fn new() -> Self {
        Self {
            mount_backend: Arc::new(MountBackend::new()),
            udisks2_backend: Arc::new(tokio::sync::Mutex::new(UDisks2Backend::new())),
            mounts: Arc::new(RwLock::new(Vec::new())),
            drives: Arc::new(RwLock::new(Vec::new())),
            volumes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Loads all devices (mounts, drives, volumes)
    pub async fn load(&self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        // Load mounts
        let mounts = self.mount_backend.get_mounts().await?;
        let mut mounts_guard = self.mounts.write().await;
        *mounts_guard = mounts;
        drop(mounts_guard);

        // Try to load drives and volumes from UDisks2
        let mut udisks2 = self.udisks2_backend.lock().await;
        if udisks2.is_available().await {
            // Load drives
            if let Ok(drives) = udisks2.get_drives(cancellable).await {
                let mut drives_guard = self.drives.write().await;
                *drives_guard = drives;
                drop(drives_guard);
            }

            // Load volumes
            if let Ok(volumes) = udisks2.get_volumes(cancellable).await {
                let mut volumes_guard = self.volumes.write().await;
                *volumes_guard = volumes;
                drop(volumes_guard);
            }
        }

        Ok(())
    }

    /// Gets all mounts
    pub async fn get_mounts(&self) -> Vec<Box<dyn Mount>> {
        // Note: We can't clone Box<dyn Mount>, so we reload from backend
        // In a real implementation, we'd maintain a cache differently
        self.mount_backend.get_mounts().await.unwrap_or_default()
    }

    /// Gets all drives
    pub async fn get_drives(&self) -> Vec<Box<dyn Drive>> {
        let drives_guard = self.drives.read().await;
        // Note: We can't clone Box<dyn Drive>, so we need to reload
        // In a real implementation, we'd maintain a cache differently
        drop(drives_guard);
        
        // Try to get fresh drives from UDisks2
        let mut udisks2 = self.udisks2_backend.lock().await;
        if udisks2.is_available().await {
            udisks2.get_drives(None).await.unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Gets all volumes
    pub async fn get_volumes(&self) -> Vec<Box<dyn Volume>> {
        let volumes_guard = self.volumes.read().await;
        // Note: We can't clone Box<dyn Volume>, so we need to reload
        drop(volumes_guard);
        
        // Try to get fresh volumes from UDisks2
        let mut udisks2 = self.udisks2_backend.lock().await;
        if udisks2.is_available().await {
            udisks2.get_volumes(None).await.unwrap_or_default()
        } else {
            Vec::new()
        }
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

