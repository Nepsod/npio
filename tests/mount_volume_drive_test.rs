use async_trait::async_trait;
use npio::{Drive, Volume, Mount, File, Cancellable};
use npio::error::{NpioResult, NpioError, IOErrorEnum};

// Mock implementations for testing

#[derive(Debug)]
struct MockDrive {
    name: String,
    icon: String,
    removable: bool,
    has_media: bool,
    can_eject: bool,
}

#[async_trait]
impl Drive for MockDrive {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_icon(&self) -> String {
        self.icon.clone()
    }

    fn has_volumes(&self) -> bool {
        false
    }

    fn get_volumes(&self) -> Vec<Box<dyn Volume>> {
        vec![]
    }

    fn is_removable(&self) -> bool {
        self.removable
    }

    fn is_media_removable(&self) -> bool {
        self.removable
    }

    fn has_media(&self) -> bool {
        self.has_media
    }

    fn is_media_check_automatic(&self) -> bool {
        true
    }

    fn can_poll_for_media(&self) -> bool {
        false
    }

    fn can_eject(&self) -> bool {
        self.can_eject
    }

    async fn eject(&self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if self.can_eject {
            Ok(())
        } else {
            Err(NpioError::new(IOErrorEnum::NotSupported, "Cannot eject"))
        }
    }

    async fn poll_for_media(&self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        Ok(())
    }

    fn get_identifier(&self, kind: &str) -> Option<String> {
        if kind == "unix-device" {
            Some("/dev/sda".to_string())
        } else {
            None
        }
    }

    fn enumerate_identifiers(&self) -> Vec<String> {
        vec!["unix-device".to_string()]
    }
}

#[derive(Debug)]
struct MockVolume {
    name: String,
    icon: String,
    uuid: Option<String>,
    can_mount: bool,
    can_eject: bool,
    #[allow(dead_code)]
    drive: Option<Box<dyn Drive>>,
}

#[async_trait]
impl Volume for MockVolume {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_icon(&self) -> String {
        self.icon.clone()
    }

    fn get_uuid(&self) -> Option<String> {
        self.uuid.clone()
    }

    fn get_drive(&self) -> Option<Box<dyn Drive>> {
        // Can't clone trait objects, so return None for testing
        // In real implementation, this would use Arc or similar
        None
    }

    fn get_mount(&self) -> Option<Box<dyn Mount>> {
        None
    }

    fn can_mount(&self) -> bool {
        self.can_mount
    }

    fn can_eject(&self) -> bool {
        self.can_eject
    }

    fn should_automount(&self) -> bool {
        true
    }

    async fn mount(&self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if self.can_mount {
            Ok(())
        } else {
            Err(NpioError::new(IOErrorEnum::NotSupported, "Cannot mount"))
        }
    }

    async fn eject(&self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if self.can_eject {
            Ok(())
        } else {
            Err(NpioError::new(IOErrorEnum::NotSupported, "Cannot eject"))
        }
    }

    fn get_identifier(&self, kind: &str) -> Option<String> {
        match kind {
            "uuid" => self.uuid.clone(),
            "label" => Some("MyVolume".to_string()),
            _ => None,
        }
    }

    fn enumerate_identifiers(&self) -> Vec<String> {
        let mut ids = vec!["label".to_string()];
        if self.uuid.is_some() {
            ids.push("uuid".to_string());
        }
        ids
    }
}

#[derive(Debug)]
struct MockMount {
    name: String,
    icon: String,
    uuid: Option<String>,
    root_path: String,
    can_unmount: bool,
    can_eject: bool,
    #[allow(dead_code)]
    volume: Option<Box<dyn Volume>>,
    #[allow(dead_code)]
    drive: Option<Box<dyn Drive>>,
}

