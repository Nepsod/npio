use async_trait::async_trait;
use crate::cancellable::Cancellable;
use crate::error::NpioResult;
use crate::file_info::FileInfo;
use crate::file::File;

#[async_trait]
pub trait FileEnumerator: Send + Sync {
    /// Returns the next file in the enumeration.
    /// Returns Ok(None) when iteration is complete.
    async fn next_file(
        &mut self,
        cancellable: Option<&Cancellable>,
    ) -> NpioResult<Option<(FileInfo, Box<dyn File>)>>;

    /// Closes the enumerator.
    async fn close(&mut self, cancellable: Option<&Cancellable>) -> NpioResult<()>;
}
