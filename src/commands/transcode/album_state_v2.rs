use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::ops::Sub;
use std::path::{Path, PathBuf};
use std::sync::{RwLockReadGuard, RwLockWriteGuard};
use std::time::UNIX_EPOCH;

use miette::{miette, Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};

use crate::commands::transcode::jobs::{
    CancellableTaskV2,
    CopyFileJob,
    DeleteProcessedFileJob,
    FileJobMessage,
    IntoCancellableTaskV2,
    TranscodeAudioFileJob,
};
use crate::commands::transcode::views::{
    AlbumSourceFileList,
    AlbumView,
    SharedAlbumView,
    SortedFileMap,
};
use crate::console::backends::shared::queue_v2::QueueItemID;

const SOURCE_ALBUM_STATE_FILE_NAME: &str = ".album.source-state.euphony";
const SOURCE_ALBUM_STATE_SCHEMA_VERSION: u32 = 2;

const TRANSCODED_ALBUM_STATE_FILE_NAME: &str = ".album.transcode-state.euphony";
const TRANSCODED_ALBUM_STATE_SCHEMA_VERSION: u32 = 2;

/// We store file creation and modification in 64-bit floats, but we usually compare two times
/// that should match using some tolerance (usually to avoid rounding errors).
///
/// Set the `max_distance` to a tolerance of your choice. If the difference is larger,
/// this function returns `true`.
#[inline]
fn f64_approximate_eq(first: f64, second: f64, max_distance: f64) -> bool {
    (first - second).abs() < max_distance
}

/// Represents the filesystem state for the given album.
/// **This struct is album location-agnostic (meaning you can use it for generating
/// info about both the source and the transcoded album directory)!**
///
/// While perhaps obvious, do note that if loaded from (part of) a file,
/// the audio/data file sorting stays as configured when the state was saved (no resorting is done).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AlbumFileState {
    /// The audio files.
    pub audio_files: HashMap<String, FileTrackedMetadata>,

    /// The non-audio files.
    pub data_files: HashMap<String, FileTrackedMetadata>,
}

impl AlbumFileState {
    /// Generate an `AlbumFileState` instance from the `AlbumSourceFileList`
    /// you got from `AlbumView`. A bit complicated, I know.
    ///
    /// The data in the instance refers to the state in the **source (untranscoded) album directory**.
    pub fn generate_source_state_from_source_file_list<P: AsRef<Path>>(
        tracked_source_files: &AlbumSourceFileList,
        base_source_album_directory: P,
    ) -> Result<Self> {
        let base_source_album_directory = base_source_album_directory.as_ref();

        let audio_file_map = Self::build_file_map_from_paths(
            base_source_album_directory,
            &tracked_source_files.audio_files,
            true,
        )?;

        let data_file_map = Self::build_file_map_from_paths(
            base_source_album_directory,
            &tracked_source_files.data_files,
            true,
        )?;

        Ok(Self {
            audio_files: audio_file_map,
            data_files: data_file_map,
        })
    }

    /// Generate an `AlbumFileState` instance from the `AlbumSourceFileList`.
    ///
    /// The data in the instance refers to the state in the **target (transcoded) album directory**.
    pub fn generate_transcoded_state_from_source_file_list<P: AsRef<Path>>(
        tracked_source_files: &AlbumSourceFileList,
        base_transcoded_album_directory: P,
    ) -> Result<Self> {
        let base_transcoded_album_directory =
            base_transcoded_album_directory.as_ref();

        let source_to_transcoded_map = tracked_source_files
            .map_source_file_paths_to_transcoded_file_paths_relative();

        let transcoded_audio_file_list: Vec<PathBuf> =
            source_to_transcoded_map.audio.values().cloned().collect();
        let transcoded_data_file_list: Vec<PathBuf> =
            source_to_transcoded_map.data.values().cloned().collect();

        // Take the transcoded values in the map and generate metadata about the files.
        let audio_file_map = Self::build_file_map_from_paths(
            base_transcoded_album_directory,
            &transcoded_audio_file_list,
            false,
        )?;

        let data_file_map = Self::build_file_map_from_paths(
            base_transcoded_album_directory,
            &transcoded_data_file_list,
            false,
        )?;

        Ok(Self {
            audio_files: audio_file_map,
            data_files: data_file_map,
        })
    }

    /// Given a base album path and the list containing relative paths (relative to
    /// `album_directory_path`), this function builds a `HashMap` from all the relative file paths
    /// to the `FileTrackedMetadata` instances that contain additional file metadata we usually
    /// need to perform diffing between transcodes.
    fn build_file_map_from_paths<P: AsRef<Path>>(
        album_base_directory_path: P,
        relative_file_paths: &Vec<PathBuf>,
        require_all_files_to_exist: bool,
    ) -> Result<HashMap<String, FileTrackedMetadata>> {
        let album_directory_path = album_base_directory_path.as_ref();

        let mut file_map: HashMap<String, FileTrackedMetadata> =
            HashMap::with_capacity(relative_file_paths.len());

        for file_relative_path in relative_file_paths {
            let file_absolute_path =
                album_directory_path.join(file_relative_path);

            if !file_absolute_path.is_file() {
                if require_all_files_to_exist {
                    return Err(miette!(
                        "File is required to exist but doesn't!"
                    ));
                } else {
                    continue;
                }
            }

            let tracked_file_metadata = FileTrackedMetadata::from_file_path(
                album_directory_path.join(file_relative_path),
            )
            .wrap_err_with(|| miette!("Could not generate file metadata."))?;

            let file_relative_path_string =
                file_relative_path.to_string_lossy().to_string();

            file_map.insert(file_relative_path_string, tracked_file_metadata);
        }

        Ok(file_map)
    }
}


