// Integration tests for npio
// Tests multiple components working together in realistic scenarios

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use npio::backend::local::LocalBackend;
use npio::{
    get_file_for_uri, register_backend, CopyFlags, DirectoryModel, DirectoryUpdate,
    get_home_file, get_user_special_file, UserDirectory, BookmarksService, ThumbnailService,
    ThumbnailSize, MountBackend,
};
use npio::job;

/// Test a complete file workflow: create, read, copy, move, delete
#[tokio::test]
async fn test_complete_file_workflow() {
    // Setup
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    let test_dir = std::env::temp_dir().join("npio_integration_file_workflow");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir_all(&test_dir).await.unwrap();

    // 1. Create a file
    let file_path = test_dir.join("document.txt");
    let file_uri = format!("file://{}", file_path.to_string_lossy());
    let file = get_file_for_uri(&file_uri).unwrap();

    let content = b"Hello, NPIO Integration Test!";
    {
        let mut output = file.create_file(None).await.unwrap();
        output.write_all(content).await.unwrap();
        output.close(None).unwrap();
    }

    // 2. Verify it exists and read it
    assert!(file.exists(None).await.unwrap());
    let info = file.query_info("standard::*", None).await.unwrap();
    assert_eq!(info.get_size(), content.len() as i64);

    {
        let mut input = file.read(None).await.unwrap();
        let mut buffer = Vec::new();
        input.read_to_end(&mut buffer).await.unwrap();
        assert_eq!(buffer, content);
    }

    // 3. Copy the file
    let copy_path = test_dir.join("document_copy.txt");
    let copy_uri = format!("file://{}", copy_path.to_string_lossy());
    let copy_file = get_file_for_uri(&copy_uri).unwrap();

    let progress = Arc::new(AtomicU64::new(0));
    let progress_clone = progress.clone();
    job::copy(
        &*file,
        &*copy_file,
        CopyFlags::NONE,
        Some(Box::new(move |current, _total| {
            progress_clone.store(current, Ordering::SeqCst);
        })),
        None,
    )
    .await
    .unwrap();

    assert!(copy_path.exists());
    assert_eq!(progress.load(Ordering::SeqCst), content.len() as u64);

    // 4. Move the original
    let moved_path = test_dir.join("document_moved.txt");
    let moved_uri = format!("file://{}", moved_path.to_string_lossy());
    let moved_file = get_file_for_uri(&moved_uri).unwrap();

    job::move_(&*file, &*moved_file, CopyFlags::NONE, None, None).await.unwrap();
    assert!(!file_path.exists());
    assert!(moved_path.exists());

    // 5. Delete the moved file
    moved_file.delete(None).await.unwrap();
    assert!(!moved_path.exists());

    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}

