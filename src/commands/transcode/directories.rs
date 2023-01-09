use std::fmt::{Debug, Formatter};
use std::path::{Path, PathBuf};

use miette::{miette, Result};

use crate::configuration::{Config, ConfigLibrary};

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
pub struct AlbumDirectoryInfo<'a> {
    /// Name of the artist that this album belongs to.
    pub artist_name: String,
    
    /// Title of the album this `AlbumDirectoryInfo` represents.
    pub album_title: String,
    
    /// The library that this album belongs to.
    pub library: &'a ConfigLibrary,
}

impl<'a> AlbumDirectoryInfo<'a> {
    /// Deconstruct an album directory path into three components:
    /// - the base library path,
    /// - the artist name and
    /// - the album title.
    pub fn new(
        album_directory_path: &Path,
        config: &'a Config,
        library: &'a ConfigLibrary,
    ) -> Result<AlbumDirectoryInfo<'a>> {
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

        Ok(AlbumDirectoryInfo {
            artist_name: artist_name
                .to_str()
                .ok_or_else(|| miette!("Could not convert artist directory name to string!"))?
                .to_string(),
            album_title: album_title
                .to_str()
                .ok_or_else(|| miette!("Could not convert album directory name to string!"))?
                .to_string(),
            library,
        })
    }

    pub fn build_target_file_path<S: AsRef<Path>>(&self, config: &Config, file_name: S) -> PathBuf {
        let mut full_path = PathBuf::from(&config.aggregated_library.path);
        
        full_path.push(&self.artist_name);
        full_path.push(&self.album_title);
        full_path.push(file_name.as_ref());
        
        full_path
    }
    
    pub fn build_source_file_path<S: AsRef<Path>>(&self, file_name: S) -> PathBuf {
        let mut full_path = PathBuf::from(&self.library.path);
    
        full_path.push(&self.artist_name);
        full_path.push(&self.album_title);
        full_path.push(file_name.as_ref());
    
        full_path
    }
}

impl<'a> Debug for AlbumDirectoryInfo<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<AlbumDirectoryInfo {} - {} library={}>",
            self.artist_name,
            self.album_title,
            self.library.path,
        )
    }
}
