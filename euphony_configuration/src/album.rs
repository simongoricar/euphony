use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::error::ConfigurationError;


/// The file name for the album overrides (see [`AlbumConfiguration`]).
///
/// This file is not required to exist in each album directory,
/// but the user may create it to influence
/// various configuration values per-album.
pub const ALBUM_OVERRIDE_FILE_NAME: &str = ".album.override.euphony";


/// Album-specific options for `euphony`.
///
/// Usage: create a `.album.override.euphony` file in an album directory.
/// You can look at the structure below or copy a template from
/// `data/.album.override.TEMPLATE.euphony`.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct AlbumConfiguration {
    /// Scanning options.
    #[serde(default)]
    pub scan: AlbumScanConfiguration,
}

impl AlbumConfiguration {
    /// Given a `directory_path`, load its `.album.override.euphony` file (if it exists).
    ///
    /// NOTE: Any optional values will be filled with defaults
    /// (e.g. `scan.depth` will default to `0`).
    pub fn load<P: Into<PathBuf>>(
        directory_path: P,
    ) -> Result<AlbumConfiguration, ConfigurationError> {
        let file_path: PathBuf =
            directory_path.into().join(ALBUM_OVERRIDE_FILE_NAME);

        // If no override exists, just return the defaults.
        if !file_path.is_file() {
            return Ok(AlbumConfiguration::default());
        }

        // It it exists, load the configuration and fill any empty optional fields with defaults.
        let album_override_string =
            fs::read_to_string(&file_path).map_err(|error| {
                ConfigurationError::FileLoadError {
                    file_path: file_path.clone(),
                    error,
                }
            })?;

        let album_override: AlbumConfiguration =
            toml::from_str(&album_override_string).map_err(|error| {
                ConfigurationError::FileFormatError {
                    file_path,
                    error: Box::new(error),
                }
            })?;

        Ok(album_override)
    }
}


#[derive(Deserialize, Clone, Debug, Default)]
pub struct AlbumScanConfiguration {
    /// Maximum album scanning depth. Zero (the default) means no subdirectories are scanned.
    #[serde(default)]
    pub depth: u16,
}