/// Represents the entire state of the *source* album at transcode time.
/// The source state is kept in a dotfile (see `SOURCE_ALBUM_STATE_FILE_NAME`) in the
/// source album directory so it can be loaded and is compared to the transcoded state whenever
/// the user runs the transcoding command again.
///
/// This way we can deduce what files haven't been transcoded, which have been changed and which
/// have been removed from the source directory, but still exist in the target directory.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SourceAlbumState {
    /// Indicates the current album state version.
    /// See `SOURCE_ALBUM_STATE_SCHEMA_VERSION` for the current version number.
    pub schema_version: u32,

    /// A map of tracked (i.e. transcoded) files (for both audio and data files).
    ///
    /// Keys are file paths relative to the directory for which the state
    /// is being generated for / is saved inside of.
    pub tracked_files: AlbumFileState,
}

impl SourceAlbumState {
    /// Load the album state from the given file path.
    ///
    /// *NOTE: If at all possible, use `SourceAlbumState::from_directory_path` instead.
    /// This ensures we respect the `.*.euphony` file naming from `SOURCE_ALBUM_STATE_FILE_NAME`.*
    pub fn load_from_file<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        let file_path = file_path.as_ref();

        if !file_path.is_file() {
            return Err(miette!("Couldn't load source album state from file: file doesn't exist."));
        }

        let file_contents = fs::read_to_string(file_path)
            .into_diagnostic()
            .wrap_err_with(|| miette!("Could not read file."))?;

        let state: Self = serde_json::from_str(&file_contents)
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not deserialize file contents as JSON.")
            })?;

        Ok(state)
    }

    /// Load the source album state for the given album directory path. If the directory does not have
    /// an album state saved, `Ok(None)` will be returned.
    ///
    /// This method will use the `.album.source-state.euphony` file (see `SOURCE_ALBUM_STATE_FILE_NAME`)
    /// directly inside the directory.
    pub fn load_from_directory<P: AsRef<Path>>(
        directory_path: P,
    ) -> Result<Option<Self>> {
        let album_state_file_path =
            directory_path.as_ref().join(SOURCE_ALBUM_STATE_FILE_NAME);

        if !album_state_file_path.is_file() {
            return Ok(None);
        }

        Ok(Some(Self::load_from_file(album_state_file_path)?))
    }

    /// Save the source album state into the given file as JSON. If the file exists without
    /// `allow_overwrite` being `true`, the method will return an error.
    pub fn save_to_file<P: AsRef<Path>>(
        &self,
        output_file_path: P,
        allow_overwrite: bool,
    ) -> Result<()> {
        let output_file_path = output_file_path.as_ref();

        if output_file_path.exists() && !output_file_path.is_file() {
            return Err(miette!("Path exists, but it's not a file."));
        }

        if output_file_path.is_file() && !allow_overwrite {
            return Err(miette!(
                "File already exists, but overwriting is not allowed."
            ));
        }

        let serialized_state = serde_json::to_string(self)
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not serialize source album state to string.")
            })?;

        let mut output_file =
            File::create(output_file_path)
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not open file for writing."))?;

        output_file
            .write_all(serialized_state.as_bytes())
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not write source album state to file.")
            })?;

        Ok(())
    }

    /// Save the source album state into the given directory as JSON.
    /// If the file exists without `allow_overwrite` being `true`, this method will return an `Err`.
    ///
    /// *This method is preffered over `SourceAlbumState::save_to_file` since it automatically uses
    /// the correct file name (see `SOURCE_ALBUM_STATE_FILE_NAME`).*
    pub fn save_to_directory<P: AsRef<Path>>(
        &self,
        output_directory_path: P,
        allow_overwrite: bool,
    ) -> Result<()> {
        let output_file_path = output_directory_path
            .as_ref()
            .join(SOURCE_ALBUM_STATE_FILE_NAME);

        self.save_to_file(output_file_path, allow_overwrite)
    }

    /// Generate a new `SourceAlbumState` instance by looking at the file list provided by
    /// `tracked_files`.
    ///
    /// A path to the base of the source directory is also required for consistency with the
    /// `TranscodedAlbumState` version of this method.
    pub fn generate_from_tracked_files<P: AsRef<Path>>(
        tracked_album_files: &AlbumSourceFileList,
        base_source_album_directory: P,
    ) -> Result<Self> {
        let tracked_files =
            AlbumFileState::generate_source_state_from_source_file_list(
                tracked_album_files,
                base_source_album_directory,
            )?;

        Ok(Self {
            schema_version: SOURCE_ALBUM_STATE_SCHEMA_VERSION,
            tracked_files,
        })
    }
}

/// Represents the entire state of the *transcoded* side of the album.
///
/// See `SourceAlbumState` for the source part of the state.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TranscodedAlbumState {
    /// Indicates the current transcoded album state schema version.
    pub schema_version: u32,

    /// A map of transcoded file paths to original file paths
    /// (both relative to the album directory).
    pub transcoded_to_original_file_paths: SortedFileMap<String, String>,

    /// A map of transcoded files (for both audio and data files).
    /// Keys are file paths relative to the transcoded album directory.
    pub transcoded_files: AlbumFileState,
}

