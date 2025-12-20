use npio::model::devices::DevicesModel;

#[tokio::test]
async fn test_devices_model_new() {
    let _ = DevicesModel::new();
    // Should create successfully
    assert!(true);
}

#[tokio::test]
async fn test_devices_model_load() {
    let model = DevicesModel::new();
    let result = model.load(None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_devices_model_get_mounts() {
    let model = DevicesModel::new();
    let mounts = model.get_mounts().await;
    // Should have at least the root mount
    assert!(!mounts.is_empty());
}

#[tokio::test]
async fn test_devices_model_get_drives() {
    let model = DevicesModel::new();
    let drives = model.get_drives().await;
    // Currently returns empty (UDisks2 integration pending)
    assert_eq!(drives.len(), 0);
}

#[tokio::test]
async fn test_devices_model_get_volumes() {
    let model = DevicesModel::new();
    let volumes = model.get_volumes().await;
    // Currently returns empty (UDisks2 integration pending)
    assert_eq!(volumes.len(), 0);
}

#[tokio::test]
async fn test_devices_model_get_mount_for_path() {
    let model = DevicesModel::new();
    
    // Get mount for root
    let root_mount = model.get_mount_for_path(std::path::Path::new("/"), None).await;
    assert!(root_mount.is_ok());
    assert!(root_mount.unwrap().is_some());
    
    // Get mount for home (if exists)
    if let Ok(home) = std::env::var("HOME") {
        let home_mount = model.get_mount_for_path(std::path::Path::new(&home), None).await;
        assert!(home_mount.is_ok());
    }
}

#[tokio::test]
async fn test_devices_model_refresh() {
    let model = DevicesModel::new();
    let result = model.refresh(None).await;
    assert!(result.is_ok());
}

