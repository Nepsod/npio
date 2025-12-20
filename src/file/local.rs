use std::path::PathBuf;
use std::os::unix::fs::{PermissionsExt, MetadataExt};
use std::os::unix::ffi::OsStrExt;
use async_trait::async_trait;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use libc;

use crate::cancellable::Cancellable;
use crate::error::{NpioError, NpioResult, IOErrorEnum};
use crate::file::{File, FileQueryInfoFlags};
use crate::file_enumerator::FileEnumerator;
use crate::file_info::{FileInfo, FileType, FileAttributeType};
use crate::iostream::{InputStream, OutputStream};

impl InputStream for fs::File {
    fn close(&mut self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        Ok(())
    }
}

impl OutputStream for fs::File {
    fn close(&mut self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        Ok(())
    }

    fn flush(&mut self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct LocalFile {
    path: PathBuf,
}

impl LocalFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl File for LocalFile {
    fn uri(&self) -> String {
        format!("file://{}", self.path.to_string_lossy())
    }

    fn basename(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string())
    }

    fn parent(&self) -> Option<Box<dyn File>> {
        self.path.parent().map(|p| Box::new(LocalFile::new(p.to_path_buf())) as Box<dyn File>)
    }

    fn child(&self, name: &str) -> Box<dyn File> {
        Box::new(LocalFile::new(self.path.join(name)))
    }

    async fn query_info(&self, attributes: &str, cancellable: Option<&Cancellable>) -> NpioResult<FileInfo> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let metadata = fs::symlink_metadata(&self.path).await?;
        let mut info = FileInfo::new();

        info.set_name(&self.basename());
        info.set_size(metadata.len());
        
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                info.set_modification_time(duration.as_secs());
            }
        }

        let file_type = if metadata.is_dir() {
            FileType::Directory
        } else if metadata.is_symlink() {
            FileType::SymbolicLink
        } else {
            FileType::Regular
        };
        info.set_file_type(file_type);

        // MIME detection
        if attributes.contains("standard::content-type") || attributes.contains("standard::*") {
            let mime_type = if file_type == FileType::Directory {
                "inode/directory".to_string()
            } else {
                crate::metadata::MimeResolver::guess_mime_type(&self.path)
            };
            info.set_content_type(&mime_type);
            
            if attributes.contains("standard::icon") || attributes.contains("standard::*") {
                 let icon = crate::metadata::MimeResolver::get_icon_name(&mime_type);
                 info.set_attribute("standard::icon", crate::file_info::FileAttributeType::String(icon));
            }
        }
        
        Ok(info)
    }

    async fn read(&self, cancellable: Option<&Cancellable>) -> NpioResult<Box<dyn InputStream>> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        let file = fs::File::open(&self.path).await?;
        Ok(Box::new(file))
    }

    async fn replace(
        &self,
        _etag: Option<&str>,
        _make_backup: bool,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<dyn OutputStream>> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        // TODO: Handle etag and backup
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .await?;
        Ok(Box::new(file))
    }
    
    async fn create_file(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<dyn OutputStream>> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
            .await?;
        Ok(Box::new(file))
    }

    async fn append_to(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<dyn OutputStream>> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        let file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.path)
            .await?;
        Ok(Box::new(file))
    }

    async fn delete(&self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        let metadata = fs::symlink_metadata(&self.path).await?;
        if metadata.is_dir() {
            fs::remove_dir(&self.path).await?;
        } else {
            fs::remove_file(&self.path).await?;
        }
        Ok(())
    }

    async fn make_directory(&self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        fs::create_dir(&self.path).await?;
        Ok(())
    }

    async fn enumerate_children(
        &self,
        _attributes: &str,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<dyn FileEnumerator>> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        let read_dir = fs::read_dir(&self.path).await?;
        Ok(Box::new(LocalFileEnumerator { read_dir }))
    }

    async fn move_to(
        &self,
        destination: &dyn File,
        flags: crate::job::CopyFlags,
        cancellable: Option<&Cancellable>,
        progress_callback: Option<crate::job::ProgressCallback>,
    ) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        
        // Basic implementation for now: rename if local, else copy+delete
        if destination.uri().starts_with("file://") {
             // Parse destination path
             let uri = destination.uri();
             let dest_path_str = uri.trim_start_matches("file://");
             let dest_path = PathBuf::from(dest_path_str);
             
             // Check overwrite
             if dest_path.exists() && !flags.contains(crate::job::CopyFlags::OVERWRITE) {
                 return Err(NpioError::new(IOErrorEnum::Exists, "Destination exists"));
             }
             
             tokio::fs::rename(&self.path, dest_path).await
                .map_err(|e| NpioError::new(IOErrorEnum::Failed, e.to_string()))?;
                
             Ok(())
        } else {
            // Fallback to copy + delete
            self.copy(destination, flags, cancellable, progress_callback).await?;
            self.delete(cancellable).await?;
            Ok(())
        }
    }

    async fn copy(
        &self,
        destination: &dyn File,
        flags: crate::job::CopyFlags,
        cancellable: Option<&Cancellable>,
        progress_callback: Option<crate::job::ProgressCallback>,
    ) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        // Open source
        let mut input = self.read(cancellable).await?;
        
        // Open destination
        let mut output = if flags.contains(crate::job::CopyFlags::OVERWRITE) {
            destination.replace(None, false, cancellable).await?
        } else {
            destination.create_file(cancellable).await?
        };
        
        // Copy loop with progress
        let mut buffer = [0u8; 8192];
        let mut total_written = 0;
        let total_size = self.query_info("standard::size", cancellable).await
            .ok()
            .map(|i| i.get_size())
            .unwrap_or(0) as u64;

        loop {
            if let Some(c) = cancellable {
                if c.is_cancelled() {
                    return Err(NpioError::new(IOErrorEnum::Cancelled, "Operation cancelled"));
                }
            }

            let n = input.read(&mut buffer).await
                .map_err(|e| NpioError::new(IOErrorEnum::Failed, e.to_string()))?;
                
            if n == 0 {
                break;
            }
            
            output.write_all(&buffer[..n]).await
                .map_err(|e| NpioError::new(IOErrorEnum::Failed, e.to_string()))?;
                
            total_written += n as u64;
            
            if let Some(ref cb) = progress_callback {
                cb(total_written, total_size);
            }
        }
        
        output.close(cancellable)?;
        input.close(cancellable)?;
        
        Ok(())
    }

    async fn exists(&self, cancellable: Option<&Cancellable>) -> NpioResult<bool> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        Ok(self.path.exists())
    }

    async fn monitor(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<crate::monitor::FileMonitor>> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        use notify::{Watcher, RecursiveMode, EventKind};
        use tokio::sync::mpsc;
        use crate::monitor::{FileMonitor, FileMonitorEvent};

        let (tx, rx) = mpsc::channel(100);
        let path = self.path.clone();
        let tx_clone = tx.clone();
        
        // Create watcher with a closure that sends events to the tokio channel
        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            match res {
                Ok(event) => {
                    let npio_event = match event.kind {
                        EventKind::Create(_) => {
                            if let Some(p) = event.paths.first() {
                                Some(FileMonitorEvent::Created(Box::new(LocalFile::new(p.clone()))))
                            } else {
                                None
                            }
                        },
                        EventKind::Modify(_) => {
                            if let Some(p) = event.paths.first() {
                                Some(FileMonitorEvent::Changed(Box::new(LocalFile::new(p.clone())), None))
                            } else {
                                None
                            }
                        },
                        EventKind::Remove(_) => {
                            if let Some(p) = event.paths.first() {
                                Some(FileMonitorEvent::Deleted(Box::new(LocalFile::new(p.clone()))))
                            } else {
                                None
                            }
                        },
                        _ => None,
                    };

                    if let Some(e) = npio_event {
                        // We are in notify's thread, so we can block
                        let _ = tx_clone.blocking_send(e);
                    }
                },
                Err(_) => {},
            }
        }).map_err(|e| NpioError::new(IOErrorEnum::Failed, e.to_string()))?;
            
        // Watch the path
        watcher.watch(&path, RecursiveMode::NonRecursive)
            .map_err(|e| NpioError::new(IOErrorEnum::Failed, e.to_string()))?;

        Ok(Box::new(FileMonitor::new(rx, cancellable.cloned(), Some(Box::new(watcher)))))
    }

    async fn trash(&self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        use directories::ProjectDirs;
        use std::path::Path;
        use chrono::Utc;

        // Get XDG_DATA_HOME, default to ~/.local/share
        let data_home = std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                ProjectDirs::from("", "", "")
                    .map(|dirs| dirs.data_dir().to_path_buf())
            })
            .ok_or_else(|| NpioError::new(IOErrorEnum::Failed, "Could not determine XDG_DATA_HOME"))?;

        let trash_files = data_home.join("Trash").join("files");
        let trash_info = data_home.join("Trash").join("info");

        // Create trash directories if they don't exist
        fs::create_dir_all(&trash_files).await?;
        fs::create_dir_all(&trash_info).await?;

        // Get the original path as absolute path (required by FreeDesktop Trash spec)
        // First try canonicalize, which resolves symlinks and makes absolute
        let original_path = match self.path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                // If canonicalize fails (e.g., file doesn't exist yet or symlink broken),
                // convert relative path to absolute using current directory
                if self.path.is_absolute() {
                    self.path.clone()
                } else {
                    let current_dir = std::env::current_dir()
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Could not get current directory: {}", e)))?;
                    current_dir.join(&self.path)
                }
            }
        };
        let original_path_str = original_path.to_string_lossy().to_string();
        
        // Get the basename for the trash file
        let basename = self.basename();
        
        // Handle name conflicts by appending numbers
        // Use atomic rename operation to avoid race conditions
        let mut trash_file_path = trash_files.join(&basename);
        let mut counter = 1;
        
        // Try to rename atomically - if it fails due to file existing, generate new name and retry
        loop {
            match tokio::fs::rename(&self.path, &trash_file_path).await {
                Ok(_) => break, // Successfully moved
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // File exists, generate new name and retry
                    let stem = Path::new(&basename)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&basename);
                    let ext = Path::new(&basename)
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|e| format!(".{}", e))
                        .unwrap_or_else(String::new);
                    let new_name = format!("{}.{}", stem, counter);
                    trash_file_path = trash_files.join(format!("{}{}", new_name, ext));
                    counter += 1;
                }
                Err(e) => {
                    // Other error (permission denied, cross-filesystem, etc.)
                    return Err(NpioError::from(e));
                }
            }
        }

        // Create trashinfo file
        let trashinfo_name = trash_file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| NpioError::new(IOErrorEnum::Failed, "Invalid filename"))?;
        let trashinfo_path = trash_info.join(format!("{}.trashinfo", trashinfo_name));

        // Format deletion date as ISO 8601
        let deletion_date = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        
        // Percent-encode the path according to FreeDesktop Trash specification
        // The Path field must be URI-encoded (percent-encoded) to handle spaces and special characters
        use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
        
        // Define the set of characters that need encoding for file:// URIs
        // According to RFC 3986, we encode everything except unreserved characters:
        // ALPHA, DIGIT, '-', '.', '_', '~', and '/' (path separator)
        // We preserve '/' as it's the path separator, but encode spaces and special chars
        const PATH_ENCODE_SET: &AsciiSet = &CONTROLS
            .add(b' ')
            .add(b'"')
            .add(b'<')
            .add(b'>')
            .add(b'`')
            .add(b'#')
            .add(b'?')
            .add(b'{')
            .add(b'}')
            .add(b'[')
            .add(b']')
            .add(b'|')
            .add(b'\\')
            .add(b'^')
            .add(b'&')
            .add(b'*')
            .add(b'%');
        
        // Percent-encode the path, preserving forward slashes as path separators
        // We encode each path component separately to preserve the path structure
        // Note: original_path is guaranteed to be absolute at this point
        let path_components: Vec<&str> = if original_path_str.starts_with('/') {
            original_path_str[1..].split('/').collect()
        } else {
            // This should not happen as we ensure absolute path above, but handle gracefully
            original_path_str.split('/').collect()
        };
        
        let encoded_components: Vec<String> = path_components
            .iter()
            .map(|component| utf8_percent_encode(component, PATH_ENCODE_SET).to_string())
            .collect();
        
        // Ensure absolute path format (should always be absolute at this point)
        let encoded_path = format!("/{}", encoded_components.join("/"));
        
        // Create trashinfo content with properly encoded path
        let trashinfo_content = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            encoded_path, deletion_date
        );

        // Write trashinfo file
        fs::write(&trashinfo_path, trashinfo_content).await?;

        Ok(())
    }

    async fn query_filesystem_info(
        &self,
        attributes: &str,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<FileInfo> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let path = self.path.clone();
        let attrs = attributes.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
                .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid path: {}", e)))?;
            
            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
            let ret = unsafe {
                libc::statvfs(c_path.as_ptr(), &mut stat)
            };
            
            if ret != 0 {
                return Err(NpioError::new(
                    IOErrorEnum::Failed,
                    format!("statvfs failed: {}", std::io::Error::last_os_error())
                ));
            }

            let mut info = FileInfo::new();
            
            // Calculate filesystem info
            let block_size = stat.f_frsize as u64;
            let total_blocks = stat.f_blocks as u64;
            let free_blocks = stat.f_bavail as u64; // Available to non-root
            let used_blocks = total_blocks - stat.f_bfree as u64;
            
            let total_size = total_blocks * block_size;
            let free_size = free_blocks * block_size;
            let used_size = used_blocks * block_size;
            
            if attrs.contains("filesystem::size") || attrs.contains("filesystem::*") {
                info.set_attribute("filesystem::size", FileAttributeType::Uint64(total_size));
            }
            
            if attrs.contains("filesystem::free") || attrs.contains("filesystem::*") {
                info.set_attribute("filesystem::free", FileAttributeType::Uint64(free_size));
            }
            
            if attrs.contains("filesystem::used") || attrs.contains("filesystem::*") {
                info.set_attribute("filesystem::used", FileAttributeType::Uint64(used_size));
            }
            
            if attrs.contains("filesystem::readonly") || attrs.contains("filesystem::*") {
                let readonly = (stat.f_flag & libc::ST_RDONLY) != 0;
                info.set_attribute("filesystem::readonly", FileAttributeType::Boolean(readonly));
            }
            
            // Get filesystem type from /proc/mounts or statfs
            // For now, we'll use a simplified approach
            if attrs.contains("filesystem::type") || attrs.contains("filesystem::*") {
                // Try to get filesystem type from /proc/mounts
                let fs_type = get_filesystem_type(&path).unwrap_or_else(|| "unknown".to_string());
                info.set_attribute("filesystem::type", FileAttributeType::String(fs_type));
            }
            
            Ok(info)
        }).await
        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Join error: {}", e)))??;
        
        Ok(result)
    }

    async fn set_attributes_from_info(
        &self,
        info: &FileInfo,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<FileInfo> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let path = self.path.clone();
        let info_clone = info.clone();
        
        tokio::task::spawn_blocking(move || {
            set_attributes_from_info_sync(&path, &info_clone, flags)
        }).await
        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Join error: {}", e)))??;
        
        // Return updated file info
        self.query_info("standard::*,unix::*,time::*", cancellable).await
    }

    async fn set_attribute(
        &self,
        attribute: &str,
        value: &FileAttributeType,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let path = self.path.clone();
        let attr = attribute.to_string();
        let val = value.clone();
        
        tokio::task::spawn_blocking(move || {
            set_attribute_sync(&path, &attr, &val, flags)
        }).await
        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Join error: {}", e)))??;
        
        Ok(())
    }

    async fn set_attribute_string(
        &self,
        attribute: &str,
        value: &str,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        self.set_attribute(attribute, &FileAttributeType::String(value.to_string()), flags, cancellable).await
    }

    async fn set_attribute_byte_string(
        &self,
        attribute: &str,
        value: &str,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        self.set_attribute(attribute, &FileAttributeType::ByteString(value.as_bytes().to_vec()), flags, cancellable).await
    }

    async fn set_attribute_boolean(
        &self,
        attribute: &str,
        value: bool,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        self.set_attribute(attribute, &FileAttributeType::Boolean(value), flags, cancellable).await
    }

    async fn set_attribute_uint32(
        &self,
        attribute: &str,
        value: u32,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        self.set_attribute(attribute, &FileAttributeType::Uint32(value), flags, cancellable).await
    }

    async fn set_attribute_int32(
        &self,
        attribute: &str,
        value: i32,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        self.set_attribute(attribute, &FileAttributeType::Int32(value), flags, cancellable).await
    }

    async fn set_attribute_uint64(
        &self,
        attribute: &str,
        value: u64,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        self.set_attribute(attribute, &FileAttributeType::Uint64(value), flags, cancellable).await
    }

    async fn set_attribute_int64(
        &self,
        attribute: &str,
        value: i64,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        self.set_attribute(attribute, &FileAttributeType::Int64(value), flags, cancellable).await
    }
}

