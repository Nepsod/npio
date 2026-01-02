use std::path::PathBuf;
use percent_encoding::percent_decode_str;
use crate::error::{NpioError, NpioResult, IOErrorEnum};

/// Decode a file:// URI to a PathBuf.
/// 
/// Handles percent-encoding and both absolute (file:///path) and relative (file://path) formats.
pub fn decode_file_uri(uri: &str) -> NpioResult<PathBuf> {
    if !uri.starts_with("file://") {
        return Err(NpioError::new(IOErrorEnum::InvalidArg, "Invalid URI scheme for file://"));
    }
    
    let path_str = uri.trim_start_matches("file://");
    
    // Decode percent-encoded characters
    let decoded = percent_decode_str(path_str)
        .decode_utf8()
        .map_err(|_| NpioError::new(IOErrorEnum::InvalidArg, "Invalid UTF-8 in URI"))?;
    
    Ok(PathBuf::from(decoded.as_ref()))
}
