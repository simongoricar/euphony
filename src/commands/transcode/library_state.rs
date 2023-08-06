use std::collections::HashMap;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::Path;
use std::{fs, io};

use miette::{miette, Context, Diagnostic, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const LIBRARY_STATE_FILE_NAME: &str = ".library.state.euphony";
const LIBRARY_STATE_SCHEMA_VERSION: u32 = 2;

#[derive(Error, Debug, Diagnostic)]
pub enum LibraryStateLoadError {
    #[error("no state found on disk")]
    NotFound,

    #[error(
        "schema version mismatch: {0} (current is {})",
        LIBRARY_STATE_SCHEMA_VERSION
    )]
    SchemaVersionMismatch(u32),

    #[error("io::Error encountered while loading state")]
    IoError(#[from] io::Error),

    #[error("serde_json::Error encountered while loading state")]
    JSONError(#[from] serde_json::Error),
}


#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TrackedAlbum {
    pub album_title: String,

    /// Relative path from the library root to the album.
    pub album_source_relative_path: String,
}

impl Hash for TrackedAlbum {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.album_source_relative_path.hash(state)
    }
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackedArtistAlbums {
    pub tracked_albums: Vec<TrackedAlbum>,
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LibraryState {
    pub schema_version: u32,

    pub tracked_artists: HashMap<String, TrackedArtistAlbums>,
}

// TODO Integrate this into the real code, then make sure we delete albums that suddenly
//      disappear from the library between transcodes.
impl LibraryState {
    // TODO Generation methods (give list of artists and their albums, but ALL of them,
    //      not just the ones we transcoded this session).
    pub fn new(
        tracked_artists_and_albums: HashMap<String, TrackedArtistAlbums>,
    ) -> Self {
        Self {
            schema_version: LIBRARY_STATE_SCHEMA_VERSION,
            tracked_artists: tracked_artists_and_albums,
        }
    }

    pub fn load_from_file<P: AsRef<Path>>(
        file_path: P,
    ) -> Result<Self, LibraryStateLoadError> {
        let file_path = file_path.as_ref();

        if !file_path.is_file() {
            return Err(LibraryStateLoadError::NotFound);
        }

        let file_contents = fs::read_to_string(file_path)?;
        let state: Self = serde_json::from_str(&file_contents)?;

        if state.schema_version != LIBRARY_STATE_SCHEMA_VERSION {
            return Err(LibraryStateLoadError::SchemaVersionMismatch(
                state.schema_version,
            ));
        }

        Ok(state)
    }

    pub fn load_from_directory<P: AsRef<Path>>(
        directory_path: P,
    ) -> Result<Self, LibraryStateLoadError> {
        let library_state_file_path =
            directory_path.as_ref().join(LIBRARY_STATE_FILE_NAME);

        if !library_state_file_path.is_file() {
            return Err(LibraryStateLoadError::NotFound);
        }

        Self::load_from_file(library_state_file_path)
    }

    pub fn save_to_file<P: AsRef<Path>>(
        &self,
        output_file_path: P,
        allow_overwrite: bool,
    ) -> Result<()> {
        let output_file_path = output_file_path.as_ref();

        if output_file_path.exists() && !output_file_path.is_file() {
            return Err(miette!("Path exists, but it's not a file?!"));
        }

        if output_file_path.is_file() && !allow_overwrite {
            return Err(miette!(
                "File already existing and overwriting is disabled."
            ));
        }

        let serialized_state = serde_json::to_string(self)
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not serialize source library state to string.")
            })?;

        let mut output_file = File::create(output_file_path)
            .into_diagnostic()
            .wrap_err_with(|| {
            miette!("Could not open output file for writing.")
        })?;

        output_file
            .write_all(serialized_state.as_bytes())
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not write serialized library state to file.")
            })?;

        Ok(())
    }

    pub fn save_to_directory<P: AsRef<Path>>(
        &self,
        output_directory_path: P,
        allow_overwrite: bool,
    ) -> Result<()> {
        let output_file_path =
            output_directory_path.as_ref().join(LIBRARY_STATE_FILE_NAME);

        self.save_to_file(output_file_path, allow_overwrite)
    }
}
