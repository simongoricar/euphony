use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

use miette::{miette, Context, Diagnostic, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::commands::transcode::album_state::common::AlbumFileState;
use crate::commands::transcode::views::AlbumSourceFileList;
use crate::configuration::{Config, LibraryConfig};

const SOURCE_ALBUM_STATE_FILE_NAME: &str = ".album.source-state.euphony";
const SOURCE_ALBUM_STATE_SCHEMA_VERSION: u32 = 2;


#[derive(Error, Debug, Diagnostic)]
pub enum SourceAlbumStateLoadError {
    #[error("no state found on disk")]
    NotFound,

    #[error(
        "schema version mismatch: {0} (current is {})",
        SOURCE_ALBUM_STATE_SCHEMA_VERSION
    )]
    SchemaVersionMismatch(u32),

    #[error("io::Error encountered while loading state")]
    IoError(#[from] io::Error),

    #[error("serde_json::Error encountered while loading state")]
    JSONError(#[from] serde_json::Error),
}


/// Represents the entire state of the *source* album directory at either transcode time
/// (if saved to file) or runtime (if generated then).
///
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
    pub fn load_from_file<P: AsRef<Path>>(
        file_path: P,
    ) -> Result<Self, SourceAlbumStateLoadError> {
        let file_path = file_path.as_ref();

        if !file_path.is_file() {
            return Err(SourceAlbumStateLoadError::NotFound);
        }

        let file_contents = fs::read_to_string(file_path)?;
        let state: Self = serde_json::from_str(&file_contents)?;

        if state.schema_version != SOURCE_ALBUM_STATE_SCHEMA_VERSION {
            return Err(SourceAlbumStateLoadError::SchemaVersionMismatch(
                state.schema_version,
            ));
        }

        Ok(state)
    }

    /// Load the source album state for the given album directory path. If the directory does not have
    /// an album state saved, `Ok(None)` will be returned.
    ///
    /// This method will use the `.album.source-state.euphony` file (see `SOURCE_ALBUM_STATE_FILE_NAME`)
    /// directly inside the directory.
    pub fn load_from_directory<P: AsRef<Path>>(
        directory_path: P,
    ) -> Result<Self, SourceAlbumStateLoadError> {
        let album_state_file_path =
            Self::get_state_file_path_for_directory(directory_path);

        if !album_state_file_path.is_file() {
            return Err(SourceAlbumStateLoadError::NotFound);
        }

        Self::load_from_file(album_state_file_path)
    }

    /// Get default path for saving `SourceAlbumState`s inside a directory.
    /// This is set by `SOURCE_ALBUM_STATE_FILE_NAME`, which is currently `.album.source-state.euphony`.
    ///
    /// # Example
    /// ```
    /// let directory_path = Path::from("D:/MusicLibrary/Ed Harrison/Neotokyo");
    ///
    /// assert_eq!(
    ///     Self::get_state_file_path_for_directory(directory_path),
    ///     Path::from("D:/MusicLibrary/Ed Harrison/Neotokyo/.album.source-state.euphony`)
    /// );
    /// ```
    pub fn get_state_file_path_for_directory<P: AsRef<Path>>(
        directory_path: P,
    ) -> PathBuf {
        directory_path.as_ref().join(SOURCE_ALBUM_STATE_FILE_NAME)
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
    /// *This method is preferred over `SourceAlbumState::save_to_file` since it automatically uses
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

    /// Provided a source file path (relative to the source album directory),
    /// get the associated relative file path in the transcoded album directory.
    ///
    /// This method will do the necessary file extension swapping (e.g. FLAC -> MP3).
    pub fn get_transcoded_file_path<P: AsRef<Path>>(
        configuration: &Config,
        library_configuration: &LibraryConfig,
        source_file_path: P,
    ) -> Result<PathBuf> {
        let source_file_path = source_file_path.as_ref();
        if !source_file_path.is_relative() {
            return Err(miette!("Provided file path should be relative."));
        }

        if library_configuration
            .transcoding
            .is_path_audio_file_by_extension(source_file_path)
            .wrap_err_with(|| {
                miette!(
                    "Failed to check whether the file has an audio extension."
                )
            })?
        {
            Ok(source_file_path.with_extension(
                &configuration
                    .tools
                    .ffmpeg
                    .audio_transcoding_output_extension,
            ))
        } else if library_configuration
            .transcoding
            .is_path_data_file_by_extension(source_file_path)
            .wrap_err_with(|| {
                miette!("Failed to check whether the file has a data extension.")
            })?
        {
            Ok(source_file_path.to_path_buf())
        } else {
            Err(miette!(
                "Invalid file: not an audio nor data file: {:?}",
                source_file_path
            ))
        }
    }
}
