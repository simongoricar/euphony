use std::fs;
use std::io::Error;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

// This file is not required to exist in each album directory, but the user may create it
// to influence the transcoding configuration.
const ALBUM_OVERRIDE_FILE_NAME: &str = ".album.override.euphony";


/// Given a directory path, construct the full path to the album override file (.album.override.euphony).
/// Example: given "D:/hello/world" (as a Path), we would get "D:/hello/world/.album.override.euphony" (as a PathBuf).
fn get_album_override_filepath(directory_path: &Path) -> PathBuf {
    let mut final_path = directory_path.to_path_buf();
    final_path.push(ALBUM_OVERRIDE_FILE_NAME);

    final_path
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AlbumOverride {
    pub scan: Option<AlbumScanOverride>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AlbumScanOverride {
    pub depth: Option<u16>,
}

impl AlbumOverride {
    /// Check whether the .album.override.euphony file exists on disk for this album directory.
    pub fn exists<P: AsRef<Path>>(directory_path: P) -> bool {
        let file_path = get_album_override_filepath(directory_path.as_ref());
        file_path.is_file()
    }

    /// Given a directory path, load its .album.euphony file, if it exists, into a LibraryMeta struct.
    pub fn load<P: AsRef<Path>>(directory_path: P) -> Result<Option<AlbumOverride>, Error> {
        if !Self::exists(directory_path.as_ref()) {
            return Ok(None)
        }

        let file_path = get_album_override_filepath(directory_path.as_ref());

        let album_override_string = fs::read_to_string(file_path)?;
        let album_override: AlbumOverride = toml::from_str(&album_override_string)?;

        Ok(Some(album_override))
    }
}
