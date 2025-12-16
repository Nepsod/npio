use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use npio::backend::local::LocalBackend;
use npio::{get_file_for_uri, register_backend};

#[tokio::test]
async fn test_metadata_detection() {
    // 1. Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    // 2. Define test file path (text file)
    let test_dir = std::env::temp_dir();
    let test_file_path = test_dir.join("npio_test_metadata.txt");
    let uri = format!("file://{}", test_file_path.to_string_lossy());

    // 3. Create file
    let file = get_file_for_uri(&uri).expect("Failed to get file handle");
    {
        let mut output = file.create_file(None).await.expect("Failed to create file");
        output.write_all(b"Hello, Metadata!").await.expect("Failed to write");
        output.close(None).expect("Failed to close output");
    }

    // 4. Query info with content-type
    let info = file.query_info("standard::content-type,standard::icon", None).await.expect("Failed to query info");
    
    // 5. Verify MIME type
    assert_eq!(info.get_content_type().unwrap(), "text/plain");
    
    // 6. Verify Icon
    // MimeGuess might return text/plain, so icon should be text-plain
    if let Some(icon) = info.get_attribute("standard::icon") {
        match icon {
            npio::FileAttributeType::String(s) => assert_eq!(s, "text-plain"),
            _ => panic!("Icon attribute is not a string"),
        }
    } else {
        panic!("Icon attribute missing");
    }

    // 7. Cleanup
    file.delete(None).await.expect("Failed to delete");
}
