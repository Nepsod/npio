//! Icon cache for in-memory storage of loaded icons.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Cached icon data.
#[derive(Clone, Debug)]
pub enum CachedIcon {
    /// PNG/XPM image icon (stored as raw RGBA bytes, width, height).
    Image {
        /// Raw RGBA pixel data.
        data: Arc<Vec<u8>>,
        /// Image width.
        width: u32,
        /// Image height.
        height: u32,
    },
    /// SVG icon (stored as SVG source string).
    Svg(Arc<String>),
    /// Icon file path (for lazy loading).
    Path(PathBuf),
}

/// In-memory icon cache.
pub struct IconCache {
    /// Cache mapping (icon_name, size) -> CachedIcon.
    cache: Arc<Mutex<HashMap<(String, u32), CachedIcon>>>,
}

impl IconCache {
    /// Create a new icon cache.
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get a cached icon.
    pub fn get(&self, icon_name: &str, size: u32) -> Option<CachedIcon> {
        let cache = self.cache.lock().unwrap();
        cache.get(&(icon_name.to_string(), size)).cloned()
    }

    /// Store an icon in the cache.
    pub fn put(&self, icon_name: String, size: u32, icon: CachedIcon) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert((icon_name, size), icon);
    }

    /// Get cache size (number of cached icons).
    pub fn len(&self) -> usize {
        let cache = self.cache.lock().unwrap();
        cache.len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        let cache = self.cache.lock().unwrap();
        cache.is_empty()
    }

    /// Check if an icon is cached.
    pub fn contains(&self, icon_name: &str, size: u32) -> bool {
        let cache = self.cache.lock().unwrap();
        cache.contains_key(&(icon_name.to_string(), size))
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }
}

impl Default for IconCache {
    fn default() -> Self {
        Self::new()
    }
}
