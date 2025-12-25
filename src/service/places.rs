//! User directory helpers (GIO-compatible)
//!
//! Provides helper functions to get user directories, matching GLib's `g_get_user_special_dir()`
//! and `g_get_home_dir()` pattern. This is not a service - it's a set of helper functions that
//! return `File` objects, following GIO's file-centric API design.
//!
//! Reads XDG user directories from `~/.config/user-dirs.dirs` (or `$XDG_CONFIG_HOME/user-dirs.dirs`)
//! following the XDG User Directories specification, matching GLib's implementation.

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use directories::UserDirs;
use crate::backend::get_file_for_uri;
use crate::error::NpioResult;
use crate::file::File;

/// User directory types, matching GLib's `GUserDirectory` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserDirectory {
    /// The user's Desktop directory
    Desktop,
    /// The user's Documents directory
    Documents,
    /// The user's Downloads directory
    Download,
    /// The user's Music directory
    Music,
    /// The user's Pictures directory
    Pictures,
    /// The user's shared directory (Public)
    PublicShare,
    /// The user's Templates directory
    Templates,
    /// The user's Videos/Movies directory
    Videos,
}

/// Parses the XDG user-dirs.dirs file, matching GLib's implementation.
///
/// The file format is:
/// ```text
/// XDG_DESKTOP_DIR="$HOME/Desktop"
/// XDG_DOCUMENTS_DIR="$HOME/Documents"
/// ...
/// ```
///
/// Returns a map of directory types to paths.
///
/// # Note
/// This function is public for testing purposes only. It should not be used
/// in production code. Use `get_user_special_file()` instead.
#[doc(hidden)]
pub fn parse_user_dirs_file(content: &str, home_dir: &Path) -> HashMap<UserDirectory, PathBuf> {
    parse_user_dirs_file_impl(content, home_dir)
}

fn parse_user_dirs_file_impl(content: &str, home_dir: &Path) -> HashMap<UserDirectory, PathBuf> {
    let mut dirs = HashMap::new();
    
    for line in content.lines() {
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Find the directory type
        let (directory, rest) = if let Some(rest) = line.strip_prefix("XDG_DESKTOP_DIR") {
            (UserDirectory::Desktop, rest)
        } else if let Some(rest) = line.strip_prefix("XDG_DOCUMENTS_DIR") {
            (UserDirectory::Documents, rest)
        } else if let Some(rest) = line.strip_prefix("XDG_DOWNLOAD_DIR") {
            (UserDirectory::Download, rest)
        } else if let Some(rest) = line.strip_prefix("XDG_MUSIC_DIR") {
            (UserDirectory::Music, rest)
        } else if let Some(rest) = line.strip_prefix("XDG_PICTURES_DIR") {
            (UserDirectory::Pictures, rest)
        } else if let Some(rest) = line.strip_prefix("XDG_PUBLICSHARE_DIR") {
            (UserDirectory::PublicShare, rest)
        } else if let Some(rest) = line.strip_prefix("XDG_TEMPLATES_DIR") {
            (UserDirectory::Templates, rest)
        } else if let Some(rest) = line.strip_prefix("XDG_VIDEOS_DIR") {
            (UserDirectory::Videos, rest)
        } else {
            continue;
        };
        
        // Skip whitespace
        let rest = rest.trim_start();
        
        // Expect '='
        let rest = match rest.strip_prefix('=') {
            Some(r) => r.trim_start(),
            None => continue,
        };
        
        // Expect opening quote
        let rest = match rest.strip_prefix('"') {
            Some(r) => r,
            None => continue,
        };
        
        // Check if it starts with $HOME
        let (path_str, is_relative) = if let Some(rest_after_home) = rest.strip_prefix("$HOME") {
            if rest_after_home.starts_with('/') || rest_after_home.starts_with('"') {
                (rest_after_home, true)
            } else {
                continue;
            }
        } else if rest.starts_with('/') {
            (rest, false)
        } else {
            continue;
        };
        
        // Find closing quote
        let end_quote = path_str.find('"').unwrap_or(path_str.len());
        let path_str = &path_str[..end_quote];
        
        // Build the path
        let path = if is_relative {
            // Remove leading slash if present (we'll join with home_dir)
            let rel_path = path_str.trim_start_matches('/');
            home_dir.join(rel_path)
        } else {
            PathBuf::from(path_str)
        };
        
        // PathBuf automatically normalizes trailing slashes, but we need to handle
        // the case where the path might be just "/" (root). For relative paths,
        // we want to remove trailing slashes. For absolute paths, we keep at least "/".
        let final_path = if is_relative {
            // For relative paths, remove trailing slashes by converting to string and back
            let path_str = path.to_string_lossy();
            let trimmed = path_str.trim_end_matches('/');
            if trimmed.is_empty() {
                path
            } else {
                PathBuf::from(trimmed)
            }
        } else {
            // For absolute paths, keep at least "/" but remove other trailing slashes
            let path_str = path.to_string_lossy();
            let trimmed = path_str.trim_end_matches('/');
            if trimmed.is_empty() || trimmed == "/" {
                PathBuf::from("/")
            } else {
                PathBuf::from(trimmed)
            }
        };
        
        // Store (duplicates override previous value, matching GLib behavior)
        dirs.insert(directory, final_path);
    }
    
    dirs
}

