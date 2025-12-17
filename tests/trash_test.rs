use std::sync::Arc;
use npio::backend::local::LocalBackend;
use npio::{get_file_for_uri, register_backend};
use npio::job;
use std::path::PathBuf;

#[tokio::test]
async fn test_trash_file() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Create test directory in home directory to avoid cross-filesystem issues
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let test_dir = PathBuf::from(home_dir).join("npio_trash_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();

    // Create a test file
    let test_file_path = test_dir.join("test_file.txt");
    let test_file_uri = format!("file://{}", test_file_path.to_string_lossy());
    tokio::fs::write(&test_file_path, b"test content").await.unwrap();
    
    // Verify file exists
    assert!(test_file_path.exists());

    // Get file handle and trash it
    let file = get_file_for_uri(&test_file_uri).expect("Failed to get file handle");
    job::trash(&*file, None).await.expect("Failed to trash file");

    // Verify original file is gone
    assert!(!test_file_path.exists());

    // Verify file is in trash
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            directories::ProjectDirs::from("", "", "")
                .map(|dirs| dirs.data_dir().to_path_buf())
        })
        .unwrap_or_else(|| {
            directories::UserDirs::new()
                .map(|dirs| dirs.home_dir().join(".local").join("share"))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
        });

    let trash_files = data_home.join("Trash").join("files");
    let trash_info = data_home.join("Trash").join("info");

    // Check that file exists in trash/files
    let mut found_in_trash = false;
    if trash_files.exists() {
        let mut entries = tokio::fs::read_dir(&trash_files).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            if entry.file_name().to_string_lossy().starts_with("test_file") {
                found_in_trash = true;
                break;
            }
        }
    }
    assert!(found_in_trash, "File should be in trash/files directory");

    // Check that trashinfo file exists
    let mut found_trashinfo = false;
    if trash_info.exists() {
        let mut entries = tokio::fs::read_dir(&trash_info).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("test_file") && name.ends_with(".trashinfo") {
                found_trashinfo = true;
                
                // Verify trashinfo content
                let content = tokio::fs::read_to_string(entry.path()).await.unwrap();
                assert!(content.contains("[Trash Info]"));
                assert!(content.contains("Path="));
                assert!(content.contains("DeletionDate="));
                // Verify path is absolute and encoded
                let path_line = content.lines().find(|l| l.starts_with("Path=")).unwrap();
                let path_value = path_line.strip_prefix("Path=").unwrap();
                assert!(path_value.starts_with('/'), "Path should be absolute");
                break;
            }
        }
    }
    assert!(found_trashinfo, "trashinfo file should exist");

    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}

#[tokio::test]
async fn test_trash_file_with_spaces() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Create test directory in home directory to avoid cross-filesystem issues
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let test_dir = PathBuf::from(home_dir).join("npio_trash_test_spaces");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();

    // Create a test file with spaces in name
    let test_file_path = test_dir.join("test file with spaces.txt");
    let test_file_uri = format!("file://{}", test_file_path.to_string_lossy());
    tokio::fs::write(&test_file_path, b"test content").await.unwrap();
    
    // Get file handle and trash it
    let file = get_file_for_uri(&test_file_uri).expect("Failed to get file handle");
    job::trash(&*file, None).await.expect("Failed to trash file");

    // Verify original file is gone
    assert!(!test_file_path.exists());

    // Verify trashinfo has properly encoded path
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            directories::ProjectDirs::from("", "", "")
                .map(|dirs| dirs.data_dir().to_path_buf())
        })
        .unwrap_or_else(|| {
            directories::UserDirs::new()
                .map(|dirs| dirs.home_dir().join(".local").join("share"))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
        });

    let trash_info = data_home.join("Trash").join("info");
    if trash_info.exists() {
        let mut entries = tokio::fs::read_dir(&trash_info).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains("test file") && name.ends_with(".trashinfo") {
                let content = tokio::fs::read_to_string(entry.path()).await.unwrap();
                let path_line = content.lines().find(|l| l.starts_with("Path=")).unwrap();
                let path_value = path_line.strip_prefix("Path=").unwrap();
                // Path should be percent-encoded (spaces should be %20)
                assert!(path_value.contains("%20") || !path_value.contains(" "), 
                    "Path should be percent-encoded");
                break;
            }
        }
    }

    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}

#[tokio::test]
async fn test_trash_cancellable() {
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // Create test directory in home directory to avoid cross-filesystem issues
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let test_dir = PathBuf::from(home_dir).join("npio_trash_test_cancel");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();

    // Create a test file
    let test_file_path = test_dir.join("test_file.txt");
    let test_file_uri = format!("file://{}", test_file_path.to_string_lossy());
    tokio::fs::write(&test_file_path, b"test content").await.unwrap();
    
    // Create cancellable and cancel it
    let cancellable = npio::Cancellable::new();
    cancellable.cancel();

    // Get file handle and try to trash it (should fail due to cancellation)
    let file = get_file_for_uri(&test_file_uri).expect("Failed to get file handle");
    let result = job::trash(&*file, Some(&cancellable)).await;
    
    // Should fail with cancelled error
    assert!(result.is_err());
    if let Err(e) = result {
        assert_eq!(format!("{:?}", e.kind()), "Cancelled");
    }

    // Verify original file still exists (operation was cancelled)
    assert!(test_file_path.exists());

    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}

