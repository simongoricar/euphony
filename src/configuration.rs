use std::borrow::{Borrow, BorrowMut};
use std::fs;
use std::path::Path;
use std::collections::HashMap;
use serde::{Deserialize};


#[derive(Deserialize, Debug)]
pub struct Config {
    pub basics: ConfigBasics,
    pub validation: ConfigValidation,
    pub libraries: HashMap<String, ConfigLibrary>,
}

#[derive(Deserialize, Debug)]
pub struct ConfigValidation {
    pub audio_file_extensions: Vec<String>,
    pub ignored_file_extensions: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct ConfigBasics {
    pub root_library_path: String,
}

#[derive(Deserialize, Debug)]
pub struct ConfigLibrary {
    pub name: String,
    pub path: String,

    pub audio_file_extensions: Vec<String>,
    pub must_not_contain_extensions: Vec<String>,
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

        // Parse extensions to be lowercase
        for ext in config.validation.audio_file_extensions.iter_mut() {
            ext.make_ascii_lowercase();
        }

        for ext in config.validation.ignored_file_extensions.iter_mut() {
            ext.make_ascii_lowercase();
        }

        // Parse paths inside the configuration before returning.
        for (_, mut library) in config.libraries.borrow_mut() {
            library.path = library.path.replace(
                "{ROOT}",
                &config.basics.root_library_path
            );

            // Ensure the path is valid
            let true_path = Path::new(&library.path);
            if !true_path.exists() {
                panic!("Library \"{}\" does not exist (path: \"{}\")!", library.name, library.path);
            }

            // Make extensions lowercase
            for ext in library.audio_file_extensions.iter_mut() {
                ext.make_ascii_lowercase();
            }

            for ext in library.must_not_contain_extensions.iter_mut() {
                ext.make_ascii_lowercase();
            }
        }

        config
    }

    pub fn load() -> Config {
        let configuration_filepath = get_configuration_file_path();
        Config::load_from_path(configuration_filepath)
    }

    pub fn print_libraries(&self) {
        println!("Available libraries:");
        for (_, library) in self.libraries.borrow() {
            println!("  {}: {}", library.name, library.path);
        }
        println!();
    }
}
