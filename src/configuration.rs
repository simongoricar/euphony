use std::fs;
use std::path::{Path, PathBuf};
use std::collections::BTreeMap;
use std::env::args;
use serde::{Deserialize};
use crate::filesystem;


#[derive(Deserialize)]
pub struct Config {
    pub essentials: ConfigBasics,
    pub tools: ConfigTools,
    pub validation: ConfigValidation,
    pub libraries: BTreeMap<String, ConfigLibrary>,
    pub aggregated_library: ConfigAggregated,
    pub file_metadata: ConfigFileMetadata,

    #[serde(skip)]
    pub configuration_file_path: PathBuf,
}

#[derive(Deserialize)]
pub struct ConfigBasics {
    pub root_library_path: String,
}

#[derive(Deserialize)]
pub struct ConfigTools {
    pub ffmpeg: ConfigToolsFFMPEG,
}

#[derive(Deserialize)]
pub struct ConfigToolsFFMPEG {
    pub binary: String,
    pub to_mp3_v0_args: Vec<String>,
}

impl ConfigToolsFFMPEG {
    fn after_load_init(&mut self) {
        let mut ffmpeg_binary = get_running_executable_directory();
        ffmpeg_binary.push(&self.binary);

        let resolved_tool_dir = dunce::canonicalize(ffmpeg_binary)
            .expect(
                &format!(
                    "Could not canonicalize ffmpeg binary path! Executable dir: {:?}",
                    get_running_executable_directory(),
                ),
            );

        if !resolved_tool_dir.is_file() {
            panic!("ffmpeg binary does not exist at the location specified in the tools.ffmpeg.binary field!");
        }

        self.binary = resolved_tool_dir.to_string_lossy().to_string();
    }
}

#[derive(Deserialize)]
pub struct ConfigValidation {
    pub allowed_other_files_by_extension: Vec<String>,
    pub allowed_other_files_by_name: Vec<String>,
    // TODO Refactor.
    // pub audio_file_extensions: Vec<String>,
    // pub ignored_file_extensions: Vec<String>,
    // pub ignored_files: Vec<String>,
}

impl ConfigValidation {
    fn after_load_init(&mut self) {
        for ext in &mut self.allowed_other_files_by_extension {
            ext.make_ascii_lowercase();
        }
    }
}

#[derive(Deserialize)]
pub struct ConfigLibrary {
    /// Full name of the library.
    pub name: String,
    /// Absolute path to the library (can contain {ROOT} placeholder on load).
    pub path: String,
    /// A list of allowed audio extensions.
    /// Any not specified here are forbidden, see configuration template for more information.
    pub allowed_audio_files_by_extension: Vec<String>,

    // TODO Refactor.
    // pub audio_file_extensions: Vec<String>,
    // pub must_not_contain_extensions: Vec<String>,
}

impl ConfigLibrary {
    fn after_load_init(&mut self, root_library_path: &str) {
        self.path = self.path.replace("{ROOT}", root_library_path);

        // Ensure the path is valid.
        let true_path = Path::new(&self.path);
        if !true_path.exists() {
            panic!("Library \"{}\" does not exist (path: \"{}\")!", self.name, self.path);
        }

        // Make extensions lowercase.
        for ext in &mut self.allowed_audio_files_by_extension {
            ext.make_ascii_lowercase();
        }
    }
}

#[derive(Deserialize)]
pub struct ConfigAggregated {
    pub path: String,
}

impl ConfigAggregated {
    fn after_load_init(&mut self, root_library_path: &str) {
        self.path = self.path.replace("{ROOT}", root_library_path);
    }
}

#[derive(Deserialize)]
pub struct ConfigFileMetadata {
    pub tracked_audio_extensions: Vec<String>,
    pub tracked_other_extensions: Vec<String>,

    #[serde(skip)]
    pub tracked_extensions: Vec<String>,
}

impl ConfigFileMetadata {
    fn after_load_init(&mut self) {
        self.tracked_extensions.extend(
            self.tracked_audio_extensions
                .iter()
                .map(|item| item.to_string())
        );

        self.tracked_extensions.extend(
            self.tracked_other_extensions
                .iter()
                .map(|item| item.to_string())
        );
    }

