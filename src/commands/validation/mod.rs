use std::fs::DirEntry;
use std::path::Path;

use crossterm::style::Stylize;
use miette::{Context, IntoDiagnostic, miette, Result};

use crate::commands::validation::collisions::CollisionAudit;
use crate::console::{LogBackend, TerminalBackend};

use super::super::configuration::{Config, ConfigLibrary};
use super::super::filesystem as mfs;

mod collisions;

enum LibraryValidationResult {
    Valid,
    Invalid {
        invalid_file_messages: Vec<String>,
    }
}

fn validate_library(
    config: &Config,
    library: &ConfigLibrary,
    collision_auditor: &mut CollisionAudit,
) -> Result<LibraryValidationResult> {
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
    let check_nonalbum_files_for_unexpected = |entry_list: &Vec<DirEntry>, location_hint: &str| -> Result<Vec<String>> {
        let mut own_unexpected_file_messages: Vec<String> = Vec::new();

        for file_entry in entry_list {
            let file_type = file_entry.file_type()
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not get file type."))?;
            
            if !file_type.is_file() {
                return Err(miette!("Not a file!"));
            }

            let file_path = file_entry.path();
            let file_name = file_path.file_name()
                .ok_or_else(|| miette!("File has no name!"))?
                .to_str()
                .ok_or_else(|| miette!("Could not convert file name to string: invalid utf-8."))?
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

    if invalid_file_messages.is_empty() {
        Ok(LibraryValidationResult::Valid)
    } else {
        Ok(LibraryValidationResult::Invalid {
            invalid_file_messages
        })
    }
}

pub fn cmd_validate_all<T: TerminalBackend + LogBackend>(
    config: &Config,
    terminal: &mut T,
) -> bool {
    terminal.log_println("Validating all libraries.");
    terminal.log_println("-- Step 1: file types --");

    let mut collision_auditor = CollisionAudit::new();

    let mut has_unexpected_files = false;
    let mut has_collisions = false;

    // 1/2: Check for unexpected files.
    for library in config.libraries.values() {
        terminal.log_newline();
        terminal.log_println(format!(
            "Library: {}", library.name,
        ));

        let validation = match validate_library(config, library, &mut collision_auditor) {
            Ok(validation) => validation,
            Err(error) => {
                terminal.log_println(format!(
                    "❗ {} {}",
                    "Errored while validating: ".red(),
                    error,
                ));
                continue;
            }
        };

        match validation {
            LibraryValidationResult::Valid => {
                terminal.log_println("☑ Library valid!".green().bold());
            },
            LibraryValidationResult::Invalid { invalid_file_messages } => {
                has_unexpected_files = true;
                
                terminal.log_println("❌ Invalid entries!".red().bold());
                for (index, err) in invalid_file_messages.iter().enumerate() {
                    terminal.log_println(format!(
                        "  {}. {}", index + 1, err,
                    ));
                }
            }
        }
        
        terminal.log_newline();
    }

    terminal.log_newline();
    terminal.log_println("-- Step 2: album collisions between libraries --");
    terminal.log_newline();

    if collision_auditor.has_collisions() {
        has_collisions = true;
        
        terminal.log_println("❌ Found collisions!".red().bold());

        for collision in collision_auditor.collisions {
            terminal.log_println(format!(
                "Libraries: {} and {}: {} {}.",
                collision.colliding_libraries_by_name.0,
                collision.colliding_libraries_by_name.1,
                format!("{} - {}", collision.artist_name, collision.album_title)
                    .yellow()
                    .bold()
                    .underlined(),
                "collides".red().bold(),
            ));
        }

    } else {
        terminal.log_println("☑ No collisions.".green().bold());
    }

    terminal.log_newline();
    has_unexpected_files || has_collisions
}

pub fn cmd_validate_library<S: AsRef<str>, T: TerminalBackend + LogBackend>(
    config: &Config,
    library_name: S,
    terminal: &mut T,
) -> bool {
    let library = match config.get_library_by_full_name(library_name.as_ref()) {
        Some(library) => library,
        None => {
            terminal.log_println(format!(
                "{} {}",
                "No such library:".red(),
                library_name.as_ref().bold()
            ));
            return false;
        }
    };
    
    terminal.log_println(format!(
        "Validating library: {}.",
        library.name.clone().yellow(),
    ));
    terminal.log_newline();

    let mut unused_collision_auditor = CollisionAudit::new();
    let library_validation = match validate_library(config, library, &mut unused_collision_auditor) {
        Ok(validation) => validation,
        Err(error) => {
            terminal.log_println(format!(
                "{} {}",
                "❗ Errored while validating:".red().bold(),
                error.to_string().red(),
            ));
            
            return false;
        }
    };

    match library_validation {
        LibraryValidationResult::Valid => {
            terminal.log_println("☑ Library valid.".green().bold());
            terminal.log_newline();

            true
        },
        LibraryValidationResult::Invalid { invalid_file_messages } => {
            terminal.log_println("❌ Invalid entries!".red().bold());
            for (index, err) in invalid_file_messages.iter().enumerate() {
                terminal.log_println(format!(
                    "  {}. {}", index + 1, err,
                ));
            }
            terminal.log_newline();
            
            false
        }
    }
}
