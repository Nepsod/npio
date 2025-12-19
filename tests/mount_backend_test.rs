use npio::backend::mount::MountBackend;

#[tokio::test]
async fn test_mount_backend_get_mounts() {
    let backend = MountBackend::new();
    let mounts = backend.get_mounts().await;
    
    // Should succeed (unless we're in a weird environment)
    assert!(mounts.is_ok());
    
    let mounts = mounts.unwrap();
    // Should have at least the root mount
    assert!(!mounts.is_empty());
    
    // Verify we can get basic info from mounts
    for mount in &mounts {
        let name = mount.get_name();
        let icon = mount.get_icon();
        assert!(!name.is_empty());
        assert!(!icon.is_empty());
    }
}

#[tokio::test]
async fn test_mount_backend_get_mount_for_path() {
    let backend = MountBackend::new();
    
    // Get mount for root
    let root_mount = backend.get_mount_for_path(std::path::Path::new("/")).await;
    assert!(root_mount.is_ok());
    assert!(root_mount.unwrap().is_some());
    
    // Get mount for home (should work if HOME exists)
    if let Ok(home) = std::env::var("HOME") {
        let home_mount = backend.get_mount_for_path(std::path::Path::new(&home)).await;
        assert!(home_mount.is_ok());
        // May or may not be Some depending on system
    }
}

#[tokio::test]
async fn test_mount_can_unmount() {
    let backend = MountBackend::new();
    let mounts = backend.get_mounts().await.unwrap();
    
    // Root mount should not be unmountable
    let root_mount = mounts.iter().find(|m| m.get_root().uri() == "file:///");
    if let Some(mount) = root_mount {
        assert!(!mount.can_unmount());
    }
}

#[tokio::test]
async fn test_mount_can_eject() {
    let backend = MountBackend::new();
    let mounts = backend.get_mounts().await.unwrap();
    
    // System mounts should not be ejectable
    for mount in &mounts {
        let uri = mount.get_root().uri();
        if uri == "file:///" || uri.starts_with("file:///sys") || uri.starts_with("file:///proc") {
            assert!(!mount.can_eject());
        }
    }
}

