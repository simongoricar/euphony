use std::sync::mpsc;
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};
use crossterm::style::Stylize;

use miette::{IntoDiagnostic, Result};
use rayon::{ThreadPool, ThreadPoolBuilder};

use directories as dirs;

use crate::commands::transcode::packets::album::AlbumWorkPacket;
use crate::commands::transcode::packets::file::{FilePacketType, FileWorkPacket};
use crate::commands::transcode::packets::library::LibraryWorkPacket;
use crate::configuration::Config;
use crate::console_backends::{LogBackend, QueueItemID, TerminalBackend, TranscodeBackend};
use crate::globals::verbose_enabled;

mod metadata;
mod directories;
mod packets;
mod overrides;

/*
const DEFAULT_PROGRESS_BAR_TICK_INTERVAL: Duration = Duration::from_millis(100);

/// Builds a ProgressStyle that contains the requested header at the beginning of the line.
fn build_progress_bar_style_with_header<S: AsRef<str>>(header_str: S) -> ProgressStyle {
    ProgressStyle::with_template(
        &format!(
            "{}{}",
            header_str.as_ref(),
            "{msg:^42!} [{elapsed_precise} | {pos:>3}/{len:3}] [{bar:45.cyan/blue}]"
        ),
    )
        .unwrap()
        .progress_chars("#>-")
}

fn truncate_string_with_ending<S: AsRef<str>>(
    text: S,
    max_width: usize,
    overflow_string: Option<S>,
) -> String {
    const OVERFLOW_DEFAULT_ENDING: &str = "..";

    let overflow_string = match overflow_string {
        Some(overflow_string) => overflow_string.as_ref().to_string(),
        None => OVERFLOW_DEFAULT_ENDING.to_string(),
    };

    let text = text.as_ref();
    let total_width = measure_text_width(text);

    if total_width > max_width {
        let mut truncated_text = text.to_string();
        truncated_text.truncate(max_width - overflow_string.len());
        truncated_text.push_str(&overflow_string);

        truncated_text
    } else {
        text.to_string()
    }
}

/// A HOF (Higher-order-function) that takes a ProgressBar reference and a text Style and
/// *returns* a function that will then always take a single parameter: the text to set on the progress bar.
fn build_styled_progress_bar_message_fn(
    progress_bar: &ProgressBar,
    text_style: Style,
    max_text_width: usize,
) -> impl Fn(&str) + Send + Clone {
    let progress_bar = progress_bar.clone();

    move |text: &str| {
        let text = truncate_string_with_ending(text, max_text_width, None);
        progress_bar.set_message(
            format!(
                "{}",
                text_style.apply_to(text),
            ),
        );
    }
}

/// This is a higher-order-function. It is similar to `build_styled_progress_bar_message_fn`,
/// but instead builds and return a function that will take two parameters:
/// the text to set, and the progress bar to set it to.
/// Importantly, the second parameter should be behind a MutexGuard reference
/// (meaning that we have it locked at call time).
fn build_styled_progress_bar_message_fn_dynamic_locked_bar(
    text_style: Style,
    max_text_width: usize,
) -> impl Fn(&str, &MutexGuard<ProgressBar>) + Send + Clone {
    move |text: &str, progress_bar: &MutexGuard<ProgressBar>| {
        let text = truncate_string_with_ending(text, max_text_width, None);
        progress_bar.set_message(
            format!(
                "{}",
                text_style.apply_to(text),
            ),
        );
    }
}

fn build_processing_observer(
    progress_bar: Arc<Mutex<ProgressBar>>,
    progress_bar_set_text_fn: Box<dyn Fn(&str, &MutexGuard<ProgressBar>) + Send + Sync>,
) -> ProcessingObserver {
    ProcessingObserver::new(Box::new(move |event| {
        let counter_pbr_lock = progress_bar.lock().unwrap();
        if counter_pbr_lock.is_finished() {
            eprintln!("Warning: observer triggered after progress bar has finished. Ignoring event.");
            return;
        }

        if event.is_final {
            counter_pbr_lock.inc(1);
            (*progress_bar_set_text_fn)(
                &event.file_work_packet.get_file_name()
                    .unwrap(),
                &counter_pbr_lock,
            );
        }

        if verbose_enabled() && event.verbose_info.is_some() {
            counter_pbr_lock.println(
                format!(
                    "  [DEBUG] {}",
                    event.verbose_info.unwrap()
                ),
            );
        }

        if event.error.is_some() {
            if event.is_final {
                counter_pbr_lock.println(
                    format!("  [Error] {}", event.error.unwrap()),
                );
            } else {
                counter_pbr_lock.println(
                    format!("  [Error (will retry)] {}", event.error.unwrap()),
                );
            }
        }
    }))
}
 */

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
    if album_file_packets.len() == 0 {
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

// TODO Consider reimplementing transcode for specific library and specific album, like before.

/// This function lists all the albums in all of the libraries that need to be transcoded
/// and performs the transcode using ffmpeg (for audio files) and simple file copy (for data files).
pub fn cmd_transcode_all<T: TerminalBackend + LogBackend + TranscodeBackend>(
    config: &Config,
    terminal: &mut T
) -> Result<()> {
    terminal.log_println("Mode: transcode all libraries.".cyan().bold());
    terminal.log_println("Scanning all libraries for changes...");
    
    let processing_begin_time = Instant::now();

    // Generate a list of `LibraryWorkPacket` for each library.
    let library_work_packets: Vec<LibraryWorkPacket> = config.libraries
        .iter()
        .map(|(name, library)|
            LibraryWorkPacket::from_library_path(
                name,
                &library.path,
                config,
            ).into_diagnostic()
        )
        .collect::<Result<Vec<LibraryWorkPacket>>>()?;

    // Filter libraries to ones that need at least one album processed.
    let mut filtered_library_packets: Vec<(LibraryWorkPacket, Vec<AlbumWorkPacket>)> = Vec::new();
    for mut library_packet in library_work_packets {
        // Not all albums need to be processed each time - this generates only the list
        // of albums that need are either mising or have changed according to the .album.euphony file.
        let mut albums_to_process = library_packet
            .get_albums_in_need_of_processing(config)
            .into_diagnostic()?;
        
        // For convenience (and because why not), we sort the album list for each library alphabetically.
        albums_to_process.sort_unstable_by(
            |first, second|
                first.album_info.album_title.cmp(&second.album_info.album_title)
        );
        
        // (skip albums without any changed/unprocessed files)
        if albums_to_process.len() > 0 {
            filtered_library_packets.push((library_packet, albums_to_process));
        }
    }
    
    // For convenience (and because why not), we sort the libraries alphabetically.
    filtered_library_packets.sort_unstable_by(
        |(first, _), (second, _)|
            first.name.cmp(&second.name)
    );

    // Skip processing if there are no changes,
    // otherwise show a short summary of changes and start transcoding.
    if filtered_library_packets.len() == 0 {
        terminal.log_println("Transcodes are already up to date.".green().bold());
        return Ok(());
    } else {
        // Number of files that need to be processed (copied or transcoded).
        let total_filtered_packets = filtered_library_packets.iter_mut()
            .map(|(_, albums)| albums)
            .flatten()
            .map(|album| album.get_work_packets(config).unwrap_or_default().len())
            .sum::<usize>();
        
        terminal.log_println(
            format!(
                "Detected {} changed files, transcoding.",
                total_filtered_packets.to_string().bold()
            )
        );
        terminal.log_newline();
    }
    
    let mut thread_pool = build_transcode_thread_pool(config);
    
    // Iterate over all available libraries.
    for (_, album_packets) in filtered_library_packets {
        // Iterate over each library' albums that are in need of transcoding.
        for mut album_packet in album_packets {
            terminal.queue_begin();
            terminal.progress_begin();
            
            if verbose_enabled() {
                let fresh_metadata = album_packet.get_fresh_meta(config)
                    .into_diagnostic()?;
                
                terminal.log_println(format!(
                    "[VERBOSE] AlbumWorkPacket album: {:?}; files in meta: {:?}",
                    album_packet.album_info,
                    fresh_metadata.files,
                ));
            }
            
            // TODO Verbose logging per-file.
            
            let file_packets = album_packet
                .get_work_packets(config)
                .into_diagnostic()?;
    
            // Fill up the terminal queue with items.
            let queued_files = file_packets
                .into_iter()
                .map(|file_packet| {
                    let item_info = format!(
                        "[{}] {}",
                        match file_packet.file_type {
                            FilePacketType::AudioFile => "audio",
                            FilePacketType::DataFile => "data",
                        },
                        match file_packet.target_file_path.file_name() {
                            Some(name) => name.to_string_lossy().to_string(),
                            None => "UNKNOWN".into()
                        },
                    );
                    
                    // Maps the original FileWorkPacket to a tuple of `(FileWorkPacket, QueueItemID)`
                    // if adding the item to the queue was successful,
                    // otherwise returns an `Err` with the original error.
                    match terminal.queue_item_add(item_info) {
                        Ok(queue_item_id) => Ok((file_packet, queue_item_id)),
                        Err(error) => Err(error)
                    }
                })
                .collect::<Result<Vec<(FileWorkPacket, QueueItemID)>>>()?;
            
            let (tx, rx) = mpsc::channel::<WorkerMessage>();
    
            let config_clone = config.clone();
            let main_processing_thread_handle = thread::spawn(move || {
                process_album(
                    &queued_files,
                    &config_clone,
                    &thread_pool,
                    tx,
                );
    
                // Return the thread pool back.
                thread_pool
            });
            
            loop {
                // Wait for message from worker threads.
                let message = rx.recv_timeout(Duration::from_millis(50));
                match message {
                    Ok(message) => match message {
                        WorkerMessage::StartingWithFile { queue_item } => {
                            terminal.queue_item_start(queue_item)?;
                        },
                        WorkerMessage::FinishedWithFile { queue_item, was_ok } => {
                            terminal.queue_item_finish(queue_item, was_ok)?;
                        },
                    },
                    Err(error) => {
                        // If the main processing thread stopped, the channel will be disconnected,
                        // in which case we should stop waiting.
                        if error == RecvTimeoutError::Disconnected {
                            break;
                        }
                    }
                };
                
                // Make sure the main processing thread is still alive.
                if main_processing_thread_handle.is_finished() {
                    break;
                }
            }
            
            thread_pool = main_processing_thread_handle.join()
                .expect("Could not join main processing thread.");
            
            // Update the metadata in .album.euphony file, saving details that will ensure
            // they are not needlessly transcoded again next time.
            album_packet.save_fresh_meta(config, true)
                .into_diagnostic()?;
            
            terminal.queue_end();
            terminal.progress_end();
        }
    }
    
    let processing_time_delta = processing_begin_time.elapsed().as_secs_f64();
    terminal.log_println(format!(
        "Transcoding completed in {:.1}",
        processing_time_delta,
    ));
    
    Ok(())
    
    
    // DEPRECATED BELOW

    /*
    //  TODO Upgrade to custom progress implementation.
    // Set up progress bars (three bars, one for current file, another for albums, the third for libraries).
    let multi_pbr = MultiProgress::new();

    let files_progress_bar = multi_pbr.add(ProgressBar::new(0));
    files_progress_bar.set_style(
        build_progress_bar_style_with_header(format!("{:9}", "(file)")),
    );
    files_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let files_progress_bar_ref = Arc::new(Mutex::new(files_progress_bar));

    let albums_progress_bar = multi_pbr.add(ProgressBar::new(0));
    albums_progress_bar.set_style(
        build_progress_bar_style_with_header(format!("{:9}", "(album)")),
    );
    albums_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    let library_progress_bar = multi_pbr.add(ProgressBar::new(filtered_library_packets.len() as u64));
    library_progress_bar.set_style(
        build_progress_bar_style_with_header(format!("{:9}", "(library)")),
    );
    library_progress_bar.enable_steady_tick(DEFAULT_PROGRESS_BAR_TICK_INTERVAL);

    // TODO If the user interrupts a transcode, ask if they want to delete the currently half-transcoded album.

    let set_current_file = build_styled_progress_bar_message_fn_dynamic_locked_bar(
        Style::new().fg(Color256(131)).underlined(),
        42,
    );

    let set_current_album = build_styled_progress_bar_message_fn(
        &albums_progress_bar,
        Style::new().fg(Color256(131)).underlined(),
        42,
    );

    let set_current_library = build_styled_progress_bar_message_fn(
        &library_progress_bar,
        Style::new().white().underlined(),
        42,
    );

    set_current_file("/", &files_progress_bar_ref.lock().unwrap());
    set_current_album("/");
    set_current_library("/");

    let thread_pool = build_transcode_thread_pool(config);
    let observer = build_processing_observer(
        files_progress_bar_ref.clone(),
        Box::new(set_current_file.clone())
    );

    // Iterate over libraries and process each album.
    for (library, album_packets) in filtered_library_packets {
        set_current_library(&library.name);

        albums_progress_bar.reset();
        albums_progress_bar.set_length(album_packets.len() as u64);
        albums_progress_bar.set_position(0);

        for mut album_packet in album_packets {
            set_current_album(&album_packet.album_info.album_title);

            if verbose_enabled() {
                let fresh_meta = album_packet.get_fresh_meta(config)?;
                albums_progress_bar.println(
                    format!(
                        "  [DEBUG] AlbumWorkPacket album: {:?}, files in meta: {:?}",
                        album_packet.album_info,
                        fresh_meta.files,
                    ),
                );
            }

            let file_packets = album_packet.get_work_packets(config)?;

            {
                let fpb_locked = files_progress_bar_ref.lock().unwrap();
                fpb_locked.reset();
                fpb_locked.set_length(file_packets.len() as u64);
                fpb_locked.set_position(0);
            }

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
            albums_progress_bar.inc(1);
        }

        library_progress_bar.inc(1);
    }

    files_progress_bar_ref.lock().unwrap().finish();
    albums_progress_bar.finish();
    library_progress_bar.finish();

    let processing_time_delta = processing_begin_time.elapsed();
    println!(
        "Transcoding completed in {:.1?}.",
        processing_time_delta,
    );

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
     */
}

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
