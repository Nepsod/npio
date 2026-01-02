use crate::backend::Backend;
use crate::error::NpioResult;
use crate::file::File;
use crate::file::local::LocalFile;
use crate::uri::decode_file_uri;

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
        let path = decode_file_uri(uri)?;
        Ok(Box::new(LocalFile::new(path)))
    }
}
