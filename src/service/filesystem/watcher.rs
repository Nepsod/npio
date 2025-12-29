//! File system change watcher.

use crate::service::filesystem::error::FileSystemError;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// A change detected in the filesystem.
#[derive(Debug, Clone)]
pub enum FileSystemChange {
    /// A new file or directory was created.
    Created(PathBuf),
    /// An existing file or directory was modified.
    Modified(PathBuf),
    /// A file or directory was removed.
    Removed(PathBuf),
    /// A file or directory was renamed.
    Renamed {
        /// Old path.
        old: PathBuf,
        /// New path.
        new: PathBuf,
    },
}

/// Watches the filesystem for changes.
pub struct FileSystemWatcher {
    watcher: RecommendedWatcher,
    event_rx: mpsc::Receiver<notify::Result<Event>>,
}

impl FileSystemWatcher {
    /// Create a new file system watcher.
    pub fn new() -> Result<Self, FileSystemError> {
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(tx)?;

        Ok(Self {
            watcher,
            event_rx: rx,
        })
    }

    /// Start watching a path for changes.
    pub fn watch(&mut self, path: &Path) -> Result<(), FileSystemError> {
        self.watcher.watch(path, RecursiveMode::NonRecursive)?;
        Ok(())
    }

    /// Stop watching a path.
    pub fn unwatch(&mut self, path: &Path) -> Result<(), FileSystemError> {
        self.watcher.unwatch(path)?;
        Ok(())
    }

    /// Poll for filesystem events (non-blocking).
    ///
    /// Returns all pending events since the last call.
    pub fn poll_events(&self) -> Vec<FileSystemChange> {
        let mut changes = Vec::new();

        // Try to receive all pending events
        while let Ok(Ok(event)) = self.event_rx.try_recv() {
            changes.extend(Self::convert_event(event));
        }

        changes
    }

    /// Convert a notify Event into FileSystemChange events.
    fn convert_event(event: Event) -> Vec<FileSystemChange> {
        let mut changes = Vec::new();

        match event.kind {
            EventKind::Create(_) => {
                for path in event.paths {
                    changes.push(FileSystemChange::Created(path));
                }
            },
            EventKind::Modify(kind) => {
                use notify::event::ModifyKind;
                match kind {
                    ModifyKind::Name(_) => {
                        // Rename events have two paths: old and new
                        if event.paths.len() >= 2 {
                            let old = event.paths[0].clone();
                            let new = event.paths[1].clone();
                            changes.push(FileSystemChange::Renamed { old, new });
                        } else if event.paths.len() == 1 {
                            // Single path rename - treat as modified
                            changes.push(FileSystemChange::Modified(event.paths[0].clone()));
                        }
                    },
                    _ => {
                        for path in event.paths {
                            changes.push(FileSystemChange::Modified(path));
                        }
                    },
                }
            },
            EventKind::Remove(_) => {
                for path in event.paths {
                    changes.push(FileSystemChange::Removed(path));
                }
            },
            EventKind::Access(_) => {
                // Access events are treated as modifications
                for path in event.paths {
                    changes.push(FileSystemChange::Modified(path));
                }
            },
            EventKind::Other => {
                // Other event kinds are treated as modifications
                for path in event.paths {
                    changes.push(FileSystemChange::Modified(path));
                }
            },
            EventKind::Any => {
                // Any event kind - treat as modifications
                for path in event.paths {
                    changes.push(FileSystemChange::Modified(path));
                }
            },
        }

        changes
    }
}
