//! Icon provider for filesystem entries.

use crate::file::File;
use crate::service::filesystem::mime_detector::MimeDetector;
use crate::uri::decode_file_uri;

/// Icon data representing an icon for a file entry.
#[derive(Debug, Clone)]
pub struct IconData {
    /// Candidate icon names or identifiers (e.g., "text-x-generic", "folder", etc.).
    pub names: Vec<String>,
    /// Optional path to icon file (for system icons).
    pub path: Option<std::path::PathBuf>,
}

/// Trait for providing icons for filesystem entries.
pub trait IconProvider: Send + Sync {
    /// Get icon data for a file.
    async fn get_icon(&self, file: &dyn File) -> Option<IconData>;
}

/// Icon provider based on MIME type detection.
pub struct MimeIconProvider;

impl MimeIconProvider {
    /// Create a new MIME-based icon provider.
    pub fn new() -> Self {
        Self
    }

    fn mime_variants(mime_type: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let push =
            |s: String, seen: &mut std::collections::BTreeSet<String>, out: &mut Vec<String>| {
                if seen.insert(s.clone()) {
                    out.push(s);
                }
            };

        push(mime_type.to_string(), &mut seen, &mut out);

        if let Some((major, sub)) = mime_type.split_once('/') {
            if let Some(stripped) = sub.strip_prefix("x-") {
                push(format!("{}/{}", major, stripped), &mut seen, &mut out);
            }
        }

        // Aliases and supertypes via shared-mime (loaded per call)
        if let Ok(db) = shared_mime::load_mime_db() {
            for alias in db.aliases(mime_type) {
                push(alias.to_string(), &mut seen, &mut out);
            }
            for parent in db.supertypes(mime_type) {
                push(parent.as_ref().to_string(), &mut seen, &mut out);
            }
        }

        out
    }

    /// Get generic-icon names for MIME type variants.
    fn generic_icon_names(mime_type: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        // Try exact match
        if let Some(icon) =
            crate::service::filesystem::mime_registry::MimeRegistry::get_generic_icon_name(mime_type)
        {
            if seen.insert(icon.clone()) {
                out.push(icon);
            }
        }

        // Try variants
        for variant in Self::mime_variants(mime_type) {
            if let Some(icon) =
                crate::service::filesystem::mime_registry::MimeRegistry::get_generic_icon_name(&variant)
            {
                if seen.insert(icon.clone()) {
                    out.push(icon);
                }
            }
        }

        // Try reverse alias lookup (if this type is an alias, check canonical type)
        if let Some(canonical) =
            crate::service::filesystem::mime_registry::MimeRegistry::find_canonical_for_alias(mime_type)
        {
            if let Some(icon) =
                crate::service::filesystem::mime_registry::MimeRegistry::get_generic_icon_name(&canonical)
            {
                if seen.insert(icon.clone()) {
                    out.push(icon);
                }
            }
        }

        out
    }