/// Gets the path to the user-dirs.dirs file.
fn get_user_dirs_file_path() -> Option<PathBuf> {
    // Check XDG_CONFIG_HOME first
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        let path = PathBuf::from(config_home);
        if path.is_absolute() {
            return Some(path.join("user-dirs.dirs"));
        }
    }
    
    // Fall back to ~/.config/user-dirs.dirs
    if let Some(home) = std::env::var("HOME").ok() {
        return Some(PathBuf::from(home).join(".config").join("user-dirs.dirs"));
    }
    
    // Last resort: try UserDirs
    if let Some(user_dirs) = UserDirs::new() {
        let config_dir = user_dirs.home_dir().join(".config");
        return Some(config_dir.join("user-dirs.dirs"));
    }
    
    None
}

/// Cached user special directories (matching GLib's caching behavior).
static USER_SPECIAL_DIRS: Lazy<Mutex<Option<HashMap<UserDirectory, PathBuf>>>> = Lazy::new(|| Mutex::new(None));

/// Loads user special directories from XDG user-dirs.dirs file.
///
/// Returns a map of directory types to paths. Falls back to UserDirs crate
/// if the file doesn't exist or can't be parsed.
///
/// Results are cached after first load (matching GLib's `g_get_user_special_dir()` behavior).
fn load_user_special_dirs() -> HashMap<UserDirectory, PathBuf> {
    // Check cache first
    {
        let cache = USER_SPECIAL_DIRS.lock().unwrap();
        if let Some(ref dirs) = *cache {
            return dirs.clone();
        }
    }
    
    // Load directories
    let dirs = load_user_special_dirs_impl();
    
    // Cache the result
    {
        let mut cache = USER_SPECIAL_DIRS.lock().unwrap();
        *cache = Some(dirs.clone());
    }
    
    dirs
}

/// Internal implementation that actually loads the directories.
fn load_user_special_dirs_impl() -> HashMap<UserDirectory, PathBuf> {
    let mut dirs = HashMap::new();
    
    // Get home directory
    let home_dir = std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            UserDirs::new().map(|d| d.home_dir().to_path_buf())
        });
    
    let home_dir = match home_dir {
        Some(h) => h,
        None => return dirs, // Can't proceed without home directory
    };
    
    // Try to read user-dirs.dirs file
    if let Some(config_file) = get_user_dirs_file_path() {
        if let Ok(content) = std::fs::read_to_string(&config_file) {
            dirs = parse_user_dirs_file(&content, &home_dir);
        }
    }
    
    // Special-case desktop for historical compatibility (matching GLib)
    if !dirs.contains_key(&UserDirectory::Desktop) {
        dirs.insert(UserDirectory::Desktop, home_dir.join("Desktop"));
    }
    
    // Fall back to UserDirs crate for any missing directories
    if let Some(user_dirs) = UserDirs::new() {
        if !dirs.contains_key(&UserDirectory::Documents) {
            if let Some(doc) = user_dirs.document_dir() {
                dirs.insert(UserDirectory::Documents, doc.to_path_buf());
            }
        }
        if !dirs.contains_key(&UserDirectory::Download) {
            if let Some(dl) = user_dirs.download_dir() {
                dirs.insert(UserDirectory::Download, dl.to_path_buf());
            }
        }
        if !dirs.contains_key(&UserDirectory::Music) {
            if let Some(music) = user_dirs.audio_dir() {
                dirs.insert(UserDirectory::Music, music.to_path_buf());
            }
        }
        if !dirs.contains_key(&UserDirectory::Pictures) {
            if let Some(pics) = user_dirs.picture_dir() {
                dirs.insert(UserDirectory::Pictures, pics.to_path_buf());
            }
        }
        if !dirs.contains_key(&UserDirectory::PublicShare) {
            if let Some(pub_dir) = user_dirs.public_dir() {
                dirs.insert(UserDirectory::PublicShare, pub_dir.to_path_buf());
            }
        }
        if !dirs.contains_key(&UserDirectory::Templates) {
            if let Some(tmpl) = user_dirs.template_dir() {
                dirs.insert(UserDirectory::Templates, tmpl.to_path_buf());
            }
        }
        if !dirs.contains_key(&UserDirectory::Videos) {
            if let Some(vid) = user_dirs.video_dir() {
                dirs.insert(UserDirectory::Videos, vid.to_path_buf());
            }
        }
    }
    
    dirs
}