// Helper function to get filesystem type
fn get_filesystem_type(path: &PathBuf) -> Option<String> {
    use std::fs;
    use std::io::{BufRead, BufReader};
    
    // Read /proc/mounts to find the filesystem type
    let mounts_file = fs::File::open("/proc/mounts").ok()?;
    let reader = BufReader::new(mounts_file);
    
    // Try to canonicalize the path to get the mount point
    // If canonicalization fails, use the original path
    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    
    // Find the longest matching mount point (most specific)
    let mut best_match: Option<(usize, String)> = None;
    
    for line in reader.lines() {
        let line = line.ok()?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let mount_point_str = parts[1];
            let mount_point = PathBuf::from(mount_point_str);
            // Check if this path is under this mount point
            if canonical.starts_with(&mount_point) {
                let mount_len = mount_point.components().count();
                // Keep the longest (most specific) match
                let should_update = match &best_match {
                    Some((len, _)) => mount_len > *len,
                    None => true,
                };
                if should_update {
                    best_match = Some((mount_len, parts[2].to_string()));
                }
            }
        }
    }
    
    best_match.map(|(_, fs_type)| fs_type)
}

// Synchronous helper to set attributes from FileInfo
fn set_attributes_from_info_sync(
    path: &PathBuf,
    info: &FileInfo,
    _flags: FileQueryInfoFlags,
) -> NpioResult<()> {
    // Process each attribute
    for (key, value) in info.get_all_attributes() {
        set_attribute_sync(path, key, value, _flags)?;
    }
    
    Ok(())
}