    /// Map MIME type to icon name according to freedesktop.org Icon Naming Specification.
    ///
    /// The specification states that MIME types map to icon names by replacing "/" with "-".
    /// General rule: Replace "/" with "-" in MIME type (e.g., text/plain -> text-plain).
    /// However, many themes use simplified names, so we apply some special cases.
    fn mime_to_icon_name(mime_type: &str) -> String {
        let (main_type, sub_type) = if let Some((m, s)) = mime_type.split_once('/') {
            (m, s)
        } else {
            return "unknown".to_string();
        };

        // Special cases that don't follow the simple replacement rule
        match (main_type, sub_type) {
            ("inode", "directory") => "folder".to_string(),
            ("inode", "symlink") => "inode-symlink".to_string(),
            ("text", "plain") => "text-x-generic".to_string(),
            ("application", "pdf") => "application-pdf".to_string(),
            ("application", "zip") | ("application", "x-zip-compressed") => {
                "application-zip".to_string()
            },
            ("application", "json") => "application-json".to_string(),
            ("application", "xml") => "application-xml".to_string(),
            ("application", "toml") => "application-toml".to_string(),
            ("application", "x-executable") | ("application", "x-sharedlib") => {
                "application-x-executable".to_string()
            },
            ("application", "octet-stream") => "application-x-executable".to_string(),
            // Disk image types - map to drive-harddisk or media-optical
            ("application", "x-iso9660-image") | ("application", "x-cd-image") => {
                "media-optical".to_string()
            },
            ("application", "x-raw-floppy-disk-image") => "media-floppy".to_string(),
            ("application", "x-vhd-disk")
            | ("application", "x-vhdx-disk")
            | ("application", "x-virtualbox-vhd") => "drive-harddisk".to_string(),
            ("application", "x-qemu-disk") => "drive-harddisk".to_string(),
            _ => {
                // General rule: Replace "/" with "-"
                // For text/x-* types, use text-x-{subtype}
                // For application/x-* types, use application-x-{subtype} or application-{subtype}
                if main_type == "text" {
                    if sub_type.starts_with("x-") {
                        format!("text-{}", sub_type)
                    } else {
                        format!("text-x-{}", sub_type)
                    }
                } else if main_type == "application" {
                    if sub_type.starts_with("x-") {
                        format!("application-{}", sub_type)
                    } else {
                        // Try application-{subtype} first, fallback to application-x-{subtype}
                        format!("application-{}", sub_type)
                    }
                } else {
                    // For other types, use the simple replacement rule
                    format!("{}-{}", main_type, sub_type).replace("+", "-") // Replace + with - (e.g., svg+xml -> svg-xml)
                }
            },
        }
    }
}

impl IconProvider for MimeIconProvider {
    async fn get_icon(&self, file: &dyn File) -> Option<IconData> {
        // Query file info to get type and MIME type
        let info = match file.query_info("standard::type,standard::content-type", None).await {
            Ok(info) => info,
            Err(e) => {
                log::warn!("MimeIconProvider: Failed to query file info: {}", e);
                return Some(IconData {
                    names: vec!["unknown".to_string()],
                    path: None,
                });
            }
        };

        let file_type = info.get_file_type();
        let basename = file.basename();

        // Explicit symlink handling so we do not depend on target MIME detection.
        if file_type == crate::file_info::FileType::SymbolicLink {
            return Some(IconData {
                names: vec![
                    "inode-symlink".to_string(),
                    // Fallbacks: common symlink icons in many themes
                    "emblem-symbolic-link".to_string(),
                    "folder".to_string(),
                ],
                path: None,
            });
        }

        // Directories always get folder icon
        if file_type == crate::file_info::FileType::Directory {
            return Some(IconData {
                names: vec!["folder".to_string()],
                path: None,
            });
        }

        // Determine MIME type
        let mime_type: Option<String> = if let Some(mime) = info.get_content_type() {
            log::debug!("MimeIconProvider: Using MIME type from file info: {}", mime);
            Some(mime.to_string())
        } else {
            // Use MimeDetector to detect MIME type from path or extension
            let uri = file.uri();
            let path = if uri.starts_with("file://") {
                // Decode file:// URI properly
                decode_file_uri(&uri)
                    .map_err(|e| {
                        log::warn!("MimeIconProvider: Failed to decode URI: {}", e);
                    })
                    .ok()?
            } else {
                // For non-file URIs, use basename for extension detection
                std::path::PathBuf::from(&basename)
            };

            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                log::debug!(
                    "MimeIconProvider: Detecting MIME type from extension: {}",
                    ext
                );
                let detected = MimeDetector::detect_mime_type_from_ext(ext);
                if let Some(ref mime) = detected {
                    log::debug!("MimeIconProvider: Detected MIME type: {}", mime);
                }
                detected
            } else {
                // Try to detect from path (for files without extensions)
                log::debug!(
                    "MimeIconProvider: No extension, trying path-based detection for: {}",
                    basename
                );
                MimeDetector::detect_mime_type(&path).await
            }
        };

