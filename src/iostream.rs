use tokio::io::{AsyncRead, AsyncWrite};

use crate::cancellable::Cancellable;
use crate::error::NpioResult;

/// Trait representing an input stream (source of bytes).
/// Extends AsyncRead to integrate with Tokio.
pub trait InputStream: AsyncRead + Send + Unpin {
    fn close(&mut self, cancellable: Option<&Cancellable>) -> NpioResult<()>;
}

/// Trait representing an output stream (sink for bytes).
/// Extends AsyncWrite to integrate with Tokio.
pub trait OutputStream: AsyncWrite + Send + Unpin {
    fn close(&mut self, cancellable: Option<&Cancellable>) -> NpioResult<()>;
    fn flush(&mut self, cancellable: Option<&Cancellable>) -> NpioResult<()>;
}

// Implement for Box<dyn InputStream> to make it usable as an object
impl InputStream for Box<dyn InputStream> {
    fn close(&mut self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        (**self).close(cancellable)
    }
}

impl OutputStream for Box<dyn OutputStream> {
    fn close(&mut self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        (**self).close(cancellable)
    }

    fn flush(&mut self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        (**self).flush(cancellable)
    }
}