impl TranscodedAlbumState {
    /// Load the transcoded album state from the given file path.
    ///
    /// *NOTE: If at all possible, use `TranscodedAlbumState::from_directory_path` instead. This
    /// ensures we respect the `.*.euphony` file naming from `TRANSCODED_ALBUM_STATE_FILE_NAME`.*
    pub fn load_from_file<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        let file_path = file_path.as_ref();

        if !file_path.is_file() {
            return Err(miette!("Couldn't load transcoded album state from file: file does not exist."));
        }

        let file_contents = fs::read_to_string(file_path)
            .into_diagnostic()
            .wrap_err_with(|| miette!("Could not read file."))?;

        let transcoded_state: Self = serde_json::from_str(&file_contents)
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not deserialize file contents as JSON.")
            })?;

        Ok(transcoded_state)
    }

    /// Load the transcoded album state for the given transcoded album directory path.
    /// If the directory does not have a saved state, `Ok(None)` will be returned.
    ///
    /// This method will use the `.album.transcode-state.euphony` file
    /// (see `TRANSCODED_ALBUM_STATE_FILE_NAME`) directly inside the transcoded library directory.
    pub fn load_from_directory<P: AsRef<Path>>(
        directory_path: P,
    ) -> Result<Option<Self>> {
        let transcoded_album_state_path = directory_path
            .as_ref()
            .join(TRANSCODED_ALBUM_STATE_FILE_NAME);

        if !transcoded_album_state_path.is_file() {
            return Ok(None);
        }

        Ok(Some(Self::load_from_file(
            transcoded_album_state_path,
        )?))
    }

    /// Save the transcoded album state into the given file as JSON. If the file exists and
    /// `allow_overwrite` is `true`, the method will return an `Err`.
    ///
    /// *NOTE: If at all possible, use `TranscodedAlbumState::save_to_directory instead.
    /// This ensures we respect the `.*.euphony` file naming from `TRANSCODED_ALBUM_STATE_FILE_NAME`.*
    pub fn save_to_file<P: AsRef<Path>>(
        &self,
        output_file_path: P,
        allow_overwrite: bool,
    ) -> Result<()> {
        let output_file_path = output_file_path.as_ref();

        if output_file_path.exists() && !output_file_path.is_file() {
            return Err(miette!("Path exists, but it's not a file."));
        }

        if output_file_path.is_file() && !allow_overwrite {
            return Err(miette!(
                "File already exists, but overwriting is not allowed."
            ));
        }

        let serialized_state = serde_json::to_string(self)
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!(
                    "Could not serialize transcoded album state into string."
                )
            })?;

        let mut output_file =
            File::create(output_file_path)
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not open file for writing."))?;

        output_file
            .write_all(serialized_state.as_bytes())
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not write transcoded album state to file.")
            })?;

        Ok(())
    }

    /// Save the transcoded album state into the given directory as JSON. This method is preferred
    /// over `TranscodedAlbumState::save_to_file` since it automatically uses the correct file name
    /// (see `TRANSCODED_ALBUM_STATE_FILE_NAME`).
    pub fn save_to_directory<P: AsRef<Path>>(
        &self,
        output_directory_path: P,
        allow_overwrite: bool,
    ) -> Result<()> {
        let output_file_path = output_directory_path
            .as_ref()
            .join(TRANSCODED_ALBUM_STATE_FILE_NAME);

        self.save_to_file(output_file_path, allow_overwrite)
    }

    /// Build a new `TranscodedAlbumState` from the given `AlbumSourceFileList`.
    ///
    /// This takes care of modifying audio file extensions into the transcoded ones automatically,
    /// the provided `AlbumSourceFileList` should just be a normal file scan
    /// (see `AlbumView::tracked_source_files`).
    pub fn generate_from_tracked_files<P: AsRef<Path>>(
        tracked_album_files: &AlbumSourceFileList,
        base_transcoded_album_directory: P,
    ) -> Result<Self> {
        let transcoded_file_state =
            AlbumFileState::generate_transcoded_state_from_source_file_list(
                tracked_album_files,
                base_transcoded_album_directory,
            )?;


        let transcoded_to_source_map_pathbuf =
            tracked_album_files.map_transcoded_paths_to_source_paths_relative();

        let transcoded_to_source_audio_map_string: HashMap<String, String> =
            transcoded_to_source_map_pathbuf
                .audio
                .iter()
                .map(|(key, value)| {
                    (
                        key.to_string_lossy().to_string(),
                        value.to_string_lossy().to_string(),
                    )
                })
                .collect();

        let transcoded_to_source_data_map_string: HashMap<String, String> =
            transcoded_to_source_map_pathbuf
                .data
                .iter()
                .map(|(key, value)| {
                    (
                        key.to_string_lossy().to_string(),
                        value.to_string_lossy().to_string(),
                    )
                })
                .collect();


        Ok(Self {
            schema_version: TRANSCODED_ALBUM_STATE_SCHEMA_VERSION,
            transcoded_to_original_file_paths: SortedFileMap::new(
                transcoded_to_source_audio_map_string,
                transcoded_to_source_data_map_string,
            ),
            transcoded_files: transcoded_file_state,
        })
    }

    /// Provided a transcoded file path (relative to the album directory),
    /// get the associated relative file path in the album source directory.
    ///
    /// While the information is there, this method does not indicate whether the provided path
    /// is an audio or a data file.
    pub fn get_original_file_path<P: AsRef<Path>>(
        &self,
        transcoded_file_path: P,
    ) -> Result<Option<PathBuf>> {
        let transcoded_file_path = transcoded_file_path.as_ref();
        if !transcoded_file_path.is_relative() {
            return Err(miette!("Provided file path should be relative."));
        }

        let file_path_string =
            transcoded_file_path.to_string_lossy().to_string();

        // Information about potential `.mp3 -> .original audio extension` is
        // already there in the HashMap.
        Ok(self
            .transcoded_to_original_file_paths
            .get(&file_path_string)
            .and_then(|str| Some(PathBuf::from(str))))
    }
}

