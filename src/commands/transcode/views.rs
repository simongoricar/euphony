//! TODO: This is a work-in-progress rewrite of the way albums are processed.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Weak};

use miette::{miette, Context, Result};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};

use crate::commands::transcode::album_configuration::AlbumConfiguration;
use crate::commands::transcode::album_state_v2::{
    AlbumFileChangesV2,
    SourceAlbumState,
    TranscodedAlbumState,
};
use crate::configuration::{Config, ConfigLibrary};
use crate::filesystem::DirectoryScan;

/*
In order to allow the code to share the library, artist and album views, we wrap them
in an `Arc` (and its `Weak` reference variant, when stored).

`Shared*` types are essentially `RwLock`ed library/artist/album views under an `Arc`.
`Weak*` types are `Weak` references to the same views - call `upgrade` to obtain the corresponding `Shared*` type.
*/

pub type ArcRwLock<T> = Arc<RwLock<T>>;
pub type WeakRwLock<T> = Weak<RwLock<T>>;

pub type SharedLibraryView<'a> = ArcRwLock<LibraryView<'a>>;
pub type WeakLibraryView<'a> = WeakRwLock<LibraryView<'a>>;

pub type SharedArtistView<'a> = ArcRwLock<ArtistView<'a>>;
pub type WeakArtistView<'a> = WeakRwLock<ArtistView<'a>>;

pub type SharedAlbumView<'a> = ArcRwLock<AlbumView<'a>>;
pub type WeakAlbumView<'a> = WeakRwLock<AlbumView<'a>>;


pub type ChangedAlbumsMap<'a> =
    HashMap<String, (SharedAlbumView<'a>, AlbumFileChangesV2<'a>)>;
pub type ArtistsWithChangedAlbumsMap<'a> =
    HashMap<String, (SharedArtistView<'a>, ChangedAlbumsMap<'a>)>;


pub struct LibraryView<'config> {
    weak_self: WeakRwLock<Self>,

    pub euphony_configuration: &'config Config,

    /// The associated `ConfigLibrary` instance.
    pub library_configuration: &'config ConfigLibrary,
}

impl<'config> LibraryView<'config> {
    /// Instantiate a new `LibraryView` from the library's configuration struct.
    pub fn from_library_configuration(
        config: &'config Config,
        library_config: &'config ConfigLibrary,
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
    ) -> Result<SharedArtistView<'config>> {
        let self_arc = Arc::new_cyclic(|weak| {
            RwLock::new(Self {
                weak_self: weak.clone(),
                library,
                name: artist_name,
            })
        });

        {
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
    pub fn album(
        &self,
        album_title: String,
    ) -> Result<Option<SharedAlbumView<'config>>> {
        let self_arc = self.weak_self.upgrade().ok_or_else(|| {
            miette!("Could not upgrade ArtistView weak reference.")
        })?;

        let instance = AlbumView::new(self_arc, album_title)?;

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
                AlbumView::new(self_arc.clone(), album_directory_name)?,
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
            &self.artist_directory_in_source_library(),
            0,
        )
        .wrap_err_with(|| {
            miette!(
                "Errored while scanning artist directory: {:?}",
                self.artist_directory_in_source_library()
            )
        })
    }

    pub fn read_lock_library(
        &self,
    ) -> RwLockReadGuard<'_, LibraryView<'config>> {
        self.library.read()
    }

    pub fn write_lock_library(
        &self,
    ) -> RwLockWriteGuard<'_, LibraryView<'config>> {
        self.library.write()
    }
}

pub struct AlbumView<'config> {
    weak_self: WeakRwLock<Self>,

    /// Reference back to the `ArtistView` this album belongs to.
    pub artist: SharedArtistView<'config>,

    /// Per-album configuration for euphony.
    pub configuration: AlbumConfiguration,

    /// Album name.
    pub title: String,
}

impl<'config> AlbumView<'config> {
    pub fn new(
        artist: SharedArtistView<'config>,
        album_title: String,
    ) -> Result<SharedAlbumView<'config>> {
        let album_directory = {
            let artist_lock = artist.read();

            artist_lock
                .artist_directory_in_source_library()
                .join(album_title.clone())
        };

        if !album_directory.is_dir() {
            return Err(miette!(
                "Provided album directory does not exist: {:?}",
                album_directory
            ));
        }

