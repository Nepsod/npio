//! MIME type detection with override table and content-based detection.

use smol::fs;
use smol::io::AsyncReadExt;
use std::path::Path;

/// MIME type detector that uses mime_guess2 (extension-based) and tree_magic_mini (content-based).
pub struct MimeDetector;

impl MimeDetector {
    /// Small override table for file types that need manual mapping.
    /// Only includes special cases that neither mime_guess2 nor tree_magic_mini handle well.
    const MIME_OVERRIDES: &'static [(&'static str, &'static str)] = &[
        // Prefer standard types
        ("toml", "application/toml"), // Use standard MIME type instead of text/x-toml
        ("rs", "text/x-rust"),
        // Shell and scripts
        ("sh", "application/x-shellscript"),
        ("bash", "application/x-shellscript"),
        ("zsh", "application/x-shellscript"),
        // Archives and images
        ("zst", "application/zstd"),
        ("rar", "application/x-rar"),
        ("iso", "application/x-iso9660-image"),
        // Logs
        ("log", "text/x-log"),
        // Add more overrides only if both extension and content detection fail
    ];

    /// Detect MIME type from file path using hybrid approach.
    ///
    /// Strategy:
    /// 1. Check override table first (for known edge cases)
    /// 2. Try extension-based detection (mime_guess2) - fast
    /// 3. If extension-based gives generic result, try content-based detection (tree_magic_mini) - accurate
    /// 4. Return None if all methods fail
    pub async fn detect_mime_type(path: &Path) -> Option<String> {
        // Check override table first (for edge cases)
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            for (override_ext, mime_type) in Self::MIME_OVERRIDES {
                if ext_lower == *override_ext {
                    log::debug!(
                        "MimeDetector: Using override for extension '{}': {}",
                        ext,
                        mime_type
                    );
                    return Some(mime_type.to_string());
                }
            }
        }

        // Try extension-based detection first (fast)
        let ext_mime = mime_guess2::from_path(path).first_or_octet_stream();
        let ext_mime_str = ext_mime.to_string();

        // If we got a specific MIME type (not octet-stream), use it
        if ext_mime_str != "application/octet-stream" {
            log::debug!(
                "MimeDetector: Detected MIME type '{}' from extension for {:?}",
                ext_mime_str,
                path
            );
            return Some(ext_mime_str);
        }

        // Extension-based detection gave generic result, try content-based detection
        log::debug!("MimeDetector: Extension-based detection gave generic result, trying content-based detection for {:?}", path);

        if let Some(content_mime) = Self::detect_mime_type_from_content(path).await {
            log::debug!(
                "MimeDetector: Detected MIME type '{}' from content for {:?}",
                content_mime,
                path
            );
            return Some(content_mime);
        }

        // All methods failed
        log::debug!("MimeDetector: Could not detect MIME type for {:?}", path);
        None
    }

    /// Detect MIME type from file contents using tree_magic_mini.
    ///
    /// This reads a sample of the file and uses magic number detection.
    async fn detect_mime_type_from_content(path: &Path) -> Option<String> {
        // Read first 8KB of file for magic number detection
        const MAX_READ_SIZE: usize = 8192;

        let mut file = match fs::File::open(path).await {
            Ok(f) => f,
            Err(e) => {
                log::debug!(
                    "MimeDetector: Failed to open file for content detection: {}",
                    e
                );
                return None;
            },
        };

        let mut buffer = vec![0u8; MAX_READ_SIZE];
        let bytes_read = match file.read(&mut buffer).await {
            Ok(n) => n,
            Err(e) => {
                log::debug!(
                    "MimeDetector: Failed to read file for content detection: {}",
                    e
                );
                return None;
            },
        };

        if bytes_read == 0 {
            return None;
        }

        buffer.truncate(bytes_read);

        // Use tree_magic_mini to detect MIME type from content
        let mime = tree_magic_mini::from_u8(&buffer);
        let mime_str = mime.to_string();

        // Don't return "application/octet-stream" as it's too generic
        if mime_str == "application/octet-stream" {
            log::debug!(
                "MimeDetector: Content-based detection also gave generic octet-stream for {:?}",
                path
            );
            None
        } else {
            Some(mime_str)
        }
    }

    /// Detect MIME type from file extension only (when path is not available).
    ///
    /// This is a fallback for cases where we only have the extension.
    /// Note: This cannot use content-based detection since we don't have file access.
    pub fn detect_mime_type_from_ext(ext: &str) -> Option<String> {
        let ext_lower = ext.to_lowercase();

        // Check override table first
        for (override_ext, mime_type) in Self::MIME_OVERRIDES {
            if ext_lower == *override_ext {
                log::debug!(
                    "MimeDetector: Using override for extension '{}': {}",
                    ext,
                    mime_type
                );
                return Some(mime_type.to_string());
            }
        }

        // Use mime_guess2 (extension-based only, no content detection available)
        let mime = mime_guess2::from_ext(&ext_lower).first_or_octet_stream();
        let mime_str = mime.to_string();

        // Don't return "application/octet-stream" as it's too generic
        if mime_str == "application/octet-stream" {
            log::debug!(
                "MimeDetector: Got generic octet-stream for extension '{}', returning None",
                ext
            );
            None
        } else {
            log::debug!(
                "MimeDetector: Detected MIME type '{}' for extension '{}'",
                mime_str,
                ext
            );
            Some(mime_str)
        }
    }
}
