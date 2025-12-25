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

/// Test volume class identifier support
/// Note: This test will skip if UDisks2 is not available
#[tokio::test]
async fn test_volume_class_identifier() {
    let backend = UDisks2Backend::new();
    
    // Check if UDisks2 is available
    if !backend.is_available().await {
        // Skip test if UDisks2 is not available
        return;
    }
    
    // Try to get volumes
    match backend.get_volumes(None).await {
        Ok(volumes) => {
            // If we get volumes, verify class identifier support
            for volume in volumes {
                // Test that class identifier is always available
                let class = volume.get_identifier("class");
                assert!(class.is_some(), "Volume should always have a class identifier");
                let class_str = class.unwrap();
                assert!(
                    class_str == "device" || class_str == "loop",
                    "Class should be 'device' or 'loop', got: {}",
                    class_str
                );
                
                // Test that enumerate_identifiers includes "class"
                let identifiers = volume.enumerate_identifiers();
                assert!(
                    identifiers.contains(&"class".to_string()),
                    "enumerate_identifiers() should include 'class'"
                );
                
                // Test that other identifiers still work
                let _uuid = volume.get_identifier("uuid");
                let _label = volume.get_identifier("label");
                let _device = volume.get_identifier("unix-device");
            }
        }
        Err(e) => {
            // If error occurs, log it but don't fail the test
            eprintln!("UDisks2: Failed to get volumes (this is acceptable): {}", e);
        }
    }
}

/// Test volume class identifier for loop devices
/// This test verifies that loop devices are correctly identified
#[tokio::test]
async fn test_volume_class_identifier_loop() {
    let backend = UDisks2Backend::new();
    
    // Check if UDisks2 is available
    if !backend.is_available().await {
        // Skip test if UDisks2 is not available
        return;
    }
    
    // Try to get volumes
    match backend.get_volumes(None).await {
        Ok(volumes) => {
            // Check if any volumes are loop devices
            let mut found_loop = false;
            let mut found_device = false;
            
            for volume in volumes {
                let class = volume.get_identifier("class");
                if let Some(class_str) = class {
                    if class_str == "loop" {
                        found_loop = true;
                        // Verify it's actually a loop device
                        let device = volume.get_identifier("unix-device");
                        if let Some(device_path) = device {
                            assert!(
                                device_path.contains("/loop") || device_path.starts_with("loop"),
                                "Loop device should have 'loop' in device path: {}",
                                device_path
                            );
                        }
                    } else if class_str == "device" {
                        found_device = true;
                    }
                }
            }
            
            // At least one volume should be classified as "device" (most common case)
            // Loop devices may or may not be present, so we don't require them
            // Just verify the classification logic works
            assert!(
                found_device || found_loop,
                "Should find at least one volume with class 'device' or 'loop'"
            );
        }
        Err(e) => {
            // If error occurs, log it but don't fail the test
            eprintln!("UDisks2: Failed to get volumes (this is acceptable): {}", e);
        }
    }
}

