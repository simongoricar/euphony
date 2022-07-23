use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use console::Color::Color256;
use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

use directories as dirs;

use crate::console as c;
use crate::commands::transcode::packets::album::AlbumWorkPacket;
use crate::commands::transcode::packets::library::LibraryWorkPacket;
use crate::configuration::Config;

mod meta;
mod directories;
mod packets;

const DEFAULT_PROGRESS_BAR_TICK_INTERVAL: Duration = Duration::from_millis(100);

lazy_static! {
    static ref DEFAULT_PROGRESS_BAR_STYLE: ProgressStyle = ProgressStyle::with_template(
        "{msg:^50!} [{elapsed_precise} | {pos:>3}/{len:3}] [{bar:80.cyan/blue}]"
    )
        .unwrap()
        .progress_chars("#>-");
}

/// This function lists all the albums in all of the libraries that need to be transcoded
/// and performs the transcode using ffmpeg (for audio files) and simple file copy (for data files).
pub fn cmd_transcode_all(config: &Config) -> Result<(), Error> {
    c::horizontal_line_with_text(
        format!(
            "{}",
            style("transcoding (all libraries)")
                .cyan()
                .bold()
        ),
        None, None,
    );
    c::new_line();

    let processing_begin_time = Instant::now();

    println!(
        "{}",
        style("Scanning libraries for changes...")
            .yellow()
            .bright(),
    );

    // List all libraries.
    let mut library_packets: Vec<LibraryWorkPacket> = Vec::new();
    for (library_name, library) in &config.libraries {
        library_packets.push(
            LibraryWorkPacket::from_library_path(
                library_name,
                Path::new(&library.path),
                config,
            )?,
        );
    }

    let total_libraries = library_packets.len();

    // Filter libraries to ones that need at least one album processed.
    let mut filtered_library_packets: Vec<(LibraryWorkPacket, Vec<AlbumWorkPacket>)> = Vec::new();
    for mut library_packet in library_packets {
        // Not all albums need to be processed each time, this function returns only the list
        // of albums that need are either mising or have changed according to the .librarymeta file.
        let mut albums_in_need_of_processing = library_packet.get_albums_in_need_of_processing(config)?;
        albums_in_need_of_processing.sort_unstable_by(
            |first, second| {
                first.album_info.album_title.cmp(&second.album_info.album_title)
            }
        );

        if albums_in_need_of_processing.len() > 0 {
            filtered_library_packets.push((library_packet, albums_in_need_of_processing));
        }
    }

    filtered_library_packets.sort_unstable_by(
        |(first, _), (second, _)| {
            first.name.cmp(&second.name)
        }
    );

    let total_filtered_libraries = filtered_library_packets.len();
    if total_filtered_libraries == 0 {
        println!(
            "{}",
            style("All transcodes are already up to date.")
                .green()
                .bright()
                .bold(),
        );
        return Ok(());
    } else {
        println!(
            "{}/{} libraries need transcoding:",
            style(total_filtered_libraries)
                .bold()
                .italic(),
            style(total_libraries)
                .bold(),
        );
        for (library, albums) in &filtered_library_packets {
            println!(
                "  {:12} {} new or changed albums.",
                format!("{}:", library.name),
                style(albums.len())
                    .bold()
            );
        }
        c::new_line();
    }

    // Set up progress bars (three bars, one for current file, another for albums, the third for libraries).
    let multi_pbr = MultiProgress::new();

    let files_progress_bar = multi_pbr.add(ProgressBar::new(0));
    files_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    let files_progress_bar_ref = Arc::new(Mutex::new(files_progress_bar));

    let albums_progress_bar = multi_pbr.add(ProgressBar::new(0));
    albums_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    let library_progress_bar = multi_pbr.add(ProgressBar::new(filtered_library_packets.len() as u64));
    library_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    files_progress_bar_ref.clone().lock().unwrap().enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);
    albums_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);
    library_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let set_current_file  = |file_name: &str| {
        files_progress_bar_ref.clone().lock().unwrap().set_message(
            format!(
                "🎵  {}",
                style(file_name)
                    .fg(Color256(131))
                    .underlined(),
            ),
        );
    };

    let set_current_album = |album_name: &str| {
        albums_progress_bar.set_message(
            format!(
                "💽  {} {}",
                style("Album:")
                    .black()
                    .bright(),
                style(album_name)
                    .fg(Color256(103))
                    .underlined(),
            ),
        );
    };

    let set_current_library = |library_name: &str| {
        library_progress_bar.set_message(
            format!(
                "📖  {} {}",
                style("Library:")
                    .black()
                    .bright(),
                style(library_name)
                    .white()
                    .underlined(),
            ),
        );
    };

    set_current_library("/");
    set_current_album("/");
    set_current_file("/");

    // TODO Make thread num configurable.
    let thread_pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();

    // Iterate over libraries and process each album.
    for (library, album_packets) in filtered_library_packets {
        set_current_library(&library.name);

        albums_progress_bar.reset();
        albums_progress_bar.set_length(album_packets.len() as u64);
        albums_progress_bar.set_position(0);

        for mut album_packet in album_packets {
            set_current_album(&album_packet.album_info.album_title);

            let file_packets = album_packet.get_work_packets(config)?;

            {
                let fpb_locked = files_progress_bar_ref.lock().unwrap();
                fpb_locked.reset();
                fpb_locked.set_length(file_packets.len() as u64);
                fpb_locked.set_position(0);
            }

            // TODO Figure out a way to properly track progress using the progress bar.
            let fpb_threadpool_clone = files_progress_bar_ref.clone();
            let (tx, rx): (Sender<Error>, Receiver<Error>) = channel();

            thread_pool.scope(move |s| {
                for file_packet in file_packets {
                    let thread_tx = tx.clone();
                    let inner_progress_bar = fpb_threadpool_clone.clone();

                    s.spawn(move |_| {
                        let result = file_packet.process(config);

                        let progress_bar_lock = inner_progress_bar.lock().unwrap();
                        progress_bar_lock.inc(1);

                        if result.is_err() {
                            // DEBUGONLY
                            eprintln!("DEBUG: Something went wrong with packet: {:?}", file_packet);
                            thread_tx.send(result.unwrap_err())
                                .expect("Work thread could not send error to main thread!");
                        }
                    });
                }
            });

            let mut collected_thread_errors: Vec<Error> = Vec::new();
            collected_thread_errors.extend(rx.try_iter());

            if collected_thread_errors.len() > 0 {
                eprintln!(
                    "{}",
                    style("Something went wrong with one or more workers:")
                        .red(),
                );
                for err in collected_thread_errors {
                    eprintln!(
                        "  {}",
                        err,
                    );
                }
                return Err(Error::new(ErrorKind::Other, "One or more transcoding threads errored."));
            }

            album_packet.save_fresh_meta(config, true)?;
            albums_progress_bar.inc(1);
        }

        library_progress_bar.inc(1);
    }

    files_progress_bar_ref.lock().unwrap().finish();
    albums_progress_bar.finish();
    library_progress_bar.finish();

    let processing_time_delta = processing_begin_time.elapsed();
    println!(
        "Transcoding completed in {:.1?} seconds.",
        processing_time_delta,
    );

    // TODO Check why sometimes the process fails with "The system cannot find the path specified. (os error 3)"
    Ok(())
}

