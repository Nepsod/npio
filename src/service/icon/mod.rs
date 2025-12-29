//! XDG Icon Theme System
//!
//! This module provides a complete implementation of the XDG Icon Theme Specification,
//! including theme parsing, icon lookup with inheritance, caching, and loading.

mod cache;
mod error;
mod loader;
mod lookup;
mod theme;

pub use cache::CachedIcon;
pub use error::IconError;
pub use loader::IconLoader;
pub use lookup::IconLookup;
pub use theme::{DirectoryType, IconContext, IconDirectory, IconTheme};

use crate::file::File;
use crate::service::filesystem::icon::{IconProvider, MimeIconProvider};

/// Icon registry - main public API for the icon system.
pub struct IconRegistry {
    /// Current theme name.
    theme: String,
    /// Icon lookup system.
    lookup: IconLookup,
    /// Icon cache.
    cache: cache::IconCache,
    /// Icon loader.
    loader: IconLoader,
    /// MIME icon provider for mapping files to icon names.
    mime_provider: MimeIconProvider,
}

impl IconRegistry {
    /// Create a new icon registry with the default theme "Sweet-Purple".
    pub fn new() -> Result<Self, IconError> {
        Self::with_theme(None)
    }

    /// Create a new icon registry with a specific theme.
    pub fn with_theme(theme: Option<String>) -> Result<Self, IconError> {
        let theme_name = theme.unwrap_or_else(|| "Sweet-Purple".to_string());

        // Try to load the theme to verify it exists
        let lookup = IconLookup::new();
        lookup.load_theme(&theme_name)?;

        Ok(Self {
            theme: theme_name,
            lookup,
            cache: cache::IconCache::new(),
            loader: IconLoader::new(),
            mime_provider: MimeIconProvider::new(),
        })
    }

    /// Set the current theme.
    pub fn set_theme(&mut self, theme: String) -> Result<(), IconError> {
        // Verify theme exists
        self.lookup.load_theme(&theme)?;
        self.theme = theme;
        // Clear cache when theme changes
        self.cache.clear();
        Ok(())
    }

    /// Clear the icon cache.
    ///
    /// Useful when the icon lookup algorithm changes or to force reload of icons.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get the current theme name.
    pub fn theme(&self) -> &str {
        &self.theme
    }

    /// Get an icon by name and size.
    pub fn get_icon(&self, icon_name: &str, size: u32) -> Option<CachedIcon> {
        // Check cache first
        if let Some(cached) = self.cache.get(icon_name, size) {
            return Some(cached);
        }

        // Determine context based on icon name patterns
        let context = self.guess_context(icon_name);

        // Lookup icon path
        let icon_path = self
            .lookup
            .lookup_icon(icon_name, size, context, &self.theme)?;

        // Load icon (blocking call since this is in a sync context)
        // Use smol::block_on which works even within tokio runtime
        let cached_icon = smol::block_on(self.loader.load_icon(&icon_path)).ok()?;

        // Cache it
        self.cache
            .put(icon_name.to_string(), size, cached_icon.clone());

        Some(cached_icon)
    }

    /// Get an icon for a file with fallback chain.
    pub async fn get_file_icon(&self, file: &dyn File, size: u32) -> Option<CachedIcon> {
        // Get icon candidates from MIME provider
        let icon_data = self.mime_provider.get_icon(file).await?;
        
        // Get file info for logging
        let file_info = file.query_info("standard::name,standard::content-type", None).await.ok();
        let basename = file.basename();
        let file_name = file_info.as_ref()
            .and_then(|info| info.get_name())
            .unwrap_or(basename.as_str());
        let mime_type = file_info.as_ref()
            .and_then(|info| info.get_content_type());
        
        log::debug!(
            "IconRegistry: Looking up icons {:?} for file '{}' (MIME: {:?})",
            icon_data.names,
            file_name,
            mime_type
        );

        // Try specific icons in order
        for name in &icon_data.names {
            if let Some(icon) = self.get_icon(name, size) {
                return Some(icon);
            }
        }

        // Try generic category for each candidate
        for name in &icon_data.names {
            let generic_name = self.get_generic_icon_name(name);
            if let Some(icon) = self.get_icon(&generic_name, size) {
                log::debug!(
                    "IconRegistry: Found generic icon '{}' from '{}'",
                    generic_name,
                    name
                );
                return Some(icon);
            }
        }

        // Additional fallbacks for specific cases
        // For media-floppy, try drive-harddisk as fallback
        if icon_data.names.iter().any(|n| n == "media-floppy") {
            if let Some(icon) = self.get_icon("drive-harddisk", size) {
                log::debug!("IconRegistry: Using drive-harddisk as fallback for media-floppy");
                return Some(icon);
            }
        }

        // For application-toml, try text-x-generic as fallback (TOML is text-like)
        if icon_data.names.iter().any(|n| n == "application-toml") {
            if let Some(icon) = self.get_icon("text-x-generic", size) {
                log::debug!("IconRegistry: Using text-x-generic as fallback for application-toml");
                return Some(icon);
            }
        }

        // Final fallback: text-x-generic or unknown
        self.get_icon("text-x-generic", size)
            .or_else(|| self.get_icon("unknown", size))
    }

