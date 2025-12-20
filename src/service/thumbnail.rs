// Thumbnail service implementation
// Handles thumbnail generation, caching, and retrieval per freedesktop.org spec

use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::fs;
use tokio::sync::broadcast;
use crate::error::{NpioError, NpioResult, IOErrorEnum};
use crate::file::File;
use crate::backend::thumbnail::{ThumbnailBackend, ThumbnailSize};
use crate::cancellable::Cancellable;
use crate::metadata::MimeResolver;

/// Event emitted when thumbnail operations complete
#[derive(Debug, Clone)]
pub enum ThumbnailEvent {
    /// Thumbnail was successfully generated
    ThumbnailReady {
        uri: String,
        size: ThumbnailSize,
        path: PathBuf,
    },
    /// Thumbnail generation failed
    ThumbnailFailed {
        uri: String,
        size: ThumbnailSize,
        error_kind: IOErrorEnum,
        error_message: String,
    },
}

/// Decoded RGBA image data for a thumbnail
#[derive(Debug, Clone)]
pub struct ThumbnailImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA, width * height * 4 bytes
}

/// Cache for decoded thumbnail images
pub struct ThumbnailImageCache {
    cache: Arc<RwLock<HashMap<String, ThumbnailImage>>>,
}

impl ThumbnailImageCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a cached image by key (format: "{uri}:{size:?}")
    pub fn get_image(&self, key: &str) -> Option<ThumbnailImage> {
        match self.cache.read() {
            Ok(cache) => cache.get(key).cloned(),
            Err(e) => {
                eprintln!("Failed to acquire read lock on image cache: {}", e);
                // Try to recover from poisoned lock
                let cache = e.into_inner();
                cache.get(key).cloned()
            }
        }
    }

    /// Store an image in the cache
    pub fn store_image(&self, key: String, image: ThumbnailImage) {
        match self.cache.write() {
            Ok(mut cache) => {
                cache.insert(key, image);
            }
            Err(e) => {
                eprintln!("Failed to acquire write lock on image cache: {}", e);
                // Try to recover from poisoned lock
                let mut cache = e.into_inner();
                cache.insert(key, image);
            }
        }
    }

    /// Load and decode a PNG thumbnail file to RGBA
    pub async fn load_image(&self, thumbnail_path: &PathBuf) -> NpioResult<ThumbnailImage> {
        // Read file in blocking task
        let path = thumbnail_path.clone();
        let image_data = tokio::task::spawn_blocking(move || {
            image::open(&path)
        })
        .await
        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Join error: {}", e)))?
        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Failed to open image: {}", e)))?;

        // Convert to RGBA
        let rgba = image_data.to_rgba8();
        let (width, height) = rgba.dimensions();
        let data = rgba.into_raw();

        Ok(ThumbnailImage {
            width,
            height,
            data,
        })
    }

    /// Clear the cache
    pub fn clear(&self) {
        match self.cache.write() {
            Ok(mut cache) => {
                cache.clear();
            }
            Err(e) => {
                eprintln!("Failed to acquire write lock on image cache: {}", e);
                // Try to recover from poisoned lock
                let mut cache = e.into_inner();
                cache.clear();
            }
        }
    }
}

impl Default for ThumbnailImageCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Thumbnail service for managing file thumbnails
pub struct ThumbnailService {
    #[allow(dead_code)]
    backend: ThumbnailBackend,
    event_sender: broadcast::Sender<ThumbnailEvent>,
    image_cache: Arc<ThumbnailImageCache>,
}

