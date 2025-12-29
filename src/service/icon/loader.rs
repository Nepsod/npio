//! Icon loader for PNG, SVG, and XPM files.

use std::path::Path;
use std::sync::Arc;

use crate::service::filesystem::io_uring;
use crate::service::icon::cache::CachedIcon;
use crate::service::icon::error::IconError;

/// Icon loader.
pub struct IconLoader;

impl IconLoader {
    /// Create a new icon loader.
    pub fn new() -> Self {
        Self
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, IconError> {
        io_uring::read(path)
            .await
            .map_err(|e| IconError::InvalidFormat(format!("Failed to load bytes: {}", e)))
    }

    async fn read_string(&self, path: &Path) -> Result<String, IconError> {
        io_uring::read_to_string(path)
            .await
            .map_err(|e| IconError::InvalidFormat(format!("Failed to load SVG: {}", e)))
    }

    /// Load an icon from a file path.
    pub async fn load_icon(&self, path: &Path) -> Result<CachedIcon, IconError> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "svg" => self.load_svg(path).await,
            "png" => self.load_png(path).await,
            "xpm" => self.load_xpm(path).await,
            _ => Err(IconError::InvalidFormat(format!(
                "Unsupported icon format: {}",
                extension
            ))),
        }
    }

    /// Load an SVG icon.
    async fn load_svg(&self, path: &Path) -> Result<CachedIcon, IconError> {
        let svg_content = self.read_string(path).await?;
        Ok(CachedIcon::Svg(Arc::new(svg_content)))
    }

    /// Load a PNG icon.
    async fn load_png(&self, path: &Path) -> Result<CachedIcon, IconError> {
        let bytes = self.read_bytes(path).await?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| IconError::InvalidFormat(format!("Failed to load PNG: {}", e)))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let data = rgba.into_raw();

        Ok(CachedIcon::Image {
            data: Arc::new(data),
            width,
            height,
        })
    }

    /// Load an XPM icon (convert to PNG-like format).
    async fn load_xpm(&self, path: &Path) -> Result<CachedIcon, IconError> {
        // XPM is a text-based format, but for simplicity, we'll try to load it as image
        // The image crate should handle XPM
        let bytes = self.read_bytes(path).await?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| IconError::InvalidFormat(format!("Failed to load XPM: {}", e)))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let data = rgba.into_raw();

        Ok(CachedIcon::Image {
            data: Arc::new(data),
            width,
            height,
        })
    }
}

impl Default for IconLoader {
    fn default() -> Self {
        Self::new()
    }
}
