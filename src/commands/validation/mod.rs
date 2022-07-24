use std::fs::DirEntry;
use std::io::{Error, ErrorKind};
use std::path::Path;

use console::Color::Color256;
use console::style;

use crate::commands::validation::collisions::CollisionAudit;

use super::super::configuration::{Config, ConfigLibrary};
use super::super::console as c;
use super::super::filesystem as mfs;

mod collisions;

enum LibraryValidationResult {
    Valid,
    Invalid {
        invalid_file_messages: Vec<String>,
    }
}

fn validate_library(
    config: &Config, library: &ConfigLibrary,
    collision_auditor: &mut CollisionAudit,
) -> Result<LibraryValidationResult, Error> {
    let other_allowed_extensions = &config.validation.allowed_other_files_by_extension;
    let other_allowed_filenames = &config.validation.allowed_other_files_by_name;
    let audio_allowed_extension = &library.allowed_audio_files_by_extension;

    let mut invalid_file_messages: Vec<String> = Vec::new();

    let library_root_path = Path::new(&library.path);
    let (library_root_files, library_root_dirs) = mfs::list_directory_contents(library_root_path)?;

    // As explained in the README and configuration template, library structure is expected to be:
    // <library directory>
    // |-- <artist directory>
    // |   |-- <album directory>
    // |   |   |-- <... audio files (whichever types you allow inside each library's configuration)>
    // |   |   |-- <... optionally, cover art>
    // |   |   |-- <... optionally, some album-related README, logs, etc.>
    // |   |   |-- <... optionally, other directories that don't really matter for this purpose (they are ignored)>
    // |   |   |   [the second two are examples, euphony will allow whatever you set in the validation configuration]
    // |   |-- <... possibly some artist-related README, etc. (whatever you allow in the validation configuration table)>
    // | [other artist directories ...]
    // | [other files (again, whichever types/names you allow in the validation configuration) ...]

    // This closure will attempt to match the given list of file entries with
    // allowed_other_files_by_extension and allowed_other_files_by_name.
    let check_nonalbum_files_for_unexpected = |entry_list: &Vec<DirEntry>, location_hint: &str| -> Result<Vec<String>, Error> {
        let mut own_unexpected_file_messages: Vec<String> = Vec::new();

        for file_entry in entry_list {
            let file_type = file_entry.file_type()?;
            if !file_type.is_file() {
                return Err(Error::new(ErrorKind::Other, "Not a file!"));
            }

            let file_path = file_entry.path();
            let file_name = file_path.file_name()
                .ok_or(Error::new(ErrorKind::Other, "File has no name!"))?
                .to_string_lossy()
                .to_string();

            if other_allowed_filenames.contains(&file_name) {
                // This file happens to match by full name, so it is okay.
                continue;
            }

            match file_path.extension() {
                Some(ext) => {
                    let ext= ext.to_string_lossy().to_string();
                    if !other_allowed_extensions.contains(&ext) {
                        // Unexpected (invalid) file.
                        own_unexpected_file_messages.push(
                            format!(
                                "Unexpected file: {} (in {})",
                                file_name,
                                location_hint,
                            ),
                        );
                    }
                },
                None => {
                    // Hasn't matched by full name and can't match by extension: show as invalid.
                    own_unexpected_file_messages.push(
                        format!(
                            "Unexpected file: {} (in {})",
                            file_name,
                            location_hint,
                        ),
                    );
                }
            }
        }

        Ok(own_unexpected_file_messages)
    };

    // TODO Improve error messages to show better context.

    // Check for inconsistencies in the root directory.
    let unexpected_root_files = check_nonalbum_files_for_unexpected(&library_root_files, "root directory")?;
    invalid_file_messages.extend(unexpected_root_files);

    // Traverse artist directories and albums inside each one.
    for artist_directory in &library_root_dirs {
        let artist_name = artist_directory.file_name().to_string_lossy().to_string();

        // Make sure to ignore (skip) any directories matching ignored_directories_in_base_dir.
        if let Some(ignores) = &library.ignored_directories_in_base_dir {
            if ignores.contains(&artist_name) {
                continue;
            }
        }

        let (artist_dir_files, artist_dir_dirs) = mfs::list_dir_entry_contents(artist_directory)?;

        // There shouldn't be any unexpected files in the artist directory.
        let unexpected_artist_files = check_nonalbum_files_for_unexpected(&artist_dir_files, &artist_name)?;
        invalid_file_messages.extend(unexpected_artist_files);

        for artist_album_dir in &artist_dir_dirs {
            let album_name = artist_album_dir.file_name().to_string_lossy().to_string();

            collision_auditor.add_album(&artist_name, &album_name, &library.name);

            let (album_files, _) = mfs::list_dir_entry_contents(artist_album_dir)?;

            // This directory can contain both audio and "other" files.
            for album_file in album_files {
                let album_file_filename = album_file.file_name().to_string_lossy().to_string();

                if other_allowed_filenames.contains(&album_file_filename) {
                    // This file happens to match by full name, so it is okay.
                    continue;
                }

                match album_file.path().extension() {
                    Some(ext) => {
                        let ext= ext.to_string_lossy().to_string();
                        if !other_allowed_extensions.contains(&ext) && !audio_allowed_extension.contains(&ext) {
                            // Unexpected (invalid) file.
                            invalid_file_messages.push(
                                format!(
                                    "Unexpected file in album directory: {} (in {} - {})",
                                    album_file_filename,
                                    artist_name,
                                    album_name,
                                ),
                            );
                        }
                    },
                    None => {
                        // Hasn't matched by full name and can't match by extension: show as invalid.
                        invalid_file_messages.push(
                            format!(
                                "Unexpected file in album directory: {} (in {} - {})",
                                album_file_filename,
                                artist_name,
                                album_name,
                            ),
                        );
                    }
                }
            }
        }
    }

    if invalid_file_messages.len() == 0 {
        Ok(LibraryValidationResult::Valid)
    } else {
        Ok(LibraryValidationResult::Invalid {
            invalid_file_messages
        })
    }
}

