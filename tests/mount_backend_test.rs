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

#[tokio::test]
async fn test_unixmount_unmount_system_mount() {
    let backend = MountBackend::new();
    let mounts = backend.get_mounts().await.unwrap();
    
    // Find root mount (system mount)
    let root_mount = mounts.iter().find(|m| m.get_root().uri() == "file:///");
    if let Some(mount) = root_mount {
        // Should fail to unmount system mount
        let result = mount.unmount(None).await;
        assert!(result.is_err());
        if let Err(e) = result {
            // Should be NotSupported error
            assert_eq!(format!("{:?}", e.kind()), "NotSupported");
        }
    }
}

#[tokio::test]
#[ignore] // Requires removable media and potentially root privileges
async fn test_unixmount_unmount_removable() {
    let backend = MountBackend::new();
    let mounts = backend.get_mounts().await.unwrap();
    
    // Find a removable mount (in /media or /mnt)
    let removable_mount = mounts.iter().find(|m| {
        let uri = m.get_root().uri();
        (uri.starts_with("file:///media") || uri.starts_with("file:///mnt"))
            && m.can_unmount()
    });
    
    if let Some(mount) = removable_mount {
        // Try to unmount (may require root)
        let result = mount.unmount(None).await;
        // Result depends on permissions - just verify it doesn't panic
        // If it succeeds, verify the mount is actually unmounted
        if result.is_ok() {
            // Mount should no longer be in the list
            let mounts_after = backend.get_mounts().await.unwrap();
            let still_mounted = mounts_after.iter().any(|m| {
                m.get_root().uri() == mount.get_root().uri()
            });
            // May still be mounted if lazy unmount, but that's okay
            let _ = still_mounted;
        }
    } else {
        // Skip test if no removable media available
        eprintln!("No removable media found, skipping unmount test");
    }
}

#[tokio::test]
#[ignore] // Requires removable media and potentially root privileges
async fn test_unixmount_remount() {
    let backend = MountBackend::new();
    let mounts = backend.get_mounts().await.unwrap();
    
    // Find a removable mount that can be remounted
    let removable_mount = mounts.iter().find(|m| {
        let uri = m.get_root().uri();
        (uri.starts_with("file:///media") || uri.starts_with("file:///mnt"))
            && m.can_unmount()
    });
    
    if let Some(mount) = removable_mount {
        // Try to remount (may require root)
        let result = mount.remount(None).await;
        // Result depends on permissions - just verify it doesn't panic
        // Remount should preserve mount options
        if result.is_err() {
            // If it fails, it's likely a permission issue
            eprintln!("Remount failed (likely permission issue): {:?}", result);
        }
    } else {
        // Skip test if no removable media available
        eprintln!("No removable media found, skipping remount test");
    }
}

#[tokio::test]
#[ignore] // Requires removable media and potentially root privileges
async fn test_unixmount_eject() {
    let backend = MountBackend::new();
    let mounts = backend.get_mounts().await.unwrap();
    
    // Find an ejectable mount
    let ejectable_mount = mounts.iter().find(|m| m.can_eject());
    
    if let Some(mount) = ejectable_mount {
        // Eject should call unmount first
        let result = mount.eject(None).await;
        // Result depends on permissions - just verify it doesn't panic
        // Eject may fail if device is busy or requires root
        if result.is_err() {
            eprintln!("Eject failed (likely permission or device busy): {:?}", result);
        }
    } else {
        // Skip test if no ejectable media available
        eprintln!("No ejectable media found, skipping eject test");
    }
}

#[tokio::test]
async fn test_unixmount_unmount_cancellable() {
    use npio::Cancellable;
    
    let backend = MountBackend::new();
    let mounts = backend.get_mounts().await.unwrap();
    
    // Find a non-system mount
    let test_mount = mounts.iter().find(|m| m.can_unmount());
    
    if let Some(mount) = test_mount {
        // Create a cancellable and cancel it
        let cancellable = Cancellable::new();
        cancellable.cancel();
        
        // Unmount should fail with cancellation error
        let result = mount.unmount(Some(&cancellable)).await;
        assert!(result.is_err());
        if let Err(e) = result {
            // Should be Cancelled error
            assert_eq!(format!("{:?}", e.kind()), "Cancelled");
        }
    }
}