impl ThumbnailService {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self {
            backend: ThumbnailBackend::new(),
            event_sender: sender,
            image_cache: Arc::new(ThumbnailImageCache::new()),
        }
    }

    /// Subscribe to thumbnail events
    pub fn subscribe(&self) -> broadcast::Receiver<ThumbnailEvent> {
        self.event_sender.subscribe()
    }

    /// Check if a file type is supported for thumbnail generation
    pub async fn is_supported(
        &self,
        file: &dyn File,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<bool> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let uri = file.uri();
        let file_path_str = uri.trim_start_matches("file://");
        let file_path = std::path::Path::new(file_path_str);
        
        // Get MIME type
        let mime_type = MimeResolver::guess_mime_type(file_path);
        
        // Check against supported MIME types
        // thumbnailify supports: images (jpeg, png, gif, webp, bmp, tiff), PDF, and some video formats
        let supported = match mime_type.as_str() {
            // Image formats
            "image/jpeg" | "image/jpg" => true,
            "image/png" => true,
            "image/gif" => true,
            "image/webp" => true,
            "image/bmp" => true,
            "image/tiff" | "image/tif" => true,
            // PDF
            "application/pdf" => true,
            // Video formats (if thumbnailify supports them)
            mime if mime.starts_with("video/") => {
                // Check if it's a common video format
                matches!(mime, "video/mp4" | "video/avi" | "video/quicktime" | "video/x-msvideo" | "video/webm")
            }
            _ => false,
        };

        Ok(supported)
    }

    /// Get decoded RGBA image for a thumbnail
    pub async fn get_thumbnail_image(
        &self,
        file: &dyn File,
        size: ThumbnailSize,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<ThumbnailImage> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let uri = file.uri();
        let cache_key = format!("{}:{:?}", uri, size);

        // Check cache first
        if let Some(image) = self.image_cache.get_image(&cache_key) {
            return Ok(image);
        }

        // Get thumbnail path
        let thumbnail_path = match self.get_thumbnail_path(file, size, cancellable).await? {
            Some(path) => path,
            None => {
                // Generate thumbnail if it doesn't exist
                self.generate_thumbnail(file, size, cancellable).await?;
                ThumbnailBackend::get_thumbnail_path(&uri, size)?
            }
        };

        // Load and decode image
        let image = self.image_cache.load_image(&thumbnail_path).await?;
        
        // Store in cache
        self.image_cache.store_image(cache_key, image.clone());

        Ok(image)
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
        let file_path_str = uri.trim_start_matches("file://");
        let file_path = PathBuf::from(file_path_str);
        
        if !file_path.exists() {
            return Err(NpioError::new(IOErrorEnum::NotFound, "File not found"));
        }

        let thumbnail_path = ThumbnailBackend::get_thumbnail_path(&uri, size)?;
        
        // Ensure cache directory exists
        if let Some(parent) = thumbnail_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                NpioError::new(IOErrorEnum::Failed, format!("Failed to create cache dir: {}", e))
            })?;
        }

        // Map size to thumbnailify size
        let target_size = match size {
            ThumbnailSize::Normal => thumbnailify::ThumbnailSize::Normal,
            ThumbnailSize::Large => thumbnailify::ThumbnailSize::Large,
            ThumbnailSize::XLarge => thumbnailify::ThumbnailSize::XLarge,
            ThumbnailSize::XXLarge => thumbnailify::ThumbnailSize::XXLarge,
        };

        // Run generation in blocking task
        let generated_path = tokio::task::spawn_blocking(move || {
            thumbnailify::generate_thumbnail(&file_path, target_size)
        })
        .await
        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Join error: {}", e)))?
        .map_err(|e| NpioError::new(IOErrorEnum::Failed, format!("Thumbnail generation failed: {:?}", e)))?;

        // Move generated thumbnail to correct location (MD5 name)
        // thumbnailify generates with its own naming, we need to rename it to match standard
        fs::rename(&generated_path, &thumbnail_path).await.map_err(|e| {
            NpioError::new(IOErrorEnum::Failed, format!("Failed to move thumbnail to cache: {}", e))
        })?;

        // Load and cache the decoded image
        let cache_key = format!("{}:{:?}", uri, size);
        if let Ok(image) = self.image_cache.load_image(&thumbnail_path).await {
            self.image_cache.store_image(cache_key, image);
        }

        // Emit ThumbnailReady event
        let _ = self.event_sender.send(ThumbnailEvent::ThumbnailReady {
            uri: uri.clone(),
            size,
            path: thumbnail_path.clone(),
        });

        Ok(thumbnail_path)
    }

    /// Gets or generates a thumbnail for a file
    pub async fn get_or_generate_thumbnail(
        &self,
        file: &dyn File,
        size: ThumbnailSize,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<PathBuf> {
        let uri = file.uri();
        
        // First check if valid thumbnail exists
        if let Some(path) = self.get_thumbnail_path(file, size, cancellable).await? {
            return Ok(path);
        }

        // Generate new thumbnail (will emit event and populate cache)
        match self.generate_thumbnail(file, size, cancellable).await {
            Ok(path) => Ok(path),
            Err(e) => {
                // Emit ThumbnailFailed event
                let error_kind = *e.kind();
                let error_message = e.to_string();
                let _ = self.event_sender.send(ThumbnailEvent::ThumbnailFailed {
                    uri: uri.clone(),
                    size,
                    error_kind,
                    error_message,
                });
                Err(e)
            }
        }
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

