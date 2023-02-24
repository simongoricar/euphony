//! TODO: This is a work-in-progress rewrite of the way albums are processed.

use std::collections::HashMap;
use std::hash::Hash;
use std::path::PathBuf;

use miette::{miette, Context, Result};
use serde::{Deserialize, Serialize};

use crate::commands::transcode::album_configuration::AlbumConfiguration;
use crate::configuration::{Config, ConfigLibrary};
use crate::filesystem::DirectoryScan;

pub struct LibraryView<'a> {
    pub euphony_configuration: &'a Config,

    /// The associated `ConfigLibrary` instance.
    pub library_configuration: &'a ConfigLibrary,
}

impl<'a> LibraryView<'a> {
    /// Instantiate a new `LibraryView` from the library's configuration struct.
    pub fn from_library_configuration(
        config: &'a Config,
        library_config: &'a ConfigLibrary,
    ) -> Self {
        Self {
            euphony_configuration: config,
            library_configuration: library_config,
        }
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
    pub fn artist(&self, artist_name: String) -> Result<Option<ArtistView>> {
        let instance = ArtistView::new(self, artist_name)?;

        if !instance.artist_directory_in_source_library().is_dir() {
            return Ok(None);
        }

        Ok(Some(instance))
    }

    /// Get all available artists in the library.
    ///
    /// NOTE: In euphony, *"artist name" is understood as the artist's directory name*. This is because
    /// euphony does not scan the artist's albums and extract the common album artist tags from the file tags,
    /// but instead relies on the directory tree to tell artist names and album titles apart.
    pub fn artists(&self) -> Result<HashMap<String, ArtistView>> {
        let library_directory_scan = self.scan_root_directory()?;

        let mut artist_map: HashMap<String, ArtistView> =
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
                ArtistView::new(self, artist_directory_name)?,
            );
        }

        Ok(artist_map)
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

/// A filesystem abstraction that enables the user to scan and fetch specific or
/// all available albums by the artist it is about.
pub struct ArtistView<'a> {
    /// Backreference to the `Library` this `LibraryArtists` instance is from.
    pub library: &'a LibraryView<'a>,

    /// Artist name.
    pub name: String,
}

impl<'a> ArtistView<'a> {
    /// Instantiate a new `ArtistView` from the library reference and an artist's name and directory.
    pub fn new(library: &'a LibraryView, artist_name: String) -> Result<Self> {
        let instance = Self {
            library,
            name: artist_name,
        };

        if !instance.artist_directory_in_source_library().is_dir() {
            return Err(miette!(
                "Provided artist directory does not exist: {:?}",
                instance.artist_directory_in_source_library()
            ));
        }

        Ok(instance)
    }

    /// Get the artist directory in the original (untranscoded) library.
    pub fn artist_directory_in_source_library(&self) -> PathBuf {
        self.library
            .root_directory_in_source_library()
            .join(self.name.clone())
    }

    /// Get the mapped artist directory - an artist directory path inside the transcoded library.
    pub fn artist_directory_in_transcoded_library(&self) -> PathBuf {
        self.library
            .root_directory_in_transcoded_library()
            .join(self.name.clone())
    }

    /// Get a specific album by its title. Returns `None` if the album isn't present.
    ///
    /// NOTE: In euphony, *"album title" is understood as the album's directory name*. This is because
    /// euphony does not scan the album contents and extract the common album title from the tags in the file,
    /// but instead relies on the directory tree to tell artist names and album titles apart.  
    pub fn album(&self, album_title: String) -> Result<Option<AlbumView>> {
        let instance = AlbumView::new(self, album_title)?;

        if !instance.album_directory_in_source_library().is_dir() {
            return Ok(None);
        }

        Ok(Some(instance))
    }

    /// Get all available albums by the artist (in the associated library).
    ///
    /// NOTE: In euphony, *"album title" is understood as the album's directory name*. This is because
    /// euphony does not scan the album contents and extract the common album title from the tags in the file,
    /// but instead relies on the directory tree to tell artist names and album titles apart.  
    pub fn albums(&self) -> Result<HashMap<String, AlbumView>> {
        let artist_directory_scan = self.scan_artist_directory()?;

        let mut album_map: HashMap<String, AlbumView> =
            HashMap::with_capacity(artist_directory_scan.directories.len());

        for directory in artist_directory_scan.directories {
            let album_directory_name = directory
                .file_name()
                .to_str()
                .ok_or_else(|| miette!("Could not parse directory file name."))?
                .to_string();

            album_map.insert(
                album_directory_name.clone(),
                AlbumView::new(self, album_directory_name)?,
            );
        }

        Ok(album_map)
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
}

pub struct AlbumView<'a> {
    /// Reference back to the `ArtistView` this album belongs to.
    pub artist: &'a ArtistView<'a>,

