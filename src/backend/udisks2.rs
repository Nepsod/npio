//! UDisks2 backend implementation
//!
//! Integrates with UDisks2 via D-Bus to provide drive and volume management.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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
    connection: Arc<Mutex<Option<Arc<Connection>>>>,
}

impl UDisks2Backend {
    /// Creates a new UDisks2 backend
    pub fn new() -> Self {
        Self {
            connection: Arc::new(Mutex::new(None)),
        }
    }

    /// Connects to UDisks2 D-Bus service (internal)
    async fn ensure_connected(&self) -> NpioResult<Arc<Connection>> {
        // Check if already connected
        {
            let guard = self.connection.lock().map_err(|e| {
                eprintln!("Failed to acquire lock on UDisks2 connection: {}", e);
                NpioError::new(IOErrorEnum::Failed, format!("Lock poisoned: {}", e))
            })?;
            if let Some(conn) = guard.as_ref() {
                return Ok(conn.clone());
            }
        }

        // Need to connect
        let connection = Connection::system()
            .await
            .map_err(|e| NpioError::new(
                IOErrorEnum::Failed,
                format!("Failed to connect to system D-Bus: {}", e)
            ))?;
        let connection = Arc::new(connection);

        // Store the connection
        {
            let mut guard = self.connection.lock().map_err(|e| {
                eprintln!("Failed to acquire lock on UDisks2 connection: {}", e);
                NpioError::new(IOErrorEnum::Failed, format!("Lock poisoned: {}", e))
            })?;
            // Double-check in case another thread connected while we were connecting
            if guard.is_none() {
                *guard = Some(connection.clone());
            } else {
                return Ok(guard.as_ref().unwrap().clone());
            }
        }

        Ok(connection)
    }

    /// Checks if UDisks2 is available
    pub async fn is_available(&self) -> bool {
        let conn = match self.ensure_connected().await {
            Ok(conn) => conn,
            Err(_) => return false,
        };

        // Try to get manager interface
        let proxy = zbus::Proxy::new(
            &*conn,
            UDISKS2_SERVICE,
            UDISKS2_MANAGER_PATH,
            "org.freedesktop.UDisks2.Manager",
        ).await;

        proxy.is_ok()
    }

    /// Gets all drives from UDisks2
    pub async fn get_drives(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Vec<Box<dyn Drive>>> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let conn = self.ensure_connected().await?;

        // Get manager interface
        let manager = zbus::Proxy::new(
            &*conn,
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
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Vec<Box<dyn Volume>>> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        let conn = self.ensure_connected().await?;

        // Get manager interface
        let manager = zbus::Proxy::new(
            &*conn,
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

        // Helper function to extract string property with error logging
        async fn get_property_str(proxy: &zbus::Proxy<'_>, name: &str) -> String {
            match proxy.get_property(name).await {
                Ok(v) => {
                    if let zbus::zvariant::Value::Str(s) = v {
                        s.to_string()
                    } else {
                        eprintln!("UDisks2: Property '{}' has unexpected type, expected string", name);
                        String::new()
                    }
                }
                Err(e) => {
                    eprintln!("UDisks2: Failed to get property '{}': {}", name, e);
                    String::new()
                }
            }
        }

        // Helper function to extract bool property with error logging
        async fn get_property_bool(proxy: &zbus::Proxy<'_>, name: &str) -> bool {
            match proxy.get_property(name).await {
                Ok(v) => {
                    if let zbus::zvariant::Value::Bool(b) = v {
                        b
                    } else {
                        eprintln!("UDisks2: Property '{}' has unexpected type, expected bool", name);
                        false
                    }
                }
                Err(e) => {
                    eprintln!("UDisks2: Failed to get property '{}': {}", name, e);
                    false
                }
            }
        }

        // Helper function to extract optional string property with error logging
        async fn get_property_str_opt(proxy: &zbus::Proxy<'_>, name: &str) -> Option<String> {
            match proxy.get_property(name).await {
                Ok(v) => {
                    if let zbus::zvariant::Value::Str(s) = v {
                        Some(s.to_string())
                    } else {
                        eprintln!("UDisks2: Property '{}' has unexpected type, expected string", name);
                        None
                    }
                }
                Err(e) => {
                    eprintln!("UDisks2: Failed to get property '{}': {}", name, e);
                    None
                }
            }
        }

        // Get drive properties
        let vendor = get_property_str(&proxy, "Vendor").await;
        let model = get_property_str(&proxy, "Model").await;
        // Removable: whether the drive itself can be removed (e.g., USB stick)
        let is_removable = get_property_bool(&proxy, "Removable").await;
        // MediaRemovable: whether the media inside can be removed (e.g., CD in CD drive)
        let is_media_removable = get_property_bool(&proxy, "MediaRemovable").await;
        let has_media = get_property_bool(&proxy, "MediaAvailable").await;
        let can_eject = get_property_bool(&proxy, "Ejectable").await;
        let device = get_property_str_opt(&proxy, "Device").await;

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

        // Helper function to extract optional string property with error logging
        async fn get_property_str_opt(proxy: &zbus::Proxy<'_>, name: &str) -> Option<String> {
            match proxy.get_property(name).await {
                Ok(v) => {
                    if let zbus::zvariant::Value::Str(s) = v {
                        Some(s.to_string())
                    } else {
                        eprintln!("UDisks2: Property '{}' has unexpected type, expected string", name);
                        None
                    }
                }
                Err(e) => {
                    eprintln!("UDisks2: Failed to get property '{}': {}", name, e);
                    None
                }
            }
        }

        // Get properties
        let label = get_property_str_opt(&block_proxy, "IdLabel").await;
        let uuid = get_property_str_opt(&block_proxy, "IdUuid").await;
        let device = get_property_str_opt(&block_proxy, "Device").await;

        // Get mount points (array of byte arrays)
        let mount_points: Vec<Vec<u8>> = match fs_proxy.get_property("MountPoints").await {
            Ok(v) => {
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
                    result
                } else {
                    eprintln!("UDisks2: Property 'MountPoints' has unexpected type, expected array");
                    Vec::new()
                }
            }
            Err(e) => {
                eprintln!("UDisks2: Failed to get property 'MountPoints': {}", e);
                Vec::new()
            }
        };

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

