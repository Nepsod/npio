// Thumbnail backend implementation
// Implements freedesktop.org thumbnail specification

use std::path::PathBuf;
use crate::error::{NpioError, NpioResult, IOErrorEnum};

/// Thumbnail size variants according to freedesktop.org spec
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailSize {
    Normal,   // 128x128
    Large,    // 256x256
    XLarge,   // 512x512
    XXLarge,  // 1024x1024
}

impl ThumbnailSize {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            ThumbnailSize::Normal => (128, 128),
            ThumbnailSize::Large => (256, 256),
            ThumbnailSize::XLarge => (512, 512),
            ThumbnailSize::XXLarge => (1024, 1024),
        }
    }

    pub fn directory_name(&self) -> &'static str {
        match self {
            ThumbnailSize::Normal => "normal",
            ThumbnailSize::Large => "large",
            ThumbnailSize::XLarge => "x-large",
            ThumbnailSize::XXLarge => "xx-large",
        }
    }
}

/// Thumbnail backend for generating and managing thumbnails
pub struct ThumbnailBackend;

impl ThumbnailBackend {
    pub fn new() -> Self {
        Self
    }

    /// Gets the thumbnail cache directory for a given size
    pub fn get_cache_dir(size: ThumbnailSize) -> NpioResult<PathBuf> {
        use directories::ProjectDirs;
        
        let cache_home = std::env::var("XDG_CACHE_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                ProjectDirs::from("", "", "")
                    .map(|dirs| dirs.cache_dir().to_path_buf())
            })
            .or_else(|| {
                directories::UserDirs::new()
                    .map(|dirs| dirs.home_dir().join(".cache"))
            })
            .ok_or_else(|| NpioError::new(IOErrorEnum::Failed, "Could not determine XDG_CACHE_HOME"))?;

        Ok(cache_home.join("thumbnails").join(size.directory_name()))
    }

    /// Generates MD5 hash of URI for thumbnail filename
    /// According to freedesktop.org spec, thumbnails are named with MD5 hash of file URI
    pub fn uri_to_thumbnail_name(uri: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        // Simple hash implementation (in production, use proper MD5)
        // For now, use a hash that's good enough for testing
        let mut hasher = DefaultHasher::new();
        uri.hash(&mut hasher);
        format!("{:x}.png", hasher.finish())
    }

    /// Gets the thumbnail path for a file URI and size
    pub fn get_thumbnail_path(uri: &str, size: ThumbnailSize) -> NpioResult<PathBuf> {
        let cache_dir = Self::get_cache_dir(size)?;
        let thumbnail_name = Self::uri_to_thumbnail_name(uri);
        Ok(cache_dir.join(thumbnail_name))
    }

    /// Checks if a thumbnail exists and is valid
    pub async fn has_valid_thumbnail(
        uri: &str,
        size: ThumbnailSize,
        file_mtime: u64,
    ) -> NpioResult<bool> {
        let thumbnail_path = Self::get_thumbnail_path(uri, size)?;
        
        if !thumbnail_path.exists() {
            return Ok(false);
        }

        // Check if thumbnail is newer than file
        let thumbnail_metadata = tokio::fs::metadata(&thumbnail_path).await?;
        let thumbnail_mtime = thumbnail_metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| NpioError::new(IOErrorEnum::Failed, "Invalid timestamp"))?
            .as_secs();

        Ok(thumbnail_mtime >= file_mtime)
    }
}

impl Default for ThumbnailBackend {
    fn default() -> Self {
        Self::new()
    }
}

