use std::fmt::{Debug, Formatter};
use std::path::{Path, PathBuf};

use miette::{miette, Result};

use crate::configuration::Config;

pub fn directory_is_library(config: &Config, directory_path: &Path) -> bool {
    for library in config.libraries.values() {
        if Path::new(&library.path).eq(directory_path) {
            return true;
        }
    }

    false
}

pub fn directory_is_artist(config: &Config, directory_path: &Path) -> bool {
    match directory_path.parent() {
        Some(parent) => directory_is_library(config, parent),
        None => false,
    }
}

pub fn directory_is_album(config: &Config, directory_path: &Path) -> bool {
    match directory_path.parent() {
        Some(parent) => {
            directory_is_artist(config, parent)
        },
        None => false
    }
}


#[derive(Clone)]
pub struct AlbumDirectoryInfo {
    pub library_path: String,
    pub artist_name: String,
    pub album_title: String,
}

impl AlbumDirectoryInfo {
    /// Deconstruct an album directory path into three components:
    /// - the base library path,
    /// - the artist name and
    /// - the album title.
    pub fn new(album_directory_path: &Path, config: &Config) -> Result<AlbumDirectoryInfo> {
        if !directory_is_album(config, album_directory_path) {
            return Err(miette!("Target is not album directory."));
        }

        let album_title = album_directory_path.file_name()
            .ok_or_else(|| miette!("Could not get album directory name!"))?;

        let artist_directory = album_directory_path.parent()
            .ok_or_else(|| miette!("Could not get path parent!"))?;
        let artist_name = artist_directory
            .file_name()
            .ok_or_else(|| miette!("Could not get artist directory name!"))?;

        let base_library_path = artist_directory.parent()
            .ok_or_else(|| miette!("Could not get path parent!"))?;
        let base_library_path_string = base_library_path
            .to_str()
            .ok_or_else(|| miette!("Could not convert path to str: invalid utf-8."))?
            .to_string();

        Ok(AlbumDirectoryInfo {
            library_path: base_library_path_string,
            artist_name: artist_name
                .to_str()
                .ok_or_else(|| miette!("Could not convert artist directory name to string!"))?
                .to_string(),
            album_title: album_title
                .to_str()
                .ok_or_else(|| miette!("Could not convert album directory name to string!"))?
                .to_string()
        })
    }

    pub fn build_full_directory_path(&self) -> PathBuf {
        let mut full_directory_path = PathBuf::from(&self.library_path);
        full_directory_path.push(&self.artist_name);
        full_directory_path.push(&self.album_title);

        full_directory_path
    }

    pub fn build_full_file_path(&self, file_name: &Path) -> PathBuf {
        let mut full_path = self.build_full_directory_path();
        full_path.push(file_name);

        full_path
    }

    /// Create a copy of this instance with the base library being the aggregated library path.
    pub fn as_aggregated_directory(&self, config: &Config) -> Self {
        let mut cloned_self = self.clone();
        cloned_self.library_path = config.aggregated_library.path.clone();

        cloned_self
    }
}

impl Debug for AlbumDirectoryInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<AlbumDirectoryInfo {} - {} library={}>",
            self.artist_name,
            self.album_title,
            self.library_path,
        )
    }
}
