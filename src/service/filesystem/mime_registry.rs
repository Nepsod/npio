use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use cosmic_mime_apps::{apps_for_mime, associations, List};
use mime::Mime;
use shared_mime::{load_mime_db, MimeDB};

/// Registry that resolves MIME types to desktop applications using layered mimeapps.list files.
///
/// Precedence: user (~/.config/mimeapps.list) > admin (/etc/xdg/mimeapps.list) > system (/usr/share/applications/mimeapps.list).
#[derive(Clone)]
pub struct MimeRegistry {
    apps: Arc<BTreeMap<Arc<str>, Arc<cosmic_mime_apps::App>>>,
    lists: Arc<List>,
    mime_db: Arc<MimeDB>,
}

impl MimeRegistry {
    /// Load registry from the standard override paths.
    pub fn load_default() -> Self {
        let paths = default_mimeapps_paths();

        let mut lists = List::default();
        lists.load_from_paths(&paths);

        let apps = associations::by_app();
        let mime_db = load_mime_db().unwrap_or_else(|_| MimeDB::new());

        Self {
            apps: Arc::new(apps),
            lists: Arc::new(lists),
            mime_db: Arc::new(mime_db),
        }
    }

    /// Resolve the default application desktop ID for a MIME type.
    pub fn resolve(&self, mime: &str) -> Option<String> {
        let mime: Mime = mime.parse().ok()?;

        if let Some(defaults) = self.lists.default_app_for(&mime) {
            if let Some(first) = defaults.first() {
                return Some(first.to_string());
            }
        }

        // Fallback to the first associated app we know about.
        let first = apps_for_mime(&mime, &self.apps)
            .next()
            .map(|(id, _)| id.to_string());
        first
    }

    /// List all handlers (desktop IDs) that can open the given MIME type.
    pub fn list_handlers(&self, mime: &str) -> Vec<String> {
        let mime: Mime = match mime.parse() {
            Ok(m) => m,
            Err(_) => return Vec::new(),
        };

        let mut seen = std::collections::BTreeSet::new();
        let mut out = Vec::new();

        if let Some(defaults) = self.lists.default_app_for(&mime) {
            for app in defaults {
                if seen.insert(app.as_ref()) {
                    out.push(app.to_string());
                }
            }
        }

        if let Some(added) = self.lists.added_associations.get(&mime) {
            for app in added {
                if seen.insert(app.as_ref()) {
                    out.push(app.to_string());
                }
            }
        }

        for (id, _) in apps_for_mime(&mime, &self.apps) {
            if seen.insert(id.as_ref()) {
                out.push(id.to_string());
            }
        }

        out
    }

    /// Find the canonical MIME type that has the given type as an alias by parsing XML.
    pub fn find_canonical_for_alias(alias: &str) -> Option<String> {
        // Try exact file at /usr/share/mime/{major}/{minor}.xml first
        if let Some((major, minor)) = alias.split_once('/') {
            let path = Path::new("/usr/share/mime")
                .join(major)
                .join(format!("{minor}.xml"));
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(canonical) = Self::extract_canonical_from_alias(&content, alias) {
                    return Some(canonical);
                }
            }
        }

