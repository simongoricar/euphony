use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::{Duration, Instant};
use console::Color::Color256;
use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use lazy_static::lazy_static;

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

pub fn cmd_transcode_all(config: &Config) -> Result<(), Error> {
    c::horizontal_line_with_text(
        format!(
            "{}",
            style("full library aggregation")
                .cyan()
                .bold()
        ),
        None, None,
    );
    c::new_line();

    let processing_begin_time = Instant::now();

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

    println!(
        "{} {}",
        style("Total libraries: ")
            .yellow()
            .bright(),
        style(library_packets.len())
            .green()
            .bold(),
    );
    println!(
        "{}",
        style("Libraries to be processed: ")
            .yellow()
            .bright(),
    );

    let mut filtered_library_packets: Vec<(LibraryWorkPacket, Vec<AlbumWorkPacket>)> = Vec::new();
    for mut library_packet in library_packets {
        let mut albums_in_need_of_processing = library_packet.get_albums_in_need_of_processing(config)?;

        albums_in_need_of_processing.sort_unstable_by(
            |first, second| {
                first.album_info.album_title.cmp(&second.album_info.album_title)
            }
        );

        if albums_in_need_of_processing.len() > 0 {
            println!(
                "  {}: {} pending albums",
                library_packet.name,
                style(albums_in_need_of_processing.len())
                    .yellow()
                    .bright()
                    .bold(),
            );

            filtered_library_packets.push((library_packet, albums_in_need_of_processing));
        }
    }

    filtered_library_packets.sort_unstable_by(
        |(first, _), (second, _)| {
            first.name.cmp(&second.name)
        }
    );

    c::new_line();

    let multi_pbr = MultiProgress::new();

    let files_progress_bar = multi_pbr.add(ProgressBar::new(0));
    files_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    let albums_progress_bar = multi_pbr.add(ProgressBar::new(0));
    albums_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    let library_progress_bar = multi_pbr.add(ProgressBar::new(filtered_library_packets.len() as u64));
    library_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    files_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);
    albums_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);
    library_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let set_current_file  = |file_name: &str| {
        files_progress_bar.set_message(
            format!(
                "{} {}",
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
        albums_progress_bar.set_message(
            format!(
                "{} {}",
                style("💽 Album:")
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
                "{} {}",
                style("📖 Library:")
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

    for (library, album_packets) in filtered_library_packets {
        set_current_library(&library.name);

        albums_progress_bar.reset();
        albums_progress_bar.set_length(album_packets.len() as u64);
        albums_progress_bar.set_position(0);

        for mut album_packet in album_packets {
            set_current_album(&album_packet.album_info.album_title);

            let file_packets = album_packet.get_work_packets(config)?;

            files_progress_bar.reset();
            files_progress_bar.set_length(file_packets.len() as u64);
            files_progress_bar.set_position(0);

            for file_packet in file_packets {
                set_current_file(&file_packet.get_file_name()?);

                file_packet.process(config)?;
                files_progress_bar.inc(1);
            }

            album_packet.save_fresh_meta(config, true)?;
            albums_progress_bar.inc(1);
        }

        library_progress_bar.inc(1);
    }

    files_progress_bar.finish();
    albums_progress_bar.finish();
    library_progress_bar.finish();

    let processing_time_delta = processing_begin_time.elapsed();
    println!(
        "All libraries processed in {:.1?}.",
        processing_time_delta,
    );

    // TODO Check why sometimes the process fails with "The system cannot find the path specified. (os error 3)"
    Ok(())
}


pub fn cmd_transcode_library(library_directory: &PathBuf, config: &Config) -> Result<(), Error> {
    if !library_directory.is_dir() {
        println!("Directory is invalid.");
        exit(1);
    }

    c::horizontal_line_with_text(
        format!(
            "{}",
            style("library aggregation")
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
        style("Using library directory: ")
            .italic(),
        style(library_directory_string)
            .yellow()
            .bold()
    );
    c::new_line();

    if !config.is_library(library_directory) {
        println!(
            "{}",
            style("Directory is not a library, exiting.")
                .red(),
        );

        exit(1);
    }

    println!(
        "{}",
        style("Scanning library.")
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

    println!(
        "{} {}",
        style("Total albums:    ")
            .yellow()
            .bright(),
        style(library_packet.album_packets.len())
            .green()
            .bold(),
    );

    // Filter to just the albums that need to be processed.
    let mut filtered_album_packets = library_packet.get_albums_in_need_of_processing(config)?;

    println!(
        "{} {}",
        style("To be processed: ")
            .yellow()
            .bright(),
        style(filtered_album_packets.len())
            .green()
            .bold()
    );
    c::new_line();

    if filtered_album_packets.len() == 0 {
        println!(
            "{}",
            style("Aggregated library is up to date, no need to continue.")
                .green()
                .bright()
                .bold(),
        );

        return Ok(());
    }

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
                "{} {}",
                style("🎵 File:")
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
                "{} {}",
                style("💽 Album:")
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
        "Library processed in {:.1?}.",
        processing_time_delta,
    );

    Ok(())
}


pub fn cmd_transcode_album(album_directory: &Path, config: &Config) -> Result<(), Error> {
    if !album_directory.is_dir() {
        println!("Directory is invalid.");
        exit(1);
    }

    c::horizontal_line_with_text(
        format!(
            "{}",
            style("album aggregation")
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
        style("Using album directory: ")
            .italic(),
        style(album_directory_string)
            .bold()
            .yellow()
    );
    c::new_line();

    // Verify the directory is an album.
    if !dirs::directory_is_album(config, album_directory) {
        eprintln!(
            "{}",
            style("Directory is not an album directory, exiting.")
                .red()
        );

        exit(1);
    }

    println!(
        "{}",
        style("Scanning album.")
            .yellow()
            .bright(),
    );

    let mut album_packet = AlbumWorkPacket::from_album_path(
        album_directory,
        config,
    )?;
    let total_track_count = album_packet.get_total_track_count(config)?;

    println!(
        "{} {}",
        style("Total album tracks:  ")
            .yellow()
            .bright(),
        style(total_track_count)
            .green()
            .bold(),
    );

    let file_packets = album_packet.get_work_packets(config)?;

    println!(
        "{} {}",
        style("To be processed:     ")
            .yellow()
            .bright(),
        style(file_packets.len())
            .green()
            .bold(),
    );
    c::new_line();

    if file_packets.len() == 0 {
        println!(
            "{}",
            style("Aggregated album is up to date, no need to continue.")
                .green()
                .bright()
                .bold(),
        );

        return Ok(());
    }

    let file_progress_bar = ProgressBar::new(file_packets.len() as u64);
    file_progress_bar.set_style((*DEFAULT_PROGRESS_BAR_STYLE).clone());

    file_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let set_current_file = |file_name: &str| {
        file_progress_bar.set_message(
            format!(
                "{} {}",
                style("🎵 File:")
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
        "Album processed in {:.1?}.",
        processing_time_delta,
    );

    Ok(())
}