// Synchronous helper to set a single attribute
fn set_attribute_sync(
    path: &PathBuf,
    attribute: &str,
    value: &FileAttributeType,
    _flags: FileQueryInfoFlags,
) -> NpioResult<()> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH, Duration};
    
    match attribute {
        // Unix mode (permissions)
        "unix::mode" => {
            if let FileAttributeType::Uint32(mode) = value {
                let permissions = fs::Permissions::from_mode(*mode);
                fs::set_permissions(path, permissions)
                    .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Failed to set mode: {}", e)))?;
            }
        }
        // Unix UID
        "unix::uid" => {
            if let FileAttributeType::Uint32(uid) = value {
                let metadata = fs::metadata(path)
                    .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Failed to get metadata: {}", e)))?;
                let gid = metadata.gid();
                unsafe {
                    let ret = libc::chown(
                        std::ffi::CString::new(path.as_os_str().as_bytes())
                            .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid path: {}", e)))?
                            .as_ptr(),
                        *uid as libc::uid_t,
                        gid as libc::gid_t,
                    );
                    if ret != 0 {
                        return Err(NpioError::new(
                            IOErrorEnum::Failed,
                            format!("chown failed: {}", std::io::Error::last_os_error())
                        ));
                    }
                }
            }
        }
        // Unix GID
        "unix::gid" => {
            if let FileAttributeType::Uint32(gid) = value {
                let metadata = fs::metadata(path)
                    .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Failed to get metadata: {}", e)))?;
                let uid = metadata.uid();
                unsafe {
                    let ret = libc::chown(
                        std::ffi::CString::new(path.as_os_str().as_bytes())
                            .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid path: {}", e)))?
                            .as_ptr(),
                        uid as libc::uid_t,
                        *gid as libc::gid_t,
                    );
                    if ret != 0 {
                        return Err(NpioError::new(
                            IOErrorEnum::Failed,
                            format!("chown failed: {}", std::io::Error::last_os_error())
                        ));
                    }
                }
            }
        }
        // Modification time
        "time::modified" => {
            if let FileAttributeType::Uint64(timestamp) = value {
                let metadata = fs::metadata(path)
                    .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Failed to get metadata: {}", e)))?;
                let accessed = metadata.accessed()
                    .unwrap_or_else(|_| SystemTime::now());
                
                let modified = UNIX_EPOCH + Duration::from_secs(*timestamp);
                
                // Use utimes to set both access and modification time
                unsafe {
                    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid path: {}", e)))?;
                    
                    let accessed_duration = accessed.duration_since(UNIX_EPOCH)
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid access time: {}", e)))?;
                    let modified_duration = modified.duration_since(UNIX_EPOCH)
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid modified time: {}", e)))?;
                    
                    let times = [
                        libc::timeval {
                            tv_sec: accessed_duration.as_secs() as i64,
                            tv_usec: accessed_duration.subsec_micros() as i64,
                        },
                        libc::timeval {
                            tv_sec: modified_duration.as_secs() as i64,
                            tv_usec: modified_duration.subsec_micros() as i64,
                        },
                    ];
                    
                    let ret = libc::utimes(c_path.as_ptr(), times.as_ptr());
                    if ret != 0 {
                        return Err(NpioError::new(
                            IOErrorEnum::Failed,
                            format!("utimes failed: {}", std::io::Error::last_os_error())
                        ));
                    }
                }
            }
        }
        // Access time
        "time::accessed" => {
            if let FileAttributeType::Uint64(timestamp) = value {
                let metadata = fs::metadata(path)
                    .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Failed to get metadata: {}", e)))?;
                let modified = metadata.modified()
                    .unwrap_or_else(|_| SystemTime::now());
                
                let accessed = UNIX_EPOCH + Duration::from_secs(*timestamp);
                
                unsafe {
                    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid path: {}", e)))?;
                    
                    let accessed_duration = accessed.duration_since(UNIX_EPOCH)
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid access time: {}", e)))?;
                    let modified_duration = modified.duration_since(UNIX_EPOCH)
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid modified time: {}", e)))?;
                    
                    let times = [
                        libc::timeval {
                            tv_sec: accessed_duration.as_secs() as i64,
                            tv_usec: accessed_duration.subsec_micros() as i64,
                        },
                        libc::timeval {
                            tv_sec: modified_duration.as_secs() as i64,
                            tv_usec: modified_duration.subsec_micros() as i64,
                        },
                    ];
                    
                    let ret = libc::utimes(c_path.as_ptr(), times.as_ptr());
                    if ret != 0 {
                        return Err(NpioError::new(
                            IOErrorEnum::Failed,
                            format!("utimes failed: {}", std::io::Error::last_os_error())
                        ));
                    }
                }
            }
        }
        // Display name (rename file)
        "standard::display-name" => {
            if let FileAttributeType::String(name) = value {
                if let Some(parent) = path.parent() {
                    let new_path = parent.join(name);
                    // Check if target already exists and handle appropriately
                    if new_path.exists() && new_path != *path {
                        return Err(NpioError::new(
                            IOErrorEnum::Exists,
                            format!("Target file already exists: {}", new_path.display())
                        ));
                    }
                    fs::rename(path, &new_path)
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Failed to rename: {}", e)))?;
                } else {
                    return Err(NpioError::new(IOErrorEnum::Failed, "File has no parent directory"));
                }
            }
        }
        // Extended attributes (xattr)
        attr if attr.starts_with("xattr::") => {
            // Extract attribute name
            let xattr_name = attr.strip_prefix("xattr::").unwrap_or(attr);
            
            if let FileAttributeType::ByteString(bytes) = value {
                unsafe {
                    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid path: {}", e)))?;
                    let c_name = std::ffi::CString::new(xattr_name)
                        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Invalid xattr name: {}", e)))?;
                    
                    let ret = libc::setxattr(
                        c_path.as_ptr(),
                        c_name.as_ptr(),
                        bytes.as_ptr() as *const libc::c_void,
                        bytes.len(),
                        0, // flags: 0 = create or replace
                    );
                    
                    if ret != 0 {
                        return Err(NpioError::new(
                            IOErrorEnum::Failed,
                            format!("setxattr failed: {}", std::io::Error::last_os_error())
                        ));
                    }
                }
            }
        }
        _ => {
            // Unknown or unsupported attribute
            return Err(NpioError::new(
                IOErrorEnum::NotSupported,
                format!("Attribute '{}' is not supported for setting", attribute)
            ));
        }
    }
    
    Ok(())
}

struct LocalFileEnumerator {
    read_dir: fs::ReadDir,
}

#[async_trait]
impl FileEnumerator for LocalFileEnumerator {
    async fn next_file(
        &mut self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Option<(FileInfo, Box<dyn File>)>> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        
        match self.read_dir.next_entry().await? {
            Some(entry) => {
                let path = entry.path();
                let file = Box::new(LocalFile::new(path.clone()));
                
                // We could optimize this by using entry.metadata() if available without extra syscalls
                // But for consistency let's query the file object (or just basic info here)
                let mut info = FileInfo::new();
                info.set_name(&entry.file_name().to_string_lossy());
                
                // Populate basic type info from entry if possible
                if let Ok(file_type) = entry.file_type().await {
                     let ft = if file_type.is_dir() {
                        FileType::Directory
                    } else if file_type.is_symlink() {
                        FileType::SymbolicLink
                    } else {
                        FileType::Regular
                    };
                    info.set_file_type(ft);
                }

                Ok(Some((info, file)))
            }
            None => Ok(None),
        }
    }

    async fn close(&mut self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        Ok(())
    }
}
