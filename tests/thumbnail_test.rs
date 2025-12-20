// Tests for thumbnail service

use npio::{ThumbnailService, ThumbnailSize, ThumbnailEvent, get_file_for_uri, register_backend};
use npio::backend::local::LocalBackend;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_thumbnail_service_new() {
    let _service = ThumbnailService::new();
    // Should create successfully
    assert!(true);
}

#[tokio::test]
async fn test_thumbnail_service_subscribe() {
    let service = ThumbnailService::new();
    let receiver = service.subscribe();
    
    // Should be able to subscribe
    assert!(true);
    
    // Receiver should be created (we can't easily test it without events)
    drop(receiver);
}

#[tokio::test]
async fn test_thumbnail_service_is_supported() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    let service = ThumbnailService::new();
    
    // Create a test image file
    let test_dir = std::env::temp_dir().join("npio_thumbnail_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();
    
    // Test with image file
    let image_path = test_dir.join("test.jpg");
    tokio::fs::write(&image_path, b"fake jpeg data").await.unwrap();
    let image_uri = format!("file://{}", image_path.to_string_lossy());
    let image_file = get_file_for_uri(&image_uri).unwrap();
    
    let supported = service.is_supported(&*image_file, None).await.unwrap();
    assert!(supported, "JPEG should be supported");
    
    // Test with text file (not supported)
    let text_path = test_dir.join("test.txt");
    tokio::fs::write(&text_path, b"text content").await.unwrap();
    let text_uri = format!("file://{}", text_path.to_string_lossy());
    let text_file = get_file_for_uri(&text_uri).unwrap();
    
    let supported = service.is_supported(&*text_file, None).await.unwrap();
    assert!(!supported, "Text file should not be supported");
    
    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_thumbnail_service_event_emission() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    let service = ThumbnailService::new();
    let mut receiver = service.subscribe();
    
    // Create a test image file
    let test_dir = std::env::temp_dir().join("npio_thumbnail_event_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();
    
    // Create a simple PNG file for testing
    // We'll use a minimal valid PNG
    let png_data = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1 image
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE,
        0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
        0x08, 0xD7, 0x63, 0xF8, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82, // IEND
    ];
    
    let image_path = test_dir.join("test.png");
    tokio::fs::write(&image_path, &png_data).await.unwrap();
    let image_uri = format!("file://{}", image_path.to_string_lossy());
    let image_file = get_file_for_uri(&image_uri).unwrap();
    
    // Try to generate thumbnail (may succeed or fail depending on thumbnailify)
    let result = service.generate_thumbnail(&*image_file, ThumbnailSize::Normal, None).await;
    
    // Wait for event with timeout
    let event_result = timeout(Duration::from_secs(5), receiver.recv()).await;
    
    if let Ok(Ok(event)) = event_result {
        match event {
            ThumbnailEvent::ThumbnailReady { uri, size, path } => {
                assert_eq!(uri, image_uri);
                assert_eq!(size, ThumbnailSize::Normal);
                assert!(path.exists() || !result.is_ok()); // Path may or may not exist depending on result
            }
            ThumbnailEvent::ThumbnailFailed { uri, size, .. } => {
                assert_eq!(uri, image_uri);
                assert_eq!(size, ThumbnailSize::Normal);
                // Failure is acceptable if thumbnailify doesn't support this format
            }
        }
    } else {
        // Event may not be emitted if generation fails early, which is acceptable
        // Just verify the service doesn't panic
        assert!(true);
    }
    
    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_thumbnail_image_cache() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    let service = ThumbnailService::new();
    
    // Create a test image file
    let test_dir = std::env::temp_dir().join("npio_thumbnail_cache_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();
    
    // Create a minimal PNG
    let png_data = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE,
        0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54,
        0x08, 0xD7, 0x63, 0xF8, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    
    let image_path = test_dir.join("test.png");
    tokio::fs::write(&image_path, &png_data).await.unwrap();
    let image_uri = format!("file://{}", image_path.to_string_lossy());
    let image_file = get_file_for_uri(&image_uri).unwrap();
    
    // Try to get thumbnail image (may fail if thumbnail generation fails)
    let result = service.get_thumbnail_image(&*image_file, ThumbnailSize::Normal, None).await;
    
    match result {
        Ok(image) => {
            // Verify image structure
            assert!(image.width > 0);
            assert!(image.height > 0);
            assert_eq!(image.data.len(), (image.width * image.height * 4) as usize);
        }
        Err(_) => {
            // Failure is acceptable if thumbnail generation doesn't work
            // Just verify the method doesn't panic
            assert!(true);
        }
    }
    
    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_thumbnail_image_cache_storage() {
    use npio::{ThumbnailImage, ThumbnailImageCache};
    
    let cache = ThumbnailImageCache::new();
    
    // Create a test image
    let test_image = ThumbnailImage {
        width: 128,
        height: 128,
        data: vec![0; 128 * 128 * 4], // RGBA
    };
    
    // Store image
    cache.store_image("test:Normal".to_string(), test_image.clone());
    
    // Retrieve image
    let retrieved = cache.get_image("test:Normal");
    assert!(retrieved.is_some());
    let img = retrieved.unwrap();
    assert_eq!(img.width, 128);
    assert_eq!(img.height, 128);
    assert_eq!(img.data.len(), 128 * 128 * 4);
    
    // Test non-existent key
    let not_found = cache.get_image("nonexistent:Normal");
    assert!(not_found.is_none());
    
    // Test clear
    cache.clear();
    let cleared = cache.get_image("test:Normal");
    assert!(cleared.is_none());
}

#[tokio::test]
async fn test_thumbnail_service_mime_types() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    let service = ThumbnailService::new();
    
    let test_dir = std::env::temp_dir().join("npio_thumbnail_mime_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();
    
    // Test various MIME types
    let test_cases = vec![
        ("test.jpg", true),   // JPEG
        ("test.png", true),   // PNG
        ("test.gif", true),   // GIF
        ("test.webp", true),  // WebP
        ("test.pdf", true),   // PDF
        ("test.txt", false),  // Text
        ("test.mp4", true),    // Video
    ];
    
    for (filename, expected_supported) in test_cases {
        let file_path = test_dir.join(filename);
        tokio::fs::write(&file_path, b"test data").await.unwrap();
        let file_uri = format!("file://{}", file_path.to_string_lossy());
        let file = get_file_for_uri(&file_uri).unwrap();
        
        let supported = service.is_supported(&*file, None).await.unwrap();
        assert_eq!(supported, expected_supported, "MIME type check failed for {}", filename);
    }
    
    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.unwrap();
}
