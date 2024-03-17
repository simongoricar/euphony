use std::path::Path;

use miette::{miette, Result};

/// Get a file's extension (or an empty string if none).
/// Returns `Err` if the extension is not valid UTF-8.
#[inline]
pub fn get_path_extension_or_empty<P: AsRef<Path>>(path: P) -> Result<String> {
    Ok(path
        .as_ref()
        .extension()
        .unwrap_or_default()
        .to_str()
        .ok_or_else(|| miette!("Could not convert extension to UTF-8."))?
        .to_ascii_lowercase())
}
