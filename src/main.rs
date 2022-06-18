mod configuration;
mod filesystem;

use std::fs;
use std::env;
use std::io::Error;
use std::path::{Path, PathBuf};
use std::process::exit;

use clap::Parser;
use owo_colors::OwoColorize;

use configuration::{Config, ConfigLibrary};
use crate::filesystem::{get_dir_entry_file_extension, list_dir_entry_contents, list_directory_contents};


#[derive(Parser, Debug)]
struct CLIArgs {
    command: String,
}


// CMD: CONVERSION

fn cmd_convert(directory: &PathBuf, _config: &Config) {
    if !directory.is_dir() {
        println!("Current directory is invalid.");
        exit(1);
    }

    let dir_read = fs::read_dir(directory)
        .expect("Could not list files in current directory!");

    for file in dir_read {
        let _file = file.expect("Could not list file!");
    }

    // TODO
}


// CMD: VALIDATION
enum Validation {
    Valid,
    Invalid(Vec<String>)
}

fn cmd_validate(config: &Config) {
    println!("{}", "About to validate all configured libraries.".yellow());
    println!();

    for (_, library) in &config.libraries {
        println!("-- Validating: {} --", library.name);
        let validation = match validate_library(config, &library) {
            Ok(validation) => validation,
            Err(err) => {
                println!("Could not validate library because of an error: {}", err);
                continue
            }
        };

        match validation {
            Validation::Valid => {
                println!("Library is valid!");
            },
            Validation::Invalid(errors) => {
                println!("Library is not completely valid. Here are the errors:");
                for (index, err) in errors.iter().enumerate() {
                    println!("  {}. {}", index + 1, err);
                }
            }
        }

        println!();
    }

    println!();
    println!("All libraries processed.");
}

fn validate_library(config: &Config, library: &ConfigLibrary) -> Result<Validation, Error> {
    let mut invalid_list: Vec<String> = Vec::new();

    let audio_file_extensions = &library.audio_file_extensions;
    let must_not_contain_ext = &library.must_not_contain_extensions;

    let base_path = &library.path;
    let (files, directories) = list_directory_contents(Path::new(base_path))?;

    // Library structure should be:
    //  <library directory>
    //  |-- <artist>
    //  |   |-- <album>
    //  |   |   |-- <... audio files>
    //  |   |   |-- <... cover art>
    //  |   |   |-- <... possibly some album-related README, etc.>
    //  |   |   |-- <... possibly other directory that don't matter>
    //  |   |-- <... possibly some artist-related README, etc.>
    //  | ...
    //  |--

    // There should not be any audio files in the base directory.
    for file in &files {
        let file_path = file.path();
        let extension = get_dir_entry_file_extension(file)?;

        if config.validation.audio_file_extensions.contains(&extension) {
            invalid_list.push(format!("Unexpected audio file in base directory: {:?}", file_path))
        }
    }

    for artist_dir in &directories {
        let (artist_files, artist_directories) = list_dir_entry_contents(artist_dir)?;

        // There should not be any audio files in the artist directory.
        for artist_dir_file in &artist_files {
            let file_path = artist_dir_file.path();
            let extension = get_dir_entry_file_extension(artist_dir_file)?;

            if config.validation.audio_file_extensions.contains(&extension) {
                invalid_list.push(format!("Unexpected audio file in artist directory: {:?}", file_path));
            }
        }

        for album_dir in &artist_directories {
            let (album_files, _) = list_dir_entry_contents(album_dir)?;

            // This directory can contain audio files and any files specified in the ignored_file_extensions config value.
            for album_dir_file in album_files {
                let file_path = album_dir_file.path();
                let extension = get_dir_entry_file_extension(&album_dir_file)?;

                let is_ok_audio_file = audio_file_extensions.contains(&extension);
                let is_ignored = config.validation.ignored_file_extensions.contains(&extension);
                let is_specifically_forbidden = must_not_contain_ext.contains(&extension);

                if is_specifically_forbidden {
                    invalid_list.push(format!("File with forbidden extension {}: {:?}", extension, file_path));
                } else if is_ok_audio_file || is_ignored {
                    continue
                } else {
                    invalid_list.push(format!("Unexpected file: {:?}", file_path));
                }
            }
        }
    }

    // TODO Validate that there are no collisions between libraries (for conversion into main MP3 V0 MusicLibrary folder)

    if invalid_list.len() == 0 {
        Ok(Validation::Valid)
    } else {
        Ok(Validation::Invalid(invalid_list))
    }
}


// MAIN

fn main() {
    let config = Config::load();
    let args: CLIArgs = CLIArgs::parse();
    let current_directory = env::current_dir()
        .expect("Could not get current directory!");

    if args.command.eq("convert") {
        cmd_convert(&current_directory, &config);
    } else if args.command.eq("validate") {
        cmd_validate(&config);
    }
}
