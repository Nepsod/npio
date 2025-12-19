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
    udisks2_backend: Arc<UDisks2Backend>,
    mounts: Arc<RwLock<Vec<Arc<dyn Mount>>>>,
    drives: Arc<RwLock<Vec<Arc<dyn Drive>>>>,
    volumes: Arc<RwLock<Vec<Arc<dyn Volume>>>>,
}

impl DevicesModel {
    /// Creates a new devices model
    pub fn new() -> Self {
        Self {
            mount_backend: Arc::new(MountBackend::new()),
            udisks2_backend: Arc::new(UDisks2Backend::new()),
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

        // Load mounts - convert Box to Arc for efficient caching
        let mounts_box = self.mount_backend.get_mounts().await?;
        let mounts: Vec<Arc<dyn Mount>> = mounts_box.into_iter().map(|b| Arc::from(b)).collect();
        let mut mounts_guard = self.mounts.write().await;
        *mounts_guard = mounts;
        drop(mounts_guard);

        // Try to load drives and volumes from UDisks2
        if self.udisks2_backend.is_available().await {
            // Load drives - convert Box to Arc for efficient caching
            if let Ok(drives_box) = self.udisks2_backend.get_drives(cancellable).await {
                let drives: Vec<Arc<dyn Drive>> = drives_box.into_iter().map(|b| Arc::from(b)).collect();
                let mut drives_guard = self.drives.write().await;
                *drives_guard = drives;
                drop(drives_guard);
            }

            // Load volumes - convert Box to Arc for efficient caching
            if let Ok(volumes_box) = self.udisks2_backend.get_volumes(cancellable).await {
                let volumes: Vec<Arc<dyn Volume>> = volumes_box.into_iter().map(|b| Arc::from(b)).collect();
                let mut volumes_guard = self.volumes.write().await;
                *volumes_guard = volumes;
                drop(volumes_guard);
            }
        }

        Ok(())
    }

    /// Gets all mounts
    /// Returns cached values if available, otherwise loads from backend
    /// Returns empty vector if an error occurs (e.g., backend unavailable)
    pub async fn get_mounts(&self) -> Vec<Arc<dyn Mount>> {
        // Try to return cached values first
        {
            let mounts_guard = self.mounts.read().await;
            if !mounts_guard.is_empty() {
                // Clone Arc references efficiently (just increments reference count)
                return mounts_guard.iter().map(|arc| arc.clone()).collect();
            }
        }
        
        // Cache is empty, load from backend and update cache
        match self.mount_backend.get_mounts().await {
            Ok(mounts_box) => {
                // Convert Box to Arc and update cache
                let mounts: Vec<Arc<dyn Mount>> = mounts_box.into_iter().map(|b| Arc::from(b)).collect();
                {
                    let mut mounts_guard = self.mounts.write().await;
                    *mounts_guard = mounts.clone();
                }
                mounts
            }
            Err(e) => {
                eprintln!("DevicesModel: Failed to get mounts: {}", e);
                Vec::new()
            }
        }
    }

    /// Gets all drives
    /// Returns cached values if available, otherwise loads from backend
    /// Returns empty vector if UDisks2 is unavailable or an error occurs
    pub async fn get_drives(&self) -> Vec<Arc<dyn Drive>> {
        // Try to return cached values first
        {
            let drives_guard = self.drives.read().await;
            if !drives_guard.is_empty() {
                // Clone Arc references efficiently (just increments reference count)
                return drives_guard.iter().map(|arc| arc.clone()).collect();
            }
        }
        
        // Cache is empty, load from backend and update cache
        if self.udisks2_backend.is_available().await {
            match self.udisks2_backend.get_drives(None).await {
                Ok(drives_box) => {
                    // Convert Box to Arc and update cache
                    let drives: Vec<Arc<dyn Drive>> = drives_box.into_iter().map(|b| Arc::from(b)).collect();
                    {
                        let mut drives_guard = self.drives.write().await;
                        *drives_guard = drives.clone();
                    }
                    drives
                }
                Err(e) => {
                    eprintln!("DevicesModel: Failed to get drives from UDisks2: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
    }

    /// Gets all volumes
    /// Returns cached values if available, otherwise loads from backend
    /// Returns empty vector if UDisks2 is unavailable or an error occurs
    pub async fn get_volumes(&self) -> Vec<Arc<dyn Volume>> {
        // Try to return cached values first
        {
            let volumes_guard = self.volumes.read().await;
            if !volumes_guard.is_empty() {
                // Clone Arc references efficiently (just increments reference count)
                return volumes_guard.iter().map(|arc| arc.clone()).collect();
            }
        }
        
        // Cache is empty, load from backend and update cache
        if self.udisks2_backend.is_available().await {
            match self.udisks2_backend.get_volumes(None).await {
                Ok(volumes_box) => {
                    // Convert Box to Arc and update cache
                    let volumes: Vec<Arc<dyn Volume>> = volumes_box.into_iter().map(|b| Arc::from(b)).collect();
                    {
                        let mut volumes_guard = self.volumes.write().await;
                        *volumes_guard = volumes.clone();
                    }
                    volumes
                }
                Err(e) => {
                    eprintln!("DevicesModel: Failed to get volumes from UDisks2: {}", e);
                    Vec::new()
                }
            }
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

