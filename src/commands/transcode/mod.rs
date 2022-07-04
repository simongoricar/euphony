use std::io::Error;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::thread;

use owo_colors::OwoColorize;
use pbr::MultiBar;

use directories as dirs;

use crate::{console, filesystem};
use crate::commands::transcode::directories::AlbumDirectoryInfo;
use crate::configuration::Config;

mod meta;
mod directories;
mod packets;


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
    album_pbr.message("Artist albums processed | ");

    let mut artist_pbr = pbr.create_bar(artist_directories.len() as u64);
    artist_pbr.message("Processing artist: / | ");

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

        let artist_name = artist_directory.file_name();
        let artist_name = artist_name.to_str()
            .expect("Could not convert artist name to string.");

        artist_pbr.message(
            &format!(
                "Processing artist: {} | ",
                artist_name,
            )
        );

        album_pbr.total = album_directories.len() as u64;
        album_pbr.set(0);

        for album_directory in album_directories {
            let full_album_path = album_directory.path();
            let album_info = AlbumDirectoryInfo::new(&full_album_path, config)?;

            let album_packet = LibraryAlbumPacket::new(
                &album_info.library_path,
                &album_info.artist_name,
                &album_info.album_title,
            )?;
            album_packet.process_album(config)?;
            album_packet.save_librarymeta(config, true)?;

            album_pbr.inc();
        }

        artist_pbr.inc();
    }

    album_pbr.finish();
    artist_pbr.finish_println("Library transcoded.");

    Ok(())
}


pub fn cmd_transcode_album(album_directory: &Path, config: &Config) -> Result<(), Error> {
    if !album_directory.is_dir() {
        println!("Directory is invalid.");
        exit(1);
    }

    let directory_info = AlbumDirectoryInfo::new(album_directory, config)?;

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
