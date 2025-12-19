//! UDisks2 backend implementation
//!
//! Integrates with UDisks2 via D-Bus to provide drive and volume management.

use std::collections::HashMap;
use zbus::{Connection, zvariant};
use crate::error::{NpioError, NpioResult, IOErrorEnum};
use crate::drive::Drive;
use crate::volume::Volume;
use crate::mount::Mount;
use crate::cancellable::Cancellable;

/// UDisks2 D-Bus service name
const UDISKS2_SERVICE: &str = "org.freedesktop.UDisks2";
/// UDisks2 manager path
const UDISKS2_MANAGER_PATH: &str = "/org/freedesktop/UDisks2/Manager";

/// UDisks2 backend for device management
pub struct UDisks2Backend {
    connection: Option<Arc<Connection>>,
}

use std::sync::Arc;

impl UDisks2Backend {
    /// Creates a new UDisks2 backend
    pub fn new() -> Self {
        Self {
            connection: None,
        }
    }

    /// Connects to UDisks2 D-Bus service
    pub async fn connect(&mut self) -> NpioResult<()> {
        if self.connection.is_none() {
            let connection = Connection::system()
                .await
                .map_err(|e| NpioError::new(
                    IOErrorEnum::Failed,
                    format!("Failed to connect to system D-Bus: {}", e)
                ))?;
            self.connection = Some(Arc::new(connection));
        }
        Ok(())
    }

    /// Checks if UDisks2 is available
    pub async fn is_available(&mut self) -> bool {
        if self.connect().await.is_err() {
            return false;
        }

        // Try to get manager interface
        if let Some(conn) = &self.connection {
            // Check if UDisks2 service is available
            let proxy = zbus::Proxy::new(
                conn,
                UDISKS2_SERVICE,
                UDISKS2_MANAGER_PATH,
                "org.freedesktop.UDisks2.Manager",
            ).await;

            proxy.is_ok()
        } else {
            false
        }
    }

