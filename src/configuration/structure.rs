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

/// This struct contains the entire `euphony` configuration,
/// from tool paths to libraries and so forth.
#[derive(Deserialize, Clone)]
pub struct Config {
    pub essentials: ConfigEssentials,

    pub validation: ConfigValidation,

    pub tools: ConfigTools,

    pub libraries: BTreeMap<String, ConfigLibrary>,

    pub aggregated_library: ConfigAggregated,

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
            .expect("Could not canocalize configuration file path even though it has loaded!");

        // Run init methods for all configuration subtables.

        config.essentials.after_load_init()?;
        config.validation.after_load_init()?;

        for library in config.libraries.values_mut() {
            library.after_load_init(&config.essentials)?;
        }

        config
            .aggregated_library
            .after_load_init(&config.essentials)?;
        config.tools.after_load_init(&config.essentials)?;

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
    ) -> Option<&ConfigLibrary> {
        self.libraries
            .values()
            .find(|library| library.name.eq(library_name.as_ref()))
    }
}

/// Basic configuration - reusable values such as the base library path and base tools path.
#[derive(Deserialize, Clone)]
pub struct ConfigEssentials {
    pub base_library_path: String,
    pub base_tools_path: String,
}

impl AfterLoadInitable for ConfigEssentials {
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

#[derive(Deserialize, Clone)]
pub struct ConfigValidation {
    pub extensions_considered_audio_files: Vec<String>,
}

impl AfterLoadInitable for ConfigValidation {
    fn after_load_init(&mut self) -> Result<()> {
        for ext in &mut self.extensions_considered_audio_files {
            ext.make_ascii_lowercase();
        }

        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigTools {
    pub ffmpeg: ConfigToolsFFMPEG,
}

impl AfterLoadWithEssentialsInitable for ConfigTools {
    fn after_load_init(&mut self, essentials: &ConfigEssentials) -> Result<()> {
        self.ffmpeg.after_load_init(essentials)?;

        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigToolsFFMPEG {
    pub binary: String,
    pub to_mp3_v0_args: Vec<String>,
}

impl AfterLoadWithEssentialsInitable for ConfigToolsFFMPEG {
    fn after_load_init(&mut self, essentials: &ConfigEssentials) -> Result<()> {
        let ffmpeg = self
            .binary
            .replace("{TOOLS_BASE}", &essentials.base_tools_path);

        let canocalized_ffmpeg = dunce::canonicalize(ffmpeg.clone())
            .unwrap_or_else(|_| panic!(
                "Could not canocalize ffmpeg binary path: \"{ffmpeg}\", make sure the path is valid.",
            ));

        self.binary = canocalized_ffmpeg.to_string_lossy().to_string();

        if !canocalized_ffmpeg.is_file() {
            panic!("No file exists at this path: {}", self.binary);
        }

        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigLibrary {
    /// Library display name.
    pub name: String,

    /// Absolute path to the library (can include {LIBRARY_BASE},
    /// which will be dynamically replaced with `essentials.base_library_path` on load).
    pub path: String,

    pub ignored_directories_in_base_directory: Option<Vec<String>>,

    /// Validation-related configuration for this library.
    pub validation: ConfigLibraryValidation,

    /// Transcoding-related configuration for this library.
    pub transcoding: ConfigLibraryTranscoding,
}

impl AfterLoadWithEssentialsInitable for ConfigLibrary {
    fn after_load_init(&mut self, essentials: &ConfigEssentials) -> Result<()> {
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
pub struct ConfigLibraryValidation {
    /// A list of allowed audio extensions. Any not specified here are forbidden
    /// (flagged when running validation), see configuration template for more information.
    pub allowed_audio_file_extensions: Vec<String>,

    pub allowed_other_file_extensions: Vec<String>,

    pub allowed_other_files_by_name: Vec<String>,
}

impl AfterLoadInitable for ConfigLibraryValidation {
    fn after_load_init(&mut self) -> Result<()> {
        // Make extensions lowercase.
        for ext in &mut self.allowed_audio_file_extensions {
            ext.make_ascii_lowercase();
        }

        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigLibraryTranscoding {
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

impl AfterLoadInitable for ConfigLibraryTranscoding {
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
pub struct ConfigAggregated {
    pub path: String,

    pub transcode_threads: usize,

    pub failure_max_retries: u16,

    pub failure_delay_seconds: u16,
}

impl AfterLoadWithEssentialsInitable for ConfigAggregated {
    fn after_load_init(&mut self, essentials: &ConfigEssentials) -> Result<()> {
        self.path = self
            .path
            .replace("{LIBRARY_BASE}", &essentials.base_library_path);

        if self.transcode_threads == 0 {
            panic!("transcode_threads is set to 0! The minimum value is 1.");
        }

        Ok(())
    }
}
