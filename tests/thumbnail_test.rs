use npio::backend::thumbnail::{ThumbnailBackend, ThumbnailSize};
use npio::service::thumbnail::ThumbnailService;
use npio::get_file_for_uri;
use std::path::PathBuf;

#[test]
fn test_md5_hashing() {
    // Known MD5 hash for "file:///tmp/test.png"
    // echo -n "file:///tmp/test.png" | md5sum
    // 6756f54a791d53a4ece8ebb70471b573
    let uri = "file:///tmp/test.png";
    let name = ThumbnailBackend::uri_to_thumbnail_name(uri);
    assert_eq!(name, "6756f54a791d53a4ece8ebb70471b573.png");
}

#[test]
fn test_cache_dir() {
    // Set XDG_CACHE_HOME for test
    std::env::set_var("XDG_CACHE_HOME", "/tmp/npio_test_cache");
    
    let dir = ThumbnailBackend::get_cache_dir(ThumbnailSize::Normal).unwrap();
    assert_eq!(dir, PathBuf::from("/tmp/npio_test_cache/thumbnails/normal"));
    
    let dir = ThumbnailBackend::get_cache_dir(ThumbnailSize::Large).unwrap();
    assert_eq!(dir, PathBuf::from("/tmp/npio_test_cache/thumbnails/large"));
}

#[tokio::test]
async fn test_generate_thumbnail_not_found() {
    // Register backend
    let backend = std::sync::Arc::new(npio::backend::local::LocalBackend::new());
    npio::register_backend(backend);

    let service = ThumbnailService::new();
    let uri = "file:///nonexistent/file.jpg";
    let file = get_file_for_uri(uri).unwrap();
    
    let result = service.generate_thumbnail(&*file, ThumbnailSize::Normal, None).await;
    assert!(result.is_err());
}
