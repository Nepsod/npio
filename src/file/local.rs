use std::path::PathBuf;
use async_trait::async_trait;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::cancellable::Cancellable;
use crate::error::{NpioError, NpioResult, IOErrorEnum};
use crate::file::File;
use crate::file_enumerator::FileEnumerator;
use crate::file_info::{FileInfo, FileType};
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

        // Get the original path as URI-encoded string
        let original_path = self.path.canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        let original_path_str = original_path.to_string_lossy().to_string();
        
        // Get the basename for the trash file
        let basename = self.basename();
        
        // Handle name conflicts by appending numbers
        let mut trash_file_path = trash_files.join(&basename);
        let mut counter = 1;
        while trash_file_path.exists() {
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

        // Move the file to trash
        tokio::fs::rename(&self.path, &trash_file_path).await?;

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
        let is_absolute = original_path_str.starts_with('/');
        let path_components: Vec<&str> = if is_absolute {
            original_path_str[1..].split('/').collect()
        } else {
            original_path_str.split('/').collect()
        };
        
        let encoded_components: Vec<String> = path_components
            .iter()
            .map(|component| utf8_percent_encode(component, PATH_ENCODE_SET).to_string())
            .collect();
        
        let encoded_path = if is_absolute {
            format!("/{}", encoded_components.join("/"))
        } else {
            encoded_components.join("/")
        };
        
        // Create trashinfo content with properly encoded path
        let trashinfo_content = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            encoded_path, deletion_date
        );

        // Write trashinfo file
        fs::write(&trashinfo_path, trashinfo_content).await?;

        Ok(())
    }
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
