//! Example: Using user directory helpers (GIO-compatible)
//!
//! This example demonstrates how to use the user directory helper functions
//! to get common directory locations, following GIO's pattern.

use npio::service::places::{get_home_file, get_user_special_file, UserDirectory};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get home directory (separate from special directories in GIO)
    match get_home_file() {
        Ok(home_file) => {
            println!("Home directory: {}", home_file.uri());
        }
        Err(e) => {
            eprintln!("Failed to get home directory: {}", e);
        }
    }

    // Get special directories (matching GIO's g_get_user_special_dir())
    let directories = [
        UserDirectory::Desktop,
        UserDirectory::Documents,
        UserDirectory::Download,
        UserDirectory::Music,
        UserDirectory::Pictures,
        UserDirectory::Videos,
        UserDirectory::PublicShare,
        UserDirectory::Templates,
    ];

    println!("\nSpecial User Directories:");
    for dir in &directories {
        match get_user_special_file(*dir) {
            Ok(Some(file)) => {
                println!("  {:?}: {}", dir, file.uri());
            }
            Ok(None) => {
                println!("  {:?}: (not available)", dir);
            }
            Err(e) => {
                eprintln!("  {:?}: Error - {}", dir, e);
            }
        }
    }

    Ok(())
}
