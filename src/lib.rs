//! # npio - Nepsod Input-Output
//!
//! A Rust-native library inspired by GIO, focused on Linux. Provides unified filesystem,
//! device, and I/O abstractions.
//!
//! ## Overview
//!
//! npio provides a GIO-inspired API for filesystem operations, device management, and I/O
//! operations in Rust. It's designed to be async-first using Tokio and follows the same
//! architectural patterns as GLib's GIO library.
//!
//! ## Core Concepts
//!
//! - **File**: URI-based file handles that abstract over different backends
//! - **FileInfo**: Metadata bag with attribute system (standard::*, time::*, etc.)
//! - **Backend**: Pluggable backends for different URI schemes (file://, etc.)
//! - **Services**: High-level services like Bookmarks and Thumbnails
//! - **Jobs**: Async operations like copy, move, delete, trash with progress reporting
//!
//! ## Example
//!
//! ```no_run
//! use npio::{get_file_for_uri, register_backend};
//! use npio::backend::local::LocalBackend;
//! use std::sync::Arc;
//!
//! # async fn example() -> npio::NpioResult<()> {
//! // Register local filesystem backend
//! let backend = Arc::new(LocalBackend::new());
//! register_backend(backend);
//!
//! // Get a file handle
//! let file = get_file_for_uri("file:///home/user/document.txt")?;
//!
//! // Query file information
//! let info = file.query_info("standard::*,time::modified", None).await?;
//! println!("File: {}", info.get_name().unwrap_or("unknown"));
//! println!("Size: {} bytes", info.get_size());
//!
//! // Read file contents
//! let mut input = file.read(None).await?;
//! let mut contents = Vec::new();
//! tokio::io::AsyncReadExt::read_to_end(&mut input, &mut contents).await?;
//! # Ok(())
//! # }
//! ```

pub mod backend;
pub mod cancellable;
pub mod drive;
pub mod error;
pub mod file;
pub mod file_enumerator;
pub mod file_info;
pub mod iostream;
pub mod job;
pub mod metadata;
pub mod model;
pub mod monitor;
pub mod mount;
pub mod service;
pub mod volume;

pub use backend::{Backend, BackendRegistry, get_file_for_uri, register_backend};
pub use backend::mount::MountBackend;
pub use backend::udisks2::UDisks2Backend;
pub use cancellable::Cancellable;
pub use drive::Drive;
pub use error::{NpioError, NpioResult, IOErrorEnum};
pub use file::{File, FileQueryInfoFlags};
pub use file_enumerator::FileEnumerator;
pub use file_info::{FileInfo, FileAttributeType, FileType};
pub use iostream::{InputStream, OutputStream};
pub use metadata::MimeResolver;
pub use model::directory::{DirectoryModel, DirectoryUpdate};
pub use model::devices::DevicesModel;
pub use monitor::{FileMonitor, FileMonitorEvent};
pub use mount::Mount;
pub use job::{CopyFlags, ProgressCallback, trash};
pub use service::places::{get_home_file, get_user_special_file, get_home_icon_name, get_directory_icon_name, UserDirectory};
pub use service::bookmarks::{BookmarksService, Bookmark};
pub use service::thumbnail::{ThumbnailService, ThumbnailEvent, ThumbnailImage, ThumbnailImageCache};
pub use service::volumemonitor::{VolumeMonitor, VolumeMonitorEvent};
pub use backend::thumbnail::{ThumbnailBackend, ThumbnailSize};
pub use volume::Volume;