    pub fn matches_audio_extension(&self, extension: &String) -> bool {
        self.tracked_audio_extensions.contains(extension)
    }

    pub fn matches_data_extension(&self, extension: &String) -> bool {
        self.tracked_other_extensions.contains(extension)
    }
}

/// Inspect the first command line argument to extract the directory the program resides in.
/// Automatically detects whether it is running inside a debug directory (target/debug) and escapes it.
pub fn get_running_executable_directory() -> PathBuf {
    let current_args = args().next().expect("Could not get first argument!");

    // might be "debug"
    let full_path_directory = dunce::canonicalize(Path::new(&current_args))
        .expect("Could not get running executable path!")
        .parent()
        .expect("Could not get running executable directory!")
        .to_path_buf();
    let full_path_directory_name = full_path_directory.file_name()
        .expect("Could not get running executable directory name!")
        .to_string_lossy();

    // Attempt to detect if we're in "debug/target" and the parent directory contains Cargo.toml".
    if full_path_directory_name.eq("debug") {
        // might be "target"
        let full_path_parent = full_path_directory.parent()
            .expect("Could not get running executable parent directory!");
        let full_path_parent_dir_name = full_path_parent.file_name()
            .expect("Could not get running executable parent directory name!")
            .to_string_lossy();

        if full_path_parent_dir_name.eq("target") {
            // might be the real base directory
            let full_path_grandparent = full_path_parent.parent()
                .expect("Could not get running executable grandparent directory!");

            // Check for Cargo.toml.
            return match filesystem::list_directory_contents(full_path_grandparent) {
                Ok((files, _)) => {
                    for file in files {
                        if file.file_name().to_string_lossy().eq("Cargo.toml") {
                            return full_path_grandparent.to_path_buf()
                        }
                    }

                    full_path_directory
                },
                Err(_) => {
                    full_path_directory
                }
            };
        }
    }

    full_path_directory
}

pub fn get_configuration_file_path() -> String {
    let mut configuration_filepath = get_running_executable_directory();
    configuration_filepath.push("./data/configuration.toml");

    if !configuration_filepath.exists() {
        panic!("Could not find configuration.toml in data directory.");
    }

    let configuration_filepath = dunce::canonicalize(configuration_filepath)
        .expect("Could not canonicalize configuration.toml file path!");

    String::from(
        configuration_filepath.to_str()
            .expect("Could not convert configuration file path to string!")
    )
}

impl Config {
    pub fn load_from_path(configuration_filepath: String) -> Config {
        let configuration_filepath = PathBuf::from(&configuration_filepath);

        // Read the configuration file into memory.
        let configuration_string = fs::read_to_string(&configuration_filepath)
            .expect("Could not read configuration file!");

        // Parse the string into a structure.
        let mut config: Config = toml::from_str(&configuration_string)
            .expect("Could not load configuration file!");

        let root_library_path = &config.essentials.root_library_path;

        config.configuration_file_path = dunce::canonicalize(configuration_filepath)
            .expect("Could not canocalize configuration file path even though it has loaded!");

        for (_, library) in &mut config.libraries {
            library.after_load_init(root_library_path);
        }

        config.validation.after_load_init();
        config.aggregated_library.after_load_init(root_library_path);
        config.file_metadata.after_load_init();
        config.tools.ffmpeg.after_load_init();

        config
    }

    pub fn load() -> Config {
        Config::load_from_path(get_configuration_file_path())
    }

    pub fn is_library(&self, library_path: &Path) -> bool {
        for (_, library) in &self.libraries {
            let current_path = Path::new(&library.path);
            if current_path.eq(library_path) {
                return true;
            }
        }

        false
    }

    pub fn get_library_name_from_path(&self, library_path: &Path) -> Option<String> {
        for (library_name, library) in &self.libraries {
            let current_path = Path::new(&library.path);
            if current_path.eq(library_path) {
                return Some(library_name.clone());
            }
        }

        None
    }
}
