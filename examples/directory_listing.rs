//! Example: List directory contents
//!
//! This example demonstrates how to enumerate directory contents and monitor for changes.

use std::sync::Arc;
use npio::backend::local::LocalBackend;
use npio::{get_file_for_uri, register_backend};
use npio::model::directory::DirectoryModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Register local filesystem backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let dir_path = if args.len() > 1 {
        args[1].clone()
    } else {
        std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
    };

    let dir_uri = format!("file://{}", dir_path);
    let dir_file = get_file_for_uri(&dir_uri)?;

    // Create directory model
    let model = DirectoryModel::new(dir_file);
    
    // Load directory contents
    model.load(None).await?;

    // List files
    println!("Files in {}:", dir_path);
    for file_info in model.files() {
        let name = file_info.get_name().unwrap_or("unknown");
        let size = file_info.get_size();
        let file_type = match file_info.get_file_type() {
            npio::FileType::Directory => "DIR",
            npio::FileType::Regular => "FILE",
            npio::FileType::SymbolicLink => "LINK",
            _ => "OTHER",
        };
        println!("  [{}] {} ({} bytes)", file_type, name, size);
    }

    // Subscribe to updates
    let mut receiver = model.subscribe();
    println!("\nMonitoring for changes (press Ctrl+C to exit)...");

    // Listen for a few updates (in a real app, this would run indefinitely)
    tokio::spawn(async move {
        let mut count = 0;
        while let Ok(update) = receiver.recv().await {
            match update {
                npio::DirectoryUpdate::Added(info) => {
                    println!("  [+] Added: {}", info.get_name().unwrap_or("unknown"));
                }
                npio::DirectoryUpdate::Removed(info) => {
                    println!("  [-] Removed: {}", info.get_name().unwrap_or("unknown"));
                }
                npio::DirectoryUpdate::Changed(info) => {
                    println!("  [~] Changed: {}", info.get_name().unwrap_or("unknown"));
                }
                npio::DirectoryUpdate::Initial(_) => {
                    // Already handled above
                }
            }
            count += 1;
            if count >= 5 {
                break; // Limit updates for example
            }
        }
    });

    // Wait a bit
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    Ok(())
}

