use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam::channel;
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
use crossterm::style::Stylize;
use directories as dirs;
use miette::{miette, IntoDiagnostic, Result};

use crate::commands::transcode::packets::album::AlbumWorkPacket;
use crate::commands::transcode::packets::file::{
    FilePacketType,
    FileProcessingResult,
    FileWorkPacket,
};
use crate::commands::transcode::packets::library::LibraryWorkPacket;
use crate::commands::transcode::threadpool::CancellableThreadPool;
use crate::configuration::Config;
use crate::console::backends::shared::{QueueItemID, QueueType, SpinnerStyle};
use crate::console::backends::TranscodeTerminal;
use crate::console::{
    LogBackend,
    TranscodeBackend,
    UserControlMessage,
    UserControllableBackend,
};
use crate::globals::is_verbose_enabled;

mod directories;
mod metadata;
mod overrides;
mod packets;
mod threadpool;

/// A "file progress"/"log this to the console" message that worker threads send back to the main thread.
enum WorkerMessage {
    StartingWithFile {
        queue_item: QueueItemID,
    },
    WriteToLog {
        content: String,
    },
    FinishedWithFile {
        queue_item: QueueItemID,
        processing_result: FileProcessingResult,
    },
}

/// A message from the main processing thread to individual worker threads.
/// Currently the only possible message is for the worker threads to stop.
enum MainThreadMessage {
    StopProcessing,
}

/// This function processes an entire album worth of `FileWorkPacket`s. Needs a reference
/// to the current configuration, the `Sender` through which to send `WorkerMessage`s and
/// the `Receiver` through which to receive `MainThreadMessage`s from the main thread.
///
/// Returns `Ok(())` upon completing the processing of the given album.
fn process_album_files(
    album_file_packets: &Vec<(FileWorkPacket, QueueItemID)>,
    config: &Config,
    progress_sender: Sender<WorkerMessage>,
    main_processing_thread_receiver: Receiver<MainThreadMessage>,
) -> Result<()> {
    if album_file_packets.is_empty() {
        return Ok(());
    }

    // Create a new atomic boolean that will indicate the threadpool cancellation status.
    // Then create a fresh cancellable thread pool with a reference to the newly-created atomic bool.
    // Whenever the user presses "q" (wants the program to stop transcoding), we'll get the signal
    // from the `main_processing_thread_receiver` and we'll set `user_cancellation_flag` to true,
    // which will signal to the thread pool to stop.
    let user_cancellation_flag = Arc::new(AtomicBool::new(false));
    let mut thread_pool = CancellableThreadPool::new_with_user_flag(
        config.aggregated_library.transcode_threads,
        user_cancellation_flag.clone(),
        true,
    );

    if is_verbose_enabled() {
        progress_sender
            .send(WorkerMessage::WriteToLog {
                content: format!(
                    "Queueing {} threadpool tasks for this album.",
                    album_file_packets.len(),
                ),
            })
            .into_diagnostic()?;
    }

    // Queue all files in this album into the thread pool.
    // Tasks are actually executed whenever a thread becomes available, in FIFO order.
    for (file, queue_item) in album_file_packets {
        let config_clone = config.clone();
        let file_clone = file.clone();
        let queue_item_clone = *queue_item;
        let update_sender_clone = progress_sender.clone();

        thread_pool.queue_task(
            Some(format!(
                "task-{}",
                file.target_file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .replace(' ', "_")
            )),
            move |cancellation_flag| {
                update_sender_clone
                    .send(WorkerMessage::StartingWithFile {
                        queue_item: queue_item_clone,
                    })
                    .expect(
                        "Could not send message from worker to main thread.",
                    );

                let work_result =
                    file_clone.process(&config_clone, cancellation_flag);

                let cancellation_value =
                    cancellation_flag.load(Ordering::Acquire);
                if cancellation_value {
                    return;
                }

                update_sender_clone
                    .send(WorkerMessage::FinishedWithFile {
                        queue_item: queue_item_clone,
                        processing_result: work_result,
                    })
                    .expect(
                        "Could not send message from worker to main thread.",
                    );
            },
        );
    }

    if is_verbose_enabled() {
        progress_sender.send(WorkerMessage::WriteToLog {
            content: format!(
                "Queued all {} threadpool tasks, waiting for completion or cancellation.",
                album_file_packets.len(),
            ),
        }).into_diagnostic()?;
    }

    // The above loop does not block, because we only queued the tasks.
    // The blocking part of this function is actually this while loop - it keeps waiting for
    // the thread pool to finish, meanwhile waiting for any kind of control messages from the main thread.
    while thread_pool.has_tasks_left() && !thread_pool.is_stopped() {
        // Keep checking for user exit message (and set the cancellation flag when received).
        let potential_main_thread_mesage = main_processing_thread_receiver
            .recv_timeout(Duration::from_millis(20));
        match potential_main_thread_mesage {
            Ok(message) => match message {
                MainThreadMessage::StopProcessing => {
                    break;
                }
            },
            Err(error) => match error {
                RecvTimeoutError::Timeout => {
                    // No action needed, there is simply no message at this time.
                }
                RecvTimeoutError::Disconnected => {
                    panic!("Main thread sender disconnected unexpectedly!");
                }
            },
        }

        if is_verbose_enabled() {
            progress_sender
                .send(WorkerMessage::WriteToLog {
                    content: format!(
                        "ThreadPool status: has_tasks_left={}, is_stopped={}",
                        thread_pool.has_tasks_left(),
                        thread_pool.is_stopped(),
                    ),
                })
                .into_diagnostic()?;
        }
    }

    if is_verbose_enabled() {
        progress_sender.send(WorkerMessage::WriteToLog {
            content: String::from(
                "Threadpool work is finished, setting cancellation flag and waiting for threadpool."
            ),
        }).into_diagnostic()?;
    }

    // This waits for the coordinator thread to finish.
    user_cancellation_flag.store(true, Ordering::Release);

    if is_verbose_enabled() {
        progress_sender.send(WorkerMessage::WriteToLog {
            content: String::from("Cancellation flag manually set, calling join on thread pool."),
        }).into_diagnostic()?;
    }

    let thread_pool_result = thread_pool
        .join()
        .map_err(|error| miette!("Thread pool exited abnormally: {}", error))?;

    if is_verbose_enabled() {
        progress_sender
            .send(WorkerMessage::WriteToLog {
                content: format!(
                    "Threadpool stopped, reason: {thread_pool_result:?}"
                ),
            })
            .into_diagnostic()?;
    }

    Ok(())
}

