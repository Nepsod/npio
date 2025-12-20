//! Services for common filesystem operations
//!
//! Provides high-level services:
//! - User directory helpers: GIO-compatible functions for user directories (home, documents, etc.)
//! - BookmarksService: GTK bookmarks management
//! - ThumbnailService: Thumbnail generation and caching

pub mod places;
pub mod bookmarks;
pub mod thumbnail;
pub mod volumemonitor;
