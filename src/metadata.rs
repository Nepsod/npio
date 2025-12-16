use std::path::Path;
use mime_guess::MimeGuess;

pub struct MimeResolver;

impl MimeResolver {
    /// Guesses the MIME type for a file path.
    /// Currently only uses extension-based guessing for performance and async-friendliness.
    /// Content sniffing (using `infer`) can be added later if needed, but requires reading file content.
    pub fn guess_mime_type(path: &Path) -> String {
        let guess = MimeGuess::from_path(path);
        guess.first_or_octet_stream().to_string()
    }

    /// Gets the icon name for a given MIME type.
    /// Follows the freedesktop.org Icon Naming Specification.
    pub fn get_icon_name(mime_type: &str) -> String {
        // Replace '/' with '-' to get the generic icon name
        // e.g. "text/plain" -> "text-plain"
        mime_type.replace('/', "-")
    }
}
