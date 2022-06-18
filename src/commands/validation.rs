use std::io::Error;
use std::path::Path;

use owo_colors::OwoColorize;

use super::super::filesystem as mfs;
use super::super::configuration::{Config, ConfigLibrary};

enum Validation {
    Valid,
    Invalid(Vec<String>)
}


/// Validate each individual music library (e.g. one for lossless, one for private, etc.)
/// Validation is done in multiple steps:
///  1. each library is checked for unusual or forbidden files (see configuration file),
///  2. validate that there are no collisions between any of the libraries.
pub fn cmd_validate(config: &Config) {
    println!("{}", "About to validate all configured libraries.".yellow());
    println!();

    // 1. check for forbidden or unusual files.
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
    let (files, directories) = mfs::list_directory_contents(Path::new(base_path))?;

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
        let extension = mfs::get_dir_entry_file_extension(file)?;

        if config.validation.audio_file_extensions.contains(&extension) {
            invalid_list.push(format!("Unexpected audio file in base directory: {:?}", file_path))
        }
    }

    for artist_dir in &directories {
        let (artist_files, artist_directories) = mfs::list_dir_entry_contents(artist_dir)?;

        // There should not be any audio files in the artist directory.
        for artist_dir_file in &artist_files {
            let file_path = artist_dir_file.path();
            let extension = mfs::get_dir_entry_file_extension(artist_dir_file)?;

            if config.validation.audio_file_extensions.contains(&extension) {
                invalid_list.push(format!("Unexpected audio file in artist directory: {:?}", file_path));
            }
        }

        for album_dir in &artist_directories {
            let (album_files, _) = mfs::list_dir_entry_contents(album_dir)?;

            // This directory can contain audio files and any files specified in the ignored_file_extensions config value.
            for album_dir_file in album_files {
                let file_path = album_dir_file.path();
                let extension = mfs::get_dir_entry_file_extension(&album_dir_file)?;

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