/// Gets a `File` object for the user's home directory.
///
/// This matches GLib's `g_get_home_dir()` function. Note that home directory
/// is separate from special directories in GIO.
///
/// # Returns
/// - `Ok(Box<dyn File>)` - File object for home directory
/// - `Err` - If home directory cannot be determined
pub fn get_home_file() -> NpioResult<Box<dyn File>> {
    let user_dirs = UserDirs::new()
        .ok_or_else(|| crate::error::NpioError::new(
            crate::error::IOErrorEnum::NotFound,
            "Could not determine user home directory"
        ))?;
    
    let home_path = user_dirs.home_dir();
    let uri = format!("file://{}", home_path.to_string_lossy());
    get_file_for_uri(&uri)
}

/// Gets a `File` object for a special user directory.
///
/// This matches GLib's `g_get_user_special_dir()` function. Returns a `File`
/// object for the specified directory type, or `None` if that directory
/// doesn't exist or cannot be determined.
///
/// Reads from `~/.config/user-dirs.dirs` (or `$XDG_CONFIG_HOME/user-dirs.dirs`)
/// following the XDG User Directories specification, matching GLib's implementation.
///
/// Results are cached after first load (matching GLib's behavior). To reload
/// the cache (e.g., after the user-dirs.dirs file changes), use `reload_user_special_dirs_cache()`.
///
/// # Arguments
/// * `directory` - The type of special directory to get
///
/// # Returns
/// - `Ok(Some(Box<dyn File>))` - File object for the directory if it exists
/// - `Ok(None)` - If the directory doesn't exist or cannot be determined
/// - `Err` - If there's an error creating the File object
///
/// # Example
/// ```no_run
/// use npio::service::places::{get_user_special_file, UserDirectory};
///
/// # async fn example() -> npio::NpioResult<()> {
/// if let Some(docs_file) = get_user_special_file(UserDirectory::Documents)? {
///     println!("Documents directory: {}", docs_file.uri());
/// }
/// # Ok(())
/// # }
/// ```
pub fn get_user_special_file(directory: UserDirectory) -> NpioResult<Option<Box<dyn File>>> {
    let dirs = load_user_special_dirs();
    
    if let Some(path) = dirs.get(&directory) {
        let uri = format!("file://{}", path.to_string_lossy());
        get_file_for_uri(&uri).map(Some)
    } else {
        Ok(None)
    }
}

/// Gets the icon name for a user directory, matching GIO's icon selection logic.
///
/// This matches the logic in GIO's `glocalfileinfo.c:get_icon_name()`. Returns
/// the appropriate icon name for the given directory type.
///
/// # Arguments
/// * `directory` - The type of special directory
/// * `use_symbolic` - Whether to return symbolic icon name
///
/// # Returns
/// Icon name string (e.g., "user-home", "folder-documents-symbolic")
pub fn get_directory_icon_name(directory: UserDirectory, use_symbolic: bool) -> &'static str {
    match directory {
        UserDirectory::Desktop => {
            if use_symbolic { "user-desktop-symbolic" } else { "user-desktop" }
        }
        UserDirectory::Documents => {
            if use_symbolic { "folder-documents-symbolic" } else { "folder-documents" }
        }
        UserDirectory::Download => {
            if use_symbolic { "folder-download-symbolic" } else { "folder-download" }
        }
        UserDirectory::Music => {
            if use_symbolic { "folder-music-symbolic" } else { "folder-music" }
        }
        UserDirectory::Pictures => {
            if use_symbolic { "folder-pictures-symbolic" } else { "folder-pictures" }
        }
        UserDirectory::PublicShare => {
            if use_symbolic { "folder-publicshare-symbolic" } else { "folder-publicshare" }
        }
        UserDirectory::Templates => {
            if use_symbolic { "folder-templates-symbolic" } else { "folder-templates" }
        }
        UserDirectory::Videos => {
            if use_symbolic { "folder-videos-symbolic" } else { "folder-videos" }
        }
    }
}

/// Gets the icon name for the home directory.
///
/// # Arguments
/// * `use_symbolic` - Whether to return symbolic icon name
///
/// # Returns
/// Icon name string ("user-home" or "user-home-symbolic")
pub fn get_home_icon_name(use_symbolic: bool) -> &'static str {
    if use_symbolic {
        "user-home-symbolic"
    } else {
        "user-home"
    }
}

/// Reloads the cache used for `get_user_special_file()`.
///
/// This matches GLib's `g_reload_user_special_dirs_cache()` function. Call this
/// if you've changed the user-dirs.dirs file and want to see the changes without
/// restarting the application.
///
/// # Note
/// Due to thread safety, this may cause some memory to be leaked for directories
/// that changed value (matching GLib's behavior). This is generally acceptable
/// as user directories rarely change during runtime.
pub fn reload_user_special_dirs_cache() {
    let mut cache = USER_SPECIAL_DIRS.lock().unwrap();
    *cache = None; // Clear cache, will be reloaded on next access
}