    /// Get generic icon name from specific icon name.
    ///
    /// Maps specific icons to their generic category:
    /// - text-x-toml -> text-x-generic
    /// - application-pdf -> application-x-generic
    /// - image-png -> image-x-generic
    /// - media-floppy -> drive-removable-media (fallback for floppy)
    fn get_generic_icon_name(&self, icon_name: &str) -> String {
        if icon_name.starts_with("text-") && icon_name != "text-x-generic" {
            "text-x-generic".to_string()
        } else if icon_name.starts_with("application-") && !icon_name.ends_with("-generic") {
            // Try application-x-generic first, fallback to application-generic
            if icon_name.contains("-x-") {
                "application-x-generic".to_string()
            } else {
                "application-x-generic".to_string()
            }
        } else if icon_name.starts_with("image-") && icon_name != "image-x-generic" {
            "image-x-generic".to_string()
        } else if icon_name.starts_with("video-") && icon_name != "video-x-generic" {
            "video-x-generic".to_string()
        } else if icon_name.starts_with("audio-") && icon_name != "audio-x-generic" {
            "audio-x-generic".to_string()
        } else if icon_name == "media-floppy" {
            // Fallback for floppy disk: try drive-removable-media, then drive-harddisk
            "drive-removable-media".to_string()
        } else if icon_name.starts_with("media-") {
            // For other media icons, try drive-harddisk as fallback
            "drive-harddisk".to_string()
        } else {
            // Already generic or unknown, return as-is
            icon_name.to_string()
        }
    }

    /// Guess icon context from icon name.
    fn guess_context(&self, icon_name: &str) -> IconContext {
        if icon_name.starts_with("folder") || icon_name.contains("directory") {
            IconContext::Places
        } else if icon_name.starts_with("media-") || icon_name.starts_with("drive-") {
            // Media and drive icons are in Devices context
            IconContext::Devices
        } else if icon_name.starts_with("document-") || icon_name.starts_with("edit-") 
            || icon_name.starts_with("view-") || icon_name.starts_with("go-")
            || icon_name.starts_with("insert-") || icon_name.starts_with("format-")
            || icon_name.starts_with("tools-") || icon_name.starts_with("help-")
            || icon_name.contains("-save") || icon_name.contains("-open")
            || icon_name.contains("-new") || icon_name.contains("-print")
            || icon_name.contains("-undo") || icon_name.contains("-redo")
            || icon_name.contains("-cut") || icon_name.contains("-copy")
            || icon_name.contains("-paste") || icon_name.contains("-delete") {
            // Document/editing actions are in Actions context
            IconContext::Actions
        } else if icon_name.starts_with("text-") {
            IconContext::Mimetypes
        } else if icon_name.starts_with("image-") {
            IconContext::Mimetypes
        } else if icon_name.starts_with("application-") {
            IconContext::Mimetypes
        } else if icon_name.starts_with("video-") {
            IconContext::Mimetypes
        } else if icon_name.starts_with("audio-") {
            IconContext::Mimetypes
        } else {
            IconContext::Unknown
        }
    }
}

impl Default for IconRegistry {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback to hicolor if Sweet-Purple not found
            Self::with_theme(Some("hicolor".to_string())).unwrap_or_else(|_| {
                // Last resort: create with Adwaita
                Self::with_theme(Some("Adwaita".to_string()))
                    .expect("Failed to initialize icon registry")
            })
        })
    }
}
