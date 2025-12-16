use std::fmt;
use std::io;

#[derive(Debug)]
pub enum IOErrorEnum {
    NotFound,
    Exists,
    IsDirectory,
    NotDirectory,
    NotEmpty,
    Regular,
    SymbolicLink,
    Pending,
    Closed,
    Cancelled,
    NotSupported,
    PermissionDenied,
    InvalidArg,
    Failed,
    ProxyFailed,
    ProxyAuthFailed,
    ProxyNeedAuth,
    ProxyNotAllowed,
    BrokenPipe,
    ConnectionClosed,
    ConnectionRefused,
    HostUnreachable,
    NetworkUnreachable,
    ConnectionTimedOut,
    AddressInUse,
    PartialInput,
    InvalidData,
    TimedOut,
    WouldBlock,
    WriteZero,
    Interrupted,
    UnexpectedEof,
    OutOfMemory,
    Other,
}

#[derive(Debug)]
pub struct NpioError {
    domain: IOErrorEnum,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl NpioError {
    pub fn new(domain: IOErrorEnum, message: impl Into<String>) -> Self {
        Self {
            domain,
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(
        domain: IOErrorEnum,
        message: impl Into<String>,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self {
            domain,
            message: message.into(),
            source: Some(source),
        }
    }

    pub fn kind(&self) -> &IOErrorEnum {
        &self.domain
    }
}

impl fmt::Display for NpioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.domain, self.message)
    }
}

impl std::error::Error for NpioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|e| e as &dyn std::error::Error)
    }
}

impl From<io::Error> for NpioError {
    fn from(err: io::Error) -> Self {
        let kind = match err.kind() {
            io::ErrorKind::NotFound => IOErrorEnum::NotFound,
            io::ErrorKind::PermissionDenied => IOErrorEnum::PermissionDenied,
            io::ErrorKind::ConnectionRefused => IOErrorEnum::ConnectionRefused,
            io::ErrorKind::ConnectionReset => IOErrorEnum::ConnectionClosed,
            io::ErrorKind::ConnectionAborted => IOErrorEnum::ConnectionClosed,
            io::ErrorKind::NotConnected => IOErrorEnum::ConnectionClosed,
            io::ErrorKind::AddrInUse => IOErrorEnum::AddressInUse,
            io::ErrorKind::AddrNotAvailable => IOErrorEnum::AddressInUse,
            io::ErrorKind::BrokenPipe => IOErrorEnum::BrokenPipe,
            io::ErrorKind::AlreadyExists => IOErrorEnum::Exists,
            io::ErrorKind::WouldBlock => IOErrorEnum::WouldBlock,
            io::ErrorKind::InvalidInput => IOErrorEnum::InvalidArg,
            io::ErrorKind::InvalidData => IOErrorEnum::InvalidData,
            io::ErrorKind::TimedOut => IOErrorEnum::TimedOut,
            io::ErrorKind::WriteZero => IOErrorEnum::WriteZero,
            io::ErrorKind::Interrupted => IOErrorEnum::Interrupted,
            io::ErrorKind::Unsupported => IOErrorEnum::NotSupported,
            io::ErrorKind::UnexpectedEof => IOErrorEnum::UnexpectedEof,
            io::ErrorKind::OutOfMemory => IOErrorEnum::OutOfMemory,
            _ => IOErrorEnum::Failed,
        };
        
        Self::with_source(kind, err.to_string(), Box::new(err))
    }
}

pub type NpioResult<T> = Result<T, NpioError>;
