use std::env::args;
use std::path::{Path, PathBuf};

use crate::filesystem;

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
    let full_path_directory_name = full_path_directory
        .file_name()
        .expect("Could not get running executable directory name!")
        .to_string_lossy();

    // Attempt to detect if we're in "debug/target" and the parent directory contains Cargo.toml".
    if full_path_directory_name.eq("debug") {
        // might be "target"
        let full_path_parent = full_path_directory
            .parent()
            .expect("Could not get running executable parent directory!");
        let full_path_parent_dir_name = full_path_parent
            .file_name()
            .expect("Could not get running executable parent directory name!")
            .to_string_lossy();

        if full_path_parent_dir_name.eq("target") {
            // might be the real base directory
            let full_path_grandparent = full_path_parent.parent().expect(
                "Could not get running executable grandparent directory!",
            );

            // Check for Cargo.toml.
            return match filesystem::list_directory_contents(
                full_path_grandparent,
            ) {
                Ok((files, _)) => {
                    for file in files {
                        if file.file_name().to_string_lossy().eq("Cargo.toml") {
                            return full_path_grandparent.to_path_buf();
                        }
                    }

                    full_path_directory
                }
                Err(_) => full_path_directory,
            };
        }
    }

    full_path_directory
}

pub fn get_default_configuration_file_path() -> String {
    let mut configuration_filepath = get_running_executable_directory();
    configuration_filepath.push("./data/configuration.toml");

    if !configuration_filepath.exists() {
        panic!("Could not find configuration.toml in data directory.");
    }

    let configuration_filepath = dunce::canonicalize(configuration_filepath)
        .expect("Could not canonicalize configuration.toml file path!");

    configuration_filepath.to_string_lossy().to_string()
}
