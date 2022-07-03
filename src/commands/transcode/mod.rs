use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::thread;

use owo_colors::OwoColorize;
use pbr::{MultiBar, ProgressBar};

use directories as dirs;
use file_operations as fo;
use meta::LibraryMeta;

use crate::{console, filesystem};
use crate::commands::transcode::file_operations::remove_target_file;
use crate::configuration::Config;

mod meta;
mod directories;
mod file_operations;

fn process_file(
    file_name: &Path,
    config: &Config,
    album_directory: &PathBuf,
) -> Result<(), Error> {
    let mut file_path = album_directory.clone();
    file_path.push(file_name);

    let file_extension = filesystem::get_path_file_extension(&file_path)?;

    if filesystem::is_file_in_directory(&file_path, album_directory)
        && config.file_metadata.matches_any_extension(&file_extension) {
        // Detect whether this is an audio or some other file.
        // If audio file, transcode, otherwise copy.

        if config.file_metadata.matches_audio_extension(&file_extension) {
            println!(
                "{}{}",
                "Transcoding ".bright_white(),
                file_path
                    .file_name()
                    .expect("Could not get audio file name.")
                    .to_str()
                    .expect("Could not convert audio file name to string!")
                    .bright_blue()
            );

            // Run transcode into MP3 V0.
            fo::transcode_audio_file_into_mp3_v0(&file_path, config)?;

        } else if config.file_metadata.matches_data_extension(&file_extension) {
            println!(
                "{}{}",
                "Copying ".bright_white(),
                file_path
                    .file_name()
                    .expect("Could not get data file name.")
                    .to_str()
                    .expect("Could not convert data file name to string!")
                    .bright_blue()
            );

            // Run copy operation.
            fo::copy_data_file(&file_path, config)?;

        } else {
            panic!("Bug: extension matches \"any\", but does not match audio nor data.");
        }
    }

    return Ok(());
}

fn remove_processed_file(
    file_name: &Path,
    config: &Config,
    album_directory: &PathBuf,
) -> Result<(), Error> {
    let mut full_source_file_path = album_directory.clone();
    full_source_file_path.push(file_name);

    let source_file_extension = filesystem::get_path_file_extension(&full_source_file_path)?;

    if config.file_metadata.matches_audio_extension(&source_file_extension) {
        println!(
            "{}{}",
            "Removing target transcode of ".bright_white(),
            file_name
                .to_str()
                .expect("Could not convert audio file name to string!")
                .bright_blue()
        );

        remove_target_file(&full_source_file_path, config, true)

    } else if config.file_metadata.matches_data_extension(&source_file_extension) {
        println!(
            "{}{}",
            "Removing target data file copy of ".bright_white(),
            file_name
                .to_str()
                .expect("Could not convert audio file name to string!")
                .bright_blue()
        );

        remove_target_file(&full_source_file_path, config, false)

    } else {
        return Err(
            Error::new(
                ErrorKind::Other,
                "Source file is not tracked, but was requested to be removed.",
            )
        );
    }

}

pub fn cmd_transcode_library(library_directory: &PathBuf, config: &Config) -> Result<(), Error> {
    if !library_directory.is_dir() {
        println!("Directory is invalid.");
        exit(1);
    }

    console::horizontal_line(None, None);
    console::horizontal_line_with_text(
        &format!(
            "{}",
            "Library aggregation"
                .cyan()
                .bold(),
        ),
        None, None, None,
    );
    console::horizontal_line(None, None);
    console::new_line();

    println!(
        "{} {}",
        "Using library directory: ".italic(),
        library_directory
            .as_path()
            .to_str()
            .expect("Could not parse directory into string!")
            .bold()
            .yellow()
    );

    if !dirs::directory_is_library(config, library_directory) {
        println!(
            "{}", "Directory is not a library, exiting.".red(),
        );
        exit(1);
    }

    // Enumerate artist directories and traverse each one.
    // Then traverse each album inside those directories.
    // If any invalid folder is found, an error is shown.

    let (_, artist_directories) = match filesystem::list_directory_contents(library_directory) {
        Ok(data) => data,
        Err(error) => {
            eprintln!(
                "{}{}",
                "Error while listing library artists (via directory scan): ".red(),
                error,
            );
            exit(1);
        }
    };

    let pbr = MultiBar::new();

    let mut album_pbr = pbr.create_bar(0);
    let mut artist_pbr = pbr.create_bar(artist_directories.len() as u64);

    // TODO Continue from here, untested thread listener.
    let _ = thread::spawn(move || {
        pbr.listen();
    });

    for artist_directory in artist_directories {
        let (_, album_directories) = match filesystem::list_dir_entry_contents(&artist_directory) {
            Ok(data) => data,
            Err(error) => {
                eprintln!(
                    "{}{}",
                    "Error while listing artist albums (via directory scan): ".red(),
                    error,
                );
                exit(1);
            }
        };

        album_pbr.total = album_directories.len() as u64;
        album_pbr.set(0);

        for album_directory in album_directories {
            transcode_album(
                &album_directory.path(),
                config,
            )?;

            album_pbr.inc();
        }

        album_pbr.finish();
        artist_pbr.inc();
    }

    artist_pbr.finish_println("Library transcoded.");

    Ok(())
}

fn transcode_album(album_directory: &PathBuf, config: &Config) -> Result<(), Error> {
    let existing_meta = LibraryMeta::load(album_directory);

    if let Some(album_dir_meta_saved) = existing_meta {
        // Can compare with current state. Update changed files and resave the current state.
        console::new_line();
        println!(
            "{}", "Found existing .librarymeta, computing changes.".yellow(),
        );

        // TODO Check if target file exists (always do the transcode if it doesn't).

        let album_dir_meta_fresh = LibraryMeta::generate(
            album_directory,
            None,
            &config.file_metadata.tracked_extensions,
        )?;

        let changes = album_dir_meta_saved.diff(&album_dir_meta_fresh);

        // Here's what happens below
        //  - added files are transcoded/copied
        //  - removed files have their transcoded/copied target files removed
        //  - changed files are retranscoded/recopied again (overwriting any previous file)
        println!(
            "{}{}{}",
            "Files ",
            "added:"
                .green(),
            changes.files_new.len(),
        );
        println!(
            "{}{}",
            "      changed:"
                .yellow(),
            changes.files_changed.len(),
        );
        println!(
            "{}{}",
            "      removed:"
                .red(),
            changes.files_removed.len(),
        );
        console::new_line();

        for new_file_name in changes.files_new {
            process_file(Path::new(&new_file_name), config, album_directory)?;
        }

        for removed_file_name in changes.files_removed {
            remove_processed_file(Path::new(&removed_file_name), config, album_directory)?;
        }

        for changed_file_name in changes.files_changed {
            process_file(Path::new(&changed_file_name), config, album_directory)?;
        }

        // Resave fresh metadata.
        match album_dir_meta_fresh.save(album_directory, true) {
            Ok(_) => {
                println!(
                    "{}",
                    "Fresh .librarymeta file saved.".green(),
                );
            },
            Err(error) => {
                eprintln!(
                    "{}{}",
                    "Error while saving fresh .librarymeta file: ".red(),
                    error
                );
            }
        };

        Ok(())

    } else {
        // Can't compare, meaning we should do the aggregation and save the current state into the file.
        console::new_line();
        println!(
            "{}", "No .librarymeta file, assuming no transcode yet (will overwrite if any).".yellow(),
        );

        let album_dir_meta = LibraryMeta::generate(
            album_directory,
            None,
            &config.file_metadata.tracked_extensions,
        )?;

        // Perform actual transcodes (for audio files) and copies (for album art, etc.).
        // For a file to be "tracked" (and subsequently trasconded/copied), it must be:
        // - directly in the album directory,
        // - one of the tracked file extensions (file_metadata.tracked_file_extensions).
        println!(
            "{}", "Transcoding and copying album.".bright_black()
        );
        console::new_line();

        let mut file_queue: Vec<String> = album_dir_meta.files
            .keys()
            .map(|item| item.to_string())
            .collect();
        file_queue.sort_unstable();

        // TODO Progress bar.

        // TODO Paralellism.

        for file_name in file_queue {
            process_file(Path::new(&file_name), config, album_directory)?;
        }

        console::horizontal_line_with_text(
            "Album transcoding and copying finished.",
            None, None, None,
        );


        println!(
            "{}{}{} {}",
            "Saving fresh "
                .yellow(),
            ".librarymeta"
                .bold()
                .green(),
            " for album: "
                .yellow(),
            album_directory
                .file_name()
                .expect("Could not get directory name.")
                .to_str()
                .expect("Could not get directory name string.")
                .bright_blue()
                .italic()
        );

        match album_dir_meta.save(album_directory, false) {
            Ok(_) => {
                println!(
                    "{}",
                    ".librarymeta file saved."
                        .green()
                        .bold()
                );
            },
            Err(error) => {
                eprintln!(
                    "{} {}",
                    "Error while saving the .librarymeta file:"
                        .red()
                        .bold(),
                    error,
                );
            }
        };

        Ok(())
    }
}

// TODO
// TODO Make the directory optionally be a static one, meaning the
//      entire library gets scanned (maybe a --all switch?)
pub fn cmd_transcode_album(album_directory: &PathBuf, config: &Config) -> Result<(), Error> {
    if !album_directory.is_dir() {
        println!("Directory is invalid.");
        exit(1);
    }

    console::horizontal_line(None, None);
    console::horizontal_line_with_text(
        &format!(
            "{}",
            "Album aggregation"
                .cyan()
                .bold(),
        ),
        None, None, None,
    );
    console::horizontal_line(None, None);
    console::new_line();

    println!(
        "{} {}",
        "Using album directory: ".italic(),
        album_directory
            .as_path()
            .to_str()
            .expect("Could not parse directory into string!")
            .bold()
            .yellow()
    );

    // Verify the directory is an album.
    if !dirs::directory_is_album(config, album_directory) {
        println!(
            "{}", "Directory is not an album directory, exiting.".red()
        );
        exit(1);
    }

    transcode_album(album_directory, config)
}
