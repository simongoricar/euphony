use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use crate::configuration::{AfterLoadInitable, AfterLoadWithEssentialsInitable, get_default_configuration_file_path, get_running_executable_directory};

/// This struct contains the entire `euphony` configuration,
/// from tool paths to libraries and so forth.
#[derive(Deserialize, Clone)]
pub struct Config {
    pub essentials: ConfigEssentials,
    
    pub tools: ConfigTools,
    
    pub validation: ConfigValidation,
    
    pub libraries: BTreeMap<String, ConfigLibrary>,
    
    pub aggregated_library: ConfigAggregated,
    
    pub file_metadata: ConfigFileMetadata,
    
    #[serde(skip)]
    pub configuration_file_path: PathBuf,
}

#[allow(dead_code)]
impl Config {
    pub fn load_from_path<S: Into<PathBuf>>(configuration_filepath: S) -> Config {
        let configuration_filepath = configuration_filepath.into();
        
        // Read the configuration file into memory.
        let configuration_string = fs::read_to_string(&configuration_filepath)
            .expect("Could not read configuration file!");
        
        // Parse the string into a structure.
        let mut config: Config = toml::from_str(&configuration_string)
            .expect("Could not load configuration file!");
        
        config.configuration_file_path = dunce::canonicalize(configuration_filepath)
            .expect("Could not canocalize configuration file path even though it has loaded!");
        
        // Run init methods for all sub-configurations.
        config.essentials.after_load_init();
        
        for (_, library) in &mut config.libraries {
            library.after_load_init(&config.essentials);
        }
        
        config.validation.after_load_init();
        config.aggregated_library.after_load_init(&config.essentials);
        config.file_metadata.after_load_init();
        config.tools.after_load_init(&config.essentials);
        
        config
    }
    
    pub fn load_default_path() -> Config {
        Config::load_from_path(get_default_configuration_file_path())
    }
    
    pub fn is_library<P: AsRef<Path>>(&self, library_path: P) -> bool {
        for (_, library) in &self.libraries {
            let current_path = Path::new(&library.path);
            if current_path.eq(library_path.as_ref()) {
                return true;
            }
        }
        
        false
    }
    
    pub fn get_library_name_from_path<P: AsRef<Path>>(&self, library_path: P) -> Option<String> {
        for (_, library) in &self.libraries {
            let current_path = Path::new(&library.path);
            if current_path.eq(library_path.as_ref()) {
                return Some(library.name.clone());
            }
        }
        
        None
    }
    
    pub fn get_library_by_full_name<S: AsRef<str>>(&self, library_name: S) -> Option<&ConfigLibrary> {
        for (_, library) in &self.libraries {
            if library.name.eq(library_name.as_ref()) {
                return Some(&library);
            }
        }
        
        None
    }
}

/// Basic configuration - reusable values such as the base library path and base tools path.
#[derive(Deserialize, Clone)]
pub struct ConfigEssentials {
    pub base_library_path: String,
    pub base_tools_path: String,
}