// TODO Consider reimplementing transcode for specific library and specific album, like in previous versions.

/// This function lists all the albums in all of the libraries that need to be transcoded
/// and performs the transcode using ffmpeg (for audio files) and simple file copy (for data files).
pub fn cmd_transcode_all(
    config: &Config,
    terminal: &mut TranscodeTerminal,
) -> Result<String> {
    let processing_begin_time = Instant::now();

    terminal.log_println(
        "Command: transcode entire collection (skip unchanged)."
            .cyan()
            .bold(),
    );
    terminal.log_println("Scanning albums for changes...");

    // The user may send control messages through the selected backend (such as a stop message).
    // We can receive such messages through this receiver.
    let terminal_user_input = terminal.get_user_control_receiver()?;

    // Generate a list of `LibraryWorkPacket`s for each library and sort the libraries alphabetically.
    let mut all_libraries: Vec<LibraryWorkPacket> = config
        .libraries
        .values()
        .map(|library| LibraryWorkPacket::from_library(config, library))
        .collect::<Result<Vec<LibraryWorkPacket>>>()?;

    all_libraries.sort_unstable_by(|first, second| first.name.cmp(&second.name));


    // We now have a list of all libraries - the next step is generating a complete list
    // of work to be done (all the libraries, albums and individual files that will be
    // transcoded or copied). This skips libraries and their albums that have already
    // been transcoded and have not changed in their entirety.

    // Some utility types are set up here for better readability. The difference between e.g.
    // `AlbumsWorkload` and `AlbumsQueuedWorkload` is the additional `QueueItemID` element which
    // simply the ID of that album (or library) in the queue of the selected terminal backend.
    type AlbumsWorkload<'a> = Vec<(AlbumWorkPacket<'a>, Vec<FileWorkPacket>)>;
    type LibrariesWorkload<'a> =
        Vec<(LibraryWorkPacket<'a>, AlbumsWorkload<'a>)>;

    type AlbumsQueuedWorkload<'a> = Vec<(
        AlbumWorkPacket<'a>,
        QueueItemID,
        Vec<FileWorkPacket>,
    )>;
    type LibrariesQueuedWorkload<'a> = Vec<(
        LibraryWorkPacket<'a>,
        QueueItemID,
        AlbumsQueuedWorkload<'a>,
    )>;

    let mut full_workload: LibrariesWorkload = Vec::new();

    for mut library in all_libraries {
        let mut albums_to_process =
            library.get_albums_in_need_of_processing(config)?;

        // For convenience (and because why not), both libraries and albums are sorted alphabetically.
        albums_to_process.sort_unstable_by(|first, second| {
            first
                .album_info
                .album_title
                .cmp(&second.album_info.album_title)
        });

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

    // Number of files that need to be processed (copied or transcoded) across all libraries and albums.
    let total_files_to_process = full_workload
        .iter_mut()
        .flat_map(|(_, albums)| albums)
        .map(|(_, files)| files.len())
        .sum::<usize>();

    // Skip entire processing stage if there are simply no changes,
    // otherwise show a short summary of changes and start transcoding.
    if full_workload.is_empty() {
        return Ok("Transcodes are already up to date."
            .green()
            .bold()
            .to_string());
    } else {
        terminal.log_println(format!(
            "Detected {} changed files, transcoding.",
            total_files_to_process.to_string().bold()
        ));
    }

    // Details depend on terminal backend implementation, but essentially this enables
    // the queue and progress bar.
    terminal.queue_begin();
    terminal.progress_begin();

    let mut files_finished_so_far: usize = 0;
    terminal.progress_set_current(files_finished_so_far)?;
    terminal.progress_set_total(total_files_to_process)?;

    // Go over the entire workload we generated earlier and enqueue all libraries and albums,
    // returning an expanded vector containing one additional item - their `QueueItemID`s.
    let queued_workload: LibrariesQueuedWorkload = full_workload
        .into_iter()
        .map(|(library, albums)| {
            let library_description = format!(
                "{} ({} album{})",
                library.name,
                albums.len(),
                if albums.len() > 1 { "s" } else { "" }
            );

            let library_queue_item = terminal
                .queue_item_add(library_description, QueueType::Library)?;

            terminal.queue_item_modify(
                library_queue_item,
                Box::new(|item| item.spaces_when_spinner_is_disabled = false),
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
                    let album_queue_item = terminal
                        .queue_item_add(album_description, QueueType::Album)?;

                    Ok((album, album_queue_item, files))
                })
                .collect::<Result<AlbumsQueuedWorkload>>()?;

            Ok((library, library_queue_item, queued_albums))
        })
        .collect::<Result<LibrariesQueuedWorkload>>()?;


    // Finally, iterate over the entire queued workload,
    // transcoding each file in each album and updating the terminal backend on the way.
    for (library, library_queue_item, albums) in queued_workload {
        let time_library_start = Instant::now();

        terminal.queue_item_start(library_queue_item)?;
        terminal.queue_item_modify(
            library_queue_item,
            Box::new(|item| item.set_suffix(" [active]")),
        )?;

        terminal.log_println(format!(
            "Transcoding contents of library: {} ({} albums)",
            library.name.clone().bold(),
            albums.len(),
        ));

        // Transcode each album in this library.
        for (mut album, album_queue_id, files) in albums {
            let time_album_start = Instant::now();

            terminal.queue_item_start(album_queue_id)?;
            terminal.queue_item_modify(
                album_queue_id,
                Box::new(|item| {
                    item.clear_prefix();
                    item.enable_spinner(SpinnerStyle::Square, None);
                }),
            )?;

            terminal.log_println(format!(
                "|-> Transcoding album: {} ({} files)",
                format!(
                    "{} - {}",
                    album.album_info.artist_name, album.album_info.album_title,
                )
                .underlined(),
                files.len(),
            ));

            if is_verbose_enabled() {
                let fresh_metadata = album.get_fresh_meta()?;

                terminal.log_println(format!(
                    "[VERBOSE] AlbumWorkPacket album: {:?}; files in meta: {:?}",
                    album.album_info, fresh_metadata.files,
                ));
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
                    match terminal
                        .queue_item_add(item_description, QueueType::File)
                    {
                        Ok(queue_item_id) => Ok((file, queue_item_id)),
                        Err(error) => Err(error),
                    }
                })
                .collect::<Result<Vec<(FileWorkPacket, QueueItemID)>>>()?;

            let (worker_progress_tx, worker_progress_rx) =
                channel::unbounded::<WorkerMessage>();
            let (worker_ctrl_tx, worker_ctrl_rx) =
                channel::unbounded::<MainThreadMessage>();

            // Spawn a processing thread to avoid blocking.
            let config_thread_clone = config.clone();
            let processing_thread_handle = thread::spawn(move || {
                process_album_files(
                    &queued_files,
                    &config_thread_clone,
                    worker_progress_tx,
                    worker_ctrl_rx,
                )
            });

            // Wait for processing thread to complete. Meanwhile, keep receiving progress messages
            // from the processing thread and update the terminal backend accordingly.
            let mut exit_requested: bool = false;
            loop {
                // Periodically receive file progress from worker threads and update the terminal accordingly.
                let worker_message =
                    worker_progress_rx.recv_timeout(Duration::from_millis(1));
                match worker_message {
                    Ok(message) => match message {
                        WorkerMessage::StartingWithFile { queue_item } => {
                            terminal.queue_item_start(queue_item)?;
                            terminal.queue_item_modify(
                                queue_item,
                                Box::new(|item| {
                                    item.clear_prefix();
                                    item.enable_spinner(
                                        SpinnerStyle::Pixel,
                                        None,
                                    );
                                }),
                            )?;
                        }
                        WorkerMessage::WriteToLog { content } => {
                            terminal.log_println(content);
                        }
                        WorkerMessage::FinishedWithFile {
                            queue_item,
                            processing_result,
                        } => {
                            terminal.queue_item_finish(
                                queue_item,
                                processing_result.is_ok(),
                            )?;
                            terminal.queue_item_modify(
                                queue_item,
                                Box::new(|item| item.disable_spinner()),
                            )?;

                            if is_verbose_enabled() {
                                terminal.log_println(format!(
                                    "[VERBOSE] File finished, result: {processing_result:?}",
                                ));
                            }

                            // Update progress bar with new percentage.
                            files_finished_so_far += 1;
                            terminal
                                .progress_set_current(files_finished_so_far)?;

                            if !processing_result.is_ok() {
                                // File errored, stop transcoding.

                                // Eventually an implementation with retrying and such will be done,
                                // but that's in `file.rs`.
                                return Err(miette!(
                                    "File {} failed while processing:\n{}",
                                    processing_result
                                        .file_work_packet
                                        .target_file_path
                                        .file_name()
                                        .map(|file_name| file_name
                                            .to_string_lossy()
                                            .to_string())
                                        .unwrap_or_else(|| String::from(
                                            "UNKNOWN"
                                        )),
                                    processing_result.error.unwrap()
                                ));
                            }
                        }
                    },
                    Err(error) => {
                        // If the main processing thread stopped, the channel will be disconnected,
                        // in which case we should stop waiting.
                        if error == RecvTimeoutError::Disconnected {
                            if is_verbose_enabled() {
                                terminal.log_println("Exiting infinite processing wait: processing thread dropped sender.");
                            }

                            break;
                        }
                    }
                }

                // TODO Add additional user input, such as a way to view an expanded log view, etc.
                // Periodically receive user control messages, such as the exit command.
                let user_control_message =
                    terminal_user_input.recv_timeout(Duration::from_millis(1));
                if let Ok(message) = user_control_message {
                    match message {
                        UserControlMessage::Exit => {
                            exit_requested = true;
                            worker_ctrl_tx
                                .send(MainThreadMessage::StopProcessing)
                                .into_diagnostic()?;

                            if is_verbose_enabled() {
                                terminal.log_println("Exiting infinite processing wait: user requested exit.");
                            }

                            break;
                        }
                    }
                };


                // Make sure the main processing thread is still alive.
                if processing_thread_handle.is_finished() {
                    if is_verbose_enabled() {
                        terminal.log_println("Exiting infinite processing wait: process_album thread has finished.");
                    }

                    break;
                }
            }

            if is_verbose_enabled() {
                terminal.log_println(
                    "Waiting for process_album thread to finish (calling join).",
                );
            }

            processing_thread_handle
                .join()
                .expect("Processing thread panicked!")?;

            if exit_requested {
                // Exited mid-processing at user request.

                // TODO Implement deletion of partial transcodes (e.g. when the user cancels transcoding).

                terminal.log_println(
                    format!(
                        "NOTE: A half-transcoded album ({} - {}) has potentially been left behind \
                        at the target directory - clean it up before running again \
                        (deletion of partial transcodes is not yet implemented).",
                        album.album_info.artist_name,
                        album.album_info.album_title,
                    ),
                );

                return Err(miette!(
                    "Stopped mid-transcoding at user request!"
                ));
            } else {
                // Update the metadata in .album.euphony file, saving details that will ensure
                // they are not needlessly transcoded again next time.
                album.save_fresh_meta(true)?;

                terminal.queue_item_finish(album_queue_id, true)?;
                terminal.queue_item_modify(
                    album_queue_id,
                    Box::new(|item| {
                        item.spaces_when_spinner_is_disabled = false;
                        item.disable_spinner();
                        item.set_prefix(" ☑ ");
                    }),
                )?;
                terminal.queue_clear(QueueType::File)?;

                let time_album_elapsed =
                    time_album_start.elapsed().as_secs_f64();

                terminal.log_println(format!(
                    "|-> Album {} transcoded in {:.2} seconds.",
                    format!(
                        "{} - {}",
                        album.album_info.artist_name,
                        album.album_info.album_title,
                    )
                    .underlined(),
                    time_album_elapsed,
                ));
            }
        }

        terminal.queue_item_finish(library_queue_item, true)?;
        terminal.queue_item_modify(
            library_queue_item,
            Box::new(|item| item.clear_suffix()),
        )?;

        let time_library_elapsed = time_library_start.elapsed().as_secs_f64();

        terminal.log_println(format!(
            "|-> Library {} transcoded in {:.2} seconds.",
            library.name.clone().bold(),
            time_library_elapsed
        ));
    }

    let processing_time_delta = processing_begin_time.elapsed().as_secs_f64();

    Ok(format!(
        "Full library transcoding completed in {processing_time_delta:.2} seconds.",
    ))
}
