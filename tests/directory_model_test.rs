use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;
use npio::backend::local::LocalBackend;
use npio::{get_file_for_uri, register_backend, DirectoryModel, DirectoryUpdate};

#[tokio::test]
async fn test_directory_model_lifecycle() {
    // 1. Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // 2. Define test dir
    let test_dir = std::env::temp_dir().join("npio_dir_model_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();
    
    let uri = format!("file://{}", test_dir.to_string_lossy());
    let dir = get_file_for_uri(&uri).expect("Failed to get dir handle");

    // 3. Create Model
    let model = DirectoryModel::new(dir);
    let mut rx = model.subscribe();
    
    // 4. Load
    model.load(None).await.expect("Failed to load model");
    
    // 5. Verify initial state (empty)
    assert!(model.files().is_empty());

    // 6. Create file
    let file_path = test_dir.join("test_model.txt");
    let file_uri = format!("file://{}", file_path.to_string_lossy());
    let file = get_file_for_uri(&file_uri).expect("Failed to get file handle");
    
    {
        let mut output = file.create_file(None).await.expect("Failed to create file");
        output.write_all(b"init").await.expect("Failed to write");
        output.close(None).expect("Failed to close");
    }

    // 7. Wait for Added event
    let start = std::time::Instant::now();
    let mut found_added = false;
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(Ok(event)) = timeout(Duration::from_millis(500), rx.recv()).await {
             match event {
                DirectoryUpdate::Added(info) => {
                    if info.get_name() == Some("test_model.txt") {
                        found_added = true;
                        break;
                    }
                },
                _ => {},
            }
        }
    }
    assert!(found_added, "Did not receive Added event");
    
    // Verify internal state updated
    assert_eq!(model.files().len(), 1);
    assert_eq!(model.files()[0].get_name(), Some("test_model.txt"));

    // 8. Delete file
    file.delete(None).await.expect("Failed to delete");

    // 9. Wait for Removed event
    let start = std::time::Instant::now();
    let mut found_removed = false;
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(Ok(event)) = timeout(Duration::from_millis(500), rx.recv()).await {
             match event {
                DirectoryUpdate::Removed(info) => {
                    if info.get_name() == Some("test_model.txt") {
                        found_removed = true;
                        break;
                    }
                },
                _ => {},
            }
        }
    }
    assert!(found_removed, "Did not receive Removed event");
    
    // Verify internal state updated
    assert!(model.files().is_empty());

    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}
