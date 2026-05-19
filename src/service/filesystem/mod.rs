//! Filesystem model for nptk, similar to Qt6's QFileSystemModel.
//!
//! Provides async filesystem operations, lazy loading, automatic file watching,
//! caching, and icon support for file manager widgets, file chooser dialogs,
//! and desktop widgets.

pub mod mime_detector;
pub mod mime_registry;
pub mod watcher;
pub mod io_uring;
pub mod error;
pub mod icon;
pub mod trash;

// Re-export public API
pub use mime_detector::MimeDetector;
pub use mime_registry::MimeRegistry;
pub use watcher::FileSystemWatcher;
pub use error::FileSystemError;
pub use trash::{
    TrashEntry, delete_permanently as trash_delete_permanently, empty_trash,
    list_items as trash_list_items, restore as trash_restore, trash_files_directory,
};