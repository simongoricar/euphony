use std::path::Path;

use miette::Result;
use serde::Deserialize;

use crate::{
    filesystem::get_path_extension_or_empty,
    paths::PathsConfiguration,
    traits::{ResolvableConfiguration, ResolvableWithPathsConfiguration},
};


#[derive(Clone)]
pub struct LibraryConfiguration {
    /// Library display name.
    pub name: String,

    /// Absolute path to the library (can include {LIBRARY_BASE},
    /// which will be dynamically replaced with `essentials.base_library_path` on load).
    pub path: String,

    pub ignored_directories_in_base_directory: Option<Vec<String>>,

    /// Validation-related configuration for this library.
    pub validation: LibraryValidationConfiguration,

    /// Transcoding-related configuration for this library.
    pub transcoding: LibraryTranscodingConfiguration,
}

#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedLibraryConfiguration {
    name: String,

    path: String,

    ignored_directories_in_base_directory: Option<Vec<String>>,

    validation: UnresolvedLibraryValidationConfiguration,

    transcoding: UnresolvedLibraryTranscodingConfiguration,
}

impl ResolvableWithPathsConfiguration for UnresolvedLibraryConfiguration {
    type Resolved = LibraryConfiguration;

    fn resolve(
        self,
        paths: &PathsConfiguration,
    ) -> miette::Result<Self::Resolved> {
        let parsed_path = self
            .path
            .replace("{LIBRARY_BASE}", &paths.base_library_path);

        let canonicalized_path = dunce::canonicalize(parsed_path)
            .unwrap_or_else(|_| {
                panic!(
                    "Library \"{}\" could not be found at path \"{}\"!",
                    self.name, self.path,
                )
            });

        if !canonicalized_path.is_dir() {
            panic!(
                "Library \"{}\" has path set to \"{}\", but this path is not a directory!",
                self.name,
                self.path,
            );
        }

        let path = canonicalized_path.to_string_lossy().to_string();


        Ok(LibraryConfiguration {
            name: self.name,
            path,
            ignored_directories_in_base_directory: self
                .ignored_directories_in_base_directory,
            validation: self.validation.resolve()?,
            transcoding: self.transcoding.resolve()?,
        })
    }
}



#[derive(Clone)]
pub struct LibraryValidationConfiguration {
    /// A list of allowed audio extensions. Any not specified here are forbidden
    /// (flagged when running validation), see configuration template for more information.
    pub allowed_audio_file_extensions: Vec<String>,

    pub allowed_other_file_extensions: Vec<String>,

    pub allowed_other_files_by_name: Vec<String>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedLibraryValidationConfiguration {
    allowed_audio_file_extensions: Vec<String>,

    allowed_other_file_extensions: Vec<String>,

    allowed_other_files_by_name: Vec<String>,
}

impl ResolvableConfiguration for UnresolvedLibraryValidationConfiguration {
    type Resolved = LibraryValidationConfiguration;

    fn resolve(self) -> miette::Result<Self::Resolved> {
        let allowed_audio_file_extensions = self
            .allowed_audio_file_extensions
            .into_iter()
            .map(|extension| extension.to_ascii_lowercase())
            .collect();

        let allowed_other_file_extensions = self
            .allowed_other_file_extensions
            .into_iter()
            .map(|extension| extension.to_ascii_lowercase())
            .collect();


        Ok(LibraryValidationConfiguration {
            allowed_audio_file_extensions,
            allowed_other_file_extensions,
            allowed_other_files_by_name: self.allowed_other_files_by_name,
        })
    }
}



#[derive(Clone)]
pub struct LibraryTranscodingConfiguration {
    /// A list of audio file extensions (e.g. "mp3", "flac" - don't include ".").
    /// Files with these extensions are considered audio files and are transcoded using ffmpeg
    /// (see `tools.ffmpeg`).
    pub audio_file_extensions: Vec<String>,

    /// A list of other tracked file extensions (e.g. `jpg`, `png` - don't include ".").
    /// Files with these extensions are considered data files and are copied when transcoding.
    pub other_file_extensions: Vec<String>,

    /// Dynamically contains extensions from both `audio_file_extensions` and `other_file_extensions`.
    pub all_tracked_extensions: Vec<String>,
}

impl LibraryTranscodingConfiguration {
    /// Returns `Ok(true)` when the given file path's extension is considered an audio file.
    /// Returns `Err` if the extension is invalid UTF-8.
    pub fn is_path_audio_file_by_extension<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<bool> {
        let extension = get_path_extension_or_empty(file_path)?;

        Ok(self.audio_file_extensions.contains(&extension))
    }

    /// Returns `Ok(true)` when the given file path's extension is considered a data file.
    /// Returns `Err` if the extension is invalid UTF-8.
    pub fn is_path_data_file_by_extension<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<bool> {
        let extension = get_path_extension_or_empty(file_path)?;

        Ok(self.other_file_extensions.contains(&extension))
    }
}

#[derive(Deserialize, Clone)]
pub(crate) struct UnresolvedLibraryTranscodingConfiguration {
    audio_file_extensions: Vec<String>,
    other_file_extensions: Vec<String>,
}

impl ResolvableConfiguration for UnresolvedLibraryTranscodingConfiguration {
    type Resolved = LibraryTranscodingConfiguration;

    fn resolve(self) -> miette::Result<Self::Resolved> {
        let audio_file_extensions: Vec<String> = self
            .audio_file_extensions
            .into_iter()
            .map(|extention| extention.to_ascii_lowercase())
            .collect();

        let other_file_extensions: Vec<String> = self
            .other_file_extensions
            .into_iter()
            .map(|extention| extention.to_ascii_lowercase())
            .collect();

        let mut all_tracked_extensions = Vec::with_capacity(
            audio_file_extensions.len() + other_file_extensions.len(),
        );
        all_tracked_extensions.extend(audio_file_extensions.iter().cloned());
        all_tracked_extensions.extend(other_file_extensions.iter().cloned());


        Ok(LibraryTranscodingConfiguration {
            audio_file_extensions,
            other_file_extensions,
            all_tracked_extensions,
        })
    }
}