#[async_trait]
impl Mount for MockMount {
    fn get_root(&self) -> Box<dyn File> {
        use npio::backend::local::LocalBackend;
        use npio::{get_file_for_uri, register_backend};
        use std::sync::Arc;
        
        // Register backend if not already registered
        let backend = Arc::new(LocalBackend::new());
        register_backend(backend);
        
        let uri = format!("file://{}", self.root_path);
        get_file_for_uri(&uri).expect("Failed to create file")
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_icon(&self) -> String {
        self.icon.clone()
    }

    fn get_uuid(&self) -> Option<String> {
        self.uuid.clone()
    }

    fn get_volume(&self) -> Option<Box<dyn Volume>> {
        // Can't clone trait objects, so return None for testing
        // In real implementation, this would use Arc or similar
        None
    }

    fn get_drive(&self) -> Option<Box<dyn Drive>> {
        // Can't clone trait objects, so return None for testing
        // In real implementation, this would use Arc or similar
        None
    }

    fn can_unmount(&self) -> bool {
        self.can_unmount
    }

    fn can_eject(&self) -> bool {
        self.can_eject
    }

    async fn unmount(&self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if self.can_unmount {
            Ok(())
        } else {
            Err(NpioError::new(IOErrorEnum::NotSupported, "Cannot unmount"))
        }
    }

    async fn eject(&self, _cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if self.can_eject {
            Ok(())
        } else {
            Err(NpioError::new(IOErrorEnum::NotSupported, "Cannot eject"))
        }
    }
}

// Tests

#[test]
fn test_drive_basic() {
    let drive = MockDrive {
        name: "USB Drive".to_string(),
        icon: "drive-removable-media".to_string(),
        removable: true,
        has_media: true,
        can_eject: true,
    };

    assert_eq!(drive.get_name(), "USB Drive");
    assert_eq!(drive.get_icon(), "drive-removable-media");
    assert!(drive.is_removable());
    assert!(drive.has_media());
    assert!(drive.can_eject());
    assert!(!drive.has_volumes());
    assert_eq!(drive.get_volumes().len(), 0);
}

#[test]
fn test_drive_identifiers() {
    let drive = MockDrive {
        name: "USB Drive".to_string(),
        icon: "drive-removable-media".to_string(),
        removable: true,
        has_media: true,
        can_eject: true,
    };

    let identifier = drive.get_identifier("unix-device");
    assert_eq!(identifier, Some("/dev/sda".to_string()));

    let identifiers = drive.enumerate_identifiers();
    assert!(identifiers.contains(&"unix-device".to_string()));
}

#[tokio::test]
async fn test_drive_eject() {
    let drive = MockDrive {
        name: "USB Drive".to_string(),
        icon: "drive-removable-media".to_string(),
        removable: true,
        has_media: true,
        can_eject: true,
    };

    let result = drive.eject(None).await;
    assert!(result.is_ok());

    let drive_no_eject = MockDrive {
        name: "Internal Drive".to_string(),
        icon: "drive-harddisk".to_string(),
        removable: false,
        has_media: true,
        can_eject: false,
    };

    let result = drive_no_eject.eject(None).await;
    assert!(result.is_err());
}

#[test]
fn test_volume_basic() {
    let volume = MockVolume {
        name: "My Volume".to_string(),
        icon: "drive-removable-media-usb".to_string(),
        uuid: Some("1234-5678".to_string()),
        can_mount: true,
        can_eject: true,
        drive: None,
    };

    assert_eq!(volume.get_name(), "My Volume");
    assert_eq!(volume.get_icon(), "drive-removable-media-usb");
    assert_eq!(volume.get_uuid(), Some("1234-5678".to_string()));
    assert!(volume.can_mount());
    assert!(volume.can_eject());
    assert!(volume.should_automount());
}

#[test]
fn test_volume_identifiers() {
    let volume = MockVolume {
        name: "My Volume".to_string(),
        icon: "drive-removable-media-usb".to_string(),
        uuid: Some("1234-5678".to_string()),
        can_mount: true,
        can_eject: true,
        drive: None,
    };

    let uuid = volume.get_identifier("uuid");
    assert_eq!(uuid, Some("1234-5678".to_string()));

    let label = volume.get_identifier("label");
    assert_eq!(label, Some("MyVolume".to_string()));

    let identifiers = volume.enumerate_identifiers();
    assert!(identifiers.contains(&"uuid".to_string()));
    assert!(identifiers.contains(&"label".to_string()));
}

#[tokio::test]
async fn test_volume_mount_eject() {
    let volume = MockVolume {
        name: "My Volume".to_string(),
        icon: "drive-removable-media-usb".to_string(),
        uuid: Some("1234-5678".to_string()),
        can_mount: true,
        can_eject: true,
        drive: None,
    };

    let mount_result = volume.mount(None).await;
    assert!(mount_result.is_ok());

    let eject_result = volume.eject(None).await;
    assert!(eject_result.is_ok());

    let volume_no_mount = MockVolume {
        name: "Read-only Volume".to_string(),
        icon: "drive-removable-media-usb".to_string(),
        uuid: None,
        can_mount: false,
        can_eject: false,
        drive: None,
    };

    let mount_result = volume_no_mount.mount(None).await;
    assert!(mount_result.is_err());
}

#[test]
fn test_mount_basic() {
    let mount = MockMount {
        name: "/mnt/usb".to_string(),
        icon: "drive-removable-media-usb".to_string(),
        uuid: Some("1234-5678".to_string()),
        root_path: "/mnt/usb".to_string(),
        can_unmount: true,
        can_eject: true,
        volume: None,
        drive: None,
    };

    assert_eq!(mount.get_name(), "/mnt/usb");
    assert_eq!(mount.get_icon(), "drive-removable-media-usb");
    assert_eq!(mount.get_uuid(), Some("1234-5678".to_string()));
    assert!(mount.can_unmount());
    assert!(mount.can_eject());
}

#[tokio::test]
async fn test_mount_unmount_eject() {
    let mount = MockMount {
        name: "/mnt/usb".to_string(),
        icon: "drive-removable-media-usb".to_string(),
        uuid: Some("1234-5678".to_string()),
        root_path: "/mnt/usb".to_string(),
        can_unmount: true,
        can_eject: true,
        volume: None,
        drive: None,
    };

    let unmount_result = mount.unmount(None).await;
    assert!(unmount_result.is_ok());

    let eject_result = mount.eject(None).await;
    assert!(eject_result.is_ok());

    let mount_no_unmount = MockMount {
        name: "/".to_string(),
        icon: "drive-harddisk".to_string(),
        uuid: None,
        root_path: "/".to_string(),
        can_unmount: false,
        can_eject: false,
        volume: None,
        drive: None,
    };

    let unmount_result = mount_no_unmount.unmount(None).await;
    assert!(unmount_result.is_err());
}

#[tokio::test]
async fn test_mount_get_root() {
    use std::sync::Arc;
    use npio::backend::local::LocalBackend;
    use npio::register_backend;
    
    // Register backend
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);

    let mount = MockMount {
        name: "/tmp".to_string(),
        icon: "folder".to_string(),
        uuid: None,
        root_path: "/tmp".to_string(),
        can_unmount: false,
        can_eject: false,
        volume: None,
        drive: None,
    };

    let root = mount.get_root();
    let uri = root.uri();
    assert!(uri.starts_with("file://"));
    assert!(uri.contains("/tmp"));
}

#[tokio::test]
async fn test_mount_remount() {
    let mount = MockMount {
        name: "/mnt/usb".to_string(),
        icon: "drive-removable-media-usb".to_string(),
        uuid: Some("1234-5678".to_string()),
        root_path: "/mnt/usb".to_string(),
        can_unmount: true,
        can_eject: true,
        volume: None,
        drive: None,
    };

    // Default remount implementation should return NotSupported
    let remount_result = mount.remount(None).await;
    assert!(remount_result.is_err());
    if let Err(e) = remount_result {
        assert_eq!(format!("{:?}", e.kind()), "NotSupported");
    }
}

