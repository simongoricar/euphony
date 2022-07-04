use std::fs;
use std::path::Path;
use std::collections::HashMap;
use serde::{Deserialize};


#[derive(Deserialize)]
pub struct Config {
    pub basics: ConfigBasics,
    pub tools: ConfigTools,
    pub validation: ConfigValidation,
    pub libraries: HashMap<String, ConfigLibrary>,
    pub aggregated_library: ConfigAggregated,
    pub file_metadata: ConfigFileMetadata,
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

#[derive(Deserialize)]
pub struct ConfigValidation {
    pub audio_file_extensions: Vec<String>,
    pub ignored_file_extensions: Vec<String>,
}

impl ConfigValidation {
    fn after_load_init(&mut self) {
        for ext in &mut self.audio_file_extensions {
            ext.make_ascii_lowercase();
        }

        for ext in &mut self.ignored_file_extensions {
            ext.make_ascii_lowercase();
        }
    }
}

#[derive(Deserialize)]
pub struct ConfigLibrary {
    pub name: String,
    pub path: String,

    pub audio_file_extensions: Vec<String>,
    pub must_not_contain_extensions: Vec<String>,
}

impl ConfigLibrary {
    fn after_load_init(&mut self, root_library_path: &str) {
        self.path = self.path.replace(
            "{ROOT}",
            root_library_path
        );

        // Ensure the path is valid
        let true_path = Path::new(&self.path);
        if !true_path.exists() {
            panic!("Library \"{}\" does not exist (path: \"{}\")!", self.name, self.path);
        }

        // Make extensions lowercase
        for ext in self.audio_file_extensions.iter_mut() {
            ext.make_ascii_lowercase();
        }

        for ext in self.must_not_contain_extensions.iter_mut() {
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
        self.path = self.path.replace(
            "{ROOT}",
            root_library_path,
        );
    }
}

#[derive(Deserialize)]
pub struct ConfigFileMetadata {
    pub tracked_audio_extensions: Vec<String>,
    pub tracked_data_extensions: Vec<String>,

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
            self.tracked_data_extensions
                .iter()
                .map(|item| item.to_string())
        );
    }

    pub fn matches_audio_extension(&self, extension: &String) -> bool {
        self.tracked_audio_extensions.contains(extension)
    }

    pub fn matches_data_extension(&self, extension: &String) -> bool {
        self.tracked_data_extensions.contains(extension)
    }
}


pub fn get_configuration_file_path() -> String {
    let configuration_filepath = Path::new("./data/configuration.toml");
    if !configuration_filepath.exists() {
        panic!("Could not find configuration.toml in data directory.");
    }

    String::from(
        configuration_filepath.to_str()
            .expect("Could not convert configuration file path to string!")
    )
}

impl Config {
    pub fn load_from_path(configuration_filepath: String) -> Config {
        // Read the configuration file into memory.
        let configuration_string = fs::read_to_string(configuration_filepath)
            .expect("Could not read configuration file!");

        // Parse the string into a structure.
        let mut config: Config = toml::from_str(&configuration_string)
            .expect("Could not load configuration file!");

        let root_library_path = &config.basics.root_library_path;

        for (_, library) in &mut config.libraries {
            library.after_load_init(root_library_path);
        }

        config.validation.after_load_init();
        config.aggregated_library.after_load_init(root_library_path);
        config.file_metadata.after_load_init();

        config
    }

    pub fn load() -> Config {
        let configuration_filepath = get_configuration_file_path();
        Config::load_from_path(configuration_filepath)
    }
}
