//! Mount point validation for advanced operations.

use std::path::Path;
use std::fs;
use std::fmt;
use std::os::unix::fs::{PermissionsExt, FileTypeExt};
use crate::mount::advanced::{DeviceInfo, config::ValidationConfig};

/// Pre-operation validation with detailed checks.
#[derive(Debug)]
pub struct MountValidator {
    config: ValidationConfig,
}

impl MountValidator {
    /// Creates a new mount validator.
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Validates a mount operation.
    pub async fn validate_mount(
        &self,
        source: &str,
        target: &Path,
    ) -> ValidationResult {
        let mut result = ValidationResult {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            metadata: ValidationMetadata::default(),
        };

        // Check mount point existence
        if self.config.check_mount_point_exists {
            if let Err(error) = self.check_mount_point_exists(target) {
                result.errors.push(error);
                result.is_valid = false;
            }
        }

        // Check mount point availability
        if self.config.check_mount_point_available {
            match self.check_mount_point_available(target).await {
                Ok(current_mounts) => {
                    result.metadata.current_mounts = current_mounts;
                }
                Err(error) => {
                    result.errors.push(error);
                    result.is_valid = false;
                }
            }
        }

        // Check permissions
        if self.config.check_permissions {
            if let Err(error) = self.check_permissions(target) {
                result.errors.push(error);
                result.is_valid = false;
            }
        }

        // Check filesystem type
        if self.config.check_filesystem {
            match self.detect_filesystem_type(source).await {
                Ok(fs_type) => {
                    result.metadata.filesystem_type = fs_type;
                }
                Err(warning) => {
                    result.warnings.push(warning);
                }
            }
        }

        // Check device availability
        if self.config.check_device_availability {
            match self.check_device_availability(source).await {
                Ok(device_info) => {
                    result.metadata.device_info = device_info;
                }
                Err(error) => {
                    result.errors.push(error);
                    result.is_valid = false;
                }
            }
        }

        // Collect available space information
        if let Ok(space) = self.get_available_space(target) {
            result.metadata.available_space = Some(space);
        }

        // Collect detailed mount information
        if let Ok(detailed_mounts) = self.get_detailed_mount_info().await {
            result.metadata.detailed_mounts = detailed_mounts;
        }

        // Collect filesystem features if filesystem type is known
        if let Some(ref fs_type) = result.metadata.filesystem_type {
            result.metadata.filesystem_features = self.get_filesystem_features(fs_type);
            result.metadata.mount_options_supported = self.get_supported_mount_options(fs_type);
        }

        // Collect performance characteristics
        if let Some(ref device_info) = result.metadata.device_info {
            if let Ok(perf_info) = self.get_performance_info(&device_info.device_path).await {
                result.metadata.performance_characteristics = Some(perf_info);
            }
        }

        // Collect security context information
        if let Ok(security_info) = self.get_security_info(source, target).await {
            result.metadata.security_context = Some(security_info);
        }

        result
    }

    /// Validates an unmount operation.
    pub async fn validate_unmount(&self, target: &Path) -> ValidationResult {
        let mut result = ValidationResult {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            metadata: ValidationMetadata::default(),
        };

        // Check if mount point exists
        if self.config.check_mount_point_exists {
            if let Err(error) = self.check_mount_point_exists(target) {
                result.errors.push(error);
                result.is_valid = false;
            }
        }

        // Check if mount point is actually mounted
        if self.config.check_mount_point_available {
            match self.check_mount_point_mounted(target).await {
                Ok(mount_info) => {
                    if let Some(mount) = mount_info {
                        result.metadata.current_mounts = vec![mount];
                    } else {
                        result.errors.push(ValidationError::MountPointNotMounted {
                            path: target.to_string_lossy().to_string(),
                        });
                        result.is_valid = false;
                    }
                }
                Err(error) => {
                    result.errors.push(error);
                    result.is_valid = false;
                }
            }
        }

        // Check permissions for unmount
        if self.config.check_permissions {
            if let Err(error) = self.check_unmount_permissions(target) {
                result.errors.push(error);
                result.is_valid = false;
            }
        }

        result
    }