    /// Gets all drives from UDisks2
    pub async fn get_drives(
        &mut self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Vec<Box<dyn Drive>>> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        self.connect().await?;

        let conn = self.connection.as_ref().ok_or_else(|| {
            NpioError::new(IOErrorEnum::Failed, "Not connected to D-Bus")
        })?;

        // Get manager interface
        let manager = zbus::Proxy::new(
            conn,
            UDISKS2_SERVICE,
            UDISKS2_MANAGER_PATH,
            "org.freedesktop.UDisks2.Manager",
        )
        .await
        .map_err(|e| NpioError::new(
            IOErrorEnum::Failed,
            format!("Failed to get UDisks2 manager: {}", e)
        ))?;

        // Get all managed objects
        // GetManagedObjects returns: a{oa{sa{sv}}}
        // This is a dict of object paths to dicts of interface names to dicts of property names to values
        let reply = manager
            .call_method("GetManagedObjects", &())
            .await
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Failed to get managed objects: {}", e)
            ))?;
        
        let body = reply.body();
        let objects: std::collections::HashMap<
            zbus::zvariant::OwnedObjectPath,
            std::collections::HashMap<String, std::collections::HashMap<String, zbus::zvariant::Value>>
        > = body.deserialize().map_err(|e| NpioError::new(
            IOErrorEnum::Failed,
            format!("Failed to parse response: {}", e)
        ))?;

        let mut result = Vec::new();

        // Parse drive objects and create Drive instances
        for (path, interfaces) in objects {
            let path_str = path.as_str();
            if path_str.contains("/drives/") {
                if let Some(c) = cancellable {
                    c.check()?;
                }

                // Check if it has Drive interface
                if interfaces.contains_key("org.freedesktop.UDisks2.Drive") {
                    if let Ok(drive) = UDisks2Drive::new(conn.clone(), path_str).await {
                        result.push(Box::new(drive) as Box<dyn Drive>);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Gets all volumes (block devices with filesystems) from UDisks2
    pub async fn get_volumes(
        &mut self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Vec<Box<dyn Volume>>> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        self.connect().await?;

        let conn = self.connection.as_ref().ok_or_else(|| {
            NpioError::new(IOErrorEnum::Failed, "Not connected to D-Bus")
        })?;

        // Get manager interface
        let manager = zbus::Proxy::new(
            conn,
            UDISKS2_SERVICE,
            UDISKS2_MANAGER_PATH,
            "org.freedesktop.UDisks2.Manager",
        )
        .await
        .map_err(|e| NpioError::new(
            IOErrorEnum::Failed,
            format!("Failed to get UDisks2 manager: {}", e)
        ))?;

        // Get all block device objects
        let reply = manager
            .call_method("GetManagedObjects", &())
            .await
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Failed to get managed objects: {}", e)
            ))?;
        
        let body = reply.body();
        let objects: std::collections::HashMap<
            zbus::zvariant::OwnedObjectPath,
            std::collections::HashMap<String, std::collections::HashMap<String, zbus::zvariant::Value>>
        > = body.deserialize().map_err(|e| NpioError::new(
            IOErrorEnum::Failed,
            format!("Failed to parse response: {}", e)
        ))?;

        let mut result = Vec::new();

        // Find block devices with filesystem interface
        for (path, interfaces) in objects {
            if path.contains("/block_devices/") {
                if let Some(c) = cancellable {
                    c.check()?;
                }

                // Check if it has filesystem interface
                if interfaces.contains_key("org.freedesktop.UDisks2.Filesystem") {
                    if let Ok(volume) = UDisks2Volume::new(conn.clone(), path.as_str()).await {
                        result.push(Box::new(volume) as Box<dyn Volume>);
                    }
                }
            }
        }

        Ok(result)
    }
}

impl Default for UDisks2Backend {
    fn default() -> Self {
        Self::new()
    }
}

/// UDisks2 Drive implementation
#[derive(Debug)]
struct UDisks2Drive {
    connection: Arc<Connection>,
    path: String,
    name: String,
    vendor: String,
    model: String,
    is_removable: bool,
    is_media_removable: bool,
    has_media: bool,
    can_eject: bool,
    device: Option<String>,
}

impl UDisks2Drive {
    async fn new(connection: Arc<Connection>, path: &str) -> NpioResult<Self> {
        let path_obj = zbus::zvariant::ObjectPath::try_from(path)
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Invalid object path: {}", e)
            ))?;
        let proxy = zbus::Proxy::new(
            &*connection,
            UDISKS2_SERVICE,
            path_obj,
            "org.freedesktop.UDisks2.Drive",
        )
        .await
        .map_err(|e| NpioError::new(
            IOErrorEnum::Failed,
            format!("Failed to create drive proxy: {}", e)
        ))?;

        // Get drive properties
        // Note: get_property returns Value, we need to extract the actual type
        let vendor: String = proxy.get_property("Vendor")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Str(s) = v {
                    Some(s.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| String::new());
        let model: String = proxy.get_property("Model")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Str(s) = v {
                    Some(s.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| String::new());
        let is_removable: bool = proxy.get_property("MediaRemovable")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Bool(b) = v {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap_or(false);
        let is_media_removable: bool = proxy.get_property("MediaRemovable")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Bool(b) = v {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap_or(false);
        let has_media: bool = proxy.get_property("MediaAvailable")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Bool(b) = v {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap_or(false);
        let can_eject: bool = proxy.get_property("Ejectable")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Bool(b) = v {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap_or(false);
        let device: Option<String> = proxy.get_property("Device")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Str(s) = v {
                    Some(s.to_string())
                } else {
                    None
                }
            });

        let name = if !vendor.is_empty() && !model.is_empty() {
            format!("{} {}", vendor, model)
        } else if !model.is_empty() {
            model.clone()
        } else {
            path.split('/').last().unwrap_or("Unknown Drive").to_string()
        };

        Ok(Self {
            connection,
            path: path.to_string(),
            name,
            vendor,
            model,
            is_removable,
            is_media_removable,
            has_media,
            can_eject,
            device,
        })
    }

    fn get_icon_name(&self) -> String {
        if self.is_removable {
            if self.model.to_lowercase().contains("cd") || 
               self.model.to_lowercase().contains("dvd") ||
               self.model.to_lowercase().contains("optical") {
                "drive-optical".to_string()
            } else {
                "drive-removable-media".to_string()
            }
        } else {
            "drive-harddisk".to_string()
        }
    }
}

#[async_trait::async_trait]
impl Drive for UDisks2Drive {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_icon(&self) -> String {
        self.get_icon_name()
    }

    fn has_volumes(&self) -> bool {
        self.has_media
    }

    fn get_volumes(&self) -> Vec<Box<dyn Volume>> {
        // TODO: Get volumes for this drive
        Vec::new()
    }

    fn is_removable(&self) -> bool {
        self.is_removable
    }

    fn is_media_removable(&self) -> bool {
        self.is_media_removable
    }

    fn has_media(&self) -> bool {
        self.has_media
    }

    fn is_media_check_automatic(&self) -> bool {
        true // UDisks2 handles this automatically
    }

    fn can_poll_for_media(&self) -> bool {
        false // UDisks2 handles media detection automatically
    }

    fn can_eject(&self) -> bool {
        self.can_eject
    }

    async fn eject(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        if !self.can_eject {
            return Err(NpioError::new(
                IOErrorEnum::NotSupported,
                "Drive is not ejectable",
            ));
        }

        let path_obj = zbus::zvariant::ObjectPath::try_from(self.path.as_str())
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Invalid object path: {}", e)
            ))?;
        let proxy = zbus::Proxy::new(
            &*self.connection,
            UDISKS2_SERVICE,
            path_obj,
            "org.freedesktop.UDisks2.Drive",
        )
        .await
        .map_err(|e| NpioError::new(
            IOErrorEnum::Failed,
            format!("Failed to create drive proxy: {}", e)
        ))?;

        proxy
            .call_method("Eject", &(HashMap::<String, zvariant::Value>::new()))
            .await
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Failed to eject drive: {}", e)
            ))?;

        Ok(())
    }

    async fn poll_for_media(
        &self,
        _cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        // UDisks2 handles media detection automatically
        Ok(())
    }

    fn get_identifier(&self, kind: &str) -> Option<String> {
        match kind {
            "unix-device" => self.device.clone(),
            _ => None,
        }
    }

    fn enumerate_identifiers(&self) -> Vec<String> {
        let mut result = Vec::new();
        if self.device.is_some() {
            result.push("unix-device".to_string());
        }
        result
    }
}

