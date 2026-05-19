//! FreeDesktop trash can inspection and management (`~/.local/share/Trash`).
//!
//! See <https://specifications.freedesktop.org/trash-spec/trashspec-latest.html>.

use crate::error::{IOErrorEnum, NpioError, NpioResult};
use percent_encoding::percent_decode_str;
use std::path::PathBuf;

/// One item currently stored in the trash.
#[derive(Debug, Clone)]
pub struct TrashEntry {
    /// Basename of the file under `Trash/files` (unique key within trash).
    pub trash_basename: String,
    /// Full path to the data file in `Trash/files`.
    pub trash_file_path: PathBuf,
    /// Corresponding `.trashinfo` path under `Trash/info`.
    pub trash_info_path: PathBuf,
    /// Original absolute path before trashing (decoded from trashinfo).
    pub original_path: PathBuf,
    /// Raw `DeletionDate` line from trashinfo, if present.
    pub deletion_date: Option<String>,
    /// File size in bytes (the file in `Trash/files`).
    pub size: u64,
}

fn trash_dirs() -> NpioResult<(PathBuf, PathBuf)> {
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            directories::ProjectDirs::from("", "", "").map(|dirs| dirs.data_dir().to_path_buf())
        })
        .ok_or_else(|| NpioError::new(IOErrorEnum::Failed, "Could not determine XDG_DATA_HOME"))?;

    let trash_root = data_home.join("Trash");
    Ok((trash_root.join("files"), trash_root.join("info")))
}

fn parse_trashinfo(content: &str) -> Option<(PathBuf, Option<String>)> {
    let mut original: Option<PathBuf> = None;
    let mut deletion: Option<String> = None;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("Path=") {
            let decoded = percent_decode_str(rest.trim())
                .decode_utf8()
                .ok()?
                .into_owned();
            original = Some(PathBuf::from(decoded));
        } else if let Some(rest) = line.strip_prefix("DeletionDate=") {
            deletion = Some(rest.trim().to_string());
        }
    }
    Some((original?, deletion))
}

/// List all entries currently in the trash.
pub fn list_items() -> NpioResult<Vec<TrashEntry>> {
    let (files_dir, info_dir) = trash_dirs()?;
    let mut out = Vec::new();
    if !files_dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&files_dir).map_err(|e| {
        NpioError::new(
            IOErrorEnum::Failed,
            format!("list trash files: {}", e),
        )
    })? {
        let entry = entry.map_err(|e| {
            NpioError::new(IOErrorEnum::Failed, format!("trash entry: {}", e))
        })?;
        let trash_file_path = entry.path();
        let trash_basename = entry.file_name().to_string_lossy().into_owned();
        let info_name = format!("{}.trashinfo", trash_basename);
        let trash_info_path = info_dir.join(&info_name);
        let (original_path, deletion_date) = if trash_info_path.is_file() {
            let content = std::fs::read_to_string(&trash_info_path).map_err(|e| {
                NpioError::new(
                    IOErrorEnum::Failed,
                    format!("read trashinfo {}: {}", trash_info_path.display(), e),
                )
            })?;
            if let Some(parsed) = parse_trashinfo(&content) {
                parsed
            } else {
                continue;
            }
        } else {
            continue;
        };
        let size = std::fs::metadata(&trash_file_path)
            .map(|m| m.len())
            .unwrap_or(0);
        out.push(TrashEntry {
            trash_basename,
            trash_file_path,
            trash_info_path,
            original_path,
            deletion_date,
            size,
        });
    }
    Ok(out)
}

/// Restore a trashed item to its original path (overwrites destination if it exists).
pub fn restore(entry: &TrashEntry) -> NpioResult<()> {
    if let Some(parent) = entry.original_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            NpioError::new(
                IOErrorEnum::Failed,
                format!("mkdir restore parent: {}", e),
            )
        })?;
    }
    std::fs::rename(&entry.trash_file_path, &entry.original_path).map_err(|e| {
        NpioError::new(
            IOErrorEnum::Failed,
            format!("restore rename: {}", e),
        )
    })?;
    if entry.trash_info_path.is_file() {
        let _ = std::fs::remove_file(&entry.trash_info_path);
    }
    Ok(())
}

/// Permanently delete one trashed item (file in `Trash/files` and its `.trashinfo`).
pub fn delete_permanently(entry: &TrashEntry) -> NpioResult<()> {
    if entry.trash_file_path.is_dir() {
        std::fs::remove_dir_all(&entry.trash_file_path).map_err(|e| {
            NpioError::new(
                IOErrorEnum::Failed,
                format!("delete trash dir: {}", e),
            )
        })?;
    } else {
        std::fs::remove_file(&entry.trash_file_path).map_err(|e| {
            NpioError::new(
                IOErrorEnum::Failed,
                format!("delete trash file: {}", e),
            )
        })?;
    }
    if entry.trash_info_path.is_file() {
        let _ = std::fs::remove_file(&entry.trash_info_path);
    }
    Ok(())
}

/// Remove every item from the trash directories.
pub fn empty_trash() -> NpioResult<()> {
    let (files_dir, info_dir) = trash_dirs()?;
    if files_dir.is_dir() {
        for entry in std::fs::read_dir(&files_dir).map_err(|e| {
            NpioError::new(IOErrorEnum::Failed, format!("read trash files: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                NpioError::new(IOErrorEnum::Failed, format!("{}", e))
            })?;
            let p = entry.path();
            if p.is_dir() {
                std::fs::remove_dir_all(&p).ok();
            } else {
                std::fs::remove_file(&p).ok();
            }
        }
    }
    if info_dir.is_dir() {
        for entry in std::fs::read_dir(&info_dir).map_err(|e| {
            NpioError::new(IOErrorEnum::Failed, format!("read trash info: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                NpioError::new(IOErrorEnum::Failed, format!("{}", e))
            })?;
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("trashinfo") {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    Ok(())
}

/// Resolve the trash file directory (`Trash/files`).
pub fn trash_files_directory() -> NpioResult<PathBuf> {
    Ok(trash_dirs()?.0)
}
