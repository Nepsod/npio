use std::path::PathBuf;


use crate::backend::Backend;
use crate::error::{NpioError, NpioResult, IOErrorEnum};
use crate::file::File;
use crate::file::local::LocalFile;

pub struct LocalBackend;

impl LocalBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for LocalBackend {
    fn scheme(&self) -> &'static str {
        "file"
    }

    fn get_file_for_uri(&self, uri: &str) -> NpioResult<Box<dyn File>> {
        if !uri.starts_with("file://") {
             return Err(NpioError::new(IOErrorEnum::InvalidArg, "Invalid URI scheme for LocalBackend"));
        }
        
        // Simple URI decoding (TODO: Proper URL decoding)
        let path_str = uri.trim_start_matches("file://");
        let path = PathBuf::from(path_str);
        
        Ok(Box::new(LocalFile::new(path)))
    }
}