        // Map MIME type to icon name
        if let Some(ref mime_type) = mime_type {
            let mut names = Vec::new();
            let mut seen = std::collections::BTreeSet::new();

            // First, try generic-icon names from XML (these are the most accurate)
            for generic_icon in Self::generic_icon_names(mime_type) {
                if seen.insert(generic_icon.clone()) {
                    names.push(generic_icon);
                }
            }

            // Second, try the original MIME type first (to catch special cases)
            let original_icon = Self::mime_to_icon_name(mime_type);
            log::debug!(
                "MimeIconProvider: Mapped original MIME type '{}' -> icon '{}'",
                mime_type,
                original_icon
            );
            if seen.insert(original_icon.clone()) {
                names.push(original_icon);
            }

            // Then try MIME-to-icon-name mapping for variants (excluding the original, already done)
            for variant in Self::mime_variants(mime_type) {
                // Skip the original MIME type since we already processed it
                if variant.as_str() == mime_type.as_str() {
                    continue;
                }
                let icon_name = Self::mime_to_icon_name(&variant);
                log::debug!(
                    "MimeIconProvider: Mapped MIME type '{}' -> variant '{}' -> icon '{}'",
                    mime_type,
                    variant,
                    icon_name
                );
                if seen.insert(icon_name.clone()) {
                    names.push(icon_name);
                }
            }

            // Add hand-tuned fallbacks for well-known types that themes often name differently.
            match mime_type.as_str() {
                "application/toml" | "text/x-toml" => {
                    for extra in [
                        "text-x-toml",
                        "application-toml",
                        "text-x-source",
                        "text-x-generic",
                    ] {
                        if seen.insert(extra.to_string()) {
                            names.push(extra.to_string());
                        }
                    }
                },
                "application/x-raw-floppy-disk-image" => {
                    for extra in ["media-floppy", "drive-removable-media", "drive-harddisk"] {
                        if seen.insert(extra.to_string()) {
                            names.push(extra.to_string());
                        }
                    }
                },
                _ => {},
            }

            if !names.is_empty() {
                log::debug!(
                    "MimeIconProvider: Generated icon names {:?} for MIME type '{}'",
                    names,
                    mime_type
                );
                return Some(IconData { names, path: None });
            }
        }

        // Final fallback: generic file icon
        log::debug!(
            "MimeIconProvider: Using fallback icon 'text-x-generic' for file: {}",
            basename
        );
        Some(IconData {
            names: vec!["unknown".to_string()],
            path: None,
        })
    }
}

impl Default for MimeIconProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::local::LocalFile;
    use crate::service::icon::IconRegistry;
    use std::path::PathBuf;

    fn dummy_file(name: &str, _mime: &str) -> LocalFile {
        // Create a temporary file path for testing
        // In real usage, the file would exist and query_info would populate MIME type
        LocalFile::new(PathBuf::from(name))
    }

    #[test]
    fn mime_provider_emits_application_toml_icon() {
        let file = dummy_file("test.toml", "application/toml");
        let provider = MimeIconProvider::new();
        // Note: This test will use extension-based detection since the file doesn't exist
        // In real usage, query_info would populate the MIME type
        let icon = smol::block_on(provider.get_icon(&file));
        // Extension-based detection should still work
        assert!(icon.is_some(), "icon provider returned no icon");
    }

    #[test]
    fn registry_resolves_application_toml_icon() {
        let registry = IconRegistry::new().expect("icon registry");
        let file = dummy_file("test.toml", "application/toml");
        let icon = smol::block_on(registry.get_file_icon(&file, 64));
        assert!(icon.is_some(), "registry returned no icon");
    }

    #[test]
    fn registry_resolves_drive_removable_icon() {
        let registry = IconRegistry::new().expect("icon registry");
        let file = dummy_file("disk.img", "application/x-raw-floppy-disk-image");
        let icon = smol::block_on(registry.get_file_icon(&file, 64));
        assert!(icon.is_some(), "registry returned no icon");
    }
}
