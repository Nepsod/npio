// Basic mount backend implementation
// Parses /proc/self/mountinfo to provide mount information

use std::path::PathBuf;
use crate::error::{NpioError, NpioResult, IOErrorEnum};
use crate::mount::Mount;
use crate::file::local::LocalFile;

/// Represents a mount entry from /proc/self/mountinfo
#[derive(Debug, Clone)]
struct MountEntry {
    mount_id: u32,
    parent_id: u32,
    major_minor: String,
    root: PathBuf,
    mount_point: PathBuf,
    mount_options: String,
    optional_fields: String,
    filesystem_type: String,
    source: String,
    super_options: String,
}

/// Basic mount implementation
#[derive(Debug)]
pub struct UnixMount {
    mount_point: PathBuf,
    source: String,
    filesystem_type: String,
    is_read_only: bool,
    is_system_internal: bool,
}

impl UnixMount {
    fn new(entry: &MountEntry) -> Self {
        let is_read_only = entry.mount_options.contains("ro");
        let is_system_internal = entry.mount_point == PathBuf::from("/")
            || entry.mount_point.starts_with("/sys")
            || entry.mount_point.starts_with("/proc")
            || entry.mount_point.starts_with("/dev")
            || entry.filesystem_type == "tmpfs"
            || entry.filesystem_type == "devtmpfs"
            || entry.filesystem_type == "sysfs"
            || entry.filesystem_type == "proc"
            || entry.filesystem_type == "devpts";

        Self {
            mount_point: entry.mount_point.clone(),
            source: entry.source.clone(),
            filesystem_type: entry.filesystem_type.clone(),
            is_read_only,
            is_system_internal,
        }
    }

    fn get_icon_name(&self) -> String {
        // Determine icon based on filesystem type and mount point
        if self.mount_point.starts_with("/media") || self.mount_point.starts_with("/mnt") {
            if self.filesystem_type == "iso9660" || self.source.contains("sr") {
                "drive-optical".to_string()
            } else if self.source.starts_with("/dev/sd") || self.source.starts_with("/dev/nvme") {
                "drive-harddisk".to_string()
            } else if self.source.starts_with("/dev/mmc") {
                "media-flash".to_string()
            } else {
                "drive-removable-media".to_string()
            }
        } else if self.mount_point == PathBuf::from("/") {
            "drive-harddisk".to_string()
        } else {
            "drive-harddisk".to_string()
        }
    }
}

#[async_trait::async_trait]
impl Mount for UnixMount {
    fn get_root(&self) -> Box<dyn crate::file::File> {
        Box::new(LocalFile::new(self.mount_point.clone()))
    }

    fn get_name(&self) -> String {
        // Try to get a nice name from the mount point
        if let Some(name) = self.mount_point.file_name() {
            name.to_string_lossy().to_string()
        } else {
            self.mount_point.to_string_lossy().to_string()
        }
    }

    fn get_icon(&self) -> String {
        self.get_icon_name()
    }

    fn get_uuid(&self) -> Option<String> {
        // TODO: Extract UUID from blkid or /dev/disk/by-uuid
        None
    }

    fn get_volume(&self) -> Option<Box<dyn crate::volume::Volume>> {
        None // Requires UDisks2 integration
    }

    fn get_drive(&self) -> Option<Box<dyn crate::drive::Drive>> {
        None // Requires UDisks2 integration
    }

    fn can_unmount(&self) -> bool {
        !self.is_system_internal
    }

    fn can_eject(&self) -> bool {
        // Only removable media can be ejected
        self.mount_point.starts_with("/media") || self.mount_point.starts_with("/mnt")
    }

    async fn unmount(
        &self,
        _cancellable: Option<&crate::cancellable::Cancellable>,
    ) -> NpioResult<()> {
        if !self.can_unmount() {
            return Err(NpioError::new(
                IOErrorEnum::NotSupported,
                "Cannot unmount system internal mount",
            ));
        }

        // TODO: Implement actual unmounting using umount2 syscall
        Err(NpioError::new(
            IOErrorEnum::NotSupported,
            "Unmounting not yet implemented",
        ))
    }

