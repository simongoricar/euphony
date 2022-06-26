mod collision_checker;

use std::io::{Error, ErrorKind};
use std::path::Path;

use owo_colors::OwoColorize;

use super::super::configuration::{Config, ConfigLibrary};
use super::super::filesystem as mfs;
use super::super::console;

use collision_checker::CollisionChecker;


// Validation
enum Validation {
    Valid,
    Invalid(Vec<String>)
}


/// Validate each individual music library (e.g. one for lossless, one for private, etc.)
/// Validation is done in multiple steps:
///  1. each library is checked for unusual or forbidden files (see configuration file),
///  2. validate that there are no collisions between any of the libraries.
pub fn cmd_validate(config: &Config) -> bool {
    console::horizontal_line(None, None);
    console::horizontal_line_with_text(
        &format!(
            "{} {}{}{}{}",
            "LIBRARY VALIDATION".bright_cyan(),
            "(".bright_black(),
            "1/2".bold().cyan(),
            ": file types".cyan().italic(),
            ")".bright_black(),
        ),
        None, None, None
    );
    console::horizontal_line(None, None);
    console::new_line();

    let mut collision_checker = CollisionChecker::new();

    let mut step_1_errors: bool = false;
    let mut step_2_errors: bool = false;

    // 1. check for forbidden or unusual files.
    for (_, library) in &config.libraries {
        console::horizontal_line_with_text(
            &format!(
                "{}{} {}",
                "🧾 Library".bright_yellow(),
                ":".bright_black(),
                library.name.yellow(),
            ),
            None, None, None
        );

        let validation = match validate_library(config, &library, &mut collision_checker) {
            Ok(validation) => validation,
            Err(err) => {
                println!("{}", format!("❌ Could not validate library because of an error: {}", err).red().bold());
                continue
            }
        };

        match validation {
            Validation::Valid => {
                println!("{}", "☑ Library files are valid!".green().bold());
            },
            Validation::Invalid(errors) => {
                step_1_errors = true;

                println!("{}", "❌ Library files are not completely valid. Here are the errors:".red().bold());
                for (index, err) in errors.iter().enumerate() {
                    println!("  {}. {}", index + 1, err);
                }
            }
        }

        console::new_line();
    }

    // 2. check if there were any errors during collision checks.
    console::new_line();
    console::horizontal_line(None, None);
    console::horizontal_line_with_text(
        &format!(
            "{} {}{}{}{}",
            "LIBRARY VALIDATION".bright_cyan(),
            "(".bright_black(),
            "2/2".bold().cyan(),
            ": album collisions".cyan().italic(),
            ")".bright_black(),
        ),
        None, None, None
    );

    println!();
    if collision_checker.collisions.len() == 0 {
        println!("{}", "☑ No collisions found!".green().bold())
    } else {
        step_2_errors = true;

        println!("{}", "❌ Found some collisions: ".red().bold());
        for (index, collision) in collision_checker.collisions.iter().enumerate() {
            let collision_title = format!(
                "{} - {}",
                collision.artist, collision.album
            );
            let collision_title = collision_title.bold();
            let collision_title = collision_title.bright_blue();
            let collision_title = collision_title.underline();

            let collision_description = format!(
                "{} {} {} {}",
                "Libraries:".bright_white(),
                collision.already_exists_in.yellow().bold(),
                "and".bright_white(),
                collision.collision_with.yellow().bold()
            );

            println!("  {}. {}", index + 1, collision_title);
            println!("  {}  {}", " ".repeat((index + 1).to_string().len()), collision_description);
        }
    }

    console::new_line();
    console::horizontal_line(None, None);

    if step_1_errors || step_2_errors {
        console::horizontal_line_with_text(
            &format!(
                "{}",
                "All libraries processed, SOME ERRORS!".bright_red()
            ),
            None,None, None
        );
    } else {
        console::horizontal_line_with_text(
            &format!(
                "{}",
                "All libraries processed, NO ERRORS.".green(),
            ),
            None, None, None,
        );
    }

    console::horizontal_line(None, None);
    step_1_errors || step_2_errors
}

fn validate_library(
    config: &Config, library: &ConfigLibrary, collision_checker: &mut CollisionChecker
) -> Result<Validation, Error> {
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
            let file_name = match file_path.file_name() {
                Some(name) => name,
                None => {
                    return Err(Error::new(ErrorKind::Other, "Could not get file name."))
                }
            };
            invalid_list.push(
                format!(
                    "Unexpected audio file in base directory: {}",
                    file_name.to_str().expect("Could not get string from file name.")
                )
            )
        }
    }

    for artist_dir in &directories {
        let artist_name = mfs::get_dir_entry_name(artist_dir).unwrap();
        let (artist_files, artist_directories) = mfs::list_dir_entry_contents(artist_dir)?;

        // There should not be any audio files in the artist directory.
        for artist_dir_file in &artist_files {
            let extension = mfs::get_dir_entry_file_extension(artist_dir_file)?;

            if config.validation.audio_file_extensions.contains(&extension) {
                let unexpected_file_name_owned = artist_dir_file.file_name();
                let unexpected_file_name = unexpected_file_name_owned
                    .to_str()
                    .expect("Could not extract file name.");

                invalid_list.push(
                    format!(
                        "Unexpected audio file in artist base directory: {} (in {})",
                        unexpected_file_name,
                        artist_name,
                    )
                );
            }
        }

        for album_dir in &artist_directories {
            let album_name = mfs::get_dir_entry_name(album_dir).unwrap();
            let (album_files, _) = mfs::list_dir_entry_contents(album_dir)?;

            collision_checker.add_album(&artist_name, &album_name, &library.name);

            // This directory can contain audio files and any files specified in the ignored_file_extensions config value.
            for track_file in album_files {
                let file_name_owned = track_file.file_name();
                let file_name = file_name_owned
                    .to_str()
                    .expect("Could not extract file name.");

                let extension = mfs::get_dir_entry_file_extension(&track_file)?;

                let is_ok_audio_file = audio_file_extensions.contains(&extension);
                let is_ignored = config.validation.ignored_file_extensions.contains(&extension);
                let is_specifically_forbidden = must_not_contain_ext.contains(&extension);

                if is_specifically_forbidden {

                    invalid_list.push(
                        format!(
                            "Forbidden extension ({}) in album directory: {} (in {} - {})",
                            extension,
                            file_name,
                            artist_name,
                            album_name,
                        )
                    );
                } else if is_ok_audio_file || is_ignored {
                    continue
                } else {
                    invalid_list.push(
                        format!(
                            "Unexpected file in album directory: {} (in {} - {})",
                            file_name,
                            artist_name,
                            album_name,
                        )
                    );
                }
            }
        }
    }

    if invalid_list.len() == 0 {
        Ok(Validation::Valid)
    } else {
        Ok(Validation::Invalid(invalid_list))
    }
}
