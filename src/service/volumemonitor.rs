//! VolumeMonitor service
//!
//! Provides centralized monitoring of volumes, mounts, and drives with hotplug detection
//! via udev integration.

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, broadcast};
use tokio::task;
use crate::error::{NpioError, NpioResult, IOErrorEnum};
use crate::cancellable::Cancellable;
use crate::mount::Mount;
use crate::volume::Volume;
use crate::drive::Drive;
use crate::backend::udisks2::UDisks2Backend;
use crate::backend::mount::MountBackend;

/// Events emitted by VolumeMonitor
#[derive(Debug, Clone)]
pub enum VolumeMonitorEvent {
    VolumeAdded { volume: String },
    VolumeRemoved { volume: String },
    VolumeChanged { volume: String },
    MountAdded { mount: String },
    MountRemoved { mount: String },
    MountChanged { mount: String },
    DriveConnected { drive: String },
    DriveDisconnected { drive: String },
    DriveChanged { drive: String },
}

/// VolumeMonitor service for device management
pub struct VolumeMonitor {
    udisks2_backend: Arc<UDisks2Backend>,
    mount_backend: Arc<MountBackend>,
    event_sender: broadcast::Sender<VolumeMonitorEvent>,
    volumes: Arc<RwLock<HashMap<String, Box<dyn Volume>>>>,
    mounts: Arc<RwLock<HashMap<String, Box<dyn Mount>>>>,
    drives: Arc<RwLock<HashMap<String, Box<dyn Drive>>>>,
    monitor_handle: Arc<RwLock<Option<task::JoinHandle<()>>>>,
}

impl VolumeMonitor {
    /// Creates a new VolumeMonitor
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self {
            udisks2_backend: Arc::new(UDisks2Backend::new()),
            mount_backend: Arc::new(MountBackend::new()),
            event_sender: sender,
            volumes: Arc::new(RwLock::new(HashMap::new())),
            mounts: Arc::new(RwLock::new(HashMap::new())),
            drives: Arc::new(RwLock::new(HashMap::new())),
            monitor_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Starts monitoring for device changes
    pub async fn start(&self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        // Check if already started
        {
            let handle_guard = self.monitor_handle.read().await;
            if handle_guard.is_some() {
                return Ok(()); // Already started
            }
        }

        // Start polling-based monitoring (simplified approach)
        // Full udev integration would require more complex thread handling
        let sender = self.event_sender.clone();
        let udisks2 = self.udisks2_backend.clone();
        let volumes = self.volumes.clone();
        let mounts = self.mounts.clone();
        let drives = self.drives.clone();

        let handle = task::spawn(async move {
            monitor_udev_events(sender, udisks2, volumes, mounts, drives).await;
        });

        {
            let mut handle_guard = self.monitor_handle.write().await;
            *handle_guard = Some(handle);
        }

        // Initial load
        self.load(cancellable).await?;

        Ok(())
    }

    /// Stops monitoring
    pub async fn stop(&self) {
        let mut handle_guard = self.monitor_handle.write().await;
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }
    }

    /// Loads all devices
    pub async fn load(&self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        // Load from UDisks2 if available
        if self.udisks2_backend.is_available().await {
            // Load volumes
            if let Ok(volumes_list) = self.udisks2_backend.get_volumes(cancellable).await {
                let mut volumes_guard = self.volumes.write().await;
                volumes_guard.clear();
                for volume in volumes_list {
                    if let Some(uuid) = volume.get_uuid() {
                        volumes_guard.insert(uuid, volume);
                    }
                }
            }

            // Load drives
            if let Ok(drives_list) = self.udisks2_backend.get_drives(cancellable).await {
                let mut drives_guard = self.drives.write().await;
                drives_guard.clear();
                for drive in drives_list {
                    // Use device identifier as key
                    let key = drive.get_identifier("unix-device")
                        .unwrap_or_else(|| format!("drive-{}", drives_guard.len()));
                    drives_guard.insert(key, drive);
                }
            }

            // Load mounts from UDisks2
            if let Ok(mounts_list) = self.udisks2_backend.get_mounts(cancellable).await {
                let mut mounts_guard = self.mounts.write().await;
                for mount in mounts_list {
                    let root = mount.get_root();
                    let uri = root.uri();
                    let key = uri.strip_prefix("file://").unwrap_or(&uri).to_string();
                    mounts_guard.insert(key, mount);
                }
            }
        }

        // Also load from mount backend
        if let Ok(mounts_list) = self.mount_backend.get_mounts().await {
            let mut mounts_guard = self.mounts.write().await;
            for mount in mounts_list {
                let root = mount.get_root();
                let uri = root.uri();
                let key = uri.strip_prefix("file://").unwrap_or(&uri).to_string();
                // Only add if not already present (UDisks2 takes precedence)
                mounts_guard.entry(key).or_insert_with(|| mount);
            }
        }

        Ok(())
    }