        let album_configuration =
            AlbumConfiguration::load(album_directory.clone())?;

        Ok(Arc::new_cyclic(|weak| {
            RwLock::new(Self {
                weak_self: weak.clone(),
                artist,
                configuration: album_configuration,
                title: album_title,
            })
        }))
    }

    pub fn read_lock_artist(&self) -> RwLockReadGuard<'_, ArtistView<'config>> {
        self.artist.read()
    }

    pub fn write_lock_artist(
        &self,
    ) -> RwLockWriteGuard<'_, ArtistView<'config>> {
        self.artist.write()
    }

    /// Return the relevant `Config` (euphony's global configuration).
    pub fn euphony_configuration(&self) -> &'config Config {
        self.read_lock_artist()
            .read_lock_library()
            .euphony_configuration
    }

    /// Return the relevant `ConfigLibrary` (configuration for the specific library).
    pub fn library_configuration(&self) -> &'config ConfigLibrary {
        self.read_lock_artist()
            .read_lock_library()
            .library_configuration
    }

    /// Get the album directory in the original (untranscoded) library.
    pub fn album_directory_in_source_library(&self) -> PathBuf {
        self.read_lock_artist()
            .artist_directory_in_source_library()
            .join(self.title.clone())
    }

    /// Get the mapped album directory - an album path inside the transcoded library.
    pub fn album_directory_in_transcoded_library(&self) -> PathBuf {
        self.read_lock_artist()
            .artist_directory_in_transcoded_library()
            .join(self.title.clone())
    }

    /// Scan the album directory and return a list of files
    /// that should be validated against the configured validation rules.
    pub fn album_validation_files(&self) -> Result<Vec<PathBuf>> {
        let album_scan = self.scan_album_directory()?;

        Ok(album_scan
            .files
            .into_iter()
            .map(|item| item.path())
            .collect())
    }

    /// Perform a directory scan of the album directory, respecting the depth configuration
    /// for the particular album.
    fn scan_album_directory(&self) -> Result<DirectoryScan> {
        DirectoryScan::from_directory_path(
            &self.album_directory_in_source_library(),
            self.configuration.scan.depth,
        )
        .wrap_err_with(|| {
            miette!(
                "Errored while scanning album directory: {:?}",
                self.album_directory_in_source_library()
            )
        })
    }

    /// This method returns an `AlbumSourceFileList`,
    /// which is a collection of tracked audio and data files.
    ///
    /// This *does* scan the disk for files.
    fn tracked_source_files(&self) -> Result<AlbumSourceFileList<'config>> {
        let self_arc = self.weak_self.upgrade().ok_or_else(|| {
            miette!("Could not upgrade AlbumView weak reference.")
        })?;

        AlbumSourceFileList::from_album_view(self_arc)
    }

    /// Compare several filesystem snapshots (`.album.source-state.euphony`,
    /// `.album.transcode-state.euphony`, fresh files in the source and album directories)
    /// to generate a set of changes since the last transcoding.
    ///
    /// If no transcoding has been done previously, this will mean all files will be marked as new
    /// (see `added_in_source_since_last_transcode`).
    ///
    /// **This is a relatively expensive IO operation as it requires quite a bit of disk access.
    /// Reuse the results as much as possible to maintain good performance.**
    pub fn scan_for_changes(&self) -> Result<AlbumFileChangesV2<'config>> {
        // TODO Implement caching via internal mutability for this costly scan operation.
        let source_album_directory_path =
            self.album_directory_in_source_library();
        let transcoded_album_directory_path =
            self.album_directory_in_transcoded_library();

        let tracked_source_files: AlbumSourceFileList<'config> =
            self.tracked_source_files()?;

        // Load states from disk (if they exist) and generate fresh filesystem states as well.
        let saved_source_album_state =
            SourceAlbumState::load_from_directory(&source_album_directory_path)?;
        let fresh_source_album_state =
            SourceAlbumState::generate_from_tracked_files(
                &tracked_source_files,
                &source_album_directory_path,
            )?;

        let saved_transcoded_album_state =
            TranscodedAlbumState::load_from_directory(
                &transcoded_album_directory_path,
            )?;
        let fresh_transcoded_album_state =
            TranscodedAlbumState::generate_from_tracked_files(
                &tracked_source_files,
                transcoded_album_directory_path,
            )?;

        // Let `AlbumFileChangesV2` compare all the snapshots and generate a unified way
        // of detecting and listing changes (i.e. required work for transcoding).
        let full_changes: AlbumFileChangesV2<'config> =
            AlbumFileChangesV2::generate_from_source_and_transcoded_state(
                saved_source_album_state,
                fresh_source_album_state,
                saved_transcoded_album_state,
                fresh_transcoded_album_state,
                self.weak_self.upgrade().ok_or_else(|| {
                    miette!("Could not upgarde AlbumView's weak_self!")
                })?,
                tracked_source_files,
            )?;

        Ok(full_changes)
    }
}

