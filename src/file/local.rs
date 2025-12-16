use std::path::PathBuf;
use async_trait::async_trait;
use tokio::fs::{self, OpenOptions};

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
        _flags: u32,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        // Check if destination is also local
        // This is a bit hacky, ideally we'd have a way to check backend type
        if destination.uri().starts_with("file://") {
             // Extract path from URI (simplified)
             let dest_path_str = destination.uri().trim_start_matches("file://").to_string();
             fs::rename(&self.path, dest_path_str).await?;
             Ok(())
        } else {
             // Fallback to copy and delete (generic implementation)
             // For now, return NotSupported
             Err(NpioError::new(IOErrorEnum::NotSupported, "Cross-backend move not yet supported"))
        }
    }
    
    async fn copy(
        &self,
        destination: &dyn File,
        _flags: u32,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }
         if destination.uri().starts_with("file://") {
             let dest_path_str = destination.uri().trim_start_matches("file://").to_string();
             fs::copy(&self.path, dest_path_str).await?;
             Ok(())
        } else {
             Err(NpioError::new(IOErrorEnum::NotSupported, "Cross-backend copy not yet supported"))
        }
    }

    async fn exists(&self, cancellable: Option<&Cancellable>) -> NpioResult<bool> {
        if let Some(c) = cancellable {
            c.check()?;
        }
        Ok(self.path.exists())
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
