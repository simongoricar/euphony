use std::sync::mpsc;
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::style::Stylize;
use miette::Result;
use rayon::{ThreadPool, ThreadPoolBuilder};

use directories as dirs;

use crate::commands::transcode::packets::album::AlbumWorkPacket;
use crate::commands::transcode::packets::file::{FilePacketType, FileWorkPacket};
use crate::commands::transcode::packets::library::LibraryWorkPacket;
use crate::configuration::Config;
use crate::console::TranscodeLogTerminalBackend;
use crate::console::backends::shared::{QueueItemID, QueueType, SpinnerStyle};
use crate::console::utilities::term_println_tlt;
use crate::globals::verbose_enabled;

mod metadata;
mod directories;
mod packets;
mod overrides;

enum WorkerMessage {
    StartingWithFile {
        queue_item: QueueItemID,
    },
    FinishedWithFile {
        queue_item: QueueItemID,
        was_ok: bool,
    },
}

/// Builds a ThreadPool using the `transcode_threads` configuration value.
pub fn build_transcode_thread_pool(config: &Config) -> ThreadPool {
    ThreadPoolBuilder::new()
        .num_threads(config.aggregated_library.transcode_threads as usize)
        .build()
        .unwrap()
}

fn process_album(
    album_file_packets: &Vec<(FileWorkPacket, QueueItemID)>,
    config: &Config,
    thread_pool: &ThreadPool,
    update_sender: Sender<WorkerMessage>,
) {
    if album_file_packets.is_empty() {
        return;
    }
    
    thread_pool.scope_fifo(move |s| {
        for (file, queue_item) in album_file_packets {
            let update_sender_thread_clone = update_sender.clone();
            
            s.spawn_fifo(move |_| {
                update_sender_thread_clone.send(WorkerMessage::StartingWithFile {
                    queue_item: *queue_item,
                }).expect("Could not send message from worker to main thread.");
    
                // TODO Retries.
                
                let work_result = file.process(config);
                let was_ok = work_result.is_ok();
    
                update_sender_thread_clone.send(WorkerMessage::FinishedWithFile {
                    queue_item: *queue_item,
                    was_ok,
                }).expect("Could not send message from worker to main thread.");
            });
        }
    });
}

// TODO Consider reimplementing transcode for specific library and specific album, like in previous versions.

