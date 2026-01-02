// Integration tests for icon services

use npio::service::icon::IconRegistry;
use npio::service::filesystem::icon::{MimeIconProvider, IconProvider};
use npio::file::local::LocalFile;
use npio::{get_file_for_uri, register_backend};
use npio::backend::local::LocalBackend;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::fs;

#[tokio::test]
async fn test_icon_registry_real_files() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    let registry = IconRegistry::new().expect("icon registry");
    
    // Create temporary directory with various file types
    let test_dir = std::env::temp_dir().join("npio_icon_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    // Create files with different types
    let test_files = vec![
        ("test.txt", "text/plain"),
        ("test.jpg", "image/jpeg"),
        ("test.png", "image/png"),
        ("test.pdf", "application/pdf"),
        ("test.toml", "application/toml"),
    ];
    
    for (filename, _mime) in test_files {
        let file_path = test_dir.join(filename);
        fs::write(&file_path, b"test content").await.unwrap();
        
        let file_uri = format!("file://{}", file_path.to_string_lossy());
        let file = get_file_for_uri(&file_uri).unwrap();
        
        // Get icon for the file
        let icon = registry.get_file_icon(&*file, 64).await;
        assert!(icon.is_some(), "Should get icon for {}", filename);
    }
    
    // Test directory icon
    let dir_uri = format!("file://{}", test_dir.to_string_lossy());
    let dir_file = get_file_for_uri(&dir_uri).unwrap();
    let dir_icon = registry.get_file_icon(&*dir_file, 64).await;
    assert!(dir_icon.is_some(), "Should get icon for directory");
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_icon_registry_cache() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    let mut registry = IconRegistry::new().expect("icon registry");
    
    // Create a test file
    let test_dir = std::env::temp_dir().join("npio_icon_cache_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    let file_path = test_dir.join("test.txt");
    fs::write(&file_path, b"test").await.unwrap();
    
    let file_uri = format!("file://{}", file_path.to_string_lossy());
    let file = get_file_for_uri(&file_uri).unwrap();
    
    // First call - should populate cache
    let icon1 = registry.get_file_icon(&*file, 64).await;
    assert!(icon1.is_some());
    
    // Second call - should use cache
    let icon2 = registry.get_file_icon(&*file, 64).await;
    assert!(icon2.is_some());
    
    // Clear cache
    registry.clear_cache();
    
    // Third call - should repopulate cache
    let icon3 = registry.get_file_icon(&*file, 64).await;
    assert!(icon3.is_some());
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_icon_registry_theme_switching() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    let mut registry = IconRegistry::new().expect("icon registry");
    
    // Get current theme
    let initial_theme = registry.theme().to_string();
    
    // Try to switch to a different theme (may fail if theme doesn't exist)
    let result = registry.set_theme("hicolor".to_string());
    // May succeed or fail depending on available themes
    if result.is_ok() {
        assert_eq!(registry.theme(), "hicolor");
        
        // Switch back
        let result2 = registry.set_theme(initial_theme.clone());
        if result2.is_ok() {
            assert_eq!(registry.theme(), initial_theme);
        }
    }
}

#[tokio::test]
async fn test_mime_icon_provider_real_files() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    let provider = MimeIconProvider::new();
    
    // Create temporary directory with various file types
    let test_dir = std::env::temp_dir().join("npio_mime_icon_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    // Test with different file types
    let test_files = vec![
        ("test.txt", "text/plain"),
        ("test.jpg", "image/jpeg"),
        ("test.png", "image/png"),
        ("test.pdf", "application/pdf"),
        ("test.toml", "application/toml"),
    ];
    
    for (filename, _mime) in test_files {
        let file_path = test_dir.join(filename);
        fs::write(&file_path, b"test content").await.unwrap();
        
        let file_uri = format!("file://{}", file_path.to_string_lossy());
        let file = get_file_for_uri(&file_uri).unwrap();
        
        // Get icon data from provider
        let icon_data = provider.get_icon(&*file).await;
        assert!(icon_data.is_some(), "Should get icon data for {}", filename);
        
        if let Some(data) = icon_data {
            assert!(!data.names.is_empty(), "Icon data should have names");
        }
    }
    
    // Test directory
    let dir_uri = format!("file://{}", test_dir.to_string_lossy());
    let dir_file = get_file_for_uri(&dir_uri).unwrap();
    let dir_icon_data = provider.get_icon(&*dir_file).await;
    assert!(dir_icon_data.is_some(), "Should get icon data for directory");
    if let Some(data) = dir_icon_data {
        assert!(data.names.contains(&"folder".to_string()), "Directory should have folder icon");
    }
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_icon_fallback_chain() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    let registry = IconRegistry::new().expect("icon registry");
    
    // Create a file with a MIME type that might not have a specific icon
    let test_dir = std::env::temp_dir().join("npio_icon_fallback_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    let file_path = test_dir.join("test.unknown");
    fs::write(&file_path, b"test").await.unwrap();
    
    let file_uri = format!("file://{}", file_path.to_string_lossy());
    let file = get_file_for_uri(&file_uri).unwrap();
    
    // Should get an icon (even if it's a fallback)
    let icon = registry.get_file_icon(&*file, 64).await;
    // Should return some icon (may be generic fallback)
    assert!(icon.is_some(), "Should get icon even for unknown file type");
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_icon_registry_different_sizes() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    let registry = IconRegistry::new().expect("icon registry");
    
    // Create a test file
    let test_dir = std::env::temp_dir().join("npio_icon_size_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    let file_path = test_dir.join("test.txt");
    fs::write(&file_path, b"test").await.unwrap();
    
    let file_uri = format!("file://{}", file_path.to_string_lossy());
    let file = get_file_for_uri(&file_uri).unwrap();
    
    // Test different icon sizes
    let sizes = vec![16, 32, 48, 64, 128];
    
    for size in sizes {
        let icon = registry.get_file_icon(&*file, size).await;
        assert!(icon.is_some(), "Should get icon for size {}", size);
    }
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}

#[tokio::test]
async fn test_icon_registry_symlink() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    let registry = IconRegistry::new().expect("icon registry");
    
    // Create temporary directory
    let test_dir = std::env::temp_dir().join("npio_icon_symlink_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).await.unwrap();
    }
    fs::create_dir(&test_dir).await.unwrap();
    
    // Create a file
    let target_file = test_dir.join("target.txt");
    fs::write(&target_file, b"target").await.unwrap();
    
    // Create a symlink
    let symlink_file = test_dir.join("link.txt");
    #[cfg(unix)]
    {
        use std::os::unix::fs;
        fs::symlink(&target_file, &symlink_file).unwrap();
    }
    
    #[cfg(unix)]
    {
        let symlink_uri = format!("file://{}", symlink_file.to_string_lossy());
        let symlink_file_obj = get_file_for_uri(&symlink_uri).unwrap();
        
        // Should get symlink icon
        let icon = registry.get_file_icon(&*symlink_file_obj, 64).await;
        assert!(icon.is_some(), "Should get icon for symlink");
    }
    
    // Cleanup
    fs::remove_dir_all(&test_dir).await.unwrap();
}
