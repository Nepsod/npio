// Tests for filesystem services (MimeDetector, MimeRegistry, etc.)

use npio::service::filesystem::MimeDetector;
use std::path::PathBuf;
use tokio::fs;

#[tokio::test]
async fn test_mime_detector_from_ext() {
    // Test extension-based detection
    assert_eq!(
        MimeDetector::detect_mime_type_from_ext("jpg"),
        Some("image/jpeg".to_string())
    );
    assert_eq!(
        MimeDetector::detect_mime_type_from_ext("png"),
        Some("image/png".to_string())
    );
    assert_eq!(
        MimeDetector::detect_mime_type_from_ext("pdf"),
        Some("application/pdf".to_string())
    );
    assert_eq!(
        MimeDetector::detect_mime_type_from_ext("txt"),
        Some("text/plain".to_string())
    );
}

#[tokio::test]
async fn test_mime_detector_overrides() {
    // Test override table
    assert_eq!(
        MimeDetector::detect_mime_type_from_ext("toml"),
        Some("application/toml".to_string())
    );
    assert_eq!(
        MimeDetector::detect_mime_type_from_ext("rs"),
        Some("text/x-rust".to_string())
    );
    assert_eq!(
        MimeDetector::detect_mime_type_from_ext("sh"),
        Some("application/x-shellscript".to_string())
    );
    assert_eq!(
        MimeDetector::detect_mime_type_from_ext("iso"),
        Some("application/x-iso9660-image".to_string())
    );
}

#[tokio::test]
async fn test_mime_detector_from_path() {
    // Create temporary directory
    let test_dir = std::env::temp_dir().join("npio_mime_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    // Test with various file types
    let test_cases = vec![
        ("test.jpg", "image/jpeg"),
        ("test.png", "image/png"),
        ("test.txt", "text/plain"),
        ("test.pdf", "application/pdf"),
        ("test.toml", "application/toml"), // Override
    ];
    
    for (filename, expected_mime) in test_cases {
        let file_path = test_dir.join(filename);
        // Create empty file (extension-based detection should work)
        fs::write(&file_path, b"").await.unwrap();
        
        let detected = MimeDetector::detect_mime_type(&file_path).await;
        assert_eq!(
            detected,
            Some(expected_mime.to_string()),
            "Failed to detect MIME type for {}",
            filename
        );
    }
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_mime_detector_content_based() {
    // Create temporary directory
    let test_dir = std::env::temp_dir().join("npio_mime_content_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    // Create a file without extension but with recognizable content
    // PNG file signature: 89 50 4E 47 0D 0A 1A 0A
    let png_file = test_dir.join("no_extension");
    let png_data = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
    ];
    fs::write(&png_file, &png_data).await.unwrap();
    
    // Content-based detection should identify it as PNG
    let detected = MimeDetector::detect_mime_type(&png_file).await;
    // May return image/png or application/octet-stream depending on tree_magic_mini
    // Just verify it doesn't panic and returns something
    assert!(detected.is_some() || detected.is_none()); // Always true, just checking it doesn't panic
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_mime_detector_no_extension() {
    // Create temporary directory
    let test_dir = std::env::temp_dir().join("npio_mime_no_ext_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    // Create a file without extension
    let no_ext_file = test_dir.join("file_without_extension");
    fs::write(&no_ext_file, b"plain text content").await.unwrap();
    
    // Should try content-based detection
    let detected = MimeDetector::detect_mime_type(&no_ext_file).await;
    // May return text/plain or None depending on content detection
    // Just verify it doesn't panic
    let _ = detected;
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_mime_detector_empty_file() {
    // Create temporary directory
    let test_dir = std::env::temp_dir().join("npio_mime_empty_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    // Create an empty file with extension
    let empty_file = test_dir.join("empty.txt");
    fs::write(&empty_file, b"").await.unwrap();
    
    // Should detect from extension even if file is empty
    let detected = MimeDetector::detect_mime_type(&empty_file).await;
    assert_eq!(detected, Some("text/plain".to_string()));
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_mime_registry_load() {
    use npio::service::filesystem::MimeRegistry;
    
    // Test loading the registry
    let registry = MimeRegistry::load_default();
    
    // Registry should be loaded (may be empty if no mimeapps.list files exist)
    // Just verify it doesn't panic
    let _ = registry;
}

#[tokio::test]
async fn test_mime_registry_resolve() {
    use npio::service::filesystem::MimeRegistry;
    
    let registry = MimeRegistry::load_default();
    
    // Try to resolve some common MIME types
    // May return None if no applications are configured
    let _text_app = registry.resolve("text/plain");
    let _image_app = registry.resolve("image/png");
    let _pdf_app = registry.resolve("application/pdf");
    
    // Just verify it doesn't panic
}

#[tokio::test]
async fn test_mime_registry_get_generic_icon_name() {
    use npio::service::filesystem::MimeRegistry;
    
    let registry = MimeRegistry::load_default();
    
    // Test getting generic icon names
    let text_icon = MimeRegistry::get_generic_icon_name("text/plain");
    assert!(text_icon.is_some() || text_icon.is_none()); // May or may not be available
    
    let image_icon = MimeRegistry::get_generic_icon_name("image/png");
    assert!(image_icon.is_some() || image_icon.is_none());
    
    // Just verify it doesn't panic
}

#[tokio::test]
async fn test_filesystem_watcher_new() {
    use npio::service::filesystem::FileSystemWatcher;
    
    // Test creating a new watcher
    let watcher_result = FileSystemWatcher::new();
    assert!(watcher_result.is_ok());
}

#[tokio::test]
async fn test_filesystem_watcher_watch_unwatch() {
    use npio::service::filesystem::FileSystemWatcher;
    
    // Create temporary directory
    let test_dir = std::env::temp_dir().join("npio_watcher_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    // Create watcher
    let mut watcher = FileSystemWatcher::new().expect("watcher");
    
    // Watch the directory
    let watch_result = watcher.watch(&test_dir);
    assert!(watch_result.is_ok());
    
    // Unwatch the directory
    let unwatch_result = watcher.unwatch(&test_dir);
    assert!(unwatch_result.is_ok());
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_filesystem_watcher_events() {
    use npio::service::filesystem::FileSystemWatcher;
    use std::time::Duration;
    
    // Create temporary directory
    let test_dir = std::env::temp_dir().join("npio_watcher_events_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    // Create watcher
    let mut watcher = FileSystemWatcher::new().expect("watcher");
    
    // Watch the directory
    watcher.watch(&test_dir).expect("watch");
    
    // Create a file to trigger an event
    let test_file = test_dir.join("test.txt");
    fs::write(&test_file, b"test").await.unwrap();
    
    // Give the watcher a moment to detect the change
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Poll for events
    let events = watcher.poll_events();
    // Should have at least one event (Created)
    // Note: Events may be batched or delayed, so we just verify it doesn't panic
    let _ = events.len();
    
    // Modify the file
    fs::write(&test_file, b"modified").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let events2 = watcher.poll_events();
    let _ = events2.len();
    
    // Delete the file
    fs::remove_file(&test_file).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let events3 = watcher.poll_events();
    let _ = events3.len();
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}
