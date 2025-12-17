use async_trait::async_trait;
use crate::cancellable::Cancellable;
use crate::error::NpioResult;
use crate::drive::Drive;
use crate::volume::Volume;
use crate::file::File;

#[async_trait]
pub trait Mount: Send + Sync + std::fmt::Debug {
    /// Gets the root file for this mount.
    fn get_root(&self) -> Box<dyn File>;

    /// Gets the default location file for this mount.
    fn get_default_location(&self) -> Option<Box<dyn File>> {
        Some(self.get_root())
    }

    /// Gets the name of the mount.
    fn get_name(&self) -> String;

    /// Gets the icon name for the mount.
    fn get_icon(&self) -> String;

    /// Gets the symbolic icon name for the mount.
    fn get_symbolic_icon(&self) -> Option<String> {
        None
    }

    /// Gets the UUID of the mount.
    fn get_uuid(&self) -> Option<String>;

    /// Gets the volume this mount is for.
    fn get_volume(&self) -> Option<Box<dyn Volume>>;

    /// Gets the drive this mount is on.
    fn get_drive(&self) -> Option<Box<dyn Drive>>;

    /// Checks if the mount can be unmounted.
    fn can_unmount(&self) -> bool;

    /// Checks if the mount can be ejected.
    fn can_eject(&self) -> bool;

    /// Unmounts the mount.
    async fn unmount(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Ejects the mount.
    async fn eject(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Remounts the mount.
    async fn remount(
        &self,
        _cancellable: Option<&Cancellable>,
    ) -> NpioResult<()> {
        Err(crate::error::NpioError::new(
            crate::error::IOErrorEnum::NotSupported,
            "Remount not supported",
        ))
    }

    /// Gets a sort key for ordering mounts.
    fn get_sort_key(&self) -> Option<String> {
        None
    }
}

