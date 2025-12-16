use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncWriteExt;
use npio::backend::local::LocalBackend;
use npio::{get_file_for_uri, register_backend, CopyFlags};
use npio::job;

#[tokio::test]
async fn test_job_lifecycle() {
    // 1. Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // 2. Define test dir
    let test_dir = std::env::temp_dir().join("npio_job_test");
    if test_dir.exists() {
        tokio::fs::remove_dir_all(&test_dir).await.unwrap();
    }
    tokio::fs::create_dir(&test_dir).await.unwrap();
    
    // 3. Create source file
    let src_path = test_dir.join("source.txt");
    let src_uri = format!("file://{}", src_path.to_string_lossy());
    let src_file = get_file_for_uri(&src_uri).expect("Failed to get src handle");
    
    let content = vec![b'a'; 1024 * 10]; // 10KB
    {
        let mut output = src_file.create_file(None).await.expect("Failed to create file");
        output.write_all(&content).await.expect("Failed to write");
        output.close(None).expect("Failed to close");
    }

    // 4. Copy with progress
    let dest_path = test_dir.join("dest.txt");
    let dest_uri = format!("file://{}", dest_path.to_string_lossy());
    let dest_file = get_file_for_uri(&dest_uri).expect("Failed to get dest handle");
    
    let progress_bytes = Arc::new(AtomicU64::new(0));
    let progress_clone = progress_bytes.clone();
    
    job::copy(
        &*src_file,
        &*dest_file,
        CopyFlags::NONE,
        Some(Box::new(move |current, _total| {
            progress_clone.store(current, Ordering::SeqCst);
        })),
        None
    ).await.expect("Copy failed");
    
    // Verify copy
    assert!(dest_path.exists());
    assert_eq!(tokio::fs::read(&dest_path).await.unwrap().len(), content.len());
    assert_eq!(progress_bytes.load(Ordering::SeqCst), content.len() as u64);

    // 5. Move
    let moved_path = test_dir.join("moved.txt");
    let moved_uri = format!("file://{}", moved_path.to_string_lossy());
    let moved_file = get_file_for_uri(&moved_uri).expect("Failed to get moved handle");
    
    job::move_(
        &*dest_file,
        &*moved_file,
        CopyFlags::NONE,
        None,
        None
    ).await.expect("Move failed");
    
    // Verify move
    assert!(!dest_path.exists());
    assert!(moved_path.exists());

    // 6. Delete
    job::delete(&*moved_file, None).await.expect("Delete failed");
    assert!(!moved_path.exists());

    // Cleanup
    tokio::fs::remove_dir_all(&test_dir).await.ok();
}