/// UDisks2 Volume implementation
#[derive(Debug)]
struct UDisks2Volume {
    connection: Arc<Connection>,
    path: String,
    name: String,
    uuid: Option<String>,
    label: Option<String>,
    mount_point: Option<String>,
    device: Option<String>,
    can_mount: bool,
    can_eject: bool,
}

impl UDisks2Volume {
    async fn new(connection: Arc<Connection>, path: &str) -> NpioResult<Self> {
        let path_obj = zbus::zvariant::ObjectPath::try_from(path)
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Invalid object path: {}", e)
            ))?;
        let block_proxy = zbus::Proxy::new(
            &*connection,
            UDISKS2_SERVICE,
            path_obj.clone(),
            "org.freedesktop.UDisks2.Block",
        )
        .await
        .map_err(|e| NpioError::new(
            IOErrorEnum::Failed,
            format!("Failed to create block proxy: {}", e)
        ))?;

        let fs_proxy = zbus::Proxy::new(
            &*connection,
            UDISKS2_SERVICE,
            path_obj,
            "org.freedesktop.UDisks2.Filesystem",
        )
        .await
        .map_err(|e| NpioError::new(
            IOErrorEnum::Failed,
            format!("Failed to create filesystem proxy: {}", e)
        ))?;

        // Get properties
        let label: Option<String> = block_proxy.get_property("IdLabel")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Str(s) = v {
                    Some(s.to_string())
                } else {
                    None
                }
            });
        let uuid: Option<String> = block_proxy.get_property("IdUuid")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Str(s) = v {
                    Some(s.to_string())
                } else {
                    None
                }
            });
        let device: Option<String> = block_proxy.get_property("Device")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Str(s) = v {
                    Some(s.to_string())
                } else {
                    None
                }
            });

        // Get mount points (array of byte arrays)
        let mount_points: Vec<Vec<u8>> = fs_proxy.get_property("MountPoints")
            .await
            .ok()
            .and_then(|v| {
                if let zbus::zvariant::Value::Array(arr) = v {
                    let mut result = Vec::new();
                    for item in arr.iter() {
                        if let zbus::zvariant::Value::Array(bytes) = item {
                            result.push(bytes.iter().map(|b| {
                                if let zbus::zvariant::Value::U8(byte) = b {
                                    *byte
                                } else {
                                    0
                                }
                            }).collect());
                        }
                    }
                    Some(result)
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let mount_point = mount_points.first()
            .and_then(|mp| String::from_utf8(mp.clone()).ok());

        let name = label.clone()
            .or_else(|| uuid.clone())
            .unwrap_or_else(|| {
                path.split('/').last().unwrap_or("Unknown Volume").to_string()
            });

        // Check if can mount/eject
        let can_mount = mount_point.is_none();
        let can_eject = false; // Would need to check drive properties

        Ok(Self {
            connection,
            path: path.to_string(),
            name,
            uuid,
            label,
            mount_point,
            device,
            can_mount,
            can_eject,
        })
    }

    fn get_icon_name(&self) -> String {
        // Determine icon based on filesystem type or device
        if self.device.as_ref().map(|d| d.contains("sr")).unwrap_or(false) {
            "drive-optical".to_string()
        } else if self.device.as_ref().map(|d| d.contains("mmc")).unwrap_or(false) {
            "media-flash".to_string()
        } else {
            "drive-harddisk".to_string()
        }
    }
}

#[async_trait::async_trait]
impl Volume for UDisks2Volume {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_icon(&self) -> String {
        self.get_icon_name()
    }

    fn get_uuid(&self) -> Option<String> {
        self.uuid.clone()
    }

    fn get_drive(&self) -> Option<Box<dyn Drive>> {
        // TODO: Get drive for this volume
        None
    }

    fn get_mount(&self) -> Option<Box<dyn Mount>> {
        // TODO: Get mount for this volume
        None
    }

    fn can_mount(&self) -> bool {
        self.can_mount
    }

    fn can_eject(&self) -> bool {
        self.can_eject
    }

    fn should_automount(&self) -> bool {
        true // UDisks2 handles automount
    }

    async fn mount(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        if !self.can_mount {
            return Err(NpioError::new(
                IOErrorEnum::NotSupported,
                "Volume cannot be mounted",
            ));
        }

        let path_obj = zbus::zvariant::ObjectPath::try_from(self.path.as_str())
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Invalid object path: {}", e)
            ))?;
        let proxy = zbus::Proxy::new(
            &*self.connection,
            UDISKS2_SERVICE,
            path_obj,
            "org.freedesktop.UDisks2.Filesystem",
        )
        .await
        .map_err(|e| NpioError::new(
            IOErrorEnum::Failed,
            format!("Failed to create filesystem proxy: {}", e)
        ))?;

        let mut options = HashMap::<String, zvariant::Value>::new();
        options.insert("auth.no_user_interaction".to_string(), true.into());

        proxy
            .call_method("Mount", &(options))
            .await
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Failed to mount volume: {}", e)
            ))?;

        Ok(())
    }

    async fn eject(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        if !self.can_eject {
            return Err(NpioError::new(
                IOErrorEnum::NotSupported,
                "Volume cannot be ejected",
            ));
        }

        // First unmount if mounted
        if self.mount_point.is_some() {
            // Unmount first
            let path_obj = zbus::zvariant::ObjectPath::try_from(self.path.as_str())
                .map_err(|e| NpioError::new(
                    IOErrorEnum::Failed,
                    format!("Invalid object path: {}", e)
                ))?;
            let proxy = zbus::Proxy::new(
                &*self.connection,
                UDISKS2_SERVICE,
                path_obj,
                "org.freedesktop.UDisks2.Filesystem",
            )
            .await
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Failed to create filesystem proxy: {}", e)
            ))?;

            let mut options = HashMap::<String, zvariant::Value>::new();
            options.insert("force".to_string(), false.into());

            proxy
                .call_method("Unmount", &(options))
                .await
                .map_err(|e| NpioError::new(
                    IOErrorEnum::Failed,
                    format!("Failed to unmount volume: {}", e)
                ))?;
        }

        // Then eject via drive
        // TODO: Get drive and eject
        Ok(())
    }

    fn get_identifier(&self, kind: &str) -> Option<String> {
        match kind {
            "uuid" => self.uuid.clone(),
            "label" => self.label.clone(),
            "unix-device" => self.device.clone(),
            _ => None,
        }
    }

    fn enumerate_identifiers(&self) -> Vec<String> {
        let mut result = Vec::new();
        if self.uuid.is_some() {
            result.push("uuid".to_string());
        }
        if self.label.is_some() {
            result.push("label".to_string());
        }
        if self.device.is_some() {
            result.push("unix-device".to_string());
        }
        result
    }
}

