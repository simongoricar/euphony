use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use miette::{miette, Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};

use crate::commands::transcode::album_state::common::AlbumFileState;
use crate::commands::transcode::views::common::SortedFileMap;
use crate::commands::transcode::views::AlbumSourceFileList;

const TRANSCODED_ALBUM_STATE_FILE_NAME: &str = ".album.transcode-state.euphony";
const TRANSCODED_ALBUM_STATE_SCHEMA_VERSION: u32 = 2;

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
    #[allow(dead_code)]
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
            .map(PathBuf::from))
    }
}