/// A single tracked file. Contains the logic for comparing multiple tracked files between runs.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileTrackedMetadata {
    pub size_bytes: u64,
    pub time_modified: f64,
    pub time_created: f64,
}

impl FileTrackedMetadata {
    /// Instantiate a new `FileTrackedMetadata` that will contain the file's size in bytes
    /// and its creation and modification time.
    pub fn new(size_bytes: u64, time_modified: f64, time_created: f64) -> Self {
        Self {
            size_bytes,
            time_modified,
            time_created,
        }
    }

    /// Generate a new `FileTrackedMetadata` instance by getting the relevant values from
    /// the filesystem for the given `file_path`.
    pub fn from_file_path<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        let file_path = file_path.as_ref();
        if !file_path.is_file() {
            return Err(miette!("File path is not a file!"));
        }

        let file_metadata =
            file_path.metadata().into_diagnostic().wrap_err_with(|| {
                miette!(
                    "Could not retrieve metadata for file: {:?}",
                    file_path
                )
            })?;


        let file_size_bytes = file_metadata.len();

        let file_creation_time = file_metadata
            .created()
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!(
                    "Could not retrieve creation time for file: {:?}",
                    file_path
                )
            })?
            .duration_since(UNIX_EPOCH)
            .into_diagnostic()?;

        let file_modification_time = file_metadata
            .modified()
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!(
                    "Could not retrieve modification time for file: {:?}",
                    file_path
                )
            })?
            .duration_since(UNIX_EPOCH)
            .into_diagnostic()?;


        Ok(FileTrackedMetadata::new(
            file_size_bytes,
            file_modification_time.as_secs_f64(),
            file_creation_time.as_secs_f64(),
        ))
    }

    /// Check whether the `FileTrackedMetadata` pair matches.
    ///
    /// - any change in file size will cause it to return `false`,
    /// - any change in file creation/modification time (larger than 0.1) will cause it to return `false`.
    pub fn matches(&self, other: &Self) -> bool {
        if self.size_bytes != other.size_bytes {
            return false;
        }

        static DEFAULT_MAX_TIME_DISTANCE: f64 = 0.1;

        if !f64_approximate_eq(
            self.time_created,
            other.time_created,
            DEFAULT_MAX_TIME_DISTANCE,
        ) {
            return false;
        }

        if !f64_approximate_eq(
            self.time_modified,
            other.time_modified,
            DEFAULT_MAX_TIME_DISTANCE,
        ) {
            return false;
        }

        true
    }
}


/// Represents a double `Vec`: one for audio files, the other for data files.
/// If you want to deal with unknown files as well, see `ExtendedSortedFileList`.
// TODO Move to some utility module.
#[derive(Default)]
pub struct SortedFileList<T> {
    pub audio: Vec<T>,
    pub data: Vec<T>,
}

impl<T> SortedFileList<T> {
    /// Initialize a new `SortedFileList` by providing its audio and data vector.
    pub fn new(audio_list: Vec<T>, data_list: Vec<T>) -> Self {
        Self {
            audio: audio_list,
            data: data_list,
        }
    }

    /// Returns `true` if both `audio` and `data` lists are empty.
    pub fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.data.is_empty()
    }

    /// Get total length of the sorted file list.
    pub fn total_length(&self) -> usize {
        self.audio.len() + self.data.len()
    }
}


/// Unlike `SortedFileList`, `ExtendedSortedFileList` includes `unknown` types of files.
/// That is the only difference.
// TODO Move to some utility module.
pub struct ExtendedSortedFileList<T> {
    pub audio: Vec<T>,
    pub data: Vec<T>,
    pub unknown: Vec<T>,
}

impl<T> ExtendedSortedFileList<T> {
    /// Initialize a new `ExtendedSortedFileList` by providing its audio, data and unknown file vector.
    pub fn new(
        audio_list: Vec<T>,
        data_list: Vec<T>,
        unknown_list: Vec<T>,
    ) -> Self {
        Self {
            audio: audio_list,
            data: data_list,
            unknown: unknown_list,
        }
    }

    /// Returns `true` if `audio`, `data` and `unknown` lists are empty.
    pub fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.data.is_empty() && self.unknown.is_empty()
    }

    /// Get total length of the extended sorted file list.
    pub fn total_length(&self) -> usize {
        self.audio.len() + self.data.len() + self.unknown.len()
    }
}


/// Describes one of three possible file types (audio, data, unknown).
pub enum FileType {
    /// Audio files, as configured per-library.
    Audio,

    /// Data (non-audio) files, as configured per-library.
    Data,

    /// Unknown (non-audio, non-data) files.
    ///
    /// This type only appears in cases of "excess" files in the transcoded library
    /// (see `AlbumFileChangesV2::generate_from_source_and_transcoded_state`).
    Unknown,
}


