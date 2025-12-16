use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use npio::backend::local::LocalBackend;
use npio::{get_file_for_uri, register_backend};

#[tokio::test]
async fn test_local_backend_lifecycle() {
    // 1. Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // 2. Define test file path
    let test_dir = std::env::temp_dir();
    let test_file_path = test_dir.join("npio_test_file.txt");
    let uri = format!("file://{}", test_file_path.to_string_lossy());

    // 3. Get file handle
    let file = get_file_for_uri(&uri).expect("Failed to get file handle");

    // 4. Create/Write file
    {
        let mut output = file.create_file(None).await.expect("Failed to create file");
        output.write_all(b"Hello, NPIO!").await.expect("Failed to write");
        output.close(None).expect("Failed to close output");
    }

    // 5. Verify exists
    assert!(file.exists(None).await.expect("Failed to check existence"));

    // 6. Query info
    let info = file.query_info("standard::*", None).await.expect("Failed to query info");
    assert_eq!(info.get_name().unwrap(), "npio_test_file.txt");
    assert_eq!(info.get_size(), 12);

    // 7. Read file
    {
        let mut input = file.read(None).await.expect("Failed to open for read");
        let mut buffer = String::new();
        input.read_to_string(&mut buffer).await.expect("Failed to read");
        assert_eq!(buffer, "Hello, NPIO!");
        input.close(None).expect("Failed to close input");
    }

    // 8. Delete file
    file.delete(None).await.expect("Failed to delete");

    // 9. Verify deleted
    assert!(!file.exists(None).await.expect("Failed to check existence"));
}