pub fn cmd_validate_all(config: &Config) -> bool {
    c::horizontal_line_with_text(
        format!(
            "{} {}{}{}{}",
            style("Validation").fg(Color256(152)),
            style("(").fg(Color256(7)),
            style("1/2").bold().cyan(),
            style(": file types").cyan().italic(),
            style(")").fg(Color256(7)),
        ),
        None, None,
    );

    let mut collision_auditor = CollisionAudit::new();

    let mut has_unexpected_files = false;
    let mut has_collisions = false;

    // 1/2: Check for unexpected files.
    for (_, library) in &config.libraries {
        c::new_line();
        c::centered_print(
            format!(
                "{}{} {}",
                style("🧾 Library").fg(Color256(11)),
                style(":").fg(Color256(7)),
                style(&library.name).yellow(),
            ),
            None,
        );

        let validation = match validate_library(config, library, &mut collision_auditor) {
            Ok(validation) => validation,
            Err(error) => {
                eprintln!(
                    "{} Errored while validating: {}",
                    style("❗")
                        .red()
                        .bold(),
                    style(error)
                        .red()
                );
                continue;
            }
        };

        match validation {
            LibraryValidationResult::Valid => {
                println!(
                    "{} Library valid!",
                    style("☑")
                        .green()
                        .bold(),
                );
            },
            LibraryValidationResult::Invalid { invalid_file_messages } => {
                has_unexpected_files = true;

                println!(
                    "{} Invalid entries:",
                    style("❌")
                        .red()
                        .bold(),
                );
                for (index, err) in invalid_file_messages.iter().enumerate() {
                    println!(
                        "  {}. {}",
                        index + 1,
                        err,
                    );
                }
            }
        }

        c::new_line();
    }

    c::new_line();
    c::horizontal_line_with_text(
        format!(
            "{} {}{}{}{}",
            style("Validation").fg(Color256(152)),
            style("(").fg(Color256(7)),
            style("2/2").bold().cyan(),
            style(": album collisions").cyan().italic(),
            style(")").fg(Color256(7)),
        ),
        None, None,
    );
    c::new_line();

    if collision_auditor.has_collisions() {
        has_collisions = true;

        println!(
            "{} Found some collisions!",
            style("❌")
                .red()
                .bold()
        );

        // TODO
        for (index, collision) in collision_auditor.collisions.iter().enumerate() {
            let collision_title = format!(
                "{} - {}",
                collision.artist_name,
                collision.album_title,
            );
            let collision_title_styled = style(collision_title)
                .fg(Color256(117))
                .bold()
                .underlined();

            let collision_description = format!(
                "{} {} {} {}",
                style("Libraries:")
                    .bright(),
                style(&collision.colliding_libraries_by_name.0)
                    .yellow()
                    .bold(),
                style("and")
                    .bright(),
                style(&collision.colliding_libraries_by_name.1)
                    .yellow()
                    .bold()
            );

            let digit_length = ((index + 1) as f32).log10().floor() as usize;

            println!(
                "  {}. {}",
                index + 1,
                collision_title_styled,
            );
            println!(
                "  {}  {}",
                " ".repeat(digit_length),
                collision_description,
            );
        }

    } else {
        println!(
            "{} No collisions.",
            style("☑")
                .green()
                .bold()
        );
    }

    c::new_line();

    if has_unexpected_files || has_collisions {
        c::horizontal_line_with_text(
            format!(
                "{}",
                style("All libraries processed, BUT WITH SOME ERRORS!")
                    .red()
                    .bright()
            ),
            None, None,
        );
    } else {
        c::horizontal_line_with_text(
            format!(
                "{}",
                style("All libraries processed, NO ERRORS.")
                    .green()
                    .bright(),
            ),
            None, None,
        );
    }

    has_unexpected_files || has_collisions
}

