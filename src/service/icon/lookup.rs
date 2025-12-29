//! Icon lookup system with search paths and inheritance.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::service::icon::error::IconError;
use crate::service::icon::theme::{DirectoryType, IconContext, IconDirectory, IconTheme};

/// Icon lookup system.
pub struct IconLookup {
    /// Cache of loaded themes.
    theme_cache: Arc<Mutex<HashMap<String, IconTheme>>>,
    /// Search paths for icon themes.
    search_paths: Vec<PathBuf>,
}

impl IconLookup {
    /// Create a new icon lookup system.
    pub fn new() -> Self {
        let mut search_paths = Vec::new();

        // User-specific paths
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            search_paths.push(home.join(".icons"));
            search_paths.push(home.join(".local/share/icons"));
        }

        // System-wide paths
        search_paths.push(PathBuf::from("/usr/share/icons"));
        search_paths.push(PathBuf::from("/usr/share/pixmaps"));

        Self {
            theme_cache: Arc::new(Mutex::new(HashMap::new())),
            search_paths,
        }
    }

    /// Load a theme (with caching).
    pub fn load_theme(&self, theme_name: &str) -> Result<IconTheme, IconError> {
        let mut cache = self.theme_cache.lock().unwrap();

        if let Some(theme) = cache.get(theme_name) {
            return Ok(theme.clone());
        }

        // Try to find theme in search paths
        for search_path in &self.search_paths {
            let theme_path = search_path.join(theme_name);
            if theme_path.exists() && theme_path.is_dir() {
                match IconTheme::load(theme_name, theme_path) {
                    Ok(theme) => {
                        let theme_clone = theme.clone();
                        cache.insert(theme_name.to_string(), theme);
                        return Ok(theme_clone);
                    },
                    Err(_) => continue,
                }
            }
        }

        Err(IconError::ThemeNotFound(theme_name.to_string()))
    }

    /// Lookup an icon in a theme and its inherited themes.
    pub fn lookup_icon(
        &self,
        icon_name: &str,
        size: u32,
        context: IconContext,
        theme_name: &str,
    ) -> Option<PathBuf> {

        // Try current theme
        if let Ok(theme) = self.load_theme(theme_name) {
            if let Some(path) = self.lookup_in_theme(&theme, icon_name, size, context) {
                log::debug!("IconLookup: Found icon '{}' at {:?}", icon_name, path);
                return Some(path);
            }

            // Try inherited themes
            for inherited in &theme.inherits {
                log::debug!("IconLookup: Trying inherited theme '{}'", inherited);
                if let Ok(inherited_theme) = self.load_theme(inherited) {
                    if let Some(path) =
                        self.lookup_in_theme(&inherited_theme, icon_name, size, context)
                    {
                        log::debug!(
                            "IconLookup: Found icon '{}' in inherited theme '{}' at {:?}",
                            icon_name,
                            inherited,
                            path
                        );
                        return Some(path);
                    }
                }
            }
        }

        // Fallback to hicolor
        if theme_name != "hicolor" {
            log::debug!("IconLookup: Falling back to hicolor theme");
            if let Some(path) = self.lookup_icon(icon_name, size, context, "hicolor") {
                return Some(path);
            }
        }

        log::debug!("IconLookup: Icon '{}' not found in any theme", icon_name);
        None
    }

    /// Lookup icon in a specific theme.
    fn lookup_in_theme(
        &self,
        theme: &IconTheme,
        icon_name: &str,
        size: u32,
        context: IconContext,
    ) -> Option<PathBuf> {
        // Find best matching directory
        let best_dir = match self.find_best_directory(&theme.directories, size, context) {
            Some(dir) => dir,
            None => {
                log::debug!(
                    "IconLookup: No matching directory found for size {} and context {:?}",
                    size,
                    context
                );
                return None;
            },
        };

        let dir_path = theme.directory_path(&best_dir.name);

        // Try different file extensions in order of preference
        let extensions = ["svg", "png", "xpm"];
        for ext in &extensions {
            let icon_path = dir_path.join(format!("{}.{}", icon_name, ext));
            if icon_path.exists() {
                log::debug!("IconLookup: Found icon file at {:?}", icon_path);
                return Some(icon_path);
            }
        }

        // If not found in best directory, try all directories (some themes organize differently)
        log::debug!(
            "IconLookup: Icon '{}' not found in directory '{}', trying all directories",
            icon_name,
            best_dir.name
        );
        for dir in &theme.directories {
            if dir.name == best_dir.name {
                continue; // Already tried this one
            }
            let dir_path = theme.directory_path(&dir.name);
            for ext in &extensions {
                let icon_path = dir_path.join(format!("{}.{}", icon_name, ext));
                if icon_path.exists() {
                    log::debug!(
                        "IconLookup: Found icon file at {:?} in directory '{}'",
                        icon_path,
                        dir.name
                    );
                    return Some(icon_path);
                }
            }
        }

        log::debug!(
            "IconLookup: Icon '{}' not found in theme '{}'",
            icon_name,
            theme.name
        );
        None
    }

    /// Find the best matching directory for a given size and context.
    fn find_best_directory<'a>(
        &self,
        directories: &'a [IconDirectory],
        size: u32,
        context: IconContext,
    ) -> Option<&'a IconDirectory> {
        // First try: exact context match
        let mut candidates: Vec<&IconDirectory> = directories
            .iter()
            .filter(|d| d.context == context)
            .collect();

        // Second try: Unknown context (many themes use this)
        if candidates.is_empty() {
            candidates = directories
                .iter()
                .filter(|d| d.context == IconContext::Unknown)
                .collect();
        }

        // Third try: Try Actions context if we're looking for action-like icons
        if candidates.is_empty() && context == IconContext::Unknown {
            candidates = directories
                .iter()
                .filter(|d| d.context == IconContext::Actions)
                .collect();
        }

        // Fourth try: Mimetypes context (for file type icons)
        if candidates.is_empty() && context == IconContext::Mimetypes {
            candidates = directories
                .iter()
                .filter(|d| d.context == IconContext::Mimetypes)
                .collect();
        }

        // Final fallback: any directory
        if candidates.is_empty() {
            candidates = directories.iter().collect();
        }

        // Sort by how well they match the requested size
        // According to XDG spec and best practices (GNOME, Qt6):
        // 1. Prefer exact matches
        // 2. Prefer larger sizes (downscale) over smaller sizes (upscale)
        //    Downscaling preserves quality, upscaling causes blur/stretch
        candidates.sort_by(|a, b| {
            let score_a = match a.directory_type {
                DirectoryType::Fixed => {
                    if a.size == size {
                        0i64 // Exact match - best
                    } else if a.size > size {
                        // Larger size - can downscale (good quality)
                        (a.size - size) as i64
                    } else {
                        // Smaller size - must upscale (poor quality)
                        // Add large penalty (10000) to strongly prefer larger sizes
                        ((size - a.size) as i64) + 10000
                    }
                },
                DirectoryType::Scalable => {
                    let min = a.min_size.unwrap_or(16);
                    let max = a.max_size.unwrap_or(256);
                    if size >= min && size <= max {
                        // Within range, but prefer larger sizes (downscale) over smaller (upscale)
                        // Use the directory size as a tiebreaker - prefer larger directory sizes
                        if a.size >= size {
                            // Directory size is larger or equal - can downscale (good)
                            (a.size - size) as i64
                        } else {
                            // Directory size is smaller - must upscale (bad, add penalty)
                            ((size - a.size) as i64) + 10000
                        }
                    } else if size < min {
                        (min - size) as i64 // Can scale up from min
                    } else {
                        (size - max) as i64 // Can scale down from max
                    }
                },
                DirectoryType::Threshold => {
                    let threshold = a.threshold.unwrap_or(2);
                    let diff = if a.size > size {
                        a.size - size
                    } else {
                        size - a.size
                    };
                    if diff <= threshold {
                        0i64 // Within threshold
                    } else if a.size > size {
                        // Larger size - can downscale
                        diff as i64
                    } else {
                        // Smaller size - must upscale (add penalty)
                        (diff as i64) + 10000
                    }
                },
            };

            let score_b = match b.directory_type {
                DirectoryType::Fixed => {
                    if b.size == size {
                        0i64
                    } else if b.size > size {
                        (b.size - size) as i64
                    } else {
                        ((size - b.size) as i64) + 10000
                    }
                },
                DirectoryType::Scalable => {
                    let min = b.min_size.unwrap_or(16);
                    let max = b.max_size.unwrap_or(256);
                    if size >= min && size <= max {
                        // Within range, but prefer larger sizes (downscale) over smaller (upscale)
                        if b.size >= size {
                            (b.size - size) as i64
                        } else {
                            ((size - b.size) as i64) + 10000
                        }
                    } else if size < min {
                        (min - size) as i64
                    } else {
                        (size - max) as i64
                    }
                },
                DirectoryType::Threshold => {
                    let threshold = b.threshold.unwrap_or(2);
                    let diff = if b.size > size {
                        b.size - size
                    } else {
                        size - b.size
                    };
                    if diff <= threshold {
                        0i64
                    } else if b.size > size {
                        diff as i64
                    } else {
                        (diff as i64) + 10000
                    }
                },
            };

            score_a.cmp(&score_b)
        });

        candidates.first().copied()
    }
}

impl Default for IconLookup {
    fn default() -> Self {
        Self::new()
    }
}
