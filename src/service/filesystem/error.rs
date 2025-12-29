//! Error types for the filesystem module.

use std::fmt;

/// Errors that can occur in the filesystem model.
#[derive(Debug)]
pub enum FileSystemError {
    /// I/O error from std::fs operations.
    Io(std::io::Error),
    /// Error from the file watcher (notify crate).
    Notify(notify::Error),
    /// Invalid path provided.
    InvalidPath,
    /// Directory not found.
    DirectoryNotFound,
    /// Channel closed or communication error.
    ChannelClosed,
    /// Path is not a directory.
    NotADirectory,
    /// Path is not a file.
    NotAFile,
}

impl fmt::Display for FileSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileSystemError::Io(e) => write!(f, "I/O error: {}", e),
            FileSystemError::Notify(e) => write!(f, "File watcher error: {}", e),
            FileSystemError::InvalidPath => write!(f, "Invalid path provided"),
            FileSystemError::DirectoryNotFound => write!(f, "Directory not found"),
            FileSystemError::ChannelClosed => write!(f, "Channel closed"),
            FileSystemError::NotADirectory => write!(f, "Path is not a directory"),
            FileSystemError::NotAFile => write!(f, "Path is not a file"),
        }
    }
}

impl std::error::Error for FileSystemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FileSystemError::Io(e) => Some(e),
            FileSystemError::Notify(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for FileSystemError {
    fn from(err: std::io::Error) -> Self {
        FileSystemError::Io(err)
    }
}

impl From<notify::Error> for FileSystemError {
    fn from(err: notify::Error) -> Self {
        FileSystemError::Notify(err)
    }
}
