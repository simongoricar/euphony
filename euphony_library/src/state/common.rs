use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use miette::{miette, Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};

use crate::utilities::f64_approximate_eq;
use crate::view::AlbumSourceFileList;

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

    /// Given a base album path and the list containing paths relative to `album_directory_path`,
    /// this function builds a `HashMap` from relative file paths
    /// to `FileTrackedMetadata` instances containing per-file metadata.
    ///
    /// We usually need this to perform diffing between transcodes.
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