    /// Checks if mount point exists and is a directory.
    fn check_mount_point_exists(&self, target: &Path) -> Result<(), ValidationError> {
        match fs::metadata(target) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    Err(ValidationError::MountPointNotDirectory {
                        path: target.to_string_lossy().to_string(),
                    })
                } else {
                    Ok(())
                }
            }
            Err(_) => Err(ValidationError::MountPointNotFound {
                path: target.to_string_lossy().to_string(),
            }),
        }
    }

    /// Checks if mount point is available (not already mounted).
    async fn check_mount_point_available(&self, target: &Path) -> Result<Vec<String>, ValidationError> {
        let current_mounts = self.get_current_mounts().await?;
        let target_str = target.to_string_lossy().to_string();
        
        for mount in &current_mounts {
            if mount == &target_str {
                return Err(ValidationError::MountPointInUse {
                    path: target_str,
                    current_mount: mount.clone(),
                });
            }
        }
        
        Ok(current_mounts)
    }

    /// Checks if mount point is currently mounted.
    async fn check_mount_point_mounted(&self, target: &Path) -> Result<Option<String>, ValidationError> {
        let current_mounts = self.get_current_mounts().await?;
        let target_str = target.to_string_lossy().to_string();
        
        for mount in current_mounts {
            if mount == target_str {
                return Ok(Some(mount));
            }
        }
        
        Ok(None)
    }

    /// Gets current mount points from /proc/mounts with detailed information.
    async fn get_current_mounts(&self) -> Result<Vec<String>, ValidationError> {
        match fs::read_to_string("/proc/mounts") {
            Ok(contents) => {
                let mounts = contents
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            Some(parts[1].to_string()) // Mount point is the second field
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(mounts)
            }
            Err(_) => Err(ValidationError::SystemError {
                message: "Failed to read /proc/mounts".to_string(),
            }),
        }
    }

    /// Gets detailed mount information including filesystem types and options.
    async fn get_detailed_mount_info(&self) -> Result<Vec<MountInfo>, ValidationError> {
        match fs::read_to_string("/proc/mounts") {
            Ok(contents) => {
                let mount_info = contents
                    .lines()
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 4 {
                            Some(MountInfo {
                                device: parts[0].to_string(),
                                mount_point: parts[1].to_string(),
                                filesystem_type: parts[2].to_string(),
                                options: parts[3].split(',').map(|s| s.to_string()).collect(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(mount_info)
            }
            Err(_) => Err(ValidationError::SystemError {
                message: "Failed to read /proc/mounts".to_string(),
            }),
        }
    }

    /// Finds mount information for a specific mount point.
    async fn find_mount_info(&self, mount_point: &str) -> Result<Option<MountInfo>, ValidationError> {
        let detailed_mounts = self.get_detailed_mount_info().await?;
        Ok(detailed_mounts.into_iter().find(|mount| mount.mount_point == mount_point))
    }

    /// Checks permissions for mount operation.
    fn check_permissions(&self, target: &Path) -> Result<(), ValidationError> {
        match fs::metadata(target) {
            Ok(metadata) => {
                let permissions = metadata.permissions();
                let mode = permissions.mode();
                
                // Check if we have write access to the directory
                if mode & 0o200 == 0 {
                    Err(ValidationError::InsufficientPermissions {
                        required: "Write access to mount point".to_string(),
                    })
                } else {
                    Ok(())
                }
            }
            Err(_) => Err(ValidationError::InsufficientPermissions {
                required: "Access to mount point".to_string(),
            }),
        }
    }

    /// Checks permissions for unmount operation.
    fn check_unmount_permissions(&self, target: &Path) -> Result<(), ValidationError> {
        // For unmount, we need to check if we have appropriate privileges
        // This is a simplified check - in practice, unmount permissions are more complex
        match fs::metadata(target) {
            Ok(_) => {
                // Check if we're running as root or have appropriate capabilities
                let uid = unsafe { libc::getuid() };
                if uid != 0 {
                    Err(ValidationError::InsufficientPermissions {
                        required: "Root privileges for unmount operation".to_string(),
                    })
                } else {
                    Ok(())
                }
            }
            Err(_) => Err(ValidationError::InsufficientPermissions {
                required: "Access to mount point".to_string(),
            }),
        }
    }

    /// Detects filesystem type of the source device using multiple methods.
    async fn detect_filesystem_type(&self, source: &str) -> Result<Option<String>, ValidationWarning> {
        // Try multiple detection methods in order of preference
        
        // Method 1: Use blkid command if available
        if let Ok(fs_type) = self.detect_filesystem_with_blkid(source).await {
            return Ok(Some(fs_type));
        }
        
        // Method 2: Read filesystem superblock directly
        if let Ok(fs_type) = self.detect_filesystem_from_superblock(source).await {
            return Ok(Some(fs_type));
        }
        
        // Method 3: Use file command as fallback
        if let Ok(fs_type) = self.detect_filesystem_with_file(source).await {
            return Ok(Some(fs_type));
        }
        
        // Method 4: Heuristic detection based on device name
        if source.starts_with("/dev/") {
            if let Some(fs_type) = self.detect_filesystem_heuristic(source) {
                return Ok(Some(fs_type));
            }
        }
        
        Err(ValidationWarning::CompatibilityIssue {
            issue: format!("Could not detect filesystem type for {}", source),
        })
    }

    /// Detects filesystem type using blkid command.
    async fn detect_filesystem_with_blkid(&self, source: &str) -> Result<String, ValidationError> {
        use tokio::process::Command;
        
        let output = Command::new("blkid")
            .arg("-o")
            .arg("value")
            .arg("-s")
            .arg("TYPE")
            .arg(source)
            .output()
            .await
            .map_err(|_| ValidationError::SystemError {
                message: "Failed to execute blkid command".to_string(),
            })?;
        
        if output.status.success() {
            let fs_type = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !fs_type.is_empty() {
                Ok(fs_type)
            } else {
                Err(ValidationError::SystemError {
                    message: "blkid returned empty filesystem type".to_string(),
                })
            }
        } else {
            Err(ValidationError::SystemError {
                message: "blkid command failed".to_string(),
            })
        }
    }

    /// Detects filesystem type by reading superblock signatures.
    async fn detect_filesystem_from_superblock(&self, source: &str) -> Result<String, ValidationError> {
        use tokio::fs::File;
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        
        let mut file = File::open(source).await.map_err(|_| ValidationError::SystemError {
            message: format!("Failed to open device {}", source),
        })?;
        
        // Read first few KB to check for filesystem signatures
        let mut buffer = vec![0u8; 4096];
        file.read_exact(&mut buffer).await.map_err(|_| ValidationError::SystemError {
            message: "Failed to read device superblock".to_string(),
        })?;
        
        // Check for common filesystem signatures
        if self.check_ext_signature(&buffer) {
            Ok("ext4".to_string())
        } else if self.check_ntfs_signature(&buffer) {
            Ok("ntfs".to_string())
        } else if self.check_fat_signature(&buffer) {
            Ok("vfat".to_string())
        } else if self.check_xfs_signature(&buffer) {
            Ok("xfs".to_string())
        } else if self.check_btrfs_signature(&buffer) {
            Ok("btrfs".to_string())
        } else {
            // Try reading at different offsets for other filesystems
            file.seek(std::io::SeekFrom::Start(32768)).await.map_err(|_| ValidationError::SystemError {
                message: "Failed to seek in device".to_string(),
            })?;
            
            file.read_exact(&mut buffer).await.map_err(|_| ValidationError::SystemError {
                message: "Failed to read device at offset".to_string(),
            })?;
            
            if self.check_reiserfs_signature(&buffer) {
                Ok("reiserfs".to_string())
            } else {
                Err(ValidationError::SystemError {
                    message: "Unknown filesystem signature".to_string(),
                })
            }
        }
    }

    /// Detects filesystem type using file command.
    async fn detect_filesystem_with_file(&self, source: &str) -> Result<String, ValidationError> {
        use tokio::process::Command;
        
        let output = Command::new("file")
            .arg("-s")
            .arg(source)
            .output()
            .await
            .map_err(|_| ValidationError::SystemError {
                message: "Failed to execute file command".to_string(),
            })?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout).to_lowercase();
            
            if output_str.contains("ext4") {
                Ok("ext4".to_string())
            } else if output_str.contains("ext3") {
                Ok("ext3".to_string())
            } else if output_str.contains("ext2") {
                Ok("ext2".to_string())
            } else if output_str.contains("ntfs") {
                Ok("ntfs".to_string())
            } else if output_str.contains("fat32") {
                Ok("vfat".to_string())
            } else if output_str.contains("fat16") {
                Ok("vfat".to_string())
            } else if output_str.contains("xfs") {
                Ok("xfs".to_string())
            } else if output_str.contains("btrfs") {
                Ok("btrfs".to_string())
            } else if output_str.contains("reiserfs") {
                Ok("reiserfs".to_string())
            } else {
                Err(ValidationError::SystemError {
                    message: "file command could not identify filesystem".to_string(),
                })
            }
        } else {
            Err(ValidationError::SystemError {
                message: "file command failed".to_string(),
            })
        }
    }

    /// Heuristic filesystem detection based on device naming patterns.
    fn detect_filesystem_heuristic(&self, source: &str) -> Option<String> {
        let device_name = source.to_lowercase();
        
        // Common naming patterns
        if device_name.contains("ntfs") {
            Some("ntfs".to_string())
        } else if device_name.contains("fat") || device_name.contains("vfat") {
            Some("vfat".to_string())
        } else if device_name.contains("ext") {
            Some("ext4".to_string())
        } else if device_name.contains("xfs") {
            Some("xfs".to_string())
        } else if device_name.contains("btrfs") {
            Some("btrfs".to_string())
        } else {
            None
        }
    }

    /// Checks for ext filesystem signature.
    fn check_ext_signature(&self, buffer: &[u8]) -> bool {
        // Ext filesystems have magic number 0xEF53 at offset 1080
        if buffer.len() >= 1082 {
            buffer[1080] == 0x53 && buffer[1081] == 0xEF
        } else {
            false
        }
    }

    /// Checks for NTFS filesystem signature.
    fn check_ntfs_signature(&self, buffer: &[u8]) -> bool {
        // NTFS has "NTFS    " at offset 3
        if buffer.len() >= 11 {
            &buffer[3..11] == b"NTFS    "
        } else {
            false
        }
    }

    /// Checks for FAT filesystem signature.
    fn check_fat_signature(&self, buffer: &[u8]) -> bool {
        // FAT has various signatures
        if buffer.len() >= 512 {
            // Check for FAT32 signature
            if buffer.len() >= 82 && &buffer[82..90] == b"FAT32   " {
                return true;
            }
            // Check for FAT16/FAT12 signature
            if buffer.len() >= 54 && (&buffer[54..62] == b"FAT16   " || &buffer[54..62] == b"FAT12   ") {
                return true;
            }
            // Check boot signature
            if buffer[510] == 0x55 && buffer[511] == 0xAA {
                // Additional FAT checks could go here
                return &buffer[0..3] == b"\xEB\x3C\x90" || &buffer[0..3] == b"\xEB\x58\x90";
            }
        }
        false
    }

    /// Checks for XFS filesystem signature.
    fn check_xfs_signature(&self, buffer: &[u8]) -> bool {
        // XFS has "XFSB" magic at the beginning
        if buffer.len() >= 4 {
            &buffer[0..4] == b"XFSB"
        } else {
            false
        }
    }

    /// Checks for Btrfs filesystem signature.
    fn check_btrfs_signature(&self, buffer: &[u8]) -> bool {
        // Btrfs has "_BHRfS_M" magic at offset 65600, but we check at superblock location
        if buffer.len() >= 72 {
            &buffer[64..72] == b"_BHRfS_M"
        } else {
            false
        }
    }

    /// Checks for ReiserFS filesystem signature.
    fn check_reiserfs_signature(&self, buffer: &[u8]) -> bool {
        // ReiserFS has "ReIsErFs" or "ReIsEr2Fs" magic
        if buffer.len() >= 52 {
            &buffer[52..60] == b"ReIsErFs" || &buffer[52..61] == b"ReIsEr2Fs"
        } else {
            false
        }
    }

    /// Checks device availability and collects comprehensive device information.
    async fn check_device_availability(&self, source: &str) -> Result<Option<DeviceInfo>, ValidationError> {
        match fs::metadata(source) {
            Ok(metadata) => {
                if metadata.file_type().is_block_device() {
                    // Collect comprehensive device information
                    let device_info = self.collect_device_information(source).await?;
                    Ok(Some(device_info))
                } else {
                    Err(ValidationError::DeviceNotFound {
                        device: source.to_string(),
                    })
                }
            }
            Err(_) => Err(ValidationError::DeviceNotFound {
                device: source.to_string(),
            }),
        }
    }

    /// Collects comprehensive device information from multiple sources.
    async fn collect_device_information(&self, device_path: &str) -> Result<DeviceInfo, ValidationError> {
        let mut device_info = DeviceInfo {
            device_path: device_path.to_string(),
            device_name: None,
            vendor: None,
            model: None,
            serial: None,
            size: None,
            removable: false,
        };

        // Extract device name from path
        if let Some(name) = Path::new(device_path).file_name() {
            device_info.device_name = Some(name.to_string_lossy().to_string());
        }

        // Try to get device information from sysfs
        if let Ok(sysfs_info) = self.get_device_info_from_sysfs(device_path).await {
            device_info.vendor = sysfs_info.vendor;
            device_info.model = sysfs_info.model;
            device_info.serial = sysfs_info.serial;
            device_info.size = sysfs_info.size;
            device_info.removable = sysfs_info.removable;
        }

        // Try to get additional information from udev
        if let Ok(udev_info) = self.get_device_info_from_udev(device_path).await {
            // Merge udev information, preferring non-empty values
            if device_info.vendor.is_none() && udev_info.vendor.is_some() {
                device_info.vendor = udev_info.vendor;
            }
            if device_info.model.is_none() && udev_info.model.is_some() {
                device_info.model = udev_info.model;
            }
            if device_info.serial.is_none() && udev_info.serial.is_some() {
                device_info.serial = udev_info.serial;
            }
        }

        // Try to get size information from blockdev if not available
        if device_info.size.is_none() {
            if let Ok(size) = self.get_device_size_with_blockdev(device_path).await {
                device_info.size = Some(size);
            }
        }

        Ok(device_info)
    }

    /// Gets device information from sysfs.
    async fn get_device_info_from_sysfs(&self, device_path: &str) -> Result<DeviceInfo, ValidationError> {
        let device_name = Path::new(device_path)
            .file_name()
            .ok_or_else(|| ValidationError::SystemError {
                message: "Invalid device path".to_string(),
            })?
            .to_string_lossy();

        let sysfs_path = format!("/sys/block/{}", device_name);
        
        let mut device_info = DeviceInfo {
            device_path: device_path.to_string(),
            device_name: Some(device_name.to_string()),
            vendor: None,
            model: None,
            serial: None,
            size: None,
            removable: false,
        };

        // Read vendor information
        if let Ok(vendor) = fs::read_to_string(format!("{}/device/vendor", sysfs_path)) {
            device_info.vendor = Some(vendor.trim().to_string());
        }

        // Read model information
        if let Ok(model) = fs::read_to_string(format!("{}/device/model", sysfs_path)) {
            device_info.model = Some(model.trim().to_string());
        }

        // Read serial number
        if let Ok(serial) = fs::read_to_string(format!("{}/device/serial", sysfs_path)) {
            device_info.serial = Some(serial.trim().to_string());
        }

        // Read size in sectors and convert to bytes
        if let Ok(size_str) = fs::read_to_string(format!("{}/size", sysfs_path)) {
            if let Ok(sectors) = size_str.trim().parse::<u64>() {
                // Assume 512 bytes per sector (standard for most devices)
                device_info.size = Some(sectors * 512);
            }
        }

        // Check if device is removable
        if let Ok(removable_str) = fs::read_to_string(format!("{}/removable", sysfs_path)) {
            device_info.removable = removable_str.trim() == "1";
        }

        Ok(device_info)
    }

    /// Gets device information from udev using udevadm.
    async fn get_device_info_from_udev(&self, device_path: &str) -> Result<DeviceInfo, ValidationError> {
        use tokio::process::Command;

        let output = Command::new("udevadm")
            .arg("info")
            .arg("--query=property")
            .arg(format!("--name={}", device_path))
            .output()
            .await
            .map_err(|_| ValidationError::SystemError {
                message: "Failed to execute udevadm command".to_string(),
            })?;

        if !output.status.success() {
            return Err(ValidationError::SystemError {
                message: "udevadm command failed".to_string(),
            });
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut device_info = DeviceInfo {
            device_path: device_path.to_string(),
            device_name: None,
            vendor: None,
            model: None,
            serial: None,
            size: None,
            removable: false,
        };

        // Parse udev properties
        for line in output_str.lines() {
            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "ID_VENDOR" => device_info.vendor = Some(value.to_string()),
                    "ID_MODEL" => device_info.model = Some(value.to_string()),
                    "ID_SERIAL_SHORT" => device_info.serial = Some(value.to_string()),
                    "DEVNAME" => {
                        if let Some(name) = Path::new(value).file_name() {
                            device_info.device_name = Some(name.to_string_lossy().to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(device_info)
    }

    /// Gets device size using blockdev command.
    async fn get_device_size_with_blockdev(&self, device_path: &str) -> Result<u64, ValidationError> {
        use tokio::process::Command;

        let output = Command::new("blockdev")
            .arg("--getsize64")
            .arg(device_path)
            .output()
            .await
            .map_err(|_| ValidationError::SystemError {
                message: "Failed to execute blockdev command".to_string(),
            })?;

        if output.status.success() {
            let size_str = String::from_utf8_lossy(&output.stdout);
            size_str.trim().parse::<u64>().map_err(|_| ValidationError::SystemError {
                message: "Failed to parse device size".to_string(),
            })
        } else {
            Err(ValidationError::SystemError {
                message: "blockdev command failed".to_string(),
            })
        }
    }

    /// Gets available space at the target location.
    fn get_available_space(&self, target: &Path) -> Result<u64, ValidationError> {
        use std::mem;
        
        let path_cstr = std::ffi::CString::new(target.to_string_lossy().as_bytes())
            .map_err(|_| ValidationError::SystemError {
                message: "Invalid path for statvfs".to_string(),
            })?;
        
        unsafe {
            let mut statvfs: libc::statvfs = mem::zeroed();
            if libc::statvfs(path_cstr.as_ptr(), &mut statvfs) == 0 {
                let available_bytes = statvfs.f_bavail * statvfs.f_frsize;
                Ok(available_bytes)
            } else {
                Err(ValidationError::SystemError {
                    message: "Failed to get filesystem statistics".to_string(),
                })
            }
        }
    }

    /// Gets filesystem features for a given filesystem type.
    fn get_filesystem_features(&self, fs_type: &str) -> Vec<String> {
        match fs_type {
            "ext4" => vec![
                "journaling".to_string(),
                "extents".to_string(),
                "large_files".to_string(),
                "resize_inode".to_string(),
                "dir_index".to_string(),
                "filetype".to_string(),
            ],
            "ext3" => vec![
                "journaling".to_string(),
                "large_files".to_string(),
                "dir_index".to_string(),
                "filetype".to_string(),
            ],
            "ext2" => vec![
                "large_files".to_string(),
                "filetype".to_string(),
            ],
            "xfs" => vec![
                "journaling".to_string(),
                "large_files".to_string(),
                "extended_attributes".to_string(),
                "quotas".to_string(),
                "realtime".to_string(),
            ],
            "btrfs" => vec![
                "copy_on_write".to_string(),
                "snapshots".to_string(),
                "compression".to_string(),
                "checksums".to_string(),
                "subvolumes".to_string(),
                "raid".to_string(),
            ],
            "ntfs" => vec![
                "journaling".to_string(),
                "compression".to_string(),
                "encryption".to_string(),
                "large_files".to_string(),
                "alternate_data_streams".to_string(),
            ],
            "vfat" | "fat32" => vec![
                "long_filenames".to_string(),
                "case_insensitive".to_string(),
            ],
            "reiserfs" => vec![
                "journaling".to_string(),
                "tail_packing".to_string(),
            ],
            _ => vec!["basic_filesystem".to_string()],
        }
    }

    /// Gets supported mount options for a given filesystem type.
    fn get_supported_mount_options(&self, fs_type: &str) -> Vec<String> {
        let mut options = vec![
            "ro".to_string(),
            "rw".to_string(),
            "noexec".to_string(),
            "nosuid".to_string(),
            "nodev".to_string(),
        ];

        match fs_type {
            "ext4" | "ext3" | "ext2" => {
                options.extend(vec![
                    "acl".to_string(),
                    "user_xattr".to_string(),
                    "barrier".to_string(),
                    "data=journal".to_string(),
                    "data=ordered".to_string(),
                    "data=writeback".to_string(),
                ]);
            }
            "xfs" => {
                options.extend(vec![
                    "logbufs".to_string(),
                    "logbsize".to_string(),
                    "noalign".to_string(),
                    "swalloc".to_string(),
                ]);
            }
            "btrfs" => {
                options.extend(vec![
                    "compress".to_string(),
                    "compress-force".to_string(),
                    "datacow".to_string(),
                    "nodatacow".to_string(),
                    "subvol".to_string(),
                    "subvolid".to_string(),
                ]);
            }
            "ntfs" => {
                options.extend(vec![
                    "uid".to_string(),
                    "gid".to_string(),
                    "umask".to_string(),
                    "fmask".to_string(),
                    "dmask".to_string(),
                ]);
            }
            "vfat" | "fat32" => {
                options.extend(vec![
                    "uid".to_string(),
                    "gid".to_string(),
                    "umask".to_string(),
                    "codepage".to_string(),
                    "iocharset".to_string(),
                ]);
            }
            _ => {}
        }

        options
    }

    /// Gets performance characteristics for a device.
    async fn get_performance_info(&self, device_path: &str) -> Result<PerformanceInfo, ValidationError> {
        let mut perf_info = PerformanceInfo {
            read_speed_estimate: None,
            write_speed_estimate: None,
            random_access_performance: None,
            recommended_block_size: None,
            supports_trim: false,
            rotational: None,
        };

        // Try to determine if device is rotational (HDD vs SSD)
        if let Ok(rotational) = self.check_device_rotational(device_path).await {
            perf_info.rotational = Some(rotational);
            
            // Set performance estimates based on device type
            if rotational {
                // HDD estimates
                perf_info.read_speed_estimate = Some(100_000_000); // ~100 MB/s
                perf_info.write_speed_estimate = Some(80_000_000); // ~80 MB/s
                perf_info.random_access_performance = Some("fair".to_string());
                perf_info.recommended_block_size = Some(4096);
            } else {
                // SSD estimates
                perf_info.read_speed_estimate = Some(500_000_000); // ~500 MB/s
                perf_info.write_speed_estimate = Some(400_000_000); // ~400 MB/s
                perf_info.random_access_performance = Some("excellent".to_string());
                perf_info.recommended_block_size = Some(4096);
                perf_info.supports_trim = true;
            }
        }

        // Try to get more specific performance info from sysfs
        if let Ok(queue_info) = self.get_device_queue_info(device_path).await {
            if let Some(block_size) = queue_info.optimal_io_size {
                perf_info.recommended_block_size = Some(block_size);
            }
        }

        Ok(perf_info)
    }

    /// Checks if a device is rotational (HDD) or not (SSD).
    async fn check_device_rotational(&self, device_path: &str) -> Result<bool, ValidationError> {
        let device_name = Path::new(device_path)
            .file_name()
            .ok_or_else(|| ValidationError::SystemError {
                message: "Invalid device path".to_string(),
            })?
            .to_string_lossy();

        let rotational_path = format!("/sys/block/{}/queue/rotational", device_name);
        
        match fs::read_to_string(&rotational_path) {
            Ok(content) => Ok(content.trim() == "1"),
            Err(_) => Err(ValidationError::SystemError {
                message: "Could not determine device rotation type".to_string(),
            }),
        }
    }

    /// Gets device queue information from sysfs.
    async fn get_device_queue_info(&self, device_path: &str) -> Result<DeviceQueueInfo, ValidationError> {
        let device_name = Path::new(device_path)
            .file_name()
            .ok_or_else(|| ValidationError::SystemError {
                message: "Invalid device path".to_string(),
            })?
            .to_string_lossy();

        let queue_path = format!("/sys/block/{}/queue", device_name);
        
        let mut queue_info = DeviceQueueInfo {
            optimal_io_size: None,
            minimum_io_size: None,
            physical_block_size: None,
            logical_block_size: None,
        };

        // Read optimal I/O size
        if let Ok(content) = fs::read_to_string(format!("{}/optimal_io_size", queue_path)) {
            if let Ok(size) = content.trim().parse::<u32>() {
                if size > 0 {
                    queue_info.optimal_io_size = Some(size);
                }
            }
        }

        // Read minimum I/O size
        if let Ok(content) = fs::read_to_string(format!("{}/minimum_io_size", queue_path)) {
            if let Ok(size) = content.trim().parse::<u32>() {
                queue_info.minimum_io_size = Some(size);
            }
        }

        // Read physical block size
        if let Ok(content) = fs::read_to_string(format!("{}/physical_block_size", queue_path)) {
            if let Ok(size) = content.trim().parse::<u32>() {
                queue_info.physical_block_size = Some(size);
            }
        }

        // Read logical block size
        if let Ok(content) = fs::read_to_string(format!("{}/logical_block_size", queue_path)) {
            if let Ok(size) = content.trim().parse::<u32>() {
                queue_info.logical_block_size = Some(size);
            }
        }

        Ok(queue_info)
    }

    /// Gets security context information for a mount operation.
    async fn get_security_info(&self, source: &str, target: &Path) -> Result<SecurityInfo, ValidationError> {
        let mut security_info = SecurityInfo {
            requires_elevated_privileges: false,
            supports_access_controls: false,
            encryption_status: None,
            selinux_context: None,
            recommended_mount_options: Vec::new(),
            security_warnings: Vec::new(),
        };

        // Check if elevated privileges are required
        let uid = unsafe { libc::getuid() };
        if uid != 0 {
            security_info.requires_elevated_privileges = true;
            security_info.security_warnings.push(
                "Mount operations typically require root privileges".to_string()
            );
        }

        // Check for SELinux context if available
        if let Ok(context) = self.get_selinux_context(target).await {
            security_info.selinux_context = Some(context);
            security_info.supports_access_controls = true;
        }

        // Check for encryption status
        if let Ok(encryption) = self.check_device_encryption(source).await {
            security_info.encryption_status = Some(encryption);
        }

        // Add recommended security mount options
        security_info.recommended_mount_options = vec![
            "nodev".to_string(),
            "nosuid".to_string(),
        ];

        // Add filesystem-specific security recommendations
        if source.contains("usb") || source.contains("removable") {
            security_info.recommended_mount_options.push("noexec".to_string());
            security_info.security_warnings.push(
                "Removable devices should be mounted with restrictive options".to_string()
            );
        }

        Ok(security_info)
    }

    /// Gets SELinux context for a path.
    async fn get_selinux_context(&self, path: &Path) -> Result<String, ValidationError> {
        use tokio::process::Command;

        let output = Command::new("ls")
            .arg("-Z")
            .arg(path)
            .output()
            .await
            .map_err(|_| ValidationError::SystemError {
                message: "Failed to get SELinux context".to_string(),
            })?;

        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            // Parse SELinux context from ls -Z output
            if let Some(context) = output_str.split_whitespace().next() {
                if context.contains(':') {
                    return Ok(context.to_string());
                }
            }
        }

        Err(ValidationError::SystemError {
            message: "SELinux not available or context not found".to_string(),
        })
    }

    /// Checks device encryption status.
    async fn check_device_encryption(&self, device_path: &str) -> Result<String, ValidationError> {
        use tokio::process::Command;

        // Try to check with cryptsetup
        let output = Command::new("cryptsetup")
            .arg("isLuks")
            .arg(device_path)
            .output()
            .await;

        if let Ok(output) = output {
            if output.status.success() {
                return Ok("LUKS encrypted".to_string());
            }
        }

        // Try to check with blkid for encryption signatures
        let output = Command::new("blkid")
            .arg("-o")
            .arg("value")
            .arg("-s")
            .arg("TYPE")
            .arg(device_path)
            .output()
            .await;

        if let Ok(output) = output {
            if output.status.success() {
                let fs_type = String::from_utf8_lossy(&output.stdout);
                if fs_type.contains("crypto") || fs_type.contains("luks") {
                    return Ok("Encrypted".to_string());
                }
            }
        }

        Ok("Not encrypted".to_string())
    }
}

/// Device queue information from sysfs.
#[derive(Debug)]
struct DeviceQueueInfo {
    optimal_io_size: Option<u32>,
    minimum_io_size: Option<u32>,
    physical_block_size: Option<u32>,
    logical_block_size: Option<u32>,
}

/// Result of validation checks.
#[derive(Debug, Default)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub metadata: ValidationMetadata,
}

/// Detailed mount information.
#[derive(Debug, Clone)]
pub struct MountInfo {
    pub device: String,
    pub mount_point: String,
    pub filesystem_type: String,
    pub options: Vec<String>,
}

/// Validation error types with enhanced recovery suggestions.
#[derive(Debug, Clone)]
pub enum ValidationError {
    MountPointNotFound { path: String },
    MountPointInUse { path: String, current_mount: String },
    MountPointNotDirectory { path: String },
    MountPointNotMounted { path: String },
    InsufficientPermissions { required: String },
    InvalidFilesystem { fs_type: String },
    DeviceNotFound { device: String },
    SystemError { message: String },
}

impl ValidationError {
    /// Gets descriptive error message with context.
    pub fn message(&self) -> String {
        match self {
            ValidationError::MountPointNotFound { path } => {
                format!("Mount point '{}' does not exist", path)
            }
            ValidationError::MountPointInUse { path, current_mount } => {
                format!("Mount point '{}' is already in use by '{}'", path, current_mount)
            }
            ValidationError::MountPointNotDirectory { path } => {
                format!("Mount point '{}' exists but is not a directory", path)
            }
            ValidationError::MountPointNotMounted { path } => {
                format!("Mount point '{}' is not currently mounted", path)
            }
            ValidationError::InsufficientPermissions { required } => {
                format!("Insufficient permissions: {}", required)
            }
            ValidationError::InvalidFilesystem { fs_type } => {
                format!("Invalid or unsupported filesystem type: {}", fs_type)
            }
            ValidationError::DeviceNotFound { device } => {
                format!("Device '{}' not found or not accessible", device)
            }
            ValidationError::SystemError { message } => {
                format!("System error: {}", message)
            }
        }
    }

    /// Gets recovery suggestions for this error.
    pub fn recovery_suggestions(&self) -> Vec<String> {
        match self {
            ValidationError::MountPointNotFound { path } => {
                vec![
                    format!("Create the mount point directory: mkdir -p '{}'", path),
                    "Verify the path is correct and accessible".to_string(),
                    "Check parent directory permissions".to_string(),
                ]
            }
            ValidationError::MountPointInUse { path, current_mount } => {
                vec![
                    format!("Unmount the existing mount: umount '{}'", path),
                    format!("Use a different mount point instead of '{}'", path),
                    format!("Check if '{}' can be safely unmounted", current_mount),
                    "Use 'lsof' or 'fuser' to check what processes are using the mount point".to_string(),
                ]
            }
            ValidationError::MountPointNotDirectory { path } => {
                vec![
                    format!("Remove the file and create directory: rm '{}' && mkdir -p '{}'", path, path),
                    "Use a different path for the mount point".to_string(),
                    "Check if the existing file is important before removing".to_string(),
                ]
            }
            ValidationError::MountPointNotMounted { path } => {
                vec![
                    format!("Verify '{}' should be mounted", path),
                    "Check if the device was already unmounted".to_string(),
                    "Use 'mount' command to see current mounts".to_string(),
                ]
            }
            ValidationError::InsufficientPermissions { required } => {
                vec![
                    "Run the command with sudo or as root".to_string(),
                    format!("Ensure you have the required permission: {}", required),
                    "Check file and directory ownership and permissions".to_string(),
                    "Verify your user is in the appropriate groups (e.g., disk, storage)".to_string(),
                ]
            }
            ValidationError::InvalidFilesystem { fs_type } => {
                vec![
                    format!("Install support for '{}' filesystem", fs_type),
                    "Check if the filesystem type is correct".to_string(),
                    "Try auto-detecting the filesystem type".to_string(),
                    "Verify the device contains a valid filesystem".to_string(),
                ]
            }
            ValidationError::DeviceNotFound { device } => {
                vec![
                    format!("Check if device '{}' exists: ls -l '{}'", device, device),
                    "Verify the device is connected and recognized by the system".to_string(),
                    "Check dmesg for device detection messages".to_string(),
                    "Try refreshing device list: udevadm trigger".to_string(),
                    "For USB devices, try reconnecting the device".to_string(),
                ]
            }
            ValidationError::SystemError { message: _ } => {
                vec![
                    "Check system logs for more details: journalctl -xe".to_string(),
                    "Verify system resources are available (disk space, memory)".to_string(),
                    "Try the operation again after a brief wait".to_string(),
                    "Check if required system tools are installed".to_string(),
                ]
            }
        }
    }

    /// Gets the error severity level.
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ValidationError::MountPointNotFound { .. } => ErrorSeverity::High,
            ValidationError::MountPointInUse { .. } => ErrorSeverity::Medium,
            ValidationError::MountPointNotDirectory { .. } => ErrorSeverity::High,
            ValidationError::MountPointNotMounted { .. } => ErrorSeverity::Medium,
            ValidationError::InsufficientPermissions { .. } => ErrorSeverity::High,
            ValidationError::InvalidFilesystem { .. } => ErrorSeverity::High,
            ValidationError::DeviceNotFound { .. } => ErrorSeverity::Critical,
            ValidationError::SystemError { .. } => ErrorSeverity::High,
        }
    }
}

/// Error severity levels for validation errors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Low => write!(f, "LOW"),
            ErrorSeverity::Medium => write!(f, "MEDIUM"),
            ErrorSeverity::High => write!(f, "HIGH"),
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Validation warning types with recovery suggestions.
#[derive(Debug, Clone)]
pub enum ValidationWarning {
    PerformanceImpact { reason: String },
    SecurityConcern { concern: String },
    CompatibilityIssue { issue: String },
}

impl ValidationWarning {
    /// Gets descriptive warning message.
    pub fn message(&self) -> String {
        match self {
            ValidationWarning::PerformanceImpact { reason } => {
                format!("Performance impact warning: {}", reason)
            }
            ValidationWarning::SecurityConcern { concern } => {
                format!("Security concern: {}", concern)
            }
            ValidationWarning::CompatibilityIssue { issue } => {
                format!("Compatibility issue: {}", issue)
            }
        }
    }

    /// Gets recovery suggestions for this warning.
    pub fn recovery_suggestions(&self) -> Vec<String> {
        match self {
            ValidationWarning::PerformanceImpact { reason } => {
                if reason.contains("slow") || reason.contains("performance") {
                    vec![
                        "Consider using faster storage or different mount options".to_string(),
                        "Monitor system performance during operation".to_string(),
                        "Check if the device supports faster interfaces".to_string(),
                    ]
                } else {
                    vec![
                        "Monitor system resources during operation".to_string(),
                        "Consider alternative approaches if performance is critical".to_string(),
                    ]
                }
            }
            ValidationWarning::SecurityConcern { concern } => {
                if concern.contains("permission") {
                    vec![
                        "Review and adjust file permissions as needed".to_string(),
                        "Consider using more restrictive mount options".to_string(),
                        "Verify the security implications of this operation".to_string(),
                    ]
                } else {
                    vec![
                        "Review security implications before proceeding".to_string(),
                        "Consider using additional security measures".to_string(),
                        "Consult security policies for your environment".to_string(),
                    ]
                }
            }
            ValidationWarning::CompatibilityIssue { issue } => {
                if issue.contains("filesystem") {
                    vec![
                        "Install additional filesystem support packages".to_string(),
                        "Verify filesystem compatibility with your system".to_string(),
                        "Consider using a different filesystem type".to_string(),
                        "Check kernel module availability for this filesystem".to_string(),
                    ]
                } else if issue.contains("detect") {
                    vec![
                        "Try specifying the filesystem type explicitly".to_string(),
                        "Use filesystem detection tools like 'blkid' or 'file'".to_string(),
                        "Verify the device contains a valid filesystem".to_string(),
                    ]
                } else {
                    vec![
                        "Check system compatibility requirements".to_string(),
                        "Verify all required tools and libraries are installed".to_string(),
                        "Consider alternative approaches if compatibility issues persist".to_string(),
                    ]
                }
            }
        }
    }

    /// Gets the warning severity level.
    pub fn severity(&self) -> WarningSeverity {
        match self {
            ValidationWarning::PerformanceImpact { .. } => WarningSeverity::Medium,
            ValidationWarning::SecurityConcern { .. } => WarningSeverity::High,
            ValidationWarning::CompatibilityIssue { .. } => WarningSeverity::Medium,
        }
    }
}

/// Warning severity levels for validation warnings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WarningSeverity {
    Low,
    Medium,
    High,
}

impl fmt::Display for WarningSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WarningSeverity::Low => write!(f, "LOW"),
            WarningSeverity::Medium => write!(f, "MEDIUM"),
            WarningSeverity::High => write!(f, "HIGH"),
        }
    }
}

