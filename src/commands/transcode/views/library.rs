use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use miette::{miette, Context, Result};
use parking_lot::RwLock;

use crate::commands::transcode::views::artist::{ArtistView, SharedArtistView};
use crate::commands::transcode::views::common::{
    ArcRwLock,
    ArtistsWithChangedAlbumsMap,
    ChangedAlbumsMap,
    WeakRwLock,
};
use crate::configuration::{Config, LibraryConfig};
use crate::filesystem::DirectoryScan;

pub type SharedLibraryView<'a> = ArcRwLock<LibraryView<'a>>;
#[allow(dead_code)]
pub type WeakLibraryView<'a> = WeakRwLock<LibraryView<'a>>;


pub struct LibraryView<'config> {
    weak_self: WeakRwLock<Self>,

    pub euphony_configuration: &'config Config,

    /// The associated `ConfigLibrary` instance.
    pub library_configuration: &'config LibraryConfig,
}

impl<'config> LibraryView<'config> {
    /// Instantiate a new `LibraryView` from the library's configuration struct.
    pub fn from_library_configuration(
        config: &'config Config,
        library_config: &'config LibraryConfig,
    ) -> SharedLibraryView<'config> {
        Arc::new_cyclic(|weak| {
            RwLock::new(Self {
                weak_self: weak.clone(),
                euphony_configuration: config,
                library_configuration: library_config,
            })
        })
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

        let instance = ArtistView::new(self_arc, artist_name)?;

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
                .to_str()
                .ok_or_else(|| miette!("Could not parse directory file name."))?
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
                ArtistView::new(self_arc.clone(), artist_directory_name)?,
            );
        }

        Ok(artist_map)
    }

    /// Get all artist in this library whose albums have changes (or haven't been transcoded yet).
    ///
    /// Returns a HashMap that maps from the artist name to a tuple
    /// containing the artist view and another HashMap from the album title to
    /// a tuple containing its view and its changes.
    ///
    /// The above is very verbose, you might better off reading the following two types:
    /// `AlbumsWithChangesMap` and `ArtistWithChangesMap`.
    ///
    /// For more information, see the `artists` method.
    pub fn scan_for_artists_with_changed_albums(
        &self,
    ) -> Result<ArtistsWithChangedAlbumsMap<'config>> {
        let all_artists: HashMap<String, SharedArtistView<'config>> =
            self.artists()?;

        all_artists
            .into_iter()
            .filter_map(|(name, artist)| {
                let locked_artist = artist.read();

                let albums: ChangedAlbumsMap<'config> =
                    match locked_artist.scan_for_albums_with_changes() {
                        Ok(albums) => albums,
                        Err(error) => return Some(Err(error)),
                    };

                drop(locked_artist);

                if albums.is_empty() {
                    None
                } else {
                    Some(Ok((name, (artist, albums))))
                }
            })
            .collect()
    }

    /// Scan the root directory of the library and return a list of files at the root
    /// that should be validated against the configured validation rules.
    #[allow(dead_code)]
    pub fn library_root_validation_files(&self) -> Result<Vec<PathBuf>> {
        let library_directory_scan = self.scan_root_directory()?;

        Ok(library_directory_scan
            .files
            .into_iter()
            .map(|item| item.path())
            .collect())
    }

    /// Perform a zero-depth directory scan of the root library directory.
    fn scan_root_directory(&self) -> Result<DirectoryScan> {
        DirectoryScan::from_directory_path(&self.library_configuration.path, 0)
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