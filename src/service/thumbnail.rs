// Thumbnail service implementation
// Handles thumbnail generation, caching, and retrieval per freedesktop.org spec

use std::path::PathBuf;
use tokio::fs;
use crate::error::{NpioError, NpioResult, IOErrorEnum};
use crate::file::File;
use crate::backend::thumbnail::{ThumbnailBackend, ThumbnailSize};
use crate::cancellable::Cancellable;

/// Thumbnail service for managing file thumbnails
pub struct ThumbnailService {
    #[allow(dead_code)]
    backend: ThumbnailBackend,
}

impl ThumbnailService {
    pub fn new() -> Self {
        Self {
            backend: ThumbnailBackend::new(),
        }
    }

    /// Gets the thumbnail path for a file, if it exists and is valid
    pub async fn get_thumbnail_path(
        &self,
        file: &dyn File,
        size: ThumbnailSize,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Option<PathBuf>> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let uri = file.uri();
        
        // Get file modification time
        let file_info = file.query_info("time::modified", cancellable).await?;
        let file_mtime = file_info
            .get_attribute("time::modified")
            .and_then(|attr| {
                if let crate::file_info::FileAttributeType::Uint64(t) = attr {
                    Some(*t)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        // Check if valid thumbnail exists
        let is_valid = ThumbnailBackend::has_valid_thumbnail(&uri, size, file_mtime).await?;
        
        if is_valid {
            Ok(Some(ThumbnailBackend::get_thumbnail_path(&uri, size)?))
        } else {
            Ok(None)
        }
    }

    /// Generates a thumbnail for a file
    /// This is a placeholder - full implementation would invoke thumbnailers
    pub async fn generate_thumbnail(
        &self,
        file: &dyn File,
        size: ThumbnailSize,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<PathBuf> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let uri = file.uri();
        let thumbnail_path = ThumbnailBackend::get_thumbnail_path(&uri, size)?;
        
        // Create cache directory if it doesn't exist
        if let Some(parent) = thumbnail_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // TODO: Actually generate thumbnail using available thumbnailers
        // For now, this is a placeholder that returns the path
        // Full implementation would:
        // 1. Check for .thumbnailer files in ~/.local/share/thumbnailers/
        // 2. Find appropriate thumbnailer for file MIME type
        // 3. Invoke thumbnailer to generate thumbnail
        // 4. Save thumbnail to cache directory
        
        Err(NpioError::new(
            IOErrorEnum::NotSupported,
            "Thumbnail generation not yet fully implemented. Requires thumbnailer integration.",
        ))
    }

    /// Gets or generates a thumbnail for a file
    pub async fn get_or_generate_thumbnail(
        &self,
        file: &dyn File,
        size: ThumbnailSize,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<PathBuf> {
        // First check if valid thumbnail exists
        if let Some(path) = self.get_thumbnail_path(file, size, cancellable).await? {
            return Ok(path);
        }

        // Generate new thumbnail
        self.generate_thumbnail(file, size, cancellable).await
    }

    /// Deletes a thumbnail from cache
    pub async fn delete_thumbnail(
        &self,
        file: &dyn File,
        size: ThumbnailSize,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let uri = file.uri();
        let thumbnail_path = ThumbnailBackend::get_thumbnail_path(&uri, size)?;
        
        if thumbnail_path.exists() {
            fs::remove_file(&thumbnail_path).await?;
        }

        Ok(())
    }

    /// Cleans up old/invalid thumbnails
    pub async fn cleanup_thumbnails(
        &self,
        size: ThumbnailSize,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<usize> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let cache_dir = ThumbnailBackend::get_cache_dir(size)?;
        
        if !cache_dir.exists() {
            return Ok(0);
        }

        let mut deleted_count = 0;
        let mut entries = fs::read_dir(&cache_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if let Some(c) = cancellable {
                c.check()?;
            }

            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("png") {
                // Check if thumbnail is old (older than 7 days)
                let metadata = fs::metadata(&path).await?;
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = modified.elapsed() {
                        if age.as_secs() > 7 * 24 * 60 * 60 {
                            // Older than 7 days, delete it
                            fs::remove_file(&path).await.ok();
                            deleted_count += 1;
                        }
                    }
                }
            }
        }

        Ok(deleted_count)
    }
}

impl Default for ThumbnailService {
    fn default() -> Self {
        Self::new()
    }
}