    async fn eject(
        &self,
        _cancellable: Option<&crate::cancellable::Cancellable>,
    ) -> NpioResult<()> {
        if !self.can_eject() {
            return Err(NpioError::new(
                IOErrorEnum::NotSupported,
                "Cannot eject this mount",
            ));
        }

        // TODO: Implement actual ejection
        Err(NpioError::new(
            IOErrorEnum::NotSupported,
            "Ejection not yet implemented",
        ))
    }
}

/// Mount backend that parses /proc/self/mountinfo
pub struct MountBackend;

impl MountBackend {
    pub fn new() -> Self {
        Self
    }

    /// Parses a line from /proc/self/mountinfo
    pub(crate) fn parse_mountinfo_line(line: &str) -> Option<MountEntry> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            return None;
        }

        let mount_id = parts[0].parse::<u32>().ok()?;
        let parent_id = parts[1].parse::<u32>().ok()?;
        let major_minor = parts[2].to_string();
        let root = PathBuf::from(parts[3]);
        let mount_point = PathBuf::from(parts[4]);
        let mount_options = parts[5].to_string();

        // Find the separator "-" that separates optional fields from filesystem info
        let mut optional_fields = String::new();
        let mut filesystem_type_idx = 6;
        for (i, part) in parts.iter().enumerate().skip(6) {
            if *part == "-" {
                filesystem_type_idx = i + 1;
                break;
            }
            if !optional_fields.is_empty() {
                optional_fields.push(' ');
            }
            optional_fields.push_str(part);
        }

        if parts.len() <= filesystem_type_idx + 1 {
            return None;
        }

        let filesystem_type = parts[filesystem_type_idx].to_string();
        let source = parts[filesystem_type_idx + 1].to_string();
        let super_options = if parts.len() > filesystem_type_idx + 2 {
            parts[filesystem_type_idx + 2..].join(" ")
        } else {
            String::new()
        };

        Some(MountEntry {
            mount_id,
            parent_id,
            major_minor,
            root,
            mount_point,
            mount_options,
            optional_fields,
            filesystem_type,
            source,
            super_options,
        })
    }

    /// Gets all mounts from /proc/self/mountinfo
    pub async fn get_mounts(&self) -> NpioResult<Vec<Box<dyn Mount>>> {
        let mountinfo_path = "/proc/self/mountinfo";
        let content = tokio::fs::read_to_string(mountinfo_path).await
            .map_err(|e| NpioError::new(
                IOErrorEnum::NotFound,
                format!("Failed to read {}: {}", mountinfo_path, e)
            ))?;

        let mut mounts = Vec::new();

        for line in content.lines() {
            if let Some(entry) = Self::parse_mountinfo_line(line) {
                // Filter out some system mounts that aren't useful
                if !entry.mount_point.starts_with("/proc") 
                    && !entry.mount_point.starts_with("/sys/kernel")
                    && !entry.mount_point.starts_with("/sys/fs/cgroup")
                    && entry.mount_point != PathBuf::from("/dev") {
                    mounts.push(Box::new(UnixMount::new(&entry)) as Box<dyn Mount>);
                }
            }
        }

        Ok(mounts)
    }

    /// Gets a mount for a specific path
    pub async fn get_mount_for_path(&self, path: &std::path::Path) -> NpioResult<Option<Box<dyn Mount>>> {
        let mounts = self.get_mounts().await?;
        
        // Find the mount that contains this path
        // We want the most specific (longest) mount point
        let mut best_match: Option<Box<dyn Mount>> = None;
        let mut best_match_len = 0;

        for mount in mounts {
            let mount_root = mount.get_root();
            let mount_uri = mount_root.uri();
            let mount_path = mount_uri.strip_prefix("file://").unwrap_or("");
            let mount_path_buf = PathBuf::from(mount_path);
            
            if path.starts_with(&mount_path_buf) {
                let len = mount_path_buf.components().count();
                if len > best_match_len {
                    best_match_len = len;
                    best_match = Some(mount);
                } else if best_match.is_some() {
                    // Drop the less specific mount
                    drop(mount);
                }
            } else {
                drop(mount);
            }
        }

        Ok(best_match)
    }
}

impl Default for MountBackend {
    fn default() -> Self {
        Self::new()
    }
}
