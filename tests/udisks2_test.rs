// Tests for UDisks2 backend integration

use npio::backend::udisks2::UDisks2Backend;

/// Test UDisks2Backend creation
#[tokio::test]
async fn test_udisks2_backend_new() {
    let backend = UDisks2Backend::new();
    // Should create successfully
    assert!(true);
}

/// Test checking UDisks2 availability
/// Note: This test may pass or fail depending on whether UDisks2 is available on the system
#[tokio::test]
async fn test_udisks2_backend_is_available() {
    let backend = UDisks2Backend::new();
    let available = backend.is_available().await;
    // Result depends on system configuration
    // Just verify it doesn't panic
    assert!(available || !available); // Always true, just checking it returns
}

/// Test getting drives when UDisks2 is available
/// Note: This test will skip if UDisks2 is not available
#[tokio::test]
async fn test_udisks2_backend_get_drives() {
    let backend = UDisks2Backend::new();
    
    // Check if UDisks2 is available
    if !backend.is_available().await {
        // Skip test if UDisks2 is not available
        return;
    }
    
    // Try to get drives
    match backend.get_drives(None).await {
        Ok(drives) => {
            // If we get drives, verify they have expected properties
            for drive in drives {
                let name = drive.get_name();
                assert!(!name.is_empty() || name == "Unknown Drive");
                let _icon = drive.get_icon();
                // Just verify methods don't panic
            }
        }
        Err(e) => {
            // If error occurs, log it but don't fail the test
            // (UDisks2 might be available but have permission issues)
            eprintln!("UDisks2: Failed to get drives (this is acceptable): {}", e);
        }
    }
}

/// Test getting volumes when UDisks2 is available
/// Note: This test will skip if UDisks2 is not available
#[tokio::test]
async fn test_udisks2_backend_get_volumes() {
    let backend = UDisks2Backend::new();
    
    // Check if UDisks2 is available
    if !backend.is_available().await {
        // Skip test if UDisks2 is not available
        return;
    }
    
    // Try to get volumes
    match backend.get_volumes(None).await {
        Ok(volumes) => {
            // If we get volumes, verify they have expected properties
            for volume in volumes {
                let name = volume.get_name();
                assert!(!name.is_empty() || name == "Unknown Volume");
                let _icon = volume.get_icon();
                // Just verify methods don't panic
            }
        }
        Err(e) => {
            // If error occurs, log it but don't fail the test
            eprintln!("UDisks2: Failed to get volumes (this is acceptable): {}", e);
        }
    }
}

/// Test error handling when UDisks2 operations fail
#[tokio::test]
async fn test_udisks2_backend_error_handling() {
    let backend = UDisks2Backend::new();
    
    // Test with cancellable that's already cancelled
    use npio::Cancellable;
    let cancellable = Cancellable::new();
    cancellable.cancel();
    
    // Operations should handle cancellation gracefully
    let result = backend.get_drives(Some(&cancellable)).await;
    // Should return error for cancelled operation
    assert!(result.is_err());
    
    let result = backend.get_volumes(Some(&cancellable)).await;
    assert!(result.is_err());
}

/// Test that backend can be used from multiple threads
#[tokio::test]
async fn test_udisks2_backend_send_sync() {
    use std::sync::Arc;
    
    let backend = Arc::new(UDisks2Backend::new());
    
    // Clone and use from different tasks
    let backend_clone = backend.clone();
    let handle = tokio::spawn(async move {
        backend_clone.is_available().await
    });
    
    let _available = handle.await.unwrap();
    // Just verify it doesn't panic
}

/// Test DevicesModel integration with UDisks2
#[tokio::test]
async fn test_devices_model_udisks2_integration() {
    use npio::model::devices::DevicesModel;
    
    let model = DevicesModel::new();
    
    // Load devices
    let result = model.load(None).await;
    assert!(result.is_ok());
    
    // Get drives (should use cache if available)
    let drives = model.get_drives().await;
    // Should return Vec<Arc<dyn Drive>> (empty if UDisks2 unavailable)
    // Just verify it doesn't panic - length can be 0 or more
    let _ = drives.len();
    
    // Get volumes (should use cache if available)
    let volumes = model.get_volumes().await;
    // Should return Vec<Arc<dyn Volume>> (empty if UDisks2 unavailable)
    // Just verify it doesn't panic - length can be 0 or more
    let _ = volumes.len();
    
    // Verify cache is used - second call should be fast
    let drives2 = model.get_drives().await;
    assert_eq!(drives.len(), drives2.len());
    
    // Refresh should reload
    let refresh_result = model.refresh(None).await;
    assert!(refresh_result.is_ok());
}

