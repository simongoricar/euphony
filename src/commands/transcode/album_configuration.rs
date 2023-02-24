use std::fs;
use std::path::PathBuf;

use miette::{miette, Context, IntoDiagnostic, Result};
use serde::Deserialize;

// This file is not required to exist in each album directory, but the user may create it
// to influence various configuration values per-album.
const ALBUM_OVERRIDE_FILE_NAME: &str = ".album.override.euphony";

/// Per-album options for euphony.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct AlbumConfiguration {
    /// Scanning options.
    #[serde(default)]
    pub scan: AlbumScanConfiguration,
}

impl AlbumConfiguration {
    /// Given a directory path, load its `.album.override.euphony` file (if it exists).
    ///
    /// NOTE: Any optional values will be filled with defaults.
    pub fn load<P: Into<PathBuf>>(
        directory_path: P,
    ) -> Result<AlbumConfiguration> {
        let file_path = directory_path.into().join(ALBUM_OVERRIDE_FILE_NAME);

        // If no override exists, just return the defaults.
        if !file_path.is_file() {
            return Ok(AlbumConfiguration::default());
        }

        // It it exists, load the configuration and fill any empty optional fields with defaults.
        let album_override_string = fs::read_to_string(&file_path)
            .into_diagnostic()
            .wrap_err_with(|| miette!("Could not read file into string."))?;

        let album_override: AlbumConfiguration =
            toml::from_str(&album_override_string)
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!("Could not load TOML contents of {:?}.", file_path)
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