    /// Gets all volumes
    pub async fn get_volumes(&self) -> Vec<Box<dyn Volume>> {
        // Reload from backend since volumes can't be cloned
        if let Ok(volumes_list) = self.udisks2_backend.get_volumes(None).await {
            volumes_list
        } else {
            Vec::new()
        }
    }

    /// Gets all mounts
    pub async fn get_mounts(&self) -> Vec<Box<dyn Mount>> {
        // Reload from backend since mounts can't be cloned
        let mut result = Vec::new();
        if let Ok(mounts_list) = self.udisks2_backend.get_mounts(None).await {
            result.extend(mounts_list);
        }
        if let Ok(mounts_list) = self.mount_backend.get_mounts().await {
            result.extend(mounts_list);
        }
        result
    }

    /// Gets all connected drives
    pub async fn get_connected_drives(&self) -> Vec<Box<dyn Drive>> {
        // Reload from backend since drives can't be cloned
        if let Ok(drives_list) = self.udisks2_backend.get_drives(None).await {
            drives_list
        } else {
            Vec::new()
        }
    }

    /// Gets a volume by UUID
    pub async fn get_volume_for_uuid(&self, uuid: &str) -> Option<Box<dyn Volume>> {
        // Reload from backend since volumes can't be cloned
        if let Ok(volumes_list) = self.udisks2_backend.get_volumes(None).await {
            for volume in volumes_list {
                if volume.get_uuid().as_ref() == Some(&uuid.to_string()) {
                    return Some(volume);
                }
            }
        }
        None
    }

    /// Gets a mount by path
    pub async fn get_mount_for_path(&self, path: &str) -> Option<Box<dyn Mount>> {
        // Reload from backend since mounts can't be cloned
        if let Ok(mounts_list) = self.udisks2_backend.get_mounts(None).await {
            for mount in mounts_list {
                let root = mount.get_root();
                let uri = root.uri();
                if uri.strip_prefix("file://").unwrap_or("") == path {
                    return Some(mount);
                }
            }
        }
        None
    }

    /// Subscribes to volume monitor events
    pub fn subscribe(&self) -> broadcast::Receiver<VolumeMonitorEvent> {
        self.event_sender.subscribe()
    }
}

impl Default for VolumeMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Monitors device changes and updates the volume monitor
/// Note: Simplified implementation using polling instead of direct udev monitor
/// due to thread safety constraints
async fn monitor_udev_events(
    sender: broadcast::Sender<VolumeMonitorEvent>,
    udisks2: Arc<UDisks2Backend>,
    volumes: Arc<RwLock<HashMap<String, Box<dyn Volume>>>>,
    _mounts: Arc<RwLock<HashMap<String, Box<dyn Mount>>>>,
    _drives: Arc<RwLock<HashMap<String, Box<dyn Drive>>>>,
) {
    // Poll UDisks2 periodically for changes instead of using udev directly
    // This is a simplified approach - a full implementation would use proper udev integration
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // Check for changes by reloading
        if let Ok(volumes_list) = udisks2.get_volumes(None).await {
            let mut volumes_guard = volumes.write().await;
            
            // Collect current UUIDs
            let current_uuids: std::collections::HashSet<String> = volumes_guard.keys().cloned().collect();
            let new_uuids: std::collections::HashSet<String> = volumes_list
                .iter()
                .filter_map(|v| v.get_uuid())
                .collect();
            
            // Detect removed volumes
            for removed_uuid in current_uuids.difference(&new_uuids) {
                let _ = sender.send(VolumeMonitorEvent::VolumeRemoved {
                    volume: removed_uuid.clone(),
                });
            }
            
            // Process volumes
            for volume in volumes_list {
                if let Some(uuid) = volume.get_uuid() {
                    if !volumes_guard.contains_key(&uuid) {
                        let _ = sender.send(VolumeMonitorEvent::VolumeAdded {
                            volume: uuid.clone(),
                        });
                    }
                    // Only send changed event if volume actually changed (simplified - would need deep comparison)
                    volumes_guard.insert(uuid, volume);
                }
            }
        }
    }
}