/// This function lists all the albums in all of the libraries that need to be transcoded
/// and performs the transcode using ffmpeg (for audio files) and simple file copy (for data files).
pub fn cmd_transcode_all(
    config: &Config,
    terminal: &mut dyn TranscodeLogTerminalBackend
) -> Result<()> {
    term_println_tlt(terminal, "Mode: transcode all libraries.".cyan().bold());
    term_println_tlt(terminal, "Scanning all libraries for changes...");
    
    let processing_begin_time = Instant::now();
    
    // Generate a list of `LibraryWorkPacket` for each library.
    let mut all_libraries: Vec<LibraryWorkPacket> = config.libraries
        .iter()
        .map(|(name, library)|
            LibraryWorkPacket::from_library_path(
                name,
                &library.path,
                config,
            )
        )
        .collect::<Result<Vec<LibraryWorkPacket>>>()?;
    
    all_libraries.sort_unstable_by(
        |first, second|
            first.name.cmp(&second.name)
    );
    
    // Generate a complete list of work to be done (all the libraries, albums and individual files
    // that will be transcoded or copied). This skips libraries and their albums that have already
    // been transcoded (and haven't changed).
    type AlbumsWorkload = Vec<(AlbumWorkPacket, Vec<FileWorkPacket>)>;
    type LibrariesWorkload = Vec<(LibraryWorkPacket, AlbumsWorkload)>;
    
    type AlbumsQueuedWorkload = Vec<(AlbumWorkPacket, QueueItemID, Vec<FileWorkPacket>)>;
    type LibrariesQueuedWorkload = Vec<(LibraryWorkPacket, QueueItemID, AlbumsQueuedWorkload)>;
    
    let mut full_workload: LibrariesWorkload = Vec::new();
    
    for mut library in all_libraries {
        let mut albums_to_process = library
            .get_albums_in_need_of_processing(config)?;
    
        // For convenience (and because why not), both libraries and albums are sorted alphabetically.
        albums_to_process.sort_unstable_by(
            |first, second|
                first.album_info.album_title.cmp(&second.album_info.album_title)
        );
        
        if !albums_to_process.is_empty() {
            // For each album in the library that was changed, generate a list of files to process.
            let mut albums_workload: AlbumsWorkload = Vec::new();
            
            for mut album in albums_to_process {
                if album.needs_processing(config)? {
                    let files = album.get_work_packets(config)?;
                    albums_workload.push((album, files));
                }
            }
            
            full_workload.push((library, albums_workload));
        }
    }
    
    // Number of files that need to be processed (copied or transcoded).
    let total_files_to_process = full_workload
        .iter_mut()
        .flat_map(|(_, albums)| albums)
        .map(|(_, files)| files.len())
        .sum::<usize>();
    
    // Skip processing if there are no changes,
    // otherwise show a short summary of changes and start transcoding.
    if full_workload.is_empty() {
        term_println_tlt(terminal, "Transcodes are already up to date.".green().bold());
        return Ok(());
    } else {
        term_println_tlt(
            terminal,
            format!(
                "Detected {} changed files, transcoding.",
                total_files_to_process
                    .to_string()
                    .bold()
            )
        );
    }
    
    terminal.queue_begin();
    terminal.progress_begin();
    
    // TODO Add keybinds to control the program (q for exit, etc.).
    
    let mut thread_pool = build_transcode_thread_pool(config);
    let mut files_finished_so_far: usize = 0;
    
    terminal.progress_set_current(files_finished_so_far)?;
    terminal.progress_set_total(total_files_to_process)?;
    
    // Enqueue all libraries and albums, returning an expanded vector containing their `QueueItemID`s.
    let queued_workload: LibrariesQueuedWorkload = full_workload
        .into_iter()
        .map(
            |(library, albums)| {
                let library_description = format!(
                    "{} ({} album{})",
                    library.name,
                    albums.len(),
                    if albums.len() > 1 { "s" } else { "" }
                );
                
                let library_queue_item = terminal.queue_item_add(library_description, QueueType::Library)?;
                
                terminal.queue_item_modify(
                    library_queue_item,
                    Box::new(|item| item.spaces_when_spinner_is_disabled = false)
                )?;
                
                let queued_albums: AlbumsQueuedWorkload = albums
                    .into_iter()
                    .map(|(album, files)| {
                        let album_description = format!(
                            "[{}] {} - {}",
                            files.len(),
                            album.album_info.artist_name,
                            album.album_info.album_title,
                        );
                        let album_queue_item = terminal.queue_item_add(album_description, QueueType::Album)?;
                        
                        Ok((album, album_queue_item, files))
                    })
                    .collect::<Result<AlbumsQueuedWorkload>>()?;
    
                Ok((library, library_queue_item, queued_albums))
            }
        )
        .collect::<Result<LibrariesQueuedWorkload>>()?;
    
    // Finally, iterate over the entire queued workload,
    // transcoding each file in each album and updating the terminal backend on the way.
    for (library, library_queue_item, albums) in queued_workload {
        let time_library_start = Instant::now();
        
        terminal.queue_item_start(library_queue_item)?;
        terminal.queue_item_modify(
            library_queue_item,
            Box::new(|item| item.set_suffix(" [active]"))
        )?;
    
        term_println_tlt(
            terminal,
            format!(
                "Transcoding contents of library: {} ({} albums)",
                library.name.clone().bold(),
                albums.len(),
            )
        );
        
        for (mut album, album_queue_id, files) in albums {
            let time_album_start = Instant::now();
            
            terminal.queue_item_start(album_queue_id)?;
            terminal.queue_item_modify(
                album_queue_id,
                Box::new(|item| {
                    item.clear_prefix();
                    item.enable_spinner(SpinnerStyle::Square, None);
                })
            )?;
    
            term_println_tlt(
                terminal,
                format!(
                    "|-> Transcoding album: {} ({} files)",
                    format!(
                        "{} - {}",
                        album.album_info.artist_name,
                        album.album_info.album_title,
                    ).underlined(),
                    files.len(),
                )
            );
            
            if verbose_enabled() {
                let fresh_metadata = album.get_fresh_meta(config)?;
                term_println_tlt(
                    terminal,
                    format!(
                        "[VERBOSE] AlbumWorkPacket album: {:?}; files in meta: {:?}",
                        album.album_info,
                        fresh_metadata.files,
                    )
                );
            }
            
            // Enter all album files into queue, generating a list of files and their associated queue IDs.
            // TODO A percentage of storage saved after each file finishes would be cool.
            let queued_files = files
                .into_iter()
                .map(|file| {
                    let item_description = format!(
                        "[{}] {}",
                        match file.file_type {
                            FilePacketType::AudioFile => "audio",
                            FilePacketType::DataFile => "data",
                        },
                        file.get_file_name()?,
                    );
    
                    // If adding the item to the queue was successful, this maps the original `FileWorkPacket`
                    // to a tuple of `(FileWorkPacket, QueueItemID)`, otherwise returns an `Err` with the original error.
                    match terminal.queue_item_add(item_description, QueueType::File) {
                        Ok(queue_item_id) => Ok((file, queue_item_id)),
                        Err(error) => Err(error),
                    }
                })
                .collect::<Result<Vec<(FileWorkPacket, QueueItemID)>>>()?;
            
            let (tx, rx) = mpsc::channel::<WorkerMessage>();
            
            let config_thread_clone = config.clone();
            let processing_thread_handle = thread::spawn(move || {
                process_album(
                    &queued_files,
                    &config_thread_clone,
                    &thread_pool,
                    tx,
                );
                
                // Return the thread pool back into the main thread.
                thread_pool
            });
            
            // Wait for processing thread to complete. Meanwhile, keep receiving progress messages
            // from the processing thread and update the terminal backend accordingly.
            loop {
                let message = rx.recv_timeout(Duration::from_millis(50));
                match message {
                    Ok(message) => match message {
                        WorkerMessage::StartingWithFile { queue_item } => {
                            terminal.queue_item_start(queue_item)?;
                            terminal.queue_item_modify(
                                queue_item,
                                Box::new(|item| {
                                    item.clear_prefix();
                                    item.enable_spinner(SpinnerStyle::Pixel, None);
                                })
                            )?;
                        },
                        WorkerMessage::FinishedWithFile { queue_item, was_ok } => {
                            terminal.queue_item_finish(queue_item, was_ok)?;
                            terminal.queue_item_modify(
                                queue_item,
                                Box::new(|item| item.disable_spinner())
                            )?;
                            
                            // Update progress bar with new percentage.
                            files_finished_so_far += 1;
                            terminal.progress_set_current(files_finished_so_far)?;
                        },
                    },
                    Err(error) => {
                        // If the main processing thread stopped, the channel will be disconnected,
                        // in which case we should stop waiting.
                        if error == RecvTimeoutError::Disconnected {
                            break;
                        }
                    }
                }
    
                // Make sure the main processing thread is still alive.
                if processing_thread_handle.is_finished() {
                    break;
                }
            }
            
            thread_pool = processing_thread_handle.join()
                .expect("Could not join main processing thread.");
    
            // Update the metadata in .album.euphony file, saving details that will ensure
            // they are not needlessly transcoded again next time.
            album.save_fresh_meta(config, true)?;
            
            terminal.queue_item_finish(album_queue_id, true)?;
            terminal.queue_item_modify(
                album_queue_id,
                Box::new(|item| {
                    item.spaces_when_spinner_is_disabled = false;
                    item.disable_spinner();
                    item.set_prefix(" ☑ ");
                })
            )?;
            terminal.queue_clear(QueueType::File)?;
            
            let time_album_elapsed = time_album_start
                .elapsed()
                .as_secs_f64();
            term_println_tlt(
                terminal,
                format!(
                    "|-> Album {} transcoded in {:.2} seconds.",
                    format!(
                        "{} - {}",
                        album.album_info.artist_name,
                        album.album_info.album_title,
                    ).underlined(),
                    time_album_elapsed,
                )
            );
        }
        
        terminal.queue_item_finish(library_queue_item, true)?;
        terminal.queue_item_modify(
            library_queue_item,
            Box::new(|item| item.clear_suffix())
        )?;
        
        let time_library_elapsed = time_library_start
            .elapsed()
            .as_secs_f64();
        term_println_tlt(
            terminal,
            format!(
                "|-> Library {} transcoded in {:.2} seconds.",
                library.name.clone().bold(),
                time_library_elapsed
            )
        );
    }
    
    let processing_time_delta = processing_begin_time.elapsed().as_secs_f64();
    term_println_tlt(
        terminal,
        format!(
            "Full library transcoding completed in {:.2} seconds.",
            format!("{:.2}", processing_time_delta).italic(),
        )
    );
    
    Ok(())
}

/*
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
    file_progress_bar.set_style(
        build_progress_bar_style_with_header(format!("{:9}", "(file)")),
    );
    file_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let album_progress_bar = multi_pbr.add(ProgressBar::new(filtered_album_packets.len() as u64));
    album_progress_bar.set_style(
        build_progress_bar_style_with_header(format!("{:9}", "(album)")),
    );
    album_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let file_progress_bar_ref = Arc::new(Mutex::new(file_progress_bar));


    let set_current_file = build_styled_progress_bar_message_fn_dynamic_locked_bar(
        Style::new().fg(Color256(131)).underlined(),
        42,
    );

    let set_current_album = build_styled_progress_bar_message_fn(
        &album_progress_bar,
        Style::new().fg(Color256(103)).underlined(),
        42,
    );


    set_current_file("/", &file_progress_bar_ref.lock().unwrap());
    set_current_album("/");

    let thread_pool = build_transcode_thread_pool(config);
    let observer = build_processing_observer(
        file_progress_bar_ref.clone(),
        Box::new(set_current_file.clone()),
    );

    // Transcode all albums that are new or have changed.
    for album_packet in &mut filtered_album_packets {
        set_current_album(&album_packet.album_info.album_title);

        let file_work_packets = album_packet.get_work_packets(config)?;

        {
            let fpb_lock = file_progress_bar_ref.lock().unwrap();
            fpb_lock.reset();
            fpb_lock.set_length(file_work_packets.len() as u64);
            fpb_lock.set_position(0);
        }

        let successful = process_file_packets_in_threadpool(
            config,
            &thread_pool,
            file_work_packets,
            &observer,
        );
        if !successful {
            return Err(Error::new(ErrorKind::Other, "One or more transcoding threads errored."))
        }

        album_packet.save_fresh_meta(config, true)?;
        album_progress_bar.inc(1);
    }

    file_progress_bar_ref.lock().unwrap().finish();
    album_progress_bar.finish();

    let processing_time_delta = processing_begin_time.elapsed();
    println!(
        "Library transcoded in {:.1?}.",
        processing_time_delta,
    );

    Ok(())
}

     */
/*
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

    let mut album_packet = AlbumWorkPacket::from_album_path(album_directory, config)?;
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
    file_progress_bar.set_style(
        build_progress_bar_style_with_header(format!("{:9}", "(file)")),
    );
    file_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let file_progress_bar_arc = Arc::new(Mutex::new(file_progress_bar));

    let set_current_file = build_styled_progress_bar_message_fn_dynamic_locked_bar(
        Style::new().fg(Color256(131)).underlined(),
        42,
    );

    let thread_pool = build_transcode_thread_pool(config);
    let observer = build_processing_observer(
        file_progress_bar_arc.clone(),
        Box::new(set_current_file.clone()),
    );

    let successful = process_file_packets_in_threadpool(
        config,
        &thread_pool,
        file_packets,
        &observer,
    );
    if !successful {
        return Err(Error::new(ErrorKind::Other, "One or more transcoding threads errored."))
    }

    album_packet.save_fresh_meta(config, true)?;

    file_progress_bar_arc.lock().unwrap().finish();

    let processing_time_delta = processing_begin_time.elapsed();
    println!(
        "Album transcoded in {:.1?}.",
        processing_time_delta,
    );

    Ok(())
}
 */
