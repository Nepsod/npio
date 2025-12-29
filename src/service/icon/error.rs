//! Error types for the icon system.

use std::path::PathBuf;

/// Errors that can occur in the icon system.
#[derive(Debug, thiserror::Error)]
pub enum IconError {
    /// Theme not found.
    #[error("Icon theme '{0}' not found")]
    ThemeNotFound(String),

    /// Error parsing index.theme file.
    #[error("Failed to parse index.theme: {0}")]
    IndexParseError(String),

    /// Icon not found.
    #[error("Icon '{0}' not found in theme")]
    IconNotFound(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Invalid image format.
    #[error("Invalid image format: {0}")]
    InvalidFormat(String),

    /// Invalid theme directory.
    #[error("Invalid theme directory: {0}")]
    InvalidThemeDirectory(PathBuf),
}
