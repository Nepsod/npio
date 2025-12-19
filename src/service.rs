//! Services for common filesystem operations
//!
//! Provides high-level services:
//! - PlacesService: XDG user directories (home, documents, etc.)
//! - BookmarksService: GTK bookmarks management
//! - ThumbnailService: Thumbnail generation and caching

pub mod places;
pub mod bookmarks;
pub mod thumbnail;