/// This function lists all the allbums in the selected library that need to be transcoded
/// and performs the actual transcode using ffmpeg (for audio files) and simple file copy (for data files).
pub fn cmd_transcode_library(library_directory: &PathBuf, config: &Config) -> Result<(), Error> {
    if !library_directory.is_dir() {
        println!("Directory is invalid.");
        exit(1);
    }

    c::horizontal_line_with_text(
        format!(
            "{}",
            style("transcoding (single library)")
                .cyan()
                .bold(),
        ),
        None, None,
    );
    c::new_line();

    let processing_begin_time = Instant::now();

    let library_directory_string = library_directory.to_string_lossy().to_string();
    println!(
        "{} {}",
        style("Library directory: ")
            .italic(),
        library_directory_string,
    );
    c::new_line();

    if !config.is_library(library_directory) {
        println!(
            "{}",
            style("Selected directory is not a registered library, exiting.")
                .red(),
        );

        exit(1);
    }

    println!(
        "{}",
        style("Scanning library for changes...")
            .yellow()
            .bright(),
    );

    let library_name = config.get_library_name_from_path(library_directory)
        .ok_or(Error::new(ErrorKind::Other, "No registered library."))?;

    let mut library_packet = LibraryWorkPacket::from_library_path(
        &library_name,
        library_directory,
        config,
    )?;

    // Filter to just the albums that need to be processed.
    let mut filtered_album_packets = library_packet.get_albums_in_need_of_processing(config)?;
    let total_filtered_albums = filtered_album_packets.len();

    if total_filtered_albums == 0 {
        println!(
            "{}",
            style("Transcodes of this library are already up to date.")
                .green()
                .bright()
                .bold(),
        );
        return Ok(());
    } else {
        println!(
            "{}/{} albums in this library are new or have changed.",
            style(total_filtered_albums)
                .bold()
                .underlined(),
            style(library_packet.album_packets.len())
                .bold(),
        );
        c::new_line();
    }

    // Set up two progress bars, one for the current file, another for the current album.
    let multi_pbr = MultiProgress::new();

    let file_progress_bar = multi_pbr.add(ProgressBar::new(0));
    file_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    let album_progress_bar = multi_pbr.add(ProgressBar::new(filtered_album_packets.len() as u64));
    album_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    file_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);
    album_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let set_current_file = |file_name: &str| {
        file_progress_bar.set_message(
            format!(
                "🎵  {} {}",
                style("File:")
                    .black()
                    .bright(),
                style(file_name)
                    .fg(Color256(131))
                    .underlined(),
            ),
        );
    };

    let set_current_album = |album_name: &str| {
        album_progress_bar.set_message(
            format!(
                "💽  {} {}",
                style("Album:")
                    .black()
                    .bright(),
                style(album_name)
                    .fg(Color256(103))
                    .underlined(),
            ),
        );
    };


    set_current_file("/");
    set_current_album("/");

    // Transcode all albums that are new or have changed.
    for album_packet in &mut filtered_album_packets {
        set_current_album(&album_packet.album_info.album_title);

        let file_work_packets = album_packet.get_work_packets(config)?;

        file_progress_bar.reset();
        file_progress_bar.set_length(file_work_packets.len() as u64);
        file_progress_bar.set_position(0);

        for file_packet in file_work_packets {
            set_current_file(&file_packet.get_file_name()?);

            file_packet.process(config)?;
            file_progress_bar.inc(1);
        }

        album_packet.save_fresh_meta(config, true)?;
        album_progress_bar.inc(1);
    }

    file_progress_bar.finish();
    album_progress_bar.finish();

    let processing_time_delta = processing_begin_time.elapsed();
    println!(
        "Library transcoded in {:.1?} seconds.",
        processing_time_delta,
    );

    Ok(())
}