/// Metadata collected during validation with comprehensive information.
#[derive(Debug, Default)]
pub struct ValidationMetadata {
    pub filesystem_type: Option<String>,
    pub device_info: Option<DeviceInfo>,
    pub current_mounts: Vec<String>,
    pub available_space: Option<u64>,
    pub detailed_mounts: Vec<MountInfo>,
    pub filesystem_features: Vec<String>,
    pub mount_options_supported: Vec<String>,
    pub performance_characteristics: Option<PerformanceInfo>,
    pub security_context: Option<SecurityInfo>,
}

impl ValidationMetadata {
    /// Creates new empty validation metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds filesystem feature information.
    pub fn add_filesystem_feature(&mut self, feature: String) {
        self.filesystem_features.push(feature);
    }

    /// Adds supported mount option.
    pub fn add_supported_mount_option(&mut self, option: String) {
        self.mount_options_supported.push(option);
    }

    /// Sets performance characteristics.
    pub fn set_performance_info(&mut self, info: PerformanceInfo) {
        self.performance_characteristics = Some(info);
    }

    /// Sets security context information.
    pub fn set_security_info(&mut self, info: SecurityInfo) {
        self.security_context = Some(info);
    }

    /// Gets a summary of collected metadata.
    pub fn summary(&self) -> String {
        let mut summary = Vec::new();
        
        if let Some(ref fs_type) = self.filesystem_type {
            summary.push(format!("Filesystem: {}", fs_type));
        }
        
        if let Some(ref device_info) = self.device_info {
            if let Some(ref model) = device_info.model {
                summary.push(format!("Device: {}", model));
            }
            if let Some(size) = device_info.size {
                summary.push(format!("Size: {} bytes", size));
            }
        }
        
        if let Some(space) = self.available_space {
            summary.push(format!("Available space: {} bytes", space));
        }
        
        summary.push(format!("Current mounts: {}", self.current_mounts.len()));
        
        if !self.filesystem_features.is_empty() {
            summary.push(format!("Features: {}", self.filesystem_features.join(", ")));
        }
        
        summary.join("; ")
    }
}

/// Performance characteristics of the storage device.
#[derive(Debug, Clone)]
pub struct PerformanceInfo {
    pub read_speed_estimate: Option<u64>, // bytes per second
    pub write_speed_estimate: Option<u64>, // bytes per second
    pub random_access_performance: Option<String>, // "excellent", "good", "fair", "poor"
    pub recommended_block_size: Option<u32>,
    pub supports_trim: bool,
    pub rotational: Option<bool>, // true for HDDs, false for SSDs
}

/// Security context information for the mount operation.
#[derive(Debug, Clone)]
pub struct SecurityInfo {
    pub requires_elevated_privileges: bool,
    pub supports_access_controls: bool,
    pub encryption_status: Option<String>,
    pub selinux_context: Option<String>,
    pub recommended_mount_options: Vec<String>,
    pub security_warnings: Vec<String>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for ValidationError {}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}