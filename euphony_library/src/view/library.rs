use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use euphony_configuration::library::LibraryConfiguration;
use euphony_configuration::Configuration;
use fs_more::directory::DirectoryScan;
use miette::{miette, Context, Diagnostic, Result};
use parking_lot::RwLock;
use thiserror::Error;

use super::common::{ArcRwLock, WeakRwLock};
use super::{ArtistView, SharedArtistView};

pub type SharedLibraryView<'config> = ArcRwLock<LibraryView<'config>>;
#[allow(dead_code)]
pub type WeakLibraryView<'config> = WeakRwLock<LibraryView<'config>>;


#[derive(Error, Debug, Diagnostic)]
pub enum LibraryViewError {
    #[error("specified library path doesn't exist: {0}")]
    NoSuchDirectory(String),
}


pub struct LibraryView<'config> {
    weak_self: WeakRwLock<Self>,

    pub euphony_configuration: &'config Configuration,

    /// The associated `ConfigLibrary` instance.
    pub library_configuration: &'config LibraryConfiguration,
}

impl<'config> LibraryView<'config> {
    /// Instantiate a new `LibraryView` from the library's configuration struct.
    pub fn from_library_configuration(
        config: &'config Configuration,
        library_config: &'config LibraryConfiguration,
    ) -> Result<SharedLibraryView<'config>, LibraryViewError> {
        let library_path = Path::new(&library_config.path);
        if !library_path.exists() || !library_path.is_dir() {
            return Err(LibraryViewError::NoSuchDirectory(
                library_config.path.clone(),
            ));
        }

        Ok(Arc::new_cyclic(|weak| {
            RwLock::new(Self {
                weak_self: weak.clone(),
                euphony_configuration: config,
                library_configuration: library_config,
            })
        }))
    }

    /// Get the library's name.
    pub fn name(&self) -> String {
        self.library_configuration.name.clone()
    }

    /// Get the library's root directory.
    pub fn root_directory_in_source_library(&self) -> PathBuf {
        PathBuf::from(self.library_configuration.path.clone())
    }

    /// Get the mapped target path for the library (inside the transcoded library).
    /// This is pretty much just the root of the transcoded library.
    pub fn root_directory_in_transcoded_library(&self) -> PathBuf {
        PathBuf::from(self.euphony_configuration.aggregated_library.path.clone())
    }

    /// Get a specific artist by their name. Returns `None` if the artist name isn't present in the library.
    ///
    /// NOTE: In euphony, *"artist name" is understood as the artist's directory name*. This is because
    /// euphony does not scan the artist's albums and extract the common album artist tags from the file tags,
    /// but instead relies on the directory tree to tell artist names and album titles apart.
    #[allow(dead_code)]
    pub fn artist(
        &self,
        artist_name: String,
    ) -> Result<Option<SharedArtistView<'config>>> {
        let self_arc: SharedLibraryView = self
            .weak_self
            .upgrade()
            .expect("Could not upgrade weak reference.");

        let instance = ArtistView::new(self_arc, artist_name, false)?;

        {
            let instance_lock = instance.read();

            if !instance_lock.artist_directory_in_source_library().is_dir() {
                return Ok(None);
            }
        }

        Ok(Some(instance))
    }

    /// Get all available artists in the library.
    ///
    /// NOTE: In euphony, *"artist name" is understood as the artist's directory name*. This is because
    /// euphony does not scan the artist's albums and extract the common album artist tags from the file tags,
    /// but instead relies on the directory tree to tell artist names and album titles apart.
    pub fn artists(&self) -> Result<HashMap<String, SharedArtistView<'config>>> {
        let self_arc: SharedLibraryView = self
            .weak_self
            .upgrade()
            .ok_or_else(|| miette!("Could not upgrade weak reference."))?;

        let library_directory_scan = self.scan_root_directory()?;

        let mut artist_map: HashMap<String, SharedArtistView> =
            HashMap::with_capacity(library_directory_scan.directories.len());

        for directory in library_directory_scan.directories {
            let artist_directory_name = directory
                .file_name()
                .ok_or_else(|| miette!("Could not parse directory file name."))?
                .to_string_lossy()
                .to_string();

            // If the current directory matches one that should be ignored in the library root,
            // we simply skip it.
            if let Some(ignored_directory_list) = &self
                .library_configuration
                .ignored_directories_in_base_directory
            {
                if ignored_directory_list.contains(&artist_directory_name) {
                    continue;
                }
            }

            artist_map.insert(
                artist_directory_name.clone(),
                ArtistView::new(self_arc.clone(), artist_directory_name, false)?,
            );
        }

        Ok(artist_map)
    }

    /// Scan the root directory of the library and return a list of files at the root
    /// that should be validated against the configured validation rules.
    #[allow(dead_code)]
    pub fn library_root_validation_files(&self) -> Result<Vec<PathBuf>> {
        let library_directory_scan = self.scan_root_directory()?;

        Ok(library_directory_scan.files.into_iter().collect())
    }

    /// Perform a zero-depth directory scan of the root library directory.
    fn scan_root_directory(&self) -> Result<DirectoryScan> {
        DirectoryScan::scan_with_options(
            &self.library_configuration.path,
            Some(0),
            true,
        )
        .wrap_err_with(|| {
            miette!(
                "Errored while scanning library directory: {:?}",
                self.library_configuration.path
            )
        })
    }
}

impl<'config> Hash for LibraryView<'config> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.library_configuration.name.hash(state);
    }
}

impl<'config> PartialEq for LibraryView<'config> {
    fn eq(&self, other: &Self) -> bool {
        self.library_configuration
            .name
            .eq(&other.library_configuration.name)
    }
}

impl<'config> Eq for LibraryView<'config> {}