// TODO: Remove all the dead code at the very end.

/// Represents a double `HashMap`: one for audio files, the other for data files.
/// TODO Move to some utility module.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SortedFileMap<K: Eq + Hash, V> {
    pub audio: HashMap<K, V>,
    pub data: HashMap<K, V>,
}

impl<K: Eq + Hash, V> SortedFileMap<K, V> {
    pub fn new(audio_map: HashMap<K, V>, data_map: HashMap<K, V>) -> Self {
        Self {
            audio: audio_map,
            data: data_map,
        }
    }

    /// Get a value by key from either `audio` or `data` map.
    /// Works like the normal `get` method on `HashMap`s.
    pub fn get(&self, key: &K) -> Option<&V> {
        let value_in_audio_map = self.audio.get(key);

        if value_in_audio_map.is_some() {
            value_in_audio_map
        } else {
            self.data.get(key)
        }
    }

    /// Consumes the `SortedFileMap` and returns a flat `HashMap` with
    /// key-value pairs from both `audio` and `data`.  
    pub fn into_flattened_map(self) -> HashMap<K, V> {
        let mut flat_hashmap: HashMap<K, V> =
            HashMap::with_capacity(self.audio.len() + self.data.len());

        flat_hashmap.extend(self.audio.into_iter());
        flat_hashmap.extend(self.data.into_iter());

        flat_hashmap
    }

    /// Returns `true` if both `audio` and `data` contain no data.
    pub fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.data.is_empty()
    }
}

impl<K: Eq + Hash + Clone, V: Eq + Hash + Clone> SortedFileMap<K, V> {
    /// Inverts the current file map: all keys become values and values become their keys.
    pub fn to_inverted_map(&self) -> SortedFileMap<V, K> {
        let audio_inverted_map: HashMap<V, K> = self
            .audio
            .iter()
            .map(|(key, value)| (value.clone(), key.clone()))
            .collect();
        let data_inverted_map: HashMap<V, K> = self
            .data
            .iter()
            .map(|(key, value)| (value.clone(), key.clone()))
            .collect();

        SortedFileMap::new(audio_inverted_map, data_inverted_map)
    }
}

/// A list of audio and other (data) files that are "tracked", meaning euphony will consider
/// transcoding or copying them when the `transcode` command is executed.
///
/// The information in this struct are only paths of the tracked files, no additional metadata
/// (see `AlbumFileState`).
///
/// File paths are relative to the source album directory.
pub struct AlbumSourceFileList<'config> {
    /// The `AlbumView` this file list is based on.
    pub album: SharedAlbumView<'config>,

    /// Audio file paths associated with the album.
    /// Paths are relative to the album source directory.
    pub audio_files: Vec<PathBuf>,

    /// Data file paths associated with the album.
    /// Paths are relative to the album source directory.
    pub data_files: Vec<PathBuf>,
}

impl<'config> AlbumSourceFileList<'config> {
    pub fn from_album_view(
        album_view: SharedAlbumView<'config>,
    ) -> Result<Self> {
        let locked_album_view = album_view.read();

        let transcoding_configuration =
            &locked_album_view.library_configuration().transcoding;

        let album_directory =
            locked_album_view.album_directory_in_source_library();

        let album_scan = DirectoryScan::from_directory_path(
            &album_directory,
            locked_album_view.configuration.scan.depth,
        )?;

        let mut audio_files: Vec<PathBuf> = Vec::new();
        let mut data_files: Vec<PathBuf> = Vec::new();

        for file in album_scan.files {
            let file_absolute_path = file.path();
            // (relative to album source directory)
            let file_relative_path =
                pathdiff::diff_paths(file_absolute_path, &album_directory)
                    .ok_or_else(|| {
                        miette!("Could not generate relative path.")
                    })?;

            if transcoding_configuration
                .is_path_audio_file_by_extension(&file_relative_path)?
            {
                audio_files.push(file_relative_path);
            } else if transcoding_configuration
                .is_path_data_file_by_extension(&file_relative_path)?
            {
                data_files.push(file_relative_path);
            }
        }

        drop(locked_album_view);

        Ok(Self {
            album: album_view,
            audio_files,
            data_files,
        })
    }

