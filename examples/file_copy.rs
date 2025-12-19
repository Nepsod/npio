//! Example: Copy a file with progress reporting
//!
//! This example demonstrates how to copy a file using npio's job API with progress callbacks.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use npio::backend::local::LocalBackend;
use npio::{get_file_for_uri, register_backend, CopyFlags, job};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Register local filesystem backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <source> <destination>", args[0]);
        std::process::exit(1);
    }

    let source_uri = format!("file://{}", args[1]);
    let dest_uri = format!("file://{}", args[2]);

    // Get file handles
    let source_file = get_file_for_uri(&source_uri)?;
    let dest_file = get_file_for_uri(&dest_uri)?;

    // Track progress
    let bytes_copied = Arc::new(AtomicU64::new(0));
    let bytes_copied_clone = bytes_copied.clone();

    // Copy with progress callback
    job::copy(
        &*source_file,
        &*dest_file,
        CopyFlags::OVERWRITE,
        Some(Box::new(move |current, total| {
            bytes_copied_clone.store(current, Ordering::SeqCst);
            if total > 0 {
                let percent = (current * 100) / total;
                println!("Progress: {}% ({}/{} bytes)", percent, current, total);
            } else {
                println!("Progress: {} bytes", current);
            }
        })),
        None,
    ).await?;

    println!("Copy completed! Total bytes: {}", bytes_copied.load(Ordering::SeqCst));
    Ok(())
}

