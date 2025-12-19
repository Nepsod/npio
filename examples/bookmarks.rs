//! Example: Manage bookmarks
//!
//! This example demonstrates how to use the BookmarksService to manage file bookmarks.

use npio::service::bookmarks::BookmarksService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create bookmarks service
    let mut service = BookmarksService::new();

    // Load existing bookmarks
    service.load().await?;

    println!("Current bookmarks:");
    for bookmark in service.get_bookmarks() {
        let name = bookmark.name.as_ref()
            .map(|n| n.as_str())
            .unwrap_or("(no name)");
        println!("  {} - {}", name, bookmark.uri);
    }

    // Add a new bookmark
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let home_uri = format!("file://{}", home);
    
    if !service.has_bookmark(&home_uri) {
        service.add_bookmark(home_uri.clone(), Some("Home Directory".to_string()));
        println!("\nAdded bookmark: Home Directory");
    }

    // Save bookmarks
    service.save().await?;
    println!("Bookmarks saved!");

    Ok(())
}