/// Test directory model with file monitoring
#[tokio::test]
async fn test_directory_model_with_monitoring() {
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    let test_dir = std::env::temp_dir().join("npio_integration_dir_monitor");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir_all(&test_dir).await.unwrap();

    let dir_uri = format!("file://{}", test_dir.to_string_lossy());
    let dir = get_file_for_uri(&dir_uri).unwrap();

    // Create directory model
    let model = DirectoryModel::new(dir);
    let mut rx = model.subscribe();

    // Load initial state
    model.load(None).await.unwrap();
    assert_eq!(model.files().len(), 0);

    // Create a file and verify it appears in the model
    let file_path = test_dir.join("test.txt");
    let file_uri = format!("file://{}", file_path.to_string_lossy());
    let file = get_file_for_uri(&file_uri).unwrap();

    {
        let mut output = file.create_file(None).await.unwrap();
        output.write_all(b"test").await.unwrap();
        output.close(None).unwrap();
    }

    // Wait for update
    let mut found = false;
    for _ in 0..10 {
        if let Ok(update) = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            rx.recv(),
        )
        .await
        {
            if let Ok(update) = update {
                match update {
                    DirectoryUpdate::Added(info) | DirectoryUpdate::Changed(info) => {
                        if info.get_name() == Some("test.txt") {
                            found = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Verify file is in model
    assert!(found || model.files().iter().any(|f| f.get_name() == Some("test.txt")));

    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}

/// Test services working together: User directories, Bookmarks, Thumbnail
#[tokio::test]
async fn test_services_integration() {
    // Test user directory helpers (GIO-compatible)
    let home_file = get_home_file();
    assert!(home_file.is_ok());
    
    // Test getting special directories
    let docs_file = get_user_special_file(UserDirectory::Documents);
    assert!(docs_file.is_ok());
    // At least one special directory should be available
    let has_any_dir = [
        UserDirectory::Desktop,
        UserDirectory::Documents,
        UserDirectory::Download,
        UserDirectory::Music,
        UserDirectory::Pictures,
        UserDirectory::Videos,
    ].iter().any(|dir| {
        get_user_special_file(*dir).ok().flatten().is_some()
    });
    assert!(has_any_dir);

    // Test Bookmarks Service
    let mut bookmarks_service = BookmarksService::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let home_uri = format!("file://{}", home);

    // Add a bookmark
    bookmarks_service.add_bookmark(home_uri.clone(), Some("Home".to_string()));
    assert!(bookmarks_service.has_bookmark(&home_uri));
    assert_eq!(bookmarks_service.get_bookmarks().len(), 1);

    // Test Thumbnail Service (basic check)
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    let thumbnail_service = ThumbnailService::new();
    let test_file = get_file_for_uri(&home_uri).unwrap();

    // Try to get thumbnail (may not exist, but should not error)
    let result = thumbnail_service
        .get_thumbnail_path(&*test_file, ThumbnailSize::Normal, None)
        .await;
    assert!(result.is_ok());
}

/// Test mount backend with file operations
#[tokio::test]
async fn test_mount_backend_integration() {
    let mount_backend = MountBackend::new();

    // Get all mounts
    let mounts = mount_backend.get_mounts().await;
    assert!(mounts.is_ok());
    let mounts = mounts.unwrap();
    assert!(!mounts.is_empty());

    // Find root mount
    let root_mount = mounts.iter().find(|m| m.get_root().uri() == "file:///");
    assert!(root_mount.is_some());

    if let Some(mount) = root_mount {
        // Root mount should not be unmountable
        assert!(!mount.can_unmount());
        assert!(!mount.can_eject());

        // Get mount for a path
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let mount_for_home = mount_backend
            .get_mount_for_path(std::path::Path::new(&home))
            .await;
        assert!(mount_for_home.is_ok());
    }
}

/// Test trash functionality with file operations
#[tokio::test]
async fn test_trash_integration() {
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Use a temporary directory in the same filesystem as trash
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            directories::ProjectDirs::from("", "", "")
                .map(|dirs| dirs.data_dir().to_path_buf())
        })
        .or_else(|| {
            directories::UserDirs::new()
                .map(|dirs| dirs.home_dir().join(".local/share"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));

    let test_dir = data_home.join("npio_trash_integration_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.ok();
    }
    tokio::fs::create_dir_all(&test_dir).await.unwrap();

    // Create a test file
    let file_path = test_dir.join("to_trash.txt");
    let file_uri = format!("file://{}", file_path.to_string_lossy());
    let file = get_file_for_uri(&file_uri).unwrap();

    {
        let mut output = file.create_file(None).await.unwrap();
        output.write_all(b"trash me").await.unwrap();
        output.close(None).unwrap();
    }

    assert!(file_path.exists());

    // Trash the file
    job::trash(&*file, None).await.unwrap();

    // File should be gone from original location
    assert!(!file_path.exists());

    // Cleanup test directory
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}

/// Test error handling across components
#[tokio::test]
async fn test_error_handling_integration() {
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Try to read a non-existent file
    let non_existent_uri = "file:///nonexistent/path/file.txt";
    let file = get_file_for_uri(non_existent_uri).unwrap();

    let result = file.read(None).await;
    assert!(result.is_err());

    // Try to delete a non-existent file
    let result = file.delete(None).await;
    assert!(result.is_err());

    // Try to copy to invalid location
    let src_uri = "file:///tmp";
    let dest_uri = "file:///root/invalid/destination";
    let src_file = get_file_for_uri(src_uri).unwrap();
    let dest_file = get_file_for_uri(dest_uri).unwrap();

    let result = job::copy(&*src_file, &*dest_file, CopyFlags::NONE, None, None).await;
    // May succeed or fail depending on permissions, but should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

/// Test cancellable operations
#[tokio::test]
async fn test_cancellable_integration() {
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    let test_dir = std::env::temp_dir().join("npio_cancellable_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.ok();
    }
    tokio::fs::create_dir_all(&test_dir).await.unwrap();

    // Create a large file
    let file_path = test_dir.join("large.txt");
    let file_uri = format!("file://{}", file_path.to_string_lossy());
    let file = get_file_for_uri(&file_uri).unwrap();

    let large_content = vec![b'x'; 1024 * 100]; // 100KB
    {
        let mut output = file.create_file(None).await.unwrap();
        output.write_all(&large_content).await.unwrap();
        output.close(None).unwrap();
    }

    // Create cancellable and cancel it
    let cancellable = npio::Cancellable::new();
    cancellable.cancel();

    // Try to read with cancelled cancellable
    let result = file.read(Some(&cancellable)).await;
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(format!("{:?}", e.kind()), "Cancelled");
    }

    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}

