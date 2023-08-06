use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use miette::{miette, Context, Result};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::commands::transcode::views::album::{AlbumView, SharedAlbumView};
use crate::commands::transcode::views::common::{
    ArcRwLock,
    ChangedAlbumsMap,
    WeakRwLock,
};
use crate::commands::transcode::views::library::{
    LibraryView,
    SharedLibraryView,
};
use crate::filesystem::DirectoryScan;

pub type SharedArtistView<'a> = ArcRwLock<ArtistView<'a>>;
#[allow(dead_code)]
pub type WeakArtistView<'a> = WeakRwLock<ArtistView<'a>>;


/// A filesystem abstraction that enables the user to scan and fetch specific or
/// all available albums by the artist it is about.
pub struct ArtistView<'config> {
    weak_self: WeakRwLock<Self>,

    /// Backreference to the `Library` this `LibraryArtists` instance is from.
    pub library: SharedLibraryView<'config>,

    /// Artist name.
    pub name: String,
}

impl<'config> ArtistView<'config> {
    /// Instantiate a new `ArtistView` from the library reference and an artist's name and directory.
    pub fn new(
        library: SharedLibraryView<'config>,
        artist_name: String,
        allow_missing_directory: bool,
    ) -> Result<SharedArtistView<'config>> {
        let self_arc = Arc::new_cyclic(|weak| {
            RwLock::new(Self {
                weak_self: weak.clone(),
                library,
                name: artist_name,
            })
        });

        if !allow_missing_directory {
            let self_locked = self_arc.write();

            if !self_locked.artist_directory_in_source_library().is_dir() {
                return Err(miette!(
                    "Provided artist directory does not exist: {:?}",
                    self_locked.artist_directory_in_source_library()
                ));
            }
        }

        Ok(self_arc)
    }

    pub fn directory_path_relative_to_library_root(&self) -> PathBuf {
        PathBuf::from(self.name.clone())
    }

    /// Get the artist directory in the original (untranscoded) library.
    pub fn artist_directory_in_source_library(&self) -> PathBuf {
        self.read_lock_library()
            .root_directory_in_source_library()
            .join(self.name.clone())
    }

    /// Get the mapped artist directory - an artist directory path inside the transcoded library.
    pub fn artist_directory_in_transcoded_library(&self) -> PathBuf {
        self.read_lock_library()
            .root_directory_in_transcoded_library()
            .join(self.name.clone())
    }

    /// Get a specific album by its title. Returns `None` if the album isn't present.
    ///
    /// NOTE: In euphony, *"album title" is understood as the album's directory name*. This is because
    /// euphony does not scan the album contents and extract the common album title from the tags in the file,
    /// but instead relies on the directory tree to tell artist names and album titles apart.  
    #[allow(dead_code)]
    pub fn album(
        &self,
        album_title: String,
    ) -> Result<Option<SharedAlbumView<'config>>> {
        let self_arc = self.weak_self.upgrade().ok_or_else(|| {
            miette!("Could not upgrade ArtistView weak reference.")
        })?;

        let instance = AlbumView::new(self_arc, album_title, false)?;

        {
            let instance_locked = instance.read();

            if !instance_locked.album_directory_in_source_library().is_dir() {
                return Ok(None);
            }
        }

        Ok(Some(instance))
    }

    /// Get all available albums by the artist (in the associated library).
    ///
    /// NOTE: In euphony, *"album title" is understood as the album's directory name*. This is because
    /// euphony does not scan the album contents and extract the common album title from the tags in the file,
    /// but instead relies on the directory tree to tell artist names and album titles apart.  
    pub fn albums(&self) -> Result<HashMap<String, SharedAlbumView<'config>>> {
        let self_arc = self.weak_self.upgrade().ok_or_else(|| {
            miette!("Could not upgrade ArtistView weak reference.")
        })?;

        let artist_directory_scan = self.scan_artist_directory()?;

        let mut album_map: HashMap<String, SharedAlbumView<'config>> =
            HashMap::with_capacity(artist_directory_scan.directories.len());

        for directory in artist_directory_scan.directories {
            let album_directory_name = directory
                .file_name()
                .to_str()
                .ok_or_else(|| miette!("Could not parse directory file name."))?
                .to_string();

            album_map.insert(
                album_directory_name.clone(),
                AlbumView::new(self_arc.clone(), album_directory_name, false)?,
            );
        }

        Ok(album_map)
    }

    /// Get all albums by this artist that have changed (or haven't been transcoded at all yet).
    /// Returns a HashMap that maps from the album title to a tuple
    /// containing the album view and the detected changes.
    ///
    /// For more information, see the `albums` method.
    pub fn scan_for_albums_with_changes(
        &self,
    ) -> Result<ChangedAlbumsMap<'config>> {
        let all_albums: HashMap<String, SharedAlbumView<'config>> =
            self.albums()?;

        all_albums
            .into_iter()
            .filter_map(|(title, album)| {
                let changes = {
                    let album_locked = album.read();

                    album_locked.scan_for_changes()
                };

                let changes = match changes {
                    Ok(changes) => changes,
                    Err(error) => return Some(Err(error)),
                };

                if changes.has_changes() {
                    Some(Ok((title, (album, changes))))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Scan the artist source directory and return a list of files
    /// that should be validated against the configured validation rules.
    #[allow(dead_code)]
    pub fn artist_directory_validation_files(&self) -> Result<Vec<PathBuf>> {
        let artist_directory_scan = self.scan_artist_directory()?;

        Ok(artist_directory_scan
            .files
            .into_iter()
            .map(|item| item.path())
            .collect())
    }

    /*
     * Private methods
     */

    /// Perform a zero-depth directory scan of the artist directory.
    fn scan_artist_directory(&self) -> Result<DirectoryScan> {
        DirectoryScan::from_directory_path(
            self.artist_directory_in_source_library(),
            0,
        )
        .wrap_err_with(|| {
            miette!(
                "Errored while scanning artist directory: {:?}",
                self.artist_directory_in_source_library()
            )
        })
    }

    #[inline]
    pub fn read_lock_library(
        &self,
    ) -> RwLockReadGuard<'_, LibraryView<'config>> {
        self.library.read()
    }

    #[allow(dead_code)]
    #[inline]
    pub fn write_lock_library(
        &self,
    ) -> RwLockWriteGuard<'_, LibraryView<'config>> {
        self.library.write()
    }
}