/// This function transcodes a single album using ffmpeg (for audio files) and simple file copy (for data files).
pub fn cmd_transcode_album(album_directory: &Path, config: &Config) -> Result<(), Error> {
    if !album_directory.is_dir() {
        println!("Directory is invalid.");
        exit(1);
    }

    c::horizontal_line_with_text(
        format!(
            "{}",
            style("transcoding (single album)")
                .cyan()
                .bold(),
        ),
        None, None,
    );
    c::new_line();

    let processing_begin_time = Instant::now();

    let album_directory_string = album_directory.to_string_lossy().to_string();
    println!(
        "{} {}",
        style("Album directory: ")
            .italic(),
        album_directory_string,
    );
    c::new_line();

    // Verify the directory is an album.
    if !dirs::directory_is_album(config, album_directory) {
        eprintln!(
            "{}",
            style("Not an album directory, exiting.")
                .red()
        );

        exit(1);
    }

    println!(
        "{}",
        style("Scanning album...")
            .yellow()
            .bright(),
    );

    let mut album_packet = AlbumWorkPacket::from_album_path(
        album_directory,
        config,
    )?;
    let total_track_count = album_packet.get_total_track_count(config)?;

    let file_packets = album_packet.get_work_packets(config)?;

    println!(
        "{}/{} files in this album are new or have changed.",
        style(file_packets.len())
            .bold()
            .underlined(),
        style(total_track_count)
            .bold(),
    );
    c::new_line();

    if file_packets.len() == 0 {
        println!(
            "{}",
            style("Transcoded album is already up to date.")
                .green()
                .bright()
                .bold(),
        );

        return Ok(());
    }

    // Set up a progress bar for the current file.
    let file_progress_bar = ProgressBar::new(file_packets.len() as u64);
    file_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    file_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let set_current_file = |file_name: &str| {
        file_progress_bar.set_message(
            format!(
                "🎵  {} {}",
                style("File:")
                    .black()
                    .bright(),
                style(file_name)
                    .fg(Color256(131))
                    .underlined(),
            ),
        );
    };

    for file_packet in file_packets {
        let file_name = file_packet.get_file_name()?;
        set_current_file(&file_name);

        file_packet.process(config)?;
        file_progress_bar.inc(1);
    }

    album_packet.save_fresh_meta(config, true)?;

    file_progress_bar.finish();

    let processing_time_delta = processing_begin_time.elapsed();
    println!(
        "Album transcoded in {:.1?} seconds.",
        processing_time_delta,
    );

    Ok(())
}
