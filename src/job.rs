use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CopyFlags: u32 {
        const NONE = 0;
        const OVERWRITE = 1;
        const BACKUP = 2;
        const NO_FALLBACK_FOR_MOVE = 4;
        const TARGET_DEFAULT_PERMS = 8;
    }
}

pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

use crate::file::File;
use crate::cancellable::Cancellable;
use crate::error::NpioResult;

pub async fn copy(
    source: &dyn File,
    destination: &dyn File,
    flags: CopyFlags,
    progress: Option<ProgressCallback>,
    cancellable: Option<&Cancellable>,
) -> NpioResult<()> {
    source.copy(destination, flags, cancellable, progress).await
}

pub async fn move_(
    source: &dyn File,
    destination: &dyn File,
    flags: CopyFlags,
    progress: Option<ProgressCallback>,
    cancellable: Option<&Cancellable>,
) -> NpioResult<()> {
    source.move_to(destination, flags, cancellable, progress).await
}

pub async fn delete(
    file: &dyn File,
    cancellable: Option<&Cancellable>,
) -> NpioResult<()> {
    file.delete(cancellable).await
}
