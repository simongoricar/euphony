use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use miette::{miette, Context, Result};
use serde::Deserialize;

use crate::configuration::{
    get_default_configuration_file_path,
    get_running_executable_directory,
    AfterLoadInitable,
    AfterLoadWithEssentialsInitable,
};
use crate::filesystem::get_path_extension_or_empty;

/// This struct contains the entire `euphony` configuration,
/// from tool paths to libraries and so forth.
#[derive(Deserialize, Clone)]
pub struct Config {
    pub paths: ConfigPaths,

    pub logging: LoggingConfig,

    pub validation: ValidationConfig,

    pub tools: ToolsConfig,

    pub libraries: BTreeMap<String, LibraryConfig>,

    // TODO Should I rename "aggregated library" to something else, like "transcoded library"?
    pub aggregated_library: AggregatedLibraryConfig,

    #[serde(skip)]
    pub configuration_file_path: PathBuf,
}

#[allow(dead_code)]
impl Config {
    pub fn load_from_path<S: Into<PathBuf>>(
        configuration_filepath: S,
    ) -> Result<Config> {
        let configuration_filepath = configuration_filepath.into();

        // Read the configuration file into memory.
        let configuration_string = fs::read_to_string(&configuration_filepath)
            .expect("Could not read configuration file!");

        // Parse the string into the `Config` structure.
        let mut config: Config = toml::from_str(&configuration_string)
            .expect("Could not load configuration file!");

        config.configuration_file_path = dunce::canonicalize(configuration_filepath)
            .expect("Could not canonicalize configuration file path even though it has loaded!");

        // Run init methods for all configuration sub-tables.

        config.paths.after_load_init()?;
        config.validation.after_load_init()?;

        for library in config.libraries.values_mut() {
            library.after_load_init(&config.paths)?;
        }

        config.aggregated_library.after_load_init(&config.paths)?;
        config.tools.after_load_init(&config.paths)?;

        Ok(config)
    }

    pub fn load_default_path() -> Result<Config> {
        Config::load_from_path(
            get_default_configuration_file_path().wrap_err_with(|| {
                miette!("Could not get default configuration file path.")
            })?,
        )
    }

    pub fn is_library<P: AsRef<Path>>(&self, library_path: P) -> bool {
        for library in self.libraries.values() {
            let current_path = Path::new(&library.path);
            if current_path.eq(library_path.as_ref()) {
                return true;
            }
        }

        false
    }

    pub fn get_library_name_from_path<P: AsRef<Path>>(
        &self,
        library_path: P,
    ) -> Option<String> {
        for library in self.libraries.values() {
            let current_path = Path::new(&library.path);
            if current_path.eq(library_path.as_ref()) {
                return Some(library.name.clone());
            }
        }

        None
    }

    pub fn get_library_by_full_name<S: AsRef<str>>(
        &self,
        library_name: S,
    ) -> Option<&LibraryConfig> {
        self.libraries
            .values()
            .find(|library| library.name.eq(library_name.as_ref()))
    }
}

/// Base paths - reusable values such as the base library path and base tools path.
#[derive(Deserialize, Clone)]
pub struct ConfigPaths {
    pub base_library_path: String,
    pub base_tools_path: String,
}

impl AfterLoadInitable for ConfigPaths {
    fn after_load_init(&mut self) -> Result<()> {
        // Replaces any placeholders and validates the paths.
        let executable_directory = get_running_executable_directory()?
            .to_string_lossy()
            .to_string();

        self.base_library_path = self
            .base_library_path
            .replace("{SELF}", &executable_directory);
        self.base_tools_path = self
            .base_tools_path
            .replace("{SELF}", &executable_directory);

        self.base_library_path = dunce::canonicalize(&self.base_library_path)
            .unwrap_or_else(|_| panic!(
                "Could not canonicalize base_library_path \"{}\", make sure it exists.",
                self.base_library_path,
            ))
            .to_string_lossy()
            .to_string();

        self.base_tools_path = dunce::canonicalize(&self.base_tools_path)
            .unwrap_or_else(|_| panic!(
                "Could not canonicalize base_tools_path \"{}\", make sure it exists.",
                self.base_tools_path,
            ))
            .to_string_lossy()
            .to_string();

        Ok(())
    }
}


// TODO Integrate (with --log-to-file override)
#[derive(Deserialize, Clone)]
pub struct LoggingConfig {
    pub default_log_output_path: Option<PathBuf>,
}


#[derive(Deserialize, Clone)]
pub struct ValidationConfig {
    pub extensions_considered_audio_files: Vec<String>,
}

impl AfterLoadInitable for ValidationConfig {
    fn after_load_init(&mut self) -> Result<()> {
        for ext in &mut self.extensions_considered_audio_files {
            ext.make_ascii_lowercase();
        }

        Ok(())
    }
}


#[derive(Deserialize, Clone)]
pub struct ToolsConfig {
    pub ffmpeg: FFMPEGToolsConfig,
}

