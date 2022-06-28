use std::io::Error;
use std::path::{Path, PathBuf};
use std::process::exit;

use owo_colors::OwoColorize;

use directories as dirs;
use file_operations as fo;
use meta::LibraryMeta;

use crate::{console, filesystem};
use crate::configuration::Config;

mod meta;
mod directories;
mod file_operations;

// TODO
// TODO Make the directory optionally be a static one, meaning the
//      entire library gets scanned (maybe a --all switch?)
pub fn cmd_transcode_album(album_directory: &PathBuf, config: &Config) -> Result<(), Error> {
    if !album_directory.is_dir() {
        println!("Current directory is invalid.");
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
        "Using directory: ".italic(),
        album_directory
            .as_path()
            .to_str()
            .expect("Could not parse directory path into string!")
            .bold()
            .yellow()
    );

    // Verify the directory is an album.
    if !dirs::directory_is_album(config, album_directory) {
        println!(
            "{}",
            "Directory is not an album directory, exiting.".red()
        );
        exit(1);
    }


    let existing_meta = LibraryMeta::load(album_directory);

    if let Some(album_dir_meta_saved) = existing_meta {
        // Can compare with current state. Update changed files and resave the current state.

        // TODO Comparisons.
        let album_dir_meta_fresh = LibraryMeta::generate(
            album_directory,
            None, &config.file_metadata.tracked_extensions,
        )?;

        let changes = album_dir_meta_saved.diff(&album_dir_meta_fresh);

        // TODO Actual aggregation.

        // TODO Resave fresh metadata.

        Ok(())
    } else {
        // Can't compare, meaning we should do the aggregation and save the current state into the file.
        console::new_line();
        println!(
            "{}",
            "No .librarymeta contents, assuming no transcode yet.".yellow(),
        );

        let album_dir_meta = LibraryMeta::generate(
            album_directory,
            None,
            &config.file_metadata.tracked_extensions,
        )?;

        // Perform actual transcodes (audio files) and copies (album art, etc.).
        // For an album audio file to be converted, it must be:
        // - directly in the album directory,
        // - one of the tracked file extensions (file_metadata.tracked_file_extensions).
        println!(
            "{}",
            "Transcoding audio files and copying others:".bright_black()
        );

        let mut file_queue: Vec<String> = album_dir_meta.files
            .keys()
            .map(|item| item.to_string())
            .collect();
        file_queue.sort_unstable();

        for file_name in file_queue {
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
        }

        console::horizontal_line_with_text(
            &format!(
                "{}",
                "Album transcode/copy finished"
            ),
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
