use std::path::PathBuf;
use std::collections::HashMap;
use tokio::fs;
use directories::{ProjectDirs, UserDirs};
use crate::error::NpioResult;

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub uri: String,
    pub name: Option<String>,
    pub icon: Option<String>,
}

pub struct BookmarksService {
    bookmarks: HashMap<String, Bookmark>,
    bookmarks_path: PathBuf,
}

impl BookmarksService {
    pub fn new() -> Self {
        // Default to ~/.config/gtk-3.0/bookmarks
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                ProjectDirs::from("", "", "")
                    .map(|dirs| dirs.config_dir().to_path_buf())
            })
            .or_else(|| {
                // Fallback: use UserDirs to get home directory, then join .config
                UserDirs::new()
                    .map(|dirs| dirs.home_dir().join(".config"))
            })
            .unwrap_or_else(|| {
                // Last resort: try HOME env var, but this should rarely be needed
                // since UserDirs should work on most systems
                std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("/tmp"))
                    .join(".config")
            });

        let bookmarks_path = config_dir.join("gtk-3.0").join("bookmarks");

        Self {
            bookmarks: HashMap::new(),
            bookmarks_path,
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self {
            bookmarks: HashMap::new(),
            bookmarks_path: path,
        }
    }

    /// Loads bookmarks from the file.
    pub async fn load(&mut self) -> NpioResult<()> {
        self.bookmarks.clear();

        if !self.bookmarks_path.exists() {
            // File doesn't exist yet, that's okay
            return Ok(());
        }

        let content = fs::read_to_string(&self.bookmarks_path).await?;
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse line: "file:///path [optional label]"
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            let uri = parts[0].to_string();
            
            let name = if parts.len() > 1 {
                Some(parts[1].trim().to_string())
            } else {
                None
            };

            let bookmark = Bookmark {
                uri: uri.clone(),
                name,
                icon: None,
            };

            self.bookmarks.insert(uri, bookmark);
        }

        Ok(())
    }

    /// Saves bookmarks to the file.
    pub async fn save(&self) -> NpioResult<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = self.bookmarks_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut lines = Vec::new();
        for bookmark in self.bookmarks.values() {
            let mut line = bookmark.uri.clone();
            if let Some(ref name) = bookmark.name {
                line.push(' ');
                line.push_str(name);
            }
            lines.push(line);
        }

        let content = lines.join("\n");
        fs::write(&self.bookmarks_path, content).await?;

        Ok(())
    }

    /// Gets all bookmarks.
    pub fn get_bookmarks(&self) -> Vec<Bookmark> {
        self.bookmarks.values().cloned().collect()
    }

    /// Adds a bookmark.
    pub fn add_bookmark(&mut self, uri: String, name: Option<String>) {
        let bookmark = Bookmark {
            uri: uri.clone(),
            name,
            icon: None,
        };
        self.bookmarks.insert(uri, bookmark);
    }

    /// Removes a bookmark by URI.
    pub fn remove_bookmark(&mut self, uri: &str) -> bool {
        self.bookmarks.remove(uri).is_some()
    }

    /// Checks if a bookmark exists.
    pub fn has_bookmark(&self, uri: &str) -> bool {
        self.bookmarks.contains_key(uri)
    }

    /// Gets a bookmark by URI.
    pub fn get_bookmark(&self, uri: &str) -> Option<&Bookmark> {
        self.bookmarks.get(uri)
    }
}

impl Default for BookmarksService {
    fn default() -> Self {
        Self::new()
    }
}

