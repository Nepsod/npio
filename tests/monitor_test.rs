use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;
use npio::backend::local::LocalBackend;
use npio::{get_file_for_uri, register_backend, FileMonitorEvent};

#[tokio::test]
async fn test_monitor_lifecycle() {
    // 1. Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // 2. Define test dir
    let test_dir = std::env::temp_dir().join("npio_monitor_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();
    
    let uri = format!("file://{}", test_dir.to_string_lossy());
    let dir = get_file_for_uri(&uri).expect("Failed to get dir handle");

    // 3. Start monitoring
    let mut monitor = dir.monitor(None).await.expect("Failed to start monitor");

    // 4. Create file
    let file_path = test_dir.join("test.txt");
    let file_uri = format!("file://{}", file_path.to_string_lossy());
    let file = get_file_for_uri(&file_uri).expect("Failed to get file handle");
    
    {
        let mut output = file.create_file(None).await.expect("Failed to create file");
        output.write_all(b"init").await.expect("Failed to write");
        output.close(None).expect("Failed to close");
    }

    // 5. Wait for Created event
    let event = timeout(Duration::from_secs(2), monitor.next_event()).await
        .expect("Timed out waiting for create event")
        .expect("Stream ended");
        
    match event {
        FileMonitorEvent::Created(f) | FileMonitorEvent::Changed(f, _) => {
            // Note: Some systems might report Changed instead of Created depending on timing/implementation
            // But notify usually reports Create.
            // Let's be lenient or check path
            assert_eq!(f.basename(), "test.txt");
        },
        _ => panic!("Unexpected event: {:?}", event),
    }

    // 6. Modify file
    {
        let mut output = file.replace(None, false, None).await.expect("Failed to replace");
        output.write_all(b"modified").await.expect("Failed to write");
        output.close(None).expect("Failed to close");
    }

    // 7. Wait for Changed event
    // We might get multiple events, so loop until we find what we want or timeout
    let start = std::time::Instant::now();
    let mut found_change = false;
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(Some(event)) = timeout(Duration::from_millis(500), monitor.next_event()).await {
             match event {
                FileMonitorEvent::Changed(f, _) => {
                    if f.basename() == "test.txt" {
                        found_change = true;
                        break;
                    }
                },
                _ => {},
            }
        }
    }
    assert!(found_change, "Did not receive Changed event");

    // 8. Delete file
    file.delete(None).await.expect("Failed to delete");

    // 9. Wait for Deleted event
    let start = std::time::Instant::now();
    let mut found_delete = false;
    while start.elapsed() < Duration::from_secs(2) {
        if let Ok(Some(event)) = timeout(Duration::from_millis(500), monitor.next_event()).await {
             match event {
                FileMonitorEvent::Deleted(f) => {
                    if f.basename() == "test.txt" {
                        found_delete = true;
                        break;
                    }
                },
                _ => {},
            }
        }
    }
    assert!(found_delete, "Did not receive Deleted event");

    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}