/// Given a set of snapshots from potential previous transcodes and the current filesystem state,
/// this struct processes the changes and sorts them into multiple groups of newly-added files,
/// modified files and removed files - essentially a diff with the previous folder contents.
///
/// This is part of the transcoding scanning process - this information is basically the last step.
/// If we have this, we know what files need to be transcoded, copied, removed, etc.
pub struct AlbumFileChangesV2<'view> {
    /// `AlbumView` these changes were generated from.
    pub album_view: SharedAlbumView<'view>,

    // List of tracked files.
    pub tracked_files: AlbumSourceFileList<'view>,

    /// Files in the source album directory that are new (haven't been processed yet).
    ///
    /// Paths are absolute and point to the source album directory.
    pub added_in_source_since_last_transcode: SortedFileList<PathBuf>,

    /// Files in the source album directory that have been previously processed,
    /// but have changed since.
    ///
    /// Paths are absolute and point to the source album directory.
    pub changed_in_source_since_last_transcode: SortedFileList<PathBuf>,

    /// Files in the source album directory that have been previously processed,
    /// but no longer exist in the source directory, meaning we should probably remove their
    /// counterparts from the transcoded album directory as well.
    ///
    /// This mostly happens when an album is transcoded and then, for example, the user runs
    /// a tagger through the audio files and applies a different naming scheme.
    ///
    /// Paths are absolute and point to the source album directory.
    pub removed_in_source_since_last_transcode: SortedFileList<PathBuf>,

    /// Files that aren't new in the source directory, but are nevertheless missing from the
    /// transcoded album directory. We'll also transcode and/or copy these files
    /// (but we'll have to find out what their source counterparts are - see the
    /// `TranscodedAlbumState::original_file_paths` map).
    ///
    /// Paths are absolute and point to the source album directory.
    pub missing_in_transcoded: SortedFileList<PathBuf>,

    /// Files that don't belong to any transcode - essentally extra files we should probably remove.
    /// This is unlikely to happen unless the user has manually modified the transcoded album directory.
    ///
    /// Paths are absolute and point to the *transcoded album directory*.
    pub excess_in_transcoded: ExtendedSortedFileList<PathBuf>,
}