impl AfterLoadWithEssentialsInitable for ToolsConfig {
    fn after_load_init(&mut self, essentials: &ConfigPaths) -> Result<()> {
        self.ffmpeg.after_load_init(essentials)?;

        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct FFMPEGToolsConfig {
    /// Configures the ffmpeg binary location.
    /// The {TOOLS_BASE} placeholder is available (see `base_tools_path` in the `essentials` table)
    pub binary: String,

    /// These are the arguments passed to ffmpeg when converting an audio file into MP3 V0.
    /// The placeholders {INPUT_FILE} and {OUTPUT_FILE} will be replaced with the absolute path to those files.
    pub audio_transcoding_args: Vec<String>,

    /// This setting should be the extension of the audio files after transcoding.
    /// The default conversion is to MP3, but the user may set any ffmpeg conversion above, which is why this exists.
    pub audio_transcoding_output_extension: String,
}

impl FFMPEGToolsConfig {
    /// Returns `Ok(true)` if the given path's extension matches the ffmpeg transcoding
    /// output path.
    /// Returns `Err` if the extension is not valid UTF-8.
    pub fn is_path_transcoding_output_by_extension<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<bool> {
        let extension = get_path_extension_or_empty(file_path)?;

        Ok(self.audio_transcoding_output_extension.eq(&extension))
    }
}

impl AfterLoadWithEssentialsInitable for FFMPEGToolsConfig {
    fn after_load_init(&mut self, essentials: &ConfigPaths) -> Result<()> {
        let ffmpeg = self
            .binary
            .replace("{TOOLS_BASE}", &essentials.base_tools_path);

        let canonicalized_ffmpeg = dunce::canonicalize(ffmpeg.clone())
            .unwrap_or_else(|_| panic!(
                "Could not canonicalize ffmpeg binary path: \"{ffmpeg}\", make sure the path is valid.",
            ));

        self.binary = canonicalized_ffmpeg.to_string_lossy().to_string();

        if !canonicalized_ffmpeg.is_file() {
            panic!("No file exists at this path: {}", self.binary);
        }

        self.audio_transcoding_output_extension
            .make_ascii_lowercase();

        Ok(())
    }
}


#[derive(Deserialize, Clone)]
pub struct LibraryConfig {
    /// Library display name.
    pub name: String,

    /// Absolute path to the library (can include {LIBRARY_BASE},
    /// which will be dynamically replaced with `essentials.base_library_path` on load).
    pub path: String,

    pub ignored_directories_in_base_directory: Option<Vec<String>>,

    /// Validation-related configuration for this library.
    pub validation: LibraryValidationConfig,

    /// Transcoding-related configuration for this library.
    pub transcoding: LibraryTranscodingConfig,
}

impl AfterLoadWithEssentialsInitable for LibraryConfig {
    fn after_load_init(&mut self, essentials: &ConfigPaths) -> Result<()> {
        let parsed_path = self
            .path
            .replace("{LIBRARY_BASE}", &essentials.base_library_path);

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

        self.path = canonicalized_path.to_string_lossy().to_string();

        self.validation.after_load_init()?;
        self.transcoding.after_load_init()?;

        Ok(())
    }
}


#[derive(Deserialize, Clone)]
pub struct LibraryValidationConfig {
    /// A list of allowed audio extensions. Any not specified here are forbidden
    /// (flagged when running validation), see configuration template for more information.
    pub allowed_audio_file_extensions: Vec<String>,

    pub allowed_other_file_extensions: Vec<String>,

    pub allowed_other_files_by_name: Vec<String>,
}

impl AfterLoadInitable for LibraryValidationConfig {
    fn after_load_init(&mut self) -> Result<()> {
        // Make extensions lowercase.
        for ext in &mut self.allowed_audio_file_extensions {
            ext.make_ascii_lowercase();
        }

        Ok(())
    }
}


#[derive(Deserialize, Clone)]
pub struct LibraryTranscodingConfig {
    /// A list of audio file extensions (e.g. "mp3", "flac" - don't include ".").
    /// Files with these extensions are considered audio files and are transcoded using ffmpeg
    /// (see `tools.ffmpeg`).
    pub audio_file_extensions: Vec<String>,

    /// A list of other tracked file extensions (e.g. `jpg`, `png` - don't include ".").
    /// Files with these extensions are considered data files and are copied when transcoding.
    pub other_file_extensions: Vec<String>,

    /// Dynamically contains extensions from both `audio_file_extensions` and `other_file_extensions`.
    #[serde(skip)]
    pub all_tracked_extensions: Vec<String>,
}

impl LibraryTranscodingConfig {
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

impl AfterLoadInitable for LibraryTranscodingConfig {
    fn after_load_init(&mut self) -> Result<()> {
        // Make extensions lowercase.
        for ext in &mut self.audio_file_extensions {
            ext.make_ascii_lowercase();
        }

        for ext in &mut self.other_file_extensions {
            ext.make_ascii_lowercase();
        }

        self.all_tracked_extensions
            .extend(self.audio_file_extensions.iter().cloned());
        self.all_tracked_extensions
            .extend(self.other_file_extensions.iter().cloned());

        Ok(())
    }
}


#[derive(Deserialize, Clone)]
pub struct AggregatedLibraryConfig {
    pub path: String,

    pub transcode_threads: usize,

    pub failure_max_retries: u16,

    pub failure_delay_seconds: u16,
}

impl AfterLoadWithEssentialsInitable for AggregatedLibraryConfig {
    fn after_load_init(&mut self, essentials: &ConfigPaths) -> Result<()> {
        self.path = self
            .path
            .replace("{LIBRARY_BASE}", &essentials.base_library_path);

        if self.transcode_threads == 0 {
            panic!("transcode_threads is set to 0! The minimum value is 1.");
        }

        Ok(())
    }
}