    /// Returns a list of references to both audio and data file paths in this scan.
    pub fn all_file_paths(&self) -> Vec<&PathBuf> {
        self.audio_files
            .iter()
            .chain(self.data_files.iter())
            .collect()
    }

    /// Returns the total file count.
    pub fn file_count(&self) -> usize {
        self.audio_files.len() + self.data_files.len()
    }

    /// Generate a HashMap that maps from relative paths in the source album directory
    /// to the relative paths of each of those files in the transcoded album directory.
    ///
    /// On the surface it might make sense that the relative paths would stay the same,
    /// *but that isn't always true* (e.g. extension changes when transcoding, etc.).
    ///
    /// *Paths are still relative.*
    pub fn map_source_file_paths_to_transcoded_file_paths_relative(
        &self,
    ) -> SortedFileMap<PathBuf, PathBuf> {
        let album = self.album_ref();
        let transcoded_audio_file_extension = &album
            .euphony_configuration()
            .tools
            .ffmpeg
            .audio_transcoding_output_extension;

        // Transform audio file extensions and create a map from original to transcoded paths.
        // Paths are *still* relative to the album directory.
        let mut map_original_to_transcoded_audio: HashMap<PathBuf, PathBuf> =
            HashMap::with_capacity(self.audio_files.len());

        for source_audio_file_path in &self.audio_files {
            let relative_transcoded_audio_file_path = source_audio_file_path
                .with_extension(transcoded_audio_file_extension);

            map_original_to_transcoded_audio.insert(
                source_audio_file_path.clone(),
                relative_transcoded_audio_file_path,
            );
        }


        let mut map_original_to_transcoded_data: HashMap<PathBuf, PathBuf> =
            HashMap::with_capacity(self.data_files.len());

        for source_data_file_path in &self.data_files {
            // Neither relative path nor the extension changes, so we just insert two copies.
            map_original_to_transcoded_data.insert(
                source_data_file_path.clone(),
                source_data_file_path.clone(),
            );
        }

        SortedFileMap::new(
            map_original_to_transcoded_audio,
            map_original_to_transcoded_data,
        )
    }

    /// Generate a HashMap that maps from relative paths in the transcoded album directory
    /// to the relative paths of each of those original files in the source album directory.
    ///
    /// *Paths are still relative.*
    pub fn map_transcoded_paths_to_source_paths_relative(
        &self,
    ) -> SortedFileMap<PathBuf, PathBuf> {
        self.map_source_file_paths_to_transcoded_file_paths_relative()
            .to_inverted_map()
    }

    pub fn map_source_file_paths_to_transcoded_file_paths_absolute(
        &self,
    ) -> SortedFileMap<PathBuf, PathBuf> {
        let (album_source_directory, album_transcoded_directory) = {
            let album = self.album.read();

            (
                album.album_directory_in_source_library(),
                album.album_directory_in_transcoded_library(),
            )
        };

        let source_to_transcoded_map =
            self.map_source_file_paths_to_transcoded_file_paths_relative();

        SortedFileMap::new(
            source_to_transcoded_map
                .audio
                .into_iter()
                .map(|(source_path, transcoded_path)| {
                    (
                        album_source_directory.join(source_path),
                        album_transcoded_directory.join(transcoded_path),
                    )
                })
                .collect(),
            source_to_transcoded_map
                .data
                .into_iter()
                .map(|(source_path, transcoded_path)| {
                    (
                        album_source_directory.join(source_path),
                        album_transcoded_directory.join(transcoded_path),
                    )
                })
                .collect(),
        )
    }

    /*
     * Private methods
     */

    fn album_ref(&self) -> RwLockReadGuard<'_, AlbumView<'config>> {
        self.album.read()
    }

    fn album_mut_ref(&self) -> RwLockWriteGuard<'_, AlbumView<'config>> {
        self.album.write()
    }
}