    /// Per-album configuration for euphony.
    pub configuration: AlbumConfiguration,

    /// Album name.
    pub title: String,
}

impl<'a> AlbumView<'a> {
    pub fn new(artist: &'a ArtistView<'a>, album_title: String) -> Result<Self> {
        let album_directory = artist
            .artist_directory_in_source_library()
            .join(album_title.clone());

        if !album_directory.is_dir() {
            return Err(miette!(
                "Provided album directory does not exist: {:?}",
                album_directory
            ));
        }

        let album_configuration =
            AlbumConfiguration::load(album_directory.clone())?;

        Ok(Self {
            artist,
            configuration: album_configuration,
            title: album_title,
        })
    }

    /// Get the album directory in the original (untranscoded) library.
    pub fn album_directory_in_source_library(&self) -> PathBuf {
        self.artist
            .artist_directory_in_source_library()
            .join(self.title.clone())
    }

    /// Get the mapped album directory - an album path inside the transcoded library.
    pub fn album_directory_in_transcoded_library(&self) -> PathBuf {
        self.artist
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

    fn tracked_source_files(&self) -> Result<AlbumSourceFileList<'a>> {
        AlbumSourceFileList::from_album_view(self)
    }

    // TODO: Reimplement things like `needs_processing`, `get_work_packets`.
}

// TODO: Figure out how to model transcoding file lists and jobs. Most other stuff is ready - the
//  input data for transcoding is basically `AlbumFileChangesV2`, so build some sort of abstraction
//  for cancellable jobs and implement three: transcode audio file, copy data file, delete file.
//  Handle removing dead code at the very end.
//  Also: it might be best to try and recreate the transcode command and see what's missing, because
//  this is turning out to be quite a rewrite.

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
pub struct AlbumSourceFileList<'a> {
    /// The `AlbumView` this file list is based on.
    pub album: &'a AlbumView<'a>,

    /// Audio file paths associated with the album.
    /// Paths are relative to the album source directory.
    pub audio_files: Vec<PathBuf>,

    /// Data file paths associated with the album.
    /// Paths are relative to the album source directory.
    pub data_files: Vec<PathBuf>,
}

impl<'a> AlbumSourceFileList<'a> {
    pub fn from_album_view(album_view: &'a AlbumView<'a>) -> Result<Self> {
        let audio_file_extensions = &album_view
            .artist
            .library
            .library_configuration
            .transcoding
            .audio_file_extensions;
        let other_file_extensions = &album_view
            .artist
            .library
            .library_configuration
            .transcoding
            .other_file_extensions;

        let album_directory = album_view.album_directory_in_source_library();

        let album_scan = DirectoryScan::from_directory_path(
            &album_directory,
            album_view.configuration.scan.depth,
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

            let file_extension = file_absolute_path
                .extension()
                .unwrap_or_default()
                .to_str()
                .ok_or_else(|| {
                    miette!("Could not convert file extension to string.")
                })?
                .to_string();

            if audio_file_extensions.contains(&file_extension) {
                audio_files.push(file_relative_path);
            } else if other_file_extensions.contains(&file_extension) {
                data_files.push(file_relative_path);
            }
        }

        Ok(Self {
            album: album_view,
            audio_files,
            data_files,
        })
    }

    /// Returns the path to the album source directory.
    pub fn album_source_directory_path(&self) -> PathBuf {
        self.album.album_directory_in_source_library().clone()
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
    /// *Paths are still relative.*
    pub fn map_source_paths_to_transcoded_paths(
        &self,
    ) -> SortedFileMap<PathBuf, PathBuf> {
        let source_directory_path = self.album_source_directory_path();

        // Oh my.
        let transcoded_audio_file_extension = &self
            .album
            .artist
            .library
            .euphony_configuration
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

        return SortedFileMap::new(
            map_original_to_transcoded_audio,
            map_original_to_transcoded_data,
        );
    }

    /// Generate a HashMap that maps from relative paths in the transcoded album directory
    /// to the relative paths of each of those original files in the source album directory.
    ///
    /// *Paths are still relative.*
    pub fn map_transcoded_paths_to_source_paths(
        &self,
    ) -> SortedFileMap<PathBuf, PathBuf> {
        self.map_source_paths_to_transcoded_paths()
            .to_inverted_map()
    }
}
