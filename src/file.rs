//! File abstraction trait
//!
//! The `File` trait provides a unified interface for file operations across different backends.
//! It mirrors GIO's GFile interface, providing async methods for common file operations.

pub mod local;

use async_trait::async_trait;
use bitflags::bitflags;
use crate::cancellable::Cancellable;
use crate::error::NpioResult;
use crate::file_info::{FileInfo, FileAttributeType};
use crate::iostream::{InputStream, OutputStream};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FileQueryInfoFlags: u32 {
        const NONE = 0;
        const NOFOLLOW_SYMLINKS = 1;
    }
}

#[async_trait]
pub trait File: Send + Sync + std::fmt::Debug {
    /// Gets the URI for this file.
    fn uri(&self) -> String;

    /// Gets the base name (filename) of the file.
    fn basename(&self) -> String;

    /// Gets the parent file, if it exists.
    fn parent(&self) -> Option<Box<dyn File>>;

    /// Gets a child file with the given name.
    fn child(&self, name: &str) -> Box<dyn File>;

    /// Queries information about the file.
    /// `attributes` is a comma-separated list of attributes to query (e.g. "standard::*,time::modified").
    async fn query_info(&self, attributes: &str, cancellable: Option<&Cancellable>) -> NpioResult<FileInfo>;

    /// Opens the file for reading.
    async fn read(&self, cancellable: Option<&Cancellable>) -> NpioResult<Box<dyn InputStream>>;

    /// Opens the file for writing (creates or overwrites).
    async fn replace(
        &self,
        etag: Option<&str>,
        make_backup: bool,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<dyn OutputStream>>;
    
    /// Creates a new file for writing. Fails if it already exists.
    async fn create_file(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<dyn OutputStream>>;

    /// Appends to the file.
    async fn append_to(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<dyn OutputStream>>;

    /// Deletes the file.
    async fn delete(&self, cancellable: Option<&Cancellable>) -> NpioResult<()>;

    /// Makes a directory.
    async fn make_directory(&self, cancellable: Option<&Cancellable>) -> NpioResult<()>;

    /// Enumerates children of this directory.
    async fn enumerate_children(
        &self,
        attributes: &str,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<dyn crate::file_enumerator::FileEnumerator>>;

    /// Moves the file to a new location.
    async fn move_to(
        &self,
        destination: &dyn File,
        flags: crate::job::CopyFlags,
        cancellable: Option<&Cancellable>,
        progress_callback: Option<crate::job::ProgressCallback>,
    ) -> NpioResult<()>;
    
    /// Copies the file to a new location.
    async fn copy(
        &self,
        destination: &dyn File,
        flags: crate::job::CopyFlags,
        cancellable: Option<&Cancellable>,
        progress_callback: Option<crate::job::ProgressCallback>,
    ) -> NpioResult<()>;

    /// Checks if the file exists.
    async fn exists(&self, cancellable: Option<&Cancellable>) -> NpioResult<bool>;

    /// Monitors the file or directory for changes.
    async fn monitor(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Box<crate::monitor::FileMonitor>>;

    /// Moves the file to the trash.
    async fn trash(&self, cancellable: Option<&Cancellable>) -> NpioResult<()>;

    /// Queries filesystem-level information (free space, type, etc.)
    async fn query_filesystem_info(
        &self,
        attributes: &str,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<FileInfo>;

    /// Sets file attributes from a FileInfo object
    async fn set_attributes_from_info(
        &self,
        info: &FileInfo,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<FileInfo>;

    /// Sets a single file attribute
    async fn set_attribute(
        &self,
        attribute: &str,
        value: &FileAttributeType,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Sets a string attribute
    async fn set_attribute_string(
        &self,
        attribute: &str,
        value: &str,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Sets a byte string attribute
    async fn set_attribute_byte_string(
        &self,
        attribute: &str,
        value: &str,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Sets a boolean attribute
    async fn set_attribute_boolean(
        &self,
        attribute: &str,
        value: bool,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Sets a uint32 attribute
    async fn set_attribute_uint32(
        &self,
        attribute: &str,
        value: u32,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Sets an int32 attribute
    async fn set_attribute_int32(
        &self,
        attribute: &str,
        value: i32,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Sets a uint64 attribute
    async fn set_attribute_uint64(
        &self,
        attribute: &str,
        value: u64,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Sets an int64 attribute
    async fn set_attribute_int64(
        &self,
        attribute: &str,
        value: i64,
        flags: FileQueryInfoFlags,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;
}
