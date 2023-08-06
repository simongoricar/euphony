use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use miette::{miette, Context, Result};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::commands::transcode::album_configuration::AlbumConfiguration;
use crate::commands::transcode::album_state::changes::AlbumFileChangesV2;
use crate::commands::transcode::album_state::source::{
    SourceAlbumState,
    SourceAlbumStateLoadError,
};
use crate::commands::transcode::album_state::transcoded::{
    TranscodedAlbumState,
    TranscodedAlbumStateLoadError,
};
use crate::commands::transcode::views::artist::{ArtistView, SharedArtistView};
use crate::commands::transcode::views::common::{
    ArcRwLock,
    SortedFileMap,
    WeakRwLock,
};
use crate::configuration::{Config, LibraryConfig};
use crate::filesystem::DirectoryScan;

pub type SharedAlbumView<'a> = ArcRwLock<AlbumView<'a>>;
#[allow(dead_code)]
pub type WeakAlbumView<'a> = WeakRwLock<AlbumView<'a>>;

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
        allow_missing_directory: bool,
    ) -> Result<SharedAlbumView<'config>> {
        let album_directory = {
            let artist_lock = artist.read();

            artist_lock
                .artist_directory_in_source_library()
                .join(album_title.clone())
        };

        if !allow_missing_directory && !album_directory.is_dir() {
            return Err(miette!(
                "Provided album directory does not exist: {:?}",
                album_directory
            ));
        }

        let album_configuration = AlbumConfiguration::load(album_directory)?;

        Ok(Arc::new_cyclic(|weak| {
            RwLock::new(Self {
                weak_self: weak.clone(),
                artist,
                configuration: album_configuration,
                title: album_title,
            })
        }))
    }

    #[inline]
    pub fn read_lock_artist(&self) -> RwLockReadGuard<'_, ArtistView<'config>> {
        self.artist.read()
    }

    #[allow(dead_code)]
    #[inline]
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
    pub fn library_configuration(&self) -> &'config LibraryConfig {
        self.read_lock_artist()
            .read_lock_library()
            .library_configuration
    }

    pub fn directory_path_relative_to_library_root(&self) -> PathBuf {
        self.read_lock_artist()
            .directory_path_relative_to_library_root()
            .join(&self.title)
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
    #[allow(dead_code)]
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
            self.album_directory_in_source_library(),
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
            match SourceAlbumState::load_from_directory(
                &source_album_directory_path,
            ) {
                Ok(state) => Some(state),
                Err(error) => match error {
                    SourceAlbumStateLoadError::NotFound
                    | SourceAlbumStateLoadError::SchemaVersionMismatch(_) => {
                        None
                    }
                    _ => return Err(error.into()),
                },
            };
        let fresh_source_album_state =
            SourceAlbumState::generate_from_tracked_files(
                &tracked_source_files,
                &source_album_directory_path,
            )?;

        let saved_transcoded_album_state =
            match TranscodedAlbumState::load_from_directory(
                &transcoded_album_directory_path,
            ) {
                Ok(state) => Some(state),
                Err(error) => match error {
                    TranscodedAlbumStateLoadError::NotFound
                    | TranscodedAlbumStateLoadError::SchemaVersionMismatch(_) => {
                        None
                    }
                    _ => return Err(error.into()),
                },
            };

        // FIXME This is returning a list of files that should exist after transcoding instead of the current filesystem state.
        //       Document this and add an obvious way to generate both, then use the current filesystem state here
        //       (2023-08-05: ?? what did I mean here, the current way works).
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
                    miette!("Could not upgrade AlbumView's weak_self!")
                })?,
                tracked_source_files,
            )?;

        Ok(full_changes)
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
        let album = self.album_read();
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

    #[inline]
    fn album_read(&self) -> RwLockReadGuard<'_, AlbumView<'config>> {
        self.album.read()
    }

    #[allow(dead_code)]
    #[inline]
    fn album_write(&self) -> RwLockWriteGuard<'_, AlbumView<'config>> {
        self.album.write()
    }
}
