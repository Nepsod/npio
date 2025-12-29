// SPDX-License-Identifier: MIT OR Apache-2.0

//! Async I/O helpers using smol::unblock for non-blocking file operations

use std::io;
use std::path::Path;

/// Read the entire contents of a file into a bytes vector asynchronously.
///
/// This function uses `smol::unblock` to execute `std::fs::read` without blocking
/// the async runtime, making it safe to use within tokio or other async runtimes.
pub async fn read_file(path: &Path) -> io::Result<Vec<u8>> {
    let path = path.to_path_buf();
    smol::unblock(move || std::fs::read(path)).await
}

/// Read the entire contents of a file into a string asynchronously.
///
/// This function uses `smol::unblock` to execute `std::fs::read_to_string` without blocking
/// the async runtime, making it safe to use within tokio or other async runtimes.
pub async fn read_file_to_string(path: &Path) -> io::Result<String> {
    let path = path.to_path_buf();
    smol::unblock(move || std::fs::read_to_string(path)).await
}

/// Copy a file from one location to another asynchronously.
///
/// This function uses `smol::unblock` to execute `std::fs::copy` without blocking
/// the async runtime, making it safe to use within tokio or other async runtimes.
pub async fn copy_file(from: &Path, to: &Path) -> io::Result<u64> {
    let from = from.to_path_buf();
    let to = to.to_path_buf();
    smol::unblock(move || std::fs::copy(from, to)).await
}