impl<'a> AlbumFileChangesV2<'a> {
    /// Generate an `AlbumFileChangesV2` instance by comparing several saved and fresh filesystem states:
    /// - `saved_source_state` is, if previously transcoded, the source album state as saved in `.album.source-state.euphony`,
    /// - `fresh_source_state` is the fresh filesystem state of the source album directory,
    /// - `saved_transcoded_state` is, if previously transcoded, the transcoded album map as saved in `.album.transcode-state.euphony`,
    /// - `fresh_transcoded_state` is the fresh filesystem state of the transcoded album directory (album directory-relative paths).
    ///
    /// `album` is a reference to the `AlbumView` the album states are associated with and
    /// `album_file_list` is the associated source file list.
    pub fn generate_from_source_and_transcoded_state(
        saved_source_state: Option<SourceAlbumState>,
        fresh_source_state: AlbumFileState,
        saved_transcoded_state: Option<TranscodedAlbumState>,
        fresh_transcoded_state: AlbumFileState,
        album: SharedAlbumView<'a>,
        album_file_list: AlbumSourceFileList<'a>,
    ) -> Result<Self> {
        let (
            transcoding_config,
            source_album_directory,
            transcoded_album_directory,
        ) = {
            let album_locked =
                album.read().expect("AlbumView RwLock has been poisoned!");

            (
                album_locked.library_configuration().transcoding.clone(),
                album_locked.album_directory_in_source_library(),
                album_locked.album_directory_in_transcoded_library(),
            )
        };


        let saved_source_files = saved_source_state
            .and_then(|inner| Some(inner.tracked_files))
            .unwrap_or_default();
        let fresh_source_files = fresh_source_state;

        // See `TranscodedAlbumState::original_file_paths` - this is a saved map from the
        // last transcode - keys are transcoded file paths and values are source file paths
        // (relative to the album directory).
        let saved_transcoded_map = saved_transcoded_state
            .and_then(|inner| Some(inner.transcoded_to_original_file_paths))
            .unwrap_or_default();
        let fresh_transcoded_files = fresh_transcoded_state;

        let saved_source_audio_files_set: HashSet<String> =
            saved_source_files.audio_files.keys().cloned().collect();
        let saved_source_data_files_set: HashSet<String> =
            saved_source_files.data_files.keys().cloned().collect();

        let fresh_source_audio_files_set: HashSet<String> =
            fresh_source_files.audio_files.keys().cloned().collect();
        let fresh_source_data_files_set: HashSet<String> =
            fresh_source_files.data_files.keys().cloned().collect();

        let fresh_transcoded_audio_files_set: HashSet<String> =
            fresh_transcoded_files.audio_files.keys().cloned().collect();
        let fresh_transcoded_data_files_set: HashSet<String> =
            fresh_transcoded_files.data_files.keys().cloned().collect();
        let fresh_transcoded_full_files_set: HashSet<String> =
            HashSet::from_iter(
                fresh_transcoded_data_files_set
                    .union(&fresh_transcoded_data_files_set)
                    .cloned(),
            );


        // Newly added files in the source directory since last transcode.
        let new_audio_files: Vec<String> = fresh_source_audio_files_set
            .sub(&saved_source_audio_files_set)
            .iter()
            .map(|item| item.to_string())
            .collect();
        let new_data_files: Vec<String> = fresh_source_data_files_set
            .sub(&saved_source_data_files_set)
            .iter()
            .map(|item| item.to_string())
            .collect();


        // Changed files in the source directory since last transcode.
        // This is done by calling a filtering method that only returns file paths of files whose
        // metadata (`FileTrackedMetadata`) has changed.
        let changed_audio_files = Self::filter_only_changed_files(
            saved_source_audio_files_set
                .intersection(&fresh_source_audio_files_set),
            &saved_source_files.audio_files,
            &fresh_source_files.audio_files,
        )?;
        let changed_data_files = Self::filter_only_changed_files(
            saved_source_data_files_set
                .intersection(&fresh_source_data_files_set),
            &saved_source_files.data_files,
            &fresh_source_files.data_files,
        )?;


        // Removed files in the source directory since last transcode.
        let removed_audio_files: Vec<String> = saved_source_audio_files_set
            .sub(&fresh_source_audio_files_set)
            .iter()
            .map(|item| item.to_string())
            .collect();
        let removed_data_files: Vec<String> = saved_source_data_files_set
            .sub(&fresh_source_data_files_set)
            .iter()
            .map(|item| item.to_string())
            .collect();


        // Files that aren't new, but are still missing in the transcoded directory (likely by user intervention).
        let source_to_transcode_map = album_file_list
            .map_source_file_paths_to_transcoded_file_paths_relative();
        let transcode_to_source_map = source_to_transcode_map.to_inverted_map();

        let expected_transcoded_audio_file_set: HashSet<String> =
            source_to_transcode_map
                .audio
                .values()
                .map(|item| item.to_string_lossy().to_string())
                .collect();
        let expected_transcoded_data_file_set: HashSet<String> =
            source_to_transcode_map
                .data
                .values()
                .map(|item| item.to_string_lossy().to_string())
                .collect();

        let missing_audio_files: Vec<String> = if saved_transcoded_map.is_empty()
        {
            // No transcode has been done previously, meaning no files can be missing.
            Vec::new()
        } else {
            expected_transcoded_audio_file_set
                .sub(&fresh_transcoded_audio_files_set)
                .iter()
                // Map each missing transcoded file to its original.
                .map(|item| {
                    transcode_to_source_map.audio.get(&PathBuf::from(item))
                        .expect("audio file path was not present in the inverted map")
                        .to_string_lossy()
                        .to_string()
                })
                .collect()
        };

        let missing_data_files: Vec<String> = if saved_transcoded_map.is_empty()
        {
            // No transcode has been done previously, meaning no files can be missing.
            Vec::new()
        } else {
            expected_transcoded_data_file_set
                .sub(&fresh_transcoded_data_files_set)
                .iter()
                // Map each missing transcoded file to its original.
                .map(|item| {
                    transcode_to_source_map.data.get(&PathBuf::from(item))
                        .expect("audio file path was not present in the inverted map")
                        .to_string_lossy()
                        .to_string()
                })
                .collect()
        };


        // Files in the transcoded directory that don't belong to any previous transcode and will not be
        // overwritten by this transcode - essentially extra files we might like to delete.
        let raw_excess_files_in_transcoded: Vec<String> = {
            let expected_transcoded_full_file_set: HashSet<String> =
                source_to_transcode_map
                    .into_flattened_map()
                    .values()
                    .map(|item| item.to_string_lossy().to_string())
                    .collect();

            fresh_transcoded_full_files_set
                .sub(&expected_transcoded_full_file_set)
                .iter()
                .map(|item| item.to_string())
                .collect()
        };

        // Now we sort them into excess audio, data and unknown files.
        let mut excess_audio_files_in_transcoded: Vec<String> = Vec::new();
        let mut excess_data_files_in_transcoded: Vec<String> = Vec::new();
        let mut excess_unknown_files_in_transcoded: Vec<String> = Vec::new();

        for excess_file in raw_excess_files_in_transcoded {
            if transcoding_config
                .is_path_audio_file_by_extension(&excess_file)?
            {
                excess_audio_files_in_transcoded.push(excess_file);
            } else if transcoding_config
                .is_path_data_file_by_extension(&excess_file)?
            {
                excess_data_files_in_transcoded.push(excess_file);
            } else {
                // This can happen if the user copies some completely other file into the
                // transcoded album directory.
                excess_unknown_files_in_transcoded.push(excess_file);
            }
        }


        // Construct final sorted file lists by converting all the `String`s to `PathBuf`s.
        let added_in_source_since_last_transcode = SortedFileList::new(
            Self::convert_relative_paths_to_absolute(
                &source_album_directory,
                new_audio_files,
            ),
            Self::convert_relative_paths_to_absolute(
                &source_album_directory,
                new_data_files,
            ),
        );

        let changed_in_source_since_last_transcode = SortedFileList::new(
            Self::convert_relative_paths_to_absolute(
                &source_album_directory,
                changed_audio_files,
            ),
            Self::convert_relative_paths_to_absolute(
                &source_album_directory,
                changed_data_files,
            ),
        );

        let removed_in_source_since_last_transcode = SortedFileList::new(
            Self::convert_relative_paths_to_absolute(
                &source_album_directory,
                removed_audio_files,
            ),
            Self::convert_relative_paths_to_absolute(
                &source_album_directory,
                removed_data_files,
            ),
        );

        let missing_in_transcoded = SortedFileList::new(
            Self::convert_relative_paths_to_absolute(
                &source_album_directory,
                missing_audio_files,
            ),
            Self::convert_relative_paths_to_absolute(
                &source_album_directory,
                missing_data_files,
            ),
        );

        let excess_in_transcoded = ExtendedSortedFileList::new(
            Self::convert_relative_paths_to_absolute(
                &transcoded_album_directory,
                excess_audio_files_in_transcoded,
            ),
            Self::convert_relative_paths_to_absolute(
                &transcoded_album_directory,
                excess_data_files_in_transcoded,
            ),
            Self::convert_relative_paths_to_absolute(
                &transcoded_album_directory,
                excess_unknown_files_in_transcoded,
            ),
        );

        // TODO Thoroughly test the new diff algorithm.

        Ok(Self {
            album_view: album,
            tracked_files: album_file_list,
            added_in_source_since_last_transcode,
            changed_in_source_since_last_transcode,
            removed_in_source_since_last_transcode,
            missing_in_transcoded,
            excess_in_transcoded,
        })
    }