impl AfterLoadInitable for ConfigEssentials {
    fn after_load_init(&mut self) {
        // Replaces any placeholders and validates the paths.
        let executable_dir = get_running_executable_directory()
            .to_string_lossy()
            .to_string();
        
        self.base_library_path = self.base_library_path
            .replace("{SELF}", &executable_dir);
        self.base_tools_path = self.base_tools_path
            .replace("{SELF}", &executable_dir);
        
        self.base_library_path = dunce::canonicalize(&self.base_library_path)
            .expect(
                &format!(
                    "Could not canonicalize base_library_path \"{}\", make sure it exists.",
                    self.base_library_path,
                ),
            )
            .to_string_lossy()
            .to_string();
        
        self.base_tools_path = dunce::canonicalize(&self.base_tools_path)
            .expect(
                &format!(
                    "Could not canonicalize base_tools_path \"{}\", make sure it exists.",
                    self.base_tools_path,
                ),
            )
            .to_string_lossy()
            .to_string();
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigTools {
    pub ffmpeg: ConfigToolsFFMPEG,
}

impl AfterLoadWithEssentialsInitable for ConfigTools {
    fn after_load_init(&mut self, essentials: &ConfigEssentials) {
        self.ffmpeg
            .after_load_init(essentials);
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigToolsFFMPEG {
    pub binary: String,
    pub to_mp3_v0_args: Vec<String>,
}

impl AfterLoadWithEssentialsInitable for ConfigToolsFFMPEG {
    fn after_load_init(&mut self, essentials: &ConfigEssentials) {
        let ffmpeg = self.binary
            .replace("{TOOLS_BASE}", &essentials.base_tools_path);
        
        let canocalized_ffmpeg = dunce::canonicalize(ffmpeg.clone())
            .expect(
                &format!(
                    "Could not canocalize ffmpeg binary path: \"{}\", make sure the path is valid.",
                    ffmpeg,
                ),
            );
        
        self.binary = canocalized_ffmpeg
            .to_string_lossy()
            .to_string();
        
        if !canocalized_ffmpeg.is_file() {
            panic!("No file exists at this path: {}", self.binary);
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigValidation {
    pub allowed_other_files_by_extension: Vec<String>,
    pub allowed_other_files_by_name: Vec<String>,
}

impl AfterLoadInitable for ConfigValidation {
    fn after_load_init(&mut self) {
        for ext in &mut self.allowed_other_files_by_extension {
            ext.make_ascii_lowercase();
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigLibrary {
    /// Full name of the library.
    pub name: String,
    
    /// Absolute path to the library (can contain {LIBRARY_BASE} placeholder on load).
    pub path: String,
    
    /// A list of allowed audio extensions.
    /// Any not specified here are forbidden, see configuration template for more information.
    pub allowed_audio_files_by_extension: Vec<String>,
    
    /// A list of directories that should be ignored when scanning for artist directories.
    pub ignored_directories_in_base_dir: Option<Vec<String>>,
}

impl AfterLoadWithEssentialsInitable for ConfigLibrary {
    fn after_load_init(&mut self, essentials: &ConfigEssentials) {
        let parsed_path = self.path
            .replace("{LIBRARY_BASE}", &essentials.base_library_path);
        
        let canonicalized_path = dunce::canonicalize(parsed_path)
            .expect(
                &format!(
                    "Library \"{}\" could not be found at path \"{}\"!",
                    self.name,
                    self.path,
                ),
            );
        
        if !canonicalized_path.is_dir() {
            panic!(
                "Library \"{}\" has path set to \"{}\", but this path is not a directory!",
                self.name,
                self.path,
            );
        }
        
        self.path = canonicalized_path.to_string_lossy().to_string();
        
        // Make extensions lowercase.
        for ext in &mut self.allowed_audio_files_by_extension {
            ext.make_ascii_lowercase();
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigAggregated {
    pub path: String,
    
    pub transcode_threads: u16,
    
    pub max_processing_retries: u16,
    
    pub processing_retry_delay_seconds: u16,
}

impl AfterLoadWithEssentialsInitable for ConfigAggregated {
    fn after_load_init(&mut self, essentials: &ConfigEssentials) {
        self.path = self.path.replace("{LIBRARY_BASE}", &essentials.base_library_path);
        
        if self.transcode_threads == 0 {
            panic!("transcode_threads is set to 0! The minimum value is 1.");
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct ConfigFileMetadata {
    pub tracked_audio_extensions: Vec<String>,
    
    pub tracked_other_extensions: Vec<String>,
    
    #[serde(skip)]
    pub tracked_extensions: Vec<String>,
}

impl AfterLoadInitable for ConfigFileMetadata {
    fn after_load_init(&mut self) {
        self.tracked_extensions
            .extend(
                self.tracked_audio_extensions
                    .iter()
                    .map(|item| item.to_string())
            );
        
        self.tracked_extensions
            .extend(
                self.tracked_other_extensions
                    .iter()
                    .map(|item| item.to_string())
            );
    }
}

impl ConfigFileMetadata {
    pub fn matches_audio_extension(&self, extension: &String) -> bool {
        self.tracked_audio_extensions.contains(extension)
    }
    
    pub fn matches_data_extension(&self, extension: &String) -> bool {
        self.tracked_other_extensions.contains(extension)
    }
}


