//! XDG Icon Theme parsing and management.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::service::icon::error::IconError;

/// Icon context (directory type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconContext {
    /// Actions context.
    Actions,
    /// Applications context.
    Apps,
    /// Devices context.
    Devices,
    /// Emblems context.
    Emblems,
    /// Emotes context.
    Emotes,
    /// MIME types context.
    Mimetypes,
    /// Places context.
    Places,
    /// Status context.
    Status,
    /// Unknown context.
    Unknown,
}

impl IconContext {
    /// Parse context from string.
    pub fn from_str(s: &str) -> Self {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "actions" => Self::Actions,
            "apps" | "applications" => Self::Apps,
            "devices" => Self::Devices,
            "emblems" => Self::Emblems,
            "emotes" => Self::Emotes,
            "mimetypes" | "mime-types" | "mime types" | "mime" | "mimes" => Self::Mimetypes,
            "places" => Self::Places,
            "status" => Self::Status,
            _ => Self::Unknown,
        }
    }

    /// Get context as string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Actions => "Actions",
            Self::Apps => "Apps",
            Self::Devices => "Devices",
            Self::Emblems => "Emblems",
            Self::Emotes => "Emotes",
            Self::Mimetypes => "Mimetypes",
            Self::Places => "Places",
            Self::Status => "Status",
            Self::Unknown => "Unknown",
        }
    }
}

/// Directory type for icon directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryType {
    /// Fixed size directory.
    Fixed,
    /// Scalable directory (SVG).
    Scalable,
    /// Threshold directory.
    Threshold,
}

impl DirectoryType {
    /// Parse directory type from string.
    pub fn from_str(s: &str) -> Self {
        match s {
            "Fixed" => Self::Fixed,
            "Scalable" => Self::Scalable,
            "Threshold" => Self::Threshold,
            _ => Self::Fixed,
        }
    }
}

/// Icon directory information.
#[derive(Debug, Clone)]
pub struct IconDirectory {
    /// Directory name.
    pub name: String,
    /// Size of icons in this directory.
    pub size: u32,
    /// Scale factor (usually 1).
    pub scale: u32,
    /// Context of icons.
    pub context: IconContext,
    /// Directory type.
    pub directory_type: DirectoryType,
    /// Minimum size (for scalable).
    pub min_size: Option<u32>,
    /// Maximum size (for scalable).
    pub max_size: Option<u32>,
    /// Threshold (for threshold directories).
    pub threshold: Option<u32>,
}

/// XDG Icon Theme.
#[derive(Debug, Clone)]
pub struct IconTheme {
    /// Theme name.
    pub name: String,
    /// Inherited themes (fallback chain).
    pub inherits: Vec<String>,
    /// Directories in this theme.
    pub directories: Vec<IconDirectory>,
    /// Base path to theme directory.
    pub base_path: PathBuf,
}

impl IconTheme {
    /// Load an icon theme from a directory.
    pub fn load(theme_name: &str, base_path: PathBuf) -> Result<Self, IconError> {
        let index_path = base_path.join("index.theme");

        if !index_path.exists() {
            return Err(IconError::ThemeNotFound(theme_name.to_string()));
        }

        let content = std::fs::read_to_string(&index_path).map_err(|e| {
            IconError::IndexParseError(format!("Failed to read index.theme: {}", e))
        })?;

        let ini = parse_ini(&content)?;

        // Get theme name and inherits from [Icon Theme] section
        let theme_section = ini.get("Icon Theme").ok_or_else(|| {
            IconError::IndexParseError("Missing [Icon Theme] section".to_string())
        })?;

        let name = theme_section
            .get("Name")
            .map(|s| s.to_string())
            .unwrap_or_else(|| theme_name.to_string());

        let inherits = theme_section
            .get("Inherits")
            .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
            .unwrap_or_default();

        let directories_str = theme_section
            .get("Directories")
            .ok_or_else(|| IconError::IndexParseError("Missing Directories key".to_string()))?;

        let directory_names: Vec<String> = directories_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        // Parse each directory section
        let mut directories = Vec::new();
        for dir_name in &directory_names {
            if let Some(dir_section) = ini.get(dir_name.as_str()) {
                let size = dir_section
                    .get("Size")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(48);

                let scale = dir_section
                    .get("Scale")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1);

                let context_str = dir_section
                    .get("Context")
                    .map(|s| s.as_str())
                    .unwrap_or("Unknown");
                let context = IconContext::from_str(context_str);

                let type_str = dir_section
                    .get("Type")
                    .map(|s| s.as_str())
                    .unwrap_or("Fixed");
                let directory_type = DirectoryType::from_str(type_str);

                let min_size = dir_section
                    .get("MinSize")
                    .and_then(|s| s.parse::<u32>().ok());

                let max_size = dir_section
                    .get("MaxSize")
                    .and_then(|s| s.parse::<u32>().ok());

                let threshold = dir_section
                    .get("Threshold")
                    .and_then(|s| s.parse::<u32>().ok());

                directories.push(IconDirectory {
                    name: dir_name.clone(),
                    size,
                    scale,
                    context,
                    directory_type,
                    min_size,
                    max_size,
                    threshold,
                });
            }
        }

        Ok(IconTheme {
            name,
            inherits,
            directories,
            base_path,
        })
    }

    /// Get the path to a directory by name.
    pub fn directory_path(&self, dir_name: &str) -> PathBuf {
        self.base_path.join(dir_name)
    }
}

/// Simple INI parser for index.theme files.
fn parse_ini(content: &str) -> Result<HashMap<String, HashMap<String, String>>, IconError> {
    let mut result = HashMap::new();
    let mut current_section = None;
    let mut current_map = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Section header: [Section Name]
        if line.starts_with('[') && line.ends_with(']') {
            // Save previous section
            if let Some(section) = current_section.take() {
                result.insert(section, current_map);
            }
            current_map = HashMap::new();
            current_section = Some(line[1..line.len() - 1].to_string());
            continue;
        }

        // Key=Value pair
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            current_map.insert(key, value);
        }
    }

    // Save last section
    if let Some(section) = current_section {
        result.insert(section, current_map);
    }

    Ok(result)
}