    /// Returns `true` if any changes were detected since last transcode
    /// (essentially always `true` if no previous transcoding has been done
    /// and the directory has some audio/data files).
    pub fn has_changes(&self) -> bool {
        !self.added_in_source_since_last_transcode.is_empty()
            || !self.changed_in_source_since_last_transcode.is_empty()
            || !self.removed_in_source_since_last_transcode.is_empty()
            || !self.missing_in_transcoded.is_empty()
            || !self.excess_in_transcoded.is_empty()
    }

    /// Return the total number of changed files.
    pub fn number_of_changed_files(&self) -> usize {
        self.added_in_source_since_last_transcode.total_length()
            + self.changed_in_source_since_last_transcode.total_length()
            + self.removed_in_source_since_last_transcode.total_length()
            + self.missing_in_transcoded.total_length()
            + self.excess_in_transcoded.total_length()
    }

    /// Generate a `SourceAlbumState` (deserialized version of `.album.source-state.euphony` file),
    /// usually with the intent to save a fresh version of it to disk.
    ///
    /// This method does no further disk lookups, all information is already in the memory.
    pub fn generate_source_album_state(&self) -> Result<SourceAlbumState> {
        SourceAlbumState::generate_from_tracked_files(
            &self.tracked_files,
            self.read_lock_album().album_directory_in_source_library(),
        )
    }

    /// Generate a `TranscodedAlbumState`
    /// (deserialized version of `.album.transcode-state.euphony` file),
    /// usually with the intent to save a fresh version of it to disk.
    ///
    /// This method **does further disk lookups**.
    pub fn generate_transcoded_album_state(
        &self,
    ) -> Result<TranscodedAlbumState> {
        TranscodedAlbumState::generate_from_tracked_files(
            &self.tracked_files,
            self.read_lock_album()
                .album_directory_in_transcoded_library(),
        )
    }

