use async_trait::async_trait;
use crate::cancellable::Cancellable;
use crate::error::NpioResult;
use crate::volume::Volume;

#[async_trait]
pub trait Drive: Send + Sync + std::fmt::Debug {
    /// Gets the name of the drive.
    fn get_name(&self) -> String;

    /// Gets the icon name for the drive.
    fn get_icon(&self) -> String;

    /// Gets the symbolic icon name for the drive.
    fn get_symbolic_icon(&self) -> Option<String> {
        None
    }

    /// Checks if the drive has volumes.
    fn has_volumes(&self) -> bool;

    /// Gets the volumes on this drive.
    fn get_volumes(&self) -> Vec<Box<dyn Volume>>;

    /// Checks if the drive is removable.
    fn is_removable(&self) -> bool;

    /// Checks if the drive supports removable media.
    fn is_media_removable(&self) -> bool;

    /// Checks if the drive has media inserted.
    fn has_media(&self) -> bool;

    /// Checks if the drive can automatically detect media changes.
    fn is_media_check_automatic(&self) -> bool;

    /// Checks if the drive can be polled for media changes.
    fn can_poll_for_media(&self) -> bool;

    /// Checks if the drive can eject media.
    fn can_eject(&self) -> bool;

    /// Ejects the drive.
    async fn eject(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Polls for media changes.
    async fn poll_for_media(
        &self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<()>;

    /// Gets an identifier of the given kind.
    /// Common kinds: "unix-device"
    fn get_identifier(&self, kind: &str) -> Option<String>;

    /// Enumerates all identifier kinds available for this drive.
    fn enumerate_identifiers(&self) -> Vec<String>;

    /// Gets a sort key for ordering drives.
    fn get_sort_key(&self) -> Option<String> {
        None
    }
}

