use npio::service::bookmarks::BookmarksService;
use std::fs;

#[tokio::test]
async fn test_bookmarks_service_new() {
    let service = BookmarksService::new();
    let bookmarks = service.get_bookmarks();
    assert_eq!(bookmarks.len(), 0);
}

#[tokio::test]
async fn test_bookmarks_add_and_get() {
    let mut service = BookmarksService::new();
    
    // Add a bookmark
    service.add_bookmark("file:///home/user/Documents".to_string(), Some("Documents".to_string()));
    
    // Get bookmarks
    let bookmarks = service.get_bookmarks();
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0].uri, "file:///home/user/Documents");
    assert_eq!(bookmarks[0].name, Some("Documents".to_string()));
}

#[tokio::test]
async fn test_bookmarks_has_bookmark() {
    let mut service = BookmarksService::new();
    
    let uri = "file:///home/user/Documents".to_string();
    assert!(!service.has_bookmark(&uri));
    
    service.add_bookmark(uri.clone(), Some("Documents".to_string()));
    assert!(service.has_bookmark(&uri));
}

#[tokio::test]
async fn test_bookmarks_remove() {
    let mut service = BookmarksService::new();
    
    let uri = "file:///home/user/Documents".to_string();
    service.add_bookmark(uri.clone(), Some("Documents".to_string()));
    assert_eq!(service.get_bookmarks().len(), 1);
    
    let removed = service.remove_bookmark(&uri);
    assert!(removed);
    assert_eq!(service.get_bookmarks().len(), 0);
    assert!(!service.has_bookmark(&uri));
}

#[tokio::test]
async fn test_bookmarks_save_and_load() {
    // Create a temporary bookmarks file
    let test_dir = std::env::temp_dir().join("npio_bookmarks_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).unwrap();
    }
    fs::create_dir_all(&test_dir).unwrap();
    
    let bookmarks_path = test_dir.join("bookmarks");
    
    // Create service with custom path
    let mut service = BookmarksService::with_path(bookmarks_path.clone());
    
    // Add some bookmarks
    service.add_bookmark("file:///home/user/Documents".to_string(), Some("Documents".to_string()));
    service.add_bookmark("file:///home/user/Downloads".to_string(), Some("Downloads".to_string()));
    service.add_bookmark("file:///home/user/Music".to_string(), None);
    
    // Save bookmarks
    service.save().await.expect("Failed to save bookmarks");
    
    // Verify file exists and has content
    assert!(bookmarks_path.exists());
    let content = fs::read_to_string(&bookmarks_path).unwrap();
    assert!(content.contains("file:///home/user/Documents Documents"));
    assert!(content.contains("file:///home/user/Downloads Downloads"));
    assert!(content.contains("file:///home/user/Music"));
    
    // Create new service and load
    let mut service2 = BookmarksService::with_path(bookmarks_path.clone());
    service2.load().await.expect("Failed to load bookmarks");
    
    // Verify loaded bookmarks
    let bookmarks = service2.get_bookmarks();
    assert_eq!(bookmarks.len(), 3);
    
    // Verify specific bookmark
    let docs_bookmark = service2.get_bookmark("file:///home/user/Documents");
    assert!(docs_bookmark.is_some());
    assert_eq!(docs_bookmark.unwrap().name, Some("Documents".to_string()));
    
    // Cleanup
    fs::remove_dir_all(&test_dir).unwrap();
}

#[tokio::test]
async fn test_bookmarks_load_gtk_format() {
    // Create a temporary bookmarks file in GTK format
    let test_dir = std::env::temp_dir().join("npio_bookmarks_test_gtk");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).unwrap();
    }
    fs::create_dir_all(&test_dir).unwrap();
    
    let bookmarks_path = test_dir.join("bookmarks");
    
    // Write GTK format bookmarks
    let gtk_content = "file:///home/user/Documents Documents\nfile:///home/user/Downloads\n# This is a comment\nfile:///home/user/Music Music Folder\n";
    fs::write(&bookmarks_path, gtk_content).unwrap();
    
    // Load bookmarks
    let mut service = BookmarksService::with_path(bookmarks_path.clone());
    service.load().await.expect("Failed to load bookmarks");
    
    // Verify loaded bookmarks (should skip comments and empty lines)
    let bookmarks = service.get_bookmarks();
    assert_eq!(bookmarks.len(), 3);
    
    // Verify first bookmark has label
    let docs = service.get_bookmark("file:///home/user/Documents");
    assert!(docs.is_some());
    assert_eq!(docs.unwrap().name, Some("Documents".to_string()));
    
    // Verify second bookmark has no label
    let downloads = service.get_bookmark("file:///home/user/Downloads");
    assert!(downloads.is_some());
    assert_eq!(downloads.unwrap().name, None);
    
    // Verify third bookmark has label
    let music = service.get_bookmark("file:///home/user/Music");
    assert!(music.is_some());
    assert_eq!(music.unwrap().name, Some("Music Folder".to_string()));
    
    // Cleanup
    fs::remove_dir_all(&test_dir).unwrap();
}

#[tokio::test]
async fn test_bookmarks_load_nonexistent_file() {
    // Create service with path to non-existent file
    let test_dir = std::env::temp_dir().join("npio_bookmarks_test_nonexistent");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).unwrap();
    }
    
    let bookmarks_path = test_dir.join("nonexistent_bookmarks");
    
    let mut service = BookmarksService::with_path(bookmarks_path.clone());
    
    // Loading non-existent file should succeed (empty bookmarks)
    service.load().await.expect("Loading non-existent file should succeed");
    assert_eq!(service.get_bookmarks().len(), 0);
}

#[tokio::test]
async fn test_bookmarks_duplicate_uri() {
    let mut service = BookmarksService::new();
    
    let uri = "file:///home/user/Documents".to_string();
    
    // Add bookmark twice
    service.add_bookmark(uri.clone(), Some("Documents".to_string()));
    service.add_bookmark(uri.clone(), Some("My Documents".to_string()));
    
    // Should only have one bookmark (last one wins)
    let bookmarks = service.get_bookmarks();
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0].name, Some("My Documents".to_string()));
}

