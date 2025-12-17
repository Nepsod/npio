use async_trait::async_trait;
use crate::cancellable::Cancellable;
use crate::error::NpioResult;
use crate::drive::Drive;
use crate::mount::Mount;
use crate::file::File;

#[async_trait]
pub trait Volume: Send + Sync + std::fmt::Debug {
    /// Gets the name of the volume.
    fn get_name(&self) -> String;

    /// Gets the icon name for the volume.
    fn get_icon(&self) -> String;

    /// Gets the symbolic icon name for the volume.
    fn get_symbolic_icon(&self) -> Option<String> {
        None
    }

    /// Gets the UUID of the volume.
    fn get_uuid(&self) -> Option<String>;

    /// Gets the drive this volume is on.
    fn get_drive(&self) -> Option<Box<dyn Drive>>;

    /// Gets the mount for this volume, if mounted.
    fn get_mount(&self) -> Option<Box<dyn Mount>>;

    /// Checks if the volume can be mounted.
    fn can_mount(&self) -> bool;

    /// Checks if the volume can be ejected.
    fn can_eject(&self) -> bool;

    /// Checks if the volume should be automatically mounted.
    fn should_automount(&self) -> bool;

    /// Mounts the volume.
    async fn mount(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Ejects the volume.
    async fn eject(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Gets an identifier of the given kind.
    /// Common kinds: "unix-device", "label", "uuid", "class"
    fn get_identifier(&self, kind: &str) -> Option<String>;

    /// Enumerates all identifier kinds available for this volume.
    fn enumerate_identifiers(&self) -> Vec<String>;

    /// Gets the activation root file for this volume.
    fn get_activation_root(&self) -> Option<Box<dyn File>> {
        None
    }

    /// Gets a sort key for ordering volumes.
    fn get_sort_key(&self) -> Option<String> {
        None
    }
}