    /// This method will generate and return a list of cancellable tasks.
    ///
    /// The `queue_item_id_generator` parameter should be a closure that will take two parameters:
    /// - `FileType`, which is the type of the file (audio or data) and
    /// - `&PathBuf`, which is the absolute path to the source file.
    ///
    /// The closure should return an `Ok(QueueItemID)`.
    /// If `Err` is returned, this method will exit early, propagating the error.
    pub fn generate_file_jobs<
        F: Fn(FileType, &PathBuf) -> Result<QueueItemID>,
    >(
        &self,
        queue_item_id_generator: F,
    ) -> Result<Vec<CancellableTaskV2<FileJobMessage>>> {
        let mut jobs: Vec<CancellableTaskV2<FileJobMessage>> =
            Vec::with_capacity(self.number_of_changed_files());

        let source_to_target_absolute_path_map = self
            .tracked_files
            .map_source_file_paths_to_transcoded_file_paths_absolute();


        // Parse file lists to separate their change types
        // (some files need to be transcoded, some copied, some deleted).
        let audio_files_to_transcode = self
            .added_in_source_since_last_transcode
            .audio
            .iter()
            .chain(self.changed_in_source_since_last_transcode.audio.iter())
            .chain(self.missing_in_transcoded.audio.iter());
        let data_files_to_copy_to_transcoded_dir = self
            .added_in_source_since_last_transcode
            .data
            .iter()
            .chain(self.changed_in_source_since_last_transcode.data.iter())
            .chain(self.missing_in_transcoded.data.iter());

        // TODO Make removing excess files configurable.
        let audio_files_to_delete_from_transcoded_dir = self
            .removed_in_source_since_last_transcode
            .audio
            .iter()
            .chain(self.excess_in_transcoded.audio.iter());
        let data_files_to_delete_from_transcoded_dir = self
            .removed_in_source_since_last_transcode
            .data
            .iter()
            .chain(self.excess_in_transcoded.data.iter());
        let unknown_files_to_delete_from_transcoded_dir =
            self.excess_in_transcoded.unknown.iter();


        // Generate jobs from the parsed file lists.
        for file_path in audio_files_to_transcode {
            let source_path = file_path;
            let target_path = source_to_target_absolute_path_map
                .audio
                .get(source_path)
                .ok_or_else(|| {
                    miette!("BUG: Map is missing audio file entry.")
                })?;

            let queue_item_id =
                queue_item_id_generator(FileType::Audio, source_path)?;

            let transcoding_job = TranscodeAudioFileJob::new(
                self.album_view.clone(),
                source_path.to_path_buf(),
                target_path.to_path_buf(),
                queue_item_id,
            )
            .wrap_err_with(|| {
                miette!("Could not create TranscodeAudioFileJob.")
            })?;

            jobs.push(transcoding_job.into_cancellable_task());
        }

        for file_path in data_files_to_copy_to_transcoded_dir {
            let source_path = file_path;
            let target_path = source_to_target_absolute_path_map
                .data
                .get(source_path)
                .ok_or_else(|| {
                    miette!("BUG: Map is missing data file entry.")
                })?;

            let queue_item_id =
                queue_item_id_generator(FileType::Data, source_path)?;

            let copy_job = CopyFileJob::new(
                self.album_view.clone(),
                source_path.to_path_buf(),
                target_path.to_path_buf(),
                queue_item_id,
            )
            .wrap_err_with(|| miette!("Could not create CopyFileJob."))?;

            jobs.push(copy_job.into_cancellable_task());
        }


        for file_path in audio_files_to_delete_from_transcoded_dir {
            let target_path = file_path;

            let queue_item_id =
                queue_item_id_generator(FileType::Audio, target_path)?;

            let delete_from_processed_job = DeleteProcessedFileJob::new(
                target_path.to_path_buf(),
                true,
                queue_item_id,
            )
            .wrap_err_with(|| {
                miette!("Could not create DeleteProcessedFileJob.")
            })?;

            jobs.push(delete_from_processed_job.into_cancellable_task());
        }

        for file_path in data_files_to_delete_from_transcoded_dir {
            let target_path = file_path;

            let queue_item_id =
                queue_item_id_generator(FileType::Data, target_path)?;

            let delete_from_processed_job = DeleteProcessedFileJob::new(
                target_path.to_path_buf(),
                true,
                queue_item_id,
            )
            .wrap_err_with(|| {
                miette!("Could not create DeleteProcessedFileJob.")
            })?;

            jobs.push(delete_from_processed_job.into_cancellable_task());
        }

        for file_path in unknown_files_to_delete_from_transcoded_dir {
            let target_path = file_path;

            let queue_item_id =
                queue_item_id_generator(FileType::Unknown, target_path)?;

            let delete_from_processed_job = DeleteProcessedFileJob::new(
                target_path.to_path_buf(),
                true,
                queue_item_id,
            )
            .wrap_err_with(|| {
                miette!("Could not create DeleteProcessedFileJob.")
            })?;

            jobs.push(delete_from_processed_job.into_cancellable_task());
        }

        Ok(jobs)
    }

    /// Utility function to filter an iterator of file paths.
    ///
    /// - `map_key_iterator` should iterate over file paths you want to filter for changes,
    /// - `first_metadata_map` and `second_metadata_map` should map from `map_key_iterator`
    ///   items (**all of them**) to a `FileTrackedMetadata` each,
    ///
    /// If either of the `HashMap`s do not contain any single key that the iterator provides,
    /// this function will return en `Err`.
    ///
    /// Process:
    /// - We iterate over each provided file path from the iterator.
    /// - Each file's associated `FileTrackedMetadata` from both maps is retrieved.
    /// - The two metadata structs are compared: if they do not match (i.e. file has changed),
    ///   the file name is retained in the returned vector of Strings. If the files are the same,
    ///   as far as the `FileTrackedMetadata` struct is concerned, it is not in the returned vector.
    fn filter_only_changed_files<'b, I: Iterator<Item = &'b String>>(
        map_key_iterator: I,
        first_metadata_map: &HashMap<String, FileTrackedMetadata>,
        second_metadata_map: &HashMap<String, FileTrackedMetadata>,
    ) -> Result<Vec<String>> {
        Ok(map_key_iterator
            .filter_map(|item| {
                let first_metadata = match first_metadata_map.get(item) {
                    None => {
                        return Some(Err(miette!(
                            "BUG: Missing saved source audio file entry."
                        )));
                    }
                    Some(meta) => meta,
                };

                let second_metadata = match second_metadata_map.get(item) {
                    None => {
                        return Some(Err(miette!(
                            "BUG: Mising fresh source audio file entry."
                        )));
                    }
                    Some(meta) => meta,
                };

                // If the metadata does not match, this means the file has changed, so we include
                // it in the final list of paths.
                if !first_metadata.matches(&second_metadata) {
                    Some(Ok(item.to_string()))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<String>>>()
            .wrap_err_with(|| {
                miette!("Could not compute changed files: invalid metadata map.")
            })?)
    }

    /// Given an iterator over relative paths (can be `String`, `str`),
    /// construct a vector that contains absolute paths.
    fn convert_relative_paths_to_absolute<
        D: AsRef<Path>,
        E: AsRef<Path>,
        I: IntoIterator<Item = E>,
    >(
        base_directory: D,
        paths: I,
    ) -> Vec<PathBuf> {
        let base_directory = base_directory.as_ref();

        paths
            .into_iter()
            .map(|item| base_directory.join(item))
            .collect()
    }

    pub fn read_lock_album(&self) -> RwLockReadGuard<'_, AlbumView<'a>> {
        self.album_view
            .read()
            .expect("Album RwLock has been poisoned!")
    }

    pub fn write_lock_library(&self) -> RwLockWriteGuard<'_, AlbumView<'a>> {
        self.album_view
            .write()
            .expect("Album RwLock has been poisoned!")
    }
}
