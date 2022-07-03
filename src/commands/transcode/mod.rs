use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::thread;

use owo_colors::OwoColorize;
use pbr::MultiBar;

use directories as dirs;
use file_operations as fo;
use meta::LibraryMeta;

use crate::{console, filesystem};
use crate::commands::transcode::directories::DirectoryInfo;
use crate::configuration::Config;

mod meta;
mod directories;
mod file_operations;

/// Represents the tiniest unit of work we generate when processing a library or an album.
struct LibraryAlbumPacket {
    source_library_path: String,
    artist_name: String,
    album_title: String,
}

impl LibraryAlbumPacket {
    /// Instantiate a new `LibraryAlbumPacket`.
    fn new(source_library_path: &str, artist_name: &str, album_title: &str) -> Result<LibraryAlbumPacket, Error> {
        let album_packet = LibraryAlbumPacket {
            source_library_path: source_library_path.to_string(),
            artist_name: artist_name.to_string(),
            album_title: album_title.to_string(),
        };

        let full_path = album_packet.get_album_source_path();
        if !full_path.is_dir() {
            return Err(
                Error::new(
                    ErrorKind::NotFound,
                    format!(
                        "No such album directory: {}",
                        full_path.to_str()
                            .expect("path to convert to string")
                    )
                )
            );
        }

        Ok(album_packet)
    }

    fn get_album_source_path(&self) -> PathBuf {
        let mut full_path = PathBuf::from(&self.source_library_path);
        full_path.push(&self.artist_name);
        full_path.push(&self.album_title);

        full_path
    }

    fn needs_processing(&self, config: &Config) -> bool {
        let full_source_path = self.get_album_source_path();

        match LibraryMeta::load(&full_source_path) {
            Some(saved_meta) => {
                let fresh_meta = LibraryMeta::generate(
                    &full_source_path,
                    None,
                    &config.file_metadata.tracked_extensions
                )
                    .expect(
                        &format!(
                                "Unable to generate LibraryMeta for album {}",
                                self.album_title
                            )
                    );

                let diff = saved_meta.diff(&fresh_meta);

                diff.has_any_changes()
            },
            None => true,
        }
    }

    /// Process the entire album (if needed).
    /// TODO Paralellism options?
    fn process_album(&self, config: &Config) -> Result<(), Error> {
        if !self.needs_processing(config) {
            return Ok(());
        }

        let full_album_path = self.get_album_source_path();
        let saved_meta = LibraryMeta::load(&full_album_path);

        let fresh_meta = LibraryMeta::generate(
            &full_album_path,
            None,
            &config.file_metadata.tracked_extensions,
        )?;

        if let Some(saved_meta) = saved_meta {
            // Can compare with current state. Update changed files and resave the current state.
            let changes = saved_meta.diff(&fresh_meta);

            for new_file_name in changes.files_new {
                let mut full_new_file_path = full_album_path.clone();
                full_new_file_path.push(new_file_name);

                process_album_file(&full_new_file_path, self, config)?;
            }

            for removed_file_name in changes.files_removed {
                let mut full_removed_file_path = full_album_path.clone();
                full_removed_file_path.push(removed_file_name);

                remove_processed_album_file(&full_removed_file_path, config)?;
            }

            for changed_file_name in changes.files_changed {
                let mut full_changed_file_path = full_album_path.clone();
                full_changed_file_path.push(changed_file_name);

                remove_processed_album_file(&full_changed_file_path, config)?;
                process_album_file(&full_changed_file_path, self, config)?;
            }

            Ok(())

        } else {
            // Can't compare, meaning we should do the aggregation and save the current state into the file.
            let mut pending_file_queue: Vec<String> = fresh_meta.files
                .keys()
                .map(|item| item.to_string())
                .collect();
            pending_file_queue.sort_unstable();

            for pending_file in pending_file_queue {
                let mut full_pending_file_path = full_album_path.clone();
                full_pending_file_path.push(pending_file);

                process_album_file(&full_pending_file_path, self, config)?;
            }

            Ok(())
        }
    }

    fn save_librarymeta(&self, config: &Config, overwrite_if_exists: bool) -> Result<(), Error> {
        let full_album_path = self.get_album_source_path();
        let fresh_meta = LibraryMeta::generate(
            &full_album_path,
            None,
            &config.file_metadata.tracked_extensions,
        )?;

        fresh_meta.save(&full_album_path, overwrite_if_exists)?;
        Ok(())
    }
}

fn process_album_file(
    source_file_path: &Path,
    album_packet: &LibraryAlbumPacket,
    config: &Config,
) -> Result<(), Error> {
    let source_file_extension = filesystem::get_path_file_extension(source_file_path)?;

    if !filesystem::is_file_inside_directory(
        source_file_path,
        Path::new(&album_packet.source_library_path),
        None,
    ) {
        return Err(
            Error::new(
                ErrorKind::Other,
                "Invalid file path: doesn't match base library path."
            )
        );
    }

    if config.file_metadata.matches_audio_extension(&source_file_extension) {
        fo::transcode_audio_file_into_mp3_v0(source_file_path, config)?;
        Ok(())

    } else if config.file_metadata.matches_data_extension(&source_file_extension) {
        fo::copy_data_file(source_file_path, config)?;
        Ok(())

    } else {
        Err(
            Error::new(
                ErrorKind::Other,
                format!(
                        "Invalid file path: extension matches neither audio nor data: {}",
                        source_file_extension,
                    ),
            )
        )
    }
}

fn remove_processed_album_file(
    source_file_path: &Path,
    config: &Config,
) -> Result<(), Error> {
    let source_file_extension = filesystem::get_path_file_extension(source_file_path)?;
    let is_audio = config.file_metadata.matches_audio_extension(&source_file_extension);

    fo::remove_target_file(source_file_path, config, is_audio)?;
    Ok(())
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
            "library aggregation"
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
            let full_album_path = album_directory.path();
            let album_info = DirectoryInfo::new(&full_album_path, config)?;

            let album_packet = LibraryAlbumPacket::new(
                &album_info.library_path,
                &album_info.artist_name,
                &album_info.album_title,
            )?;
            album_packet.process_album(config)?;

            album_pbr.inc();
        }

        album_pbr.finish();
        artist_pbr.inc();
    }

    artist_pbr.finish_println("Library transcoded.");

    Ok(())
}


pub fn cmd_transcode_album(album_directory: &Path, config: &Config) -> Result<(), Error> {
    if !album_directory.is_dir() {
        println!("Directory is invalid.");
        exit(1);
    }

    let directory_info = DirectoryInfo::new(album_directory, config)?;

    let packet = LibraryAlbumPacket::new(
        &directory_info.library_path,
        &directory_info.artist_name,
        &directory_info.album_title,
    )?;

    console::horizontal_line(None, None);
    console::horizontal_line_with_text(
        &format!(
            "{}",
            "album transcode"
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

    // TODO Test this.

    println!(
        "{}{}",
        "Processing album: "
            .bright_black(),
        directory_info.album_title
            .yellow(),
    );
    console::new_line();
    packet.process_album(config)?;

    println!(
        "{}",
        "Processing finished, all audio files transcoded and data files copied.".green()
    );

    // Resave .librarymeta
    println!(
        "{}{}",
        "Saving fresh "
            .yellow(),
        ".librarymeta"
            .bold()
            .green(),
    );
    packet.save_librarymeta(config, true)?;

    Ok(())
}
