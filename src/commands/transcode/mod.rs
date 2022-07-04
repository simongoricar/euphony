use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::exit;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use owo_colors::OwoColorize;

use directories as dirs;

use crate::{console, filesystem};
use crate::commands::transcode::packets::album::AlbumWorkPacket;
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
    console::new_line();

    if !dirs::directory_is_library(config, library_directory) {
        println!(
            "{}", "Directory is not a library, exiting.".red(),
        );
        exit(1);
    }

    // Enumerate artist directories and traverse each one.
    // Then traverse each album inside those directories.
    // If any invalid folder is found, an error is shown.
    let mut album_packets: Vec<AlbumWorkPacket> = Vec::new();

    println!(
        "{}",
        "Scanning library."
            .bright_yellow()
    );

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

    for artist_directory in artist_directories {
        let (_, album_directories) = match filesystem::list_dir_entry_contents(&artist_directory) {
            Ok(data) => data,
            Err(error) => {
                return Err(
                    Error::new(
                        ErrorKind::Other,
                        format!(
                            "Error while listing artist albums: {}",
                            error,
                        ),
                    )
                )
            },
        };

        for album_directory in album_directories {
            let album_directory_path = album_directory.path();
            let album_packet = AlbumWorkPacket::from_album_path(
                album_directory_path, config,
            )?;

            album_packets.push(album_packet);
        }
    }

    println!(
        "{} {}",
        "Total albums:    "
            .bright_yellow(),
        album_packets.len()
            .green()
            .bold()
    );

    // Filter to just the albums that need to be processed.
    let mut filtered_album_packets: Vec<AlbumWorkPacket> = Vec::new();
    for album_packet in &mut album_packets {
        if album_packet.needs_processing(config)? {
            filtered_album_packets.push(album_packet.clone());
        }
    }

    println!(
        "{} {}",
        "To be processed: "
            .bright_yellow(),
        filtered_album_packets.len()
            .green()
            .bold()
    );
    println!();

    if filtered_album_packets.len() == 0 {
        println!(
            "{}",
            "Aggregated library is up to date, no need to continue."
                .bright_green()
                .bold(),
        );
        return Ok(());
    }


    let progress_style = ProgressStyle::with_template(
    "{msg:^35!} [{elapsed_precise} / -{eta:3}] [{bar:80.cyan/blue}] {pos:>3}/{len:3}"
    )
        .unwrap()
        .progress_chars("#>-");

    let multi_pbr = MultiProgress::new();

    let file_progress_bar = multi_pbr.add(ProgressBar::new(0));
    file_progress_bar.set_style(progress_style.clone());
    file_progress_bar.set_message(
        format!(
            "{} ",
            "/"
                .bright_cyan()
                .underline()
                .bold()
        ),
    );

    let album_progress_bar = multi_pbr.add(ProgressBar::new(filtered_album_packets.len() as u64));
    album_progress_bar.set_style(progress_style.clone());
    album_progress_bar.set_message(
        format!(
            "{}",
            "Total albums"
                .bright_magenta()
                .italic(),
        ),
    );


    for album_packet in &mut filtered_album_packets {
        file_progress_bar.set_message(
            format!(
                "{}{}",
                "Album: "
                    .bright_black(),
                album_packet.album_info.album_title
                    .bright_cyan()
                    .underline()
                    .bold(),
            ),
        );

        let file_work_packets = album_packet.get_work_packets(config)?;

        file_progress_bar.reset();
        file_progress_bar.set_length(file_work_packets.len() as u64);
        file_progress_bar.set_position(0);

        for file_packet in file_work_packets {
            file_packet.process(config)?;
            file_progress_bar.inc(1);
        }

        album_packet.save_fresh_meta(config, true)?;

        album_progress_bar.inc(1);
    }

    file_progress_bar.finish();
    album_progress_bar.finish();

    Ok(())
}


pub fn cmd_transcode_album(album_directory: &Path, config: &Config) -> Result<(), Error> {
    if !album_directory.is_dir() {
        println!("Directory is invalid.");
        exit(1);
    }

    let mut album_packet = AlbumWorkPacket::from_album_path(album_directory, config)?;

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

    println!(
        "{}{}",
        "Processing album: "
            .bright_black(),
        album_packet.album_info.album_title
            .yellow(),
    );
    console::new_line();

    let work_packets = album_packet.get_work_packets(config)?;
    for file_packet in work_packets {
        file_packet.process(config)?;
    }

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

    album_packet.save_fresh_meta(config, true)?;

    Ok(())
}