pub fn cmd_validate_library<S: AsRef<str>>(config: &Config, library_name: S) -> bool {
    let library = match config.get_library_by_full_name(library_name.as_ref()) {
        Some(library) => library,
        None => {
            eprintln!(
                "{} {}",
                style("No such library:")
                    .red(),
                style(library_name.as_ref())
                    .bold(),
            );
            return false;
        }
    };


    c::horizontal_line_with_text(
        format!(
            "{}",
            style("Library validation").fg(Color256(152)),
        ),
        None, None,
    );

    c::new_line();
    c::centered_print(
        format!(
            "{}{} {}",
            style("🧾 Selected library").fg(Color256(11)),
            style(":").fg(Color256(7)),
            style(&library.name).yellow(),
        ),
        None,
    );

    let mut unused_collision_auditor = CollisionAudit::new();
    let library_validation = match validate_library(config, library, &mut unused_collision_auditor) {
        Ok(validation) => validation,
        Err(error) => {
            eprintln!(
                "{} Errored while validating: {}",
                style("❗")
                    .red()
                    .bold(),
                style(error)
                    .red()
            );
            return false;
        }
    };

    match library_validation {
        LibraryValidationResult::Valid => {
            println!(
                "{} Library valid!",
                style("☑")
                    .green()
                    .bold(),
            );

            c::new_line();
            c::horizontal_line_with_text(
                format!(
                    "{}",
                    style("Library validated, NO ERRORS.")
                        .green()
                        .bright(),
                ),
                None, None,
            );

            true
        },
        LibraryValidationResult::Invalid { invalid_file_messages } => {
            println!(
                "{} Invalid entries:",
                style("❌")
                    .red()
                    .bold(),
            );
            for (index, err) in invalid_file_messages.iter().enumerate() {
                println!(
                    "  {}. {}",
                    index + 1,
                    err,
                );
            }

            c::new_line();
            c::horizontal_line_with_text(
                format!(
                    "{}",
                    style("Library could not be validated, THERE WERE SOME ERRORS!")
                        .red()
                        .bright()
                ),
                None, None,
            );

            false
        }
    }
}