        // Fallback: scan packages XMLs
        let packages_dir = Path::new("/usr/share/mime/packages");
        if let Ok(entries) = std::fs::read_dir(packages_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("xml") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Some(canonical) = Self::extract_canonical_from_alias(&content, alias) {
                        return Some(canonical);
                    }
                }
            }
        }
        None
    }

    /// Extract canonical MIME type that has the given alias.
    fn extract_canonical_from_alias(content: &str, alias: &str) -> Option<String> {
        let alias_pattern = format!(r#"<alias type="{}""#, alias);
        if !content.contains(&alias_pattern) {
            return None;
        }

        let mut search_start = 0;
        while let Some(alias_idx) = content[search_start..].find(&alias_pattern) {
            let alias_start = search_start + alias_idx;

            // Find the mime-type block containing this alias
            // Search backwards for the opening <mime-type tag
            let mime_start = match content[..alias_start].rfind("<mime-type") {
                Some(idx) => idx,
                None => {
                    search_start = alias_start + alias_pattern.len();
                    continue;
                },
            };

            // Find the type attribute of this mime-type block
            let tag_end = match content[mime_start..].find('>') {
                Some(i) => mime_start + i,
                None => {
                    search_start = alias_start + alias_pattern.len();
                    continue;
                },
            };
            let tag_text = &content[mime_start..tag_end];

            // Extract type attribute
            let type_attr = r#"type=""#;
            let type_idx = match tag_text.find(type_attr) {
                Some(i) => i + type_attr.len(),
                None => {
                    search_start = alias_start + alias_pattern.len();
                    continue;
                },
            };
            let rest = &tag_text[type_idx..];
            let end_quote = match rest.find('"') {
                Some(i) => i,
                None => {
                    search_start = alias_start + alias_pattern.len();
                    continue;
                },
            };
            let canonical = &rest[..end_quote];

            // Verify this alias is within the same mime-type block
            let end_tag = "</mime-type>";
            let block_end = match content[tag_end..].find(end_tag) {
                Some(i) => tag_end + i + end_tag.len(),
                None => {
                    search_start = alias_start + alias_pattern.len();
                    continue;
                },
            };
            if alias_start < block_end {
                return Some(canonical.to_string());
            }
            search_start = alias_start + alias_pattern.len();
        }
        None
    }

    /// Get human-readable description for a MIME type (if available).
    pub fn description(&self, mime: &str) -> Option<String> {
        // Try exact match
        if let Some(desc) = self.mime_db.description(mime) {
            return Some(desc.to_string());
        }
        // Try aliases of this type
        for alias in self.mime_db.aliases(mime) {
            if let Some(desc) = self.mime_db.description(alias) {
                return Some(desc.to_string());
            }
        }
        // Try reverse: if this type is an alias of another, get that type's description
        if let Some(canonical) = Self::find_canonical_for_alias(mime) {
            if let Some(desc) = self.mime_db.description(&canonical) {
                return Some(desc.to_string());
            }
        }
        // Try supertypes (parents)
        for parent in self.mime_db.supertypes(mime) {
            let parent = parent.as_ref();
            if let Some(desc) = self.mime_db.description(parent) {
                return Some(desc.to_string());
            }
        }
        None
    }

    /// Get generic-icon name for a MIME type from XML files.
    pub fn generic_icon_name(&self, mime: &str) -> Option<String> {
        // Try exact match
        if let Some(icon) = Self::get_generic_icon_name(mime) {
            return Some(icon);
        }
        // Try aliases
        for alias in self.mime_db.aliases(mime) {
            if let Some(icon) = Self::get_generic_icon_name(alias) {
                return Some(icon);
            }
        }
        // Try reverse: if this type is an alias of another, get that type's generic-icon
        if let Some(canonical) = Self::find_canonical_for_alias(mime) {
            if let Some(icon) = Self::get_generic_icon_name(&canonical) {
                return Some(icon);
            }
        }
        // Try supertypes (parents)
        for parent in self.mime_db.supertypes(mime) {
            let parent = parent.as_ref();
            if let Some(icon) = Self::get_generic_icon_name(parent) {
                return Some(icon);
            }
        }
        None
    }

    /// Get generic-icon name for a MIME type from XML files (internal, public for icon provider).
    pub fn get_generic_icon_name(mime_type: &str) -> Option<String> {
        // Try exact file at /usr/share/mime/{major}/{minor}.xml
        if let Some((major, minor)) = mime_type.split_once('/') {
            let path = Path::new("/usr/share/mime")
                .join(major)
                .join(format!("{minor}.xml"));
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(icon) = Self::extract_generic_icon(&content, mime_type) {
                    return Some(icon);
                }
            }
        }

        // Fallback: scan packages XMLs
        let packages_dir = Path::new("/usr/share/mime/packages");
        if let Ok(entries) = std::fs::read_dir(packages_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("xml") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Some(icon) = Self::extract_generic_icon(&content, mime_type) {
                        return Some(icon);
                    }
                }
            }
        }
        None
    }

    /// Extract generic-icon name from a MIME XML block.
    fn extract_generic_icon(content: &str, mime_type: &str) -> Option<String> {
        // Find mime-type block matching our MIME type
        let mime_pattern = format!(r#"type="{}""#, mime_type);
        if !content.contains(&mime_pattern) {
            return None;
        }

        let mut search_start = 0;
        while let Some(idx) = content[search_start..].find("<mime-type") {
            let mime_start = search_start + idx;
            let tag_end = match content[mime_start..].find('>') {
                Some(i) => mime_start + i + 1,
                None => break,
            };
            let tag_text = &content[mime_start..tag_end];

            // Check if this mime-type block contains our MIME type
            let type_attr = r#"type=""#;
            let type_idx = match tag_text.find(type_attr) {
                Some(i) => i + type_attr.len(),
                None => {
                    search_start = tag_end;
                    continue;
                },
            };
            let rest = &tag_text[type_idx..];
            let end_quote = match rest.find('"') {
                Some(i) => i,
                None => {
                    search_start = tag_end;
                    continue;
                },
            };
            let ty = &rest[..end_quote];
            if ty != mime_type {
                search_start = tag_end;
                continue;
            }

            // Find end of this mime-type block
            let end_tag = "</mime-type>";
            let block_end = match content[tag_end..].find(end_tag) {
                Some(i) => tag_end + i + end_tag.len(),
                None => {
                    search_start = tag_end;
                    continue;
                },
            };
            let mime_block = &content[mime_start..block_end];

            // Find generic-icon element
            if let Some(icon_idx) = mime_block.find("<generic-icon") {
                let icon_tag_end = match mime_block[icon_idx..].find('>') {
                    Some(i) => icon_idx + i,
                    None => {
                        search_start = block_end;
                        continue;
                    },
                };
                let icon_tag = &mime_block[icon_idx..icon_tag_end];
                let name_attr = r#"name=""#;
                if let Some(name_idx) = icon_tag.find(name_attr) {
                    let name_start = name_idx + name_attr.len();
                    let name_rest = &icon_tag[name_start..];
                    let name_end = match name_rest.find('"') {
                        Some(i) => i,
                        None => {
                            search_start = block_end;
                            continue;
                        },
                    };
                    return Some(name_rest[..name_end].to_string());
                }
            }
            search_start = block_end;
        }
        None
    }

    /// Get all known icon name candidates derived from MIME (name, aliases, supertypes).
    pub fn icon_candidates(&self, mime: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        let push =
            |s: String, seen: &mut std::collections::BTreeSet<String>, out: &mut Vec<String>| {
                if seen.insert(s.clone()) {
                    out.push(s);
                }
            };

        push(mime.to_string(), &mut seen, &mut out);

        for alias in self.mime_db.aliases(mime) {
            push(alias.to_string(), &mut seen, &mut out);
        }

        for parent in self.mime_db.supertypes(mime) {
            push(parent.as_ref().to_string(), &mut seen, &mut out);
        }

        out
    }

    /// Launch the given desktop entry with the provided file path.
    pub fn launch(&self, desktop_id: &str, file: &Path) -> anyhow::Result<()> {
        let app = match self
            .apps
            .iter()
            .find(|(id, _)| id.as_ref() == desktop_id)
            .map(|(_, app)| app.clone())
        {
            Some(app) => app,
            None => {
                // If the entry isn't in the registry map, try gtk-launch first,
                // otherwise fall back to xdg-open to avoid silent failures.
                return fallback_launch(desktop_id, file);
            },
        };

        let exec = read_exec_line(&app.path)
            .ok_or_else(|| anyhow::anyhow!("Missing Exec line for {}", desktop_id))?;

        let file_arg = file.to_string_lossy().to_string();
        let mut parts: Vec<String> = exec.split_whitespace().map(|s| s.to_string()).collect();
        if parts.is_empty() {
            return Err(anyhow::anyhow!("Invalid Exec line for {}", desktop_id));
        }

        // Replace field codes; append path if none present.
        let mut had_field_code = false;
        for part in parts.iter_mut() {
            if part.contains("%f") || part.contains("%u") {
                *part = part.replace("%f", &file_arg).replace("%u", &file_arg);
                had_field_code = true;
            } else if part.contains("%F") || part.contains("%U") {
                *part = part.replace("%F", &file_arg).replace("%U", &file_arg);
                had_field_code = true;
            } else if part.contains("%c") {
                *part = part.replace("%c", app.name.as_ref());
            } else if part.contains("%i") {
                // Drop %i or replace with icon flag if desired; here we drop.
                *part = part.replace("%i", "");
            } else if part.contains("%k") {
                *part = part.replace("%k", app.path.to_string_lossy().as_ref());
            }
        }
        if !had_field_code {
            parts.push(file_arg);
        }

        parts.retain(|p| !p.is_empty());

        let program = parts.remove(0);
        let mut cmd = std::process::Command::new(program);
        if !parts.is_empty() {
            cmd.args(parts);
        }

        cmd.spawn()
            .map_err(|e| anyhow::anyhow!("Failed to launch {}: {}", desktop_id, e))?;
        Ok(())
    }

    /// Resolve default app and include its user-visible name.
    pub fn resolve_with_name(&self, mime: &str) -> Option<(String, String)> {
        let id = self.resolve(mime)?;
        let name = self.name_or_prettify(&id);
        Some((id, name))
    }

    /// Get the user-visible name for a desktop id.
    pub fn name_for(&self, desktop_id: &str) -> Option<String> {
        // Try exact match first
        if let Some(app) = self.apps.get(desktop_id) {
            return Some(app.name.to_string());
        }

        // Try with .desktop suffix if not present
        if !desktop_id.ends_with(".desktop") {
            let with_suffix = format!("{}.desktop", desktop_id);
            if let Some(app) = self.apps.get(&*with_suffix) {
                return Some(app.name.to_string());
            }
        }

        // Try without .desktop suffix if present
        if desktop_id.ends_with(".desktop") {
            let without_suffix = desktop_id.strip_suffix(".desktop").unwrap_or(desktop_id);
            if let Some(app) = self.apps.get(without_suffix) {
                return Some(app.name.to_string());
            }
        }

        None
    }

    /// Get a prettified name for a desktop ID, with fallback prettification if not found in registry.
    pub fn name_or_prettify(&self, desktop_id: &str) -> String {
        if let Some(name) = self.name_for(desktop_id) {
            return name;
        }

        // Fallback: prettify the desktop ID
        let trimmed = desktop_id.strip_suffix(".desktop").unwrap_or(desktop_id);

        // Handle reverse domain notation (e.g., "org.kde.gwenview" -> "Gwenview")
        let name_part = if trimmed.contains('.') {
            // Extract the last component after the last dot
            trimmed.split('.').last().unwrap_or(trimmed)
        } else {
            trimmed
        };

        // Prettify: replace separators with spaces and title-case
        let pretty = name_part
            .replace(['-', '_'], " ")
            .split_whitespace()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<String>>()
            .join(" ");

        if pretty.is_empty() {
            desktop_id.to_string()
        } else {
            pretty
        }
    }
}

fn default_mimeapps_paths() -> Vec<std::path::PathBuf> {
    cosmic_mime_apps::list_paths()
}

/// Fallback launcher when a desktop entry cannot be resolved from the registry.
fn fallback_launch(desktop_id: &str, file: &Path) -> anyhow::Result<()> {
    // Try gtk-launch with the explicit desktop id first.
    if let Ok(mut child) = Command::new("gtk-launch").arg(desktop_id).arg(file).spawn() {
        let _ = child.wait();
        return Ok(());
    }

    // Fall back to xdg-open to avoid total failure.
    Command::new("xdg-open").arg(file).spawn().map_err(|e| {
        anyhow::anyhow!(
            "Desktop entry not found: {}. xdg-open failed: {}",
            desktop_id,
            e
        )
    })?;
    Ok(())
}

fn read_exec_line(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Exec=") {
            return Some(trimmed.trim_start_matches("Exec=").trim().to_string());
        }
    }
    None
}
