use std::sync::Arc;
use npio::backend::local::LocalBackend;
use npio::backend::thumbnail::{ThumbnailBackend, ThumbnailSize};
use npio::service::thumbnail::ThumbnailService;
use npio::{get_file_for_uri, register_backend};
use std::path::PathBuf;

#[tokio::test]
async fn test_thumbnail_backend_cache_dir() {
    let cache_dir = ThumbnailBackend::get_cache_dir(ThumbnailSize::Normal).unwrap();
    assert!(cache_dir.to_string_lossy().contains("thumbnails"));
    assert!(cache_dir.to_string_lossy().contains("normal"));

    let large_dir = ThumbnailBackend::get_cache_dir(ThumbnailSize::Large).unwrap();
    assert!(large_dir.to_string_lossy().contains("large"));
}

#[test]
fn test_thumbnail_size_dimensions() {
    assert_eq!(ThumbnailSize::Normal.dimensions(), (128, 128));
    assert_eq!(ThumbnailSize::Large.dimensions(), (256, 256));
    assert_eq!(ThumbnailSize::XLarge.dimensions(), (512, 512));
    assert_eq!(ThumbnailSize::XXLarge.dimensions(), (1024, 1024));
}

#[test]
fn test_thumbnail_size_directory_name() {
    assert_eq!(ThumbnailSize::Normal.directory_name(), "normal");
    assert_eq!(ThumbnailSize::Large.directory_name(), "large");
    assert_eq!(ThumbnailSize::XLarge.directory_name(), "x-large");
    assert_eq!(ThumbnailSize::XXLarge.directory_name(), "xx-large");
}

#[test]
fn test_uri_to_thumbnail_name() {
    let uri1 = "file:///home/user/test.jpg";
    let uri2 = "file:///home/user/test2.jpg";
    
    let name1 = ThumbnailBackend::uri_to_thumbnail_name(uri1);
    let name2 = ThumbnailBackend::uri_to_thumbnail_name(uri2);
    
    // Should generate different names for different URIs
    assert_ne!(name1, name2);
    
    // Should end with .png
    assert!(name1.ends_with(".png"));
    assert!(name2.ends_with(".png"));
    
    // Should be consistent for same URI
    let name1_again = ThumbnailBackend::uri_to_thumbnail_name(uri1);
    assert_eq!(name1, name1_again);
}

#[tokio::test]
async fn test_thumbnail_backend_get_path() {
    let uri = "file:///home/user/test.jpg";
    let path = ThumbnailBackend::get_thumbnail_path(uri, ThumbnailSize::Normal).unwrap();
    
    assert!(path.to_string_lossy().contains("thumbnails"));
    assert!(path.to_string_lossy().contains("normal"));
    assert!(path.extension().and_then(|s| s.to_str()) == Some("png"));
}

#[tokio::test]
async fn test_thumbnail_service_new() {
    let service = ThumbnailService::new();
    // Service should be created successfully
    assert!(true); // Just verify it doesn't panic
}

#[tokio::test]
async fn test_thumbnail_service_get_path_nonexistent() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Create test file
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let test_file_path = PathBuf::from(home_dir).join("npio_thumbnail_test.txt");
    let test_file_uri = format!("file://{}", test_file_path.to_string_lossy());
    
    // Create file
    tokio::fs::write(&test_file_path, b"test content").await.unwrap();
    
    let file = get_file_for_uri(&test_file_uri).expect("Failed to get file handle");
    let service = ThumbnailService::new();
    
    // Get thumbnail path (should be None since thumbnail doesn't exist)
    let result = service.get_thumbnail_path(&*file, ThumbnailSize::Normal, None).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    
    // Cleanup
    tokio::fs::remove_file(&test_file_path).await.ok();
}

#[tokio::test]
async fn test_thumbnail_service_generate_not_implemented() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Create test file
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let test_file_path = PathBuf::from(home_dir).join("npio_thumbnail_test2.txt");
    let test_file_uri = format!("file://{}", test_file_path.to_string_lossy());
    
    tokio::fs::write(&test_file_path, b"test content").await.unwrap();
    
    let file = get_file_for_uri(&test_file_uri).expect("Failed to get file handle");
    let service = ThumbnailService::new();
    
    // Try to generate thumbnail (should fail with NotSupported)
    let result = service.generate_thumbnail(&*file, ThumbnailSize::Normal, None).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(format!("{:?}", e.kind()), "NotSupported");
    }
    
    // Cleanup
    tokio::fs::remove_file(&test_file_path).await.ok();
}

#[tokio::test]
async fn test_thumbnail_service_delete_nonexistent() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Create test file
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let test_file_path = PathBuf::from(home_dir).join("npio_thumbnail_test3.txt");
    let test_file_uri = format!("file://{}", test_file_path.to_string_lossy());
    
    tokio::fs::write(&test_file_path, b"test content").await.unwrap();
    
    let file = get_file_for_uri(&test_file_uri).expect("Failed to get file handle");
    let service = ThumbnailService::new();
    
    // Delete non-existent thumbnail (should succeed)
    let result = service.delete_thumbnail(&*file, ThumbnailSize::Normal, None).await;
    assert!(result.is_ok());
    
    // Cleanup
    tokio::fs::remove_file(&test_file_path).await.ok();
}

#[tokio::test]
async fn test_thumbnail_service_cleanup() {
    let service = ThumbnailService::new();
    
    // Cleanup should succeed even if cache directory doesn't exist
    // Note: This may delete actual thumbnails if cache directory exists,
    // so we just verify the operation succeeds
    let result = service.cleanup_thumbnails(ThumbnailSize::Normal, None).await;
    assert!(result.is_ok());
    // Don't assert specific count as user may have thumbnails in cache
    let _deleted_count = result.unwrap();
}

#[tokio::test]
async fn test_thumbnail_backend_has_valid_thumbnail_nonexistent() {
    let uri = "file:///home/user/nonexistent.jpg";
    let result = ThumbnailBackend::has_valid_thumbnail(uri, ThumbnailSize::Normal, 0).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), false);
}

