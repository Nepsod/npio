use std::path::{Path, PathBuf};
use std::{env, io};

use futures::future;
use uring_file::fs as urfs;
use uring_file::metadata::Metadata;

use crate::service::io_helpers;

/// Simple runtime toggle: set `NPTK_USE_IO_URING=0` to disable.
fn enabled() -> bool {
    env::var("NPTK_USE_IO_URING")
        .map(|v| v != "0")
        .unwrap_or(true)
}

/// Get metadata via io_uring.
pub async fn stat(path: &Path) -> io::Result<Metadata> {
    if !enabled() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "io_uring disabled via NPTK_USE_IO_URING",
        ));
    }
    urfs::metadata(path).await
}

/// Read the whole file into bytes via io_uring, fallback to async file read.
pub async fn read(path: &Path) -> io::Result<Vec<u8>> {
    if enabled() {
        if let Ok(bytes) = urfs::read(path).await {
            return Ok(bytes);
        }
    }
    io_helpers::read_file(path).await
}

/// Read the whole file into string via io_uring, fallback to async file read.
pub async fn read_to_string(path: &Path) -> io::Result<String> {
    if enabled() {
        if let Ok(text) = urfs::read_to_string(path).await {
            return Ok(text);
        }
    }
    io_helpers::read_file_to_string(path).await
}

/// Batch statx operations for multiple paths concurrently.
/// Returns a Vec of Results in the same order as the input paths.
pub async fn stat_batch(paths: &[PathBuf]) -> Vec<io::Result<Metadata>> {
    if !enabled() {
        return paths
            .iter()
            .map(|_| {
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    "io_uring disabled via NPTK_USE_IO_URING",
                ))
            })
            .collect();
    }

    let futures: Vec<_> = paths.iter().map(|path| urfs::metadata(path)).collect();
    future::join_all(futures).await
}
