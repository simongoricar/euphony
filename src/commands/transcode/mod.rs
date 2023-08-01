use std::thread;
use std::time::{Duration, Instant};

use crossbeam::channel;
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
use crossterm::style::Stylize;
use miette::{miette, Context, IntoDiagnostic, Result};

use crate::commands::transcode::album_state::{AlbumFileChangesV2, FileType};
use crate::commands::transcode::jobs::{
    CancellableThreadPoolV2,
    FileJobMessage,
    FileJobResult,
};
use crate::commands::transcode::views::{
    LibraryView,
    SharedAlbumView,
    SharedArtistView,
    SharedLibraryView,
};
use crate::configuration::Config;
use crate::console::backends::shared::queue::{
    AlbumQueueItem,
    AlbumQueueItemFinishedResult,
    FileQueueItem,
    FileQueueItemErrorType,
    FileQueueItemFinishedResult,
    FileQueueItemType,
    QueueItemID,
};
use crate::console::backends::TranscodeTerminal;
use crate::console::{
    LogBackend,
    TranscodeBackend,
    UserControlMessage,
    UserControllableBackend,
};
use crate::globals::is_verbose_enabled;

pub mod album_configuration;
pub mod album_state;
pub mod jobs;
pub mod views;

type SortedLibrariesWithChanges<'a> = Vec<(
    SharedLibraryView<'a>,
    SortedArtistsWithChanges<'a>,
)>;
type SortedArtistsWithChanges<'a> = Vec<(
    String,
    SharedArtistView<'a>,
    SortedAlbumsWithChanges<'a>,
)>;
type SortedAlbumsWithChanges<'a> = Vec<(
    String,
    SharedAlbumView<'a>,
    AlbumFileChangesV2<'a>,
)>;

type QueuedChangedLibraries<'a> =
    Vec<(SharedLibraryView<'a>, QueuedChangedAlbums<'a>)>;
type QueuedChangedAlbums<'a> = Vec<(
    SharedAlbumView<'a>,
    QueueItemID,
    AlbumFileChangesV2<'a>,
)>;

pub fn cmd_transcode_all<'config: 'scope, 'scope, 'scope_env: 'scope_env>(
    configuration: &'config Config,
    terminal: &TranscodeTerminal<'config, 'scope>,
) -> Result<()> {
    let time_full_processing_start = Instant::now();

    terminal.log_println(
        "Command: transcode entire collection (skip unchanged)."
            .cyan()
            .bold(),
    );
    terminal.log_println("Scanning albums for changes...");

    // The user may send control messages via the selected backend (such as an abort message).
    // We can receive such messages through this receiver.
    // The tui (fancy) backend for example implements the "q" keybind
    // that sends UserControlMessage::Exit.
    let mut terminal_user_input = terminal.get_user_control_receiver()?;


    let libraries_with_changes =
        collect_libraries_with_changes(configuration, terminal)?;
    // It is possible that no changes have been detected, in which case we should just exit.
    if libraries_with_changes.is_empty() {
        terminal.log_println(
            "All albums are up to date, no transcoding needed."
                .green()
                .bold(),
        );
        return Ok(());
    }

    let total_changed_files = libraries_with_changes
        .iter()
        .flat_map(|(_, artist_to_changed_albums)| artist_to_changed_albums)
        .flat_map(|(_, _, changed_albums)| changed_albums)
        .map(|(_, _, album_changes)| album_changes.number_of_changed_files())
        .sum::<usize>();

    terminal.log_println(format!(
        "Detected {} changed files, queueing and processing.",
        total_changed_files.to_string().bold()
    ));


    // Queue the entire workload - this way we'll generate `QueueItemID`s
    // for each item, enabling us to interact with the terminal backend
    // and display individual album and file progress.
    terminal.queue_album_enable();
    terminal.queue_file_enable();
    terminal.progress_enable();

    let queued_work_per_library =
        queue_all_libraries_with_changes(terminal, libraries_with_changes)?;

    let mut audio_files_currently_processing: usize = 0;
    let mut data_files_currently_processing: usize = 0;
    let mut audio_files_finished_ok: usize = 0;
    let mut data_files_finished_ok: usize = 0;
    let mut audio_files_errored: usize = 0;
    let mut data_files_errored: usize = 0;

    terminal.progress_set_audio_files_currently_processing(
        audio_files_currently_processing,
    )?;
    terminal.progress_set_data_files_currently_processing(
        data_files_currently_processing,
    )?;
    terminal.progress_set_audio_files_finished_ok(audio_files_finished_ok)?;
    terminal.progress_set_data_files_finished_ok(data_files_finished_ok)?;
    terminal.progress_set_audio_files_errored(audio_files_errored)?;
    terminal.progress_set_data_files_errored(data_files_errored)?;

    terminal.progress_set_total(total_changed_files)?;

    for (album, album_queue_id, album_changes) in queued_work_per_library
        .into_iter()
        .flat_map(|(_, albums)| albums)
    {
        let time_album_start = Instant::now();

        let (album_artist_name, album_title, album_library_name) = {
            let album_locked = album.read();
            let artist_locked = album_locked.read_lock_artist();
            let library_locked = artist_locked.read_lock_library();

            (
                artist_locked.name.clone(),
                album_locked.title.clone(),
                library_locked.name(),
            )
        };

        terminal.queue_album_item_start(album_queue_id)?;
        terminal.log_println(format!(
            "↳ Transcoding album\
            \n    {album_artist_name} - {album_title}\
            \n    Library: {album_library_name}"
        ));

        if is_verbose_enabled() {
            terminal.log_println(format!("Album changes: {:?}", album_changes));
        }

        // TODO A percentage of storage saved after each file finishes would be cool
        //      (but we'd need a way to display that).

        let (worker_tx, worker_rx) = channel::unbounded::<FileJobMessage>();
        let (processing_control_tx, processing_control_rx) =
            channel::unbounded::<MainThreadMessage>();

        let mut exit_was_requested = false;

        thread::scope::<'_, _, Result<()>>(|scope| {
            let processing_handle = scope.spawn(|| {
                process_album_changes(
                    terminal,
                    album.clone(),
                    &album_changes,
                    worker_tx,
                    processing_control_rx,
                )
            });

            // Above, we spawned a thread that will process the album. In the meantime, we should
            // keep monitoring the progress (parsing `FileJobMessage`s from the worker threads)
            // and monitoring the user input (in case of a cancellation request from `terminal_user_input`
            // we forward that request via `processing_control_tx`).
            loop {
                // Check for worker thread progress.
                let worker_message =
                    worker_rx.recv_timeout(Duration::from_millis(1));
                match worker_message {
                    Ok(message) => {
                        match message {
                            FileJobMessage::Starting {
                                queue_item,
                                file_type,
                                ..
                            } => {
                                terminal.queue_file_item_start(queue_item)?;

                                if file_type == FileType::Audio {
                                    audio_files_currently_processing += 1;
                                } else if file_type == FileType::Data
                                    || file_type == FileType::Unknown
                                {
                                    data_files_currently_processing += 1;
                                }

                                terminal.progress_set_audio_files_currently_processing(audio_files_currently_processing)?;
                                terminal.progress_set_data_files_currently_processing(data_files_currently_processing)?;
                            }
                            FileJobMessage::Finished {
                                queue_item,
                                file_type,
                                processing_result,
                                file_path,
                            } => {
                                if is_verbose_enabled() {
                                    terminal.log_println(format!(
                                        "File finished: {file_path} ({file_type:?}) result={processing_result:?}"
                                    ));
                                }

                                if file_type == FileType::Audio {
                                    audio_files_currently_processing -= 1;
                                } else if file_type == FileType::Data
                                    || file_type == FileType::Unknown
                                {
                                    data_files_currently_processing -= 1;
                                }

                                terminal.progress_set_audio_files_currently_processing(audio_files_currently_processing)?;
                                terminal.progress_set_data_files_currently_processing(data_files_currently_processing)?;


                                // TODO Missing verbosity print (Okay and Errored contain verbose info).
                                let item_result = match processing_result {
                                    FileJobResult::Okay { .. } => {
                                        match file_type {
                                            FileType::Audio => {
                                                audio_files_finished_ok += 1;
                                                terminal.progress_set_audio_files_finished_ok(
                                                    audio_files_finished_ok,
                                                )?;
                                            }
                                            FileType::Data => {
                                                data_files_finished_ok += 1;
                                                terminal.progress_set_data_files_finished_ok(
                                                    data_files_finished_ok,
                                                )?;
                                            }
                                            FileType::Unknown => {
                                                terminal.log_println("Developer WARNING: unexpected OK FileType::Unknown.");
                                            }
                                        };

                                        FileQueueItemFinishedResult::Ok
                                    }
                                    FileJobResult::Errored { error, .. } => {
                                        match file_type {
                                            FileType::Audio => {
                                                audio_files_errored += 1;
                                                terminal.progress_set_audio_files_errored(
                                                    audio_files_errored,
                                                )?;
                                            }
                                            FileType::Data => {
                                                data_files_errored += 1;
                                                terminal.progress_set_data_files_errored(
                                                    data_files_errored,
                                                )?;
                                            }
                                            FileType::Unknown => {
                                                terminal.log_println("Developer WARNING: unexpected ERR FileType::Unknown.");
                                            }
                                        };

                                        FileQueueItemFinishedResult::Failed(
                                            FileQueueItemErrorType::Errored {
                                                error,
                                            },
                                        )
                                    }
                                };

                                terminal.queue_file_item_finish(
                                    queue_item,
                                    item_result,
                                )?;

                                // TODO How to handle errored files? Previous implementation
                                //      returned an `Err`, but that's a bit extreme, no?
                            }
                            FileJobMessage::Cancelled { queue_item, .. } => {
                                let item_result =
                                    FileQueueItemFinishedResult::Failed(
                                        FileQueueItemErrorType::Cancelled,
                                    );

                                terminal.queue_file_item_finish(
                                    queue_item,
                                    item_result,
                                )?;
                            }
                            FileJobMessage::Log { content } => {
                                terminal.log_println(content);
                            }
                        }
                    }
                    Err(error) => {
                        if error == RecvTimeoutError::Disconnected {
                            // This happens when the sender (i.e. the album processing thread) stops.
                            // This is simply another indicator that the processing has been finished.
                            break;
                        }
                    }
                }

                // Check for user input from the terminal backend.
                // We ignore any disconnects intentionally (we shouldn't error over that, I think).
                if let Ok(message) = terminal_user_input.try_recv() {
                    match message {
                        UserControlMessage::Exit if !exit_was_requested => {
                            exit_was_requested = true;

                            terminal.log_println(
                                "User requested exit, stopping transcode.",
                            );

                            processing_control_tx
                                .send(MainThreadMessage::StopProcessing)
                                .into_diagnostic()?;
                        }
                        _ => {}
                    }
                }

                // If processing is finished, stop the loop (we finished processing the album).
                if processing_handle.is_finished() {
                    break;
                }
            }

            processing_handle
                .join()
                .expect("Processing thread panicked.")
        })?;

        if exit_was_requested {
            // TODO Implement deletion of partial transcodes and similar rollback mechanisms.
            // Processing was aborted by user.
            let album_locked = album.read();

            terminal.log_println(
                format!(
                    "NOTE: A half-transcoded album ({} - {}) has potentially been left behind \
                     at the target directory - clean it up before running again \
                     (deletion of partial transcodes is not yet implemented).",
                    album_locked.read_lock_artist().name, album_locked.title,
                )
            );

            return Err(miette!(
                "Aborted album transcoding at user request."
            ));
        }

        // The entire album is now up-to-date, so we generate two structs and save them into two files:
        // - `.album.source-state.euphony` is saved in the source album directory and contains all
        //   the metadata about tracked files.
        // - `.album.transcode-state.euphony` is saved in the transcoded album directory and contains
        //   a mapping from transcoded files back to their originals
        //   as well as metadata of the tracked transcoded files.
        let source_file_state = album_changes.generate_source_album_state()?;
        let transcoded_file_state =
            album_changes.generate_transcoded_album_state()?;

        {
            let album_locked = album.read();

            source_file_state.save_to_directory(
                album_locked.album_directory_in_source_library(),
                true,
            )?;
            transcoded_file_state.save_to_directory(
                album_locked.album_directory_in_transcoded_library(),
                true,
            )?;
        }


        terminal.queue_album_item_finish(
            album_queue_id,
            AlbumQueueItemFinishedResult::new_ok(),
        )?;
        terminal.queue_file_clear()?;

        let time_album_elapsed = time_album_start.elapsed().as_secs_f64();
        terminal.log_println(format!(
            "  Album transcoded in {time_album_elapsed:.2} seconds."
        ));
    }

    let time_full_processing_elapsed =
        time_full_processing_start.elapsed().as_secs_f64();

    terminal.log_println(format!(
        "Full transcoding completed in {time_full_processing_elapsed:.2} seconds."
    ));

    Ok(())
}

/*
 * Utility functions
 */

fn collect_libraries_with_changes<'config>(
    configuration: &'config Config,
    terminal: &TranscodeTerminal<'config, '_>,
) -> Result<SortedLibrariesWithChanges<'config>> {
    // `LibraryView` is the root abstraction here - we use it to discover artists and their albums.
    let mut libraries: Vec<SharedLibraryView<'config>> = configuration
        .libraries
        .values()
        .map(|library| {
            LibraryView::<'config>::from_library_configuration(
                configuration,
                library,
            )
        })
        .collect();

    libraries.sort_unstable_by(|first, second| {
        let first_locked = first.read();
        let second_locked = second.read();

        first_locked.name().cmp(&second_locked.name())
    });

    if is_verbose_enabled() {
        terminal.log_println(format!(
            "Collected libraries: {:?}",
            libraries
                .iter()
                .map(|library| library.read().name())
                .collect::<Vec<String>>(),
        ));
    }

    // We perform a scan on each library: for each artist in the library, we scan each
    // of their albums for changes (this includes untranscoded albums in addition to
    // albums changed since last transcode). This is a relatively expensive step (a lot of disk accesses),
    // but we will now have all the work we need to perform.
    libraries
        .into_iter()
        .filter_map(|library| {
            let library_read = library.read();

            if is_verbose_enabled() {
                terminal.log_println(format!(
                    "Scanning changes in library: {}", library_read.name(),
                ));
            }

            let scan = match library_read
                .scan_for_artists_with_changed_albums() {
                Ok(scan) => scan,
                Err(error) => {
                    return Some(Err(error))
                }
            };

            if is_verbose_enabled() {
                terminal.log_println(format!(
                    "Changed artists: {:?}",
                    scan.iter().map(|(artist_name, (_, changed_albums))| {
                        let changed_albums_formatted = changed_albums.iter()
                            .map(|(album_title, (_, changes))| {
                                format!("album={album_title};changes={changes:?}")
                            })
                            .collect::<Vec<String>>();

                        format!(
                            "artist={artist_name};changed_albums={changed_albums_formatted:?}"
                        )
                    })
                        .collect::<Vec<String>>(),
                ));
            }

            if scan.is_empty() {
                return None;
            }

            let mut ordered_artists: SortedArtistsWithChanges = scan
                .into_iter()
                .map(
                    |(artist_name, (artist_view, changed_albums_map))| {
                        let mut ordered_albums: SortedAlbumsWithChanges =
                            changed_albums_map
                                .into_iter()
                                .map(|(album_title, (album_view, changes))| {
                                    (album_title, album_view, changes)
                                })
                                .collect();

                        ordered_albums.sort_unstable_by(
                            |(first_album_title, _, _), (second_album_title, _, _)| {
                                first_album_title.cmp(second_album_title)
                            }
                        );

                        (artist_name, artist_view, ordered_albums)
                    },
                )
                .collect();

            ordered_artists.sort_unstable_by(|(first, _, _), (second, _, _)| {
                first.cmp(second)
            });

            drop(library_read);

            Some(Ok((library, ordered_artists)))
        })
        .collect::<Result<SortedLibrariesWithChanges<'config>>>()
}

fn queue_all_libraries_with_changes<'config: 'scope, 'scope>(
    terminal: &TranscodeTerminal<'config, 'scope>,
    libraries_with_changes: SortedLibrariesWithChanges<'config>,
) -> Result<QueuedChangedLibraries<'config>> {
    // Queue all libraries and all the changed albums inside it.
    let mut queued_work_per_library: QueuedChangedLibraries<'config> =
        Vec::with_capacity(libraries_with_changes.len());

    for (library, artists) in libraries_with_changes {
        let changed_albums_count = artists
            .iter()
            .map(|(_, _, changed_albums)| changed_albums.len())
            .sum::<usize>();

        let mut library_queue: QueuedChangedAlbums =
            Vec::with_capacity(changed_albums_count);


        // Queue all albums for each artist in this library.
        let all_albums_in_library = artists
            .into_iter()
            .flat_map(|(_, _, changed_albums)| changed_albums)
            .map(|(_, view, changes)| (view, changes));

        for (album_view, changes) in all_albums_in_library {
            let queued_album_item_id =
                terminal.queue_album_item_add(AlbumQueueItem::new(
                    album_view.clone(),
                    changes.number_of_changed_audio_files(),
                    changes.number_of_changed_data_files(),
                ))?;

            library_queue.push((
                album_view.clone(),
                queued_album_item_id,
                changes,
            ));
        }

        queued_work_per_library.push((library, library_queue));
    }

    Ok(queued_work_per_library)
}


/// A message type to send from the main processing thread to `process_album_changes`.
/// Currently the only possible message is for the worker threads to stop.
enum MainThreadMessage {
    StopProcessing,
}

/// Process an entire album (given its `AlbumFileChangesV2`).
///
/// `worker_progress_sender` is the `Sender` part of a channel that individual file workers
/// can use to send `FileJobMessage`s back to the main thread.
///
/// `main_thread_receiver` is the `Receiver` part of a channel that the main thread can use
/// to signal `MainThreadMessage`s (currently just an "abort processing" message).
///
/// This function returns with `Ok(())` when the album has been processed.
fn process_album_changes<'config>(
    terminal: &TranscodeTerminal<'config, '_>,
    album: SharedAlbumView<'config>,
    changes: &AlbumFileChangesV2,
    worker_progress_sender: Sender<FileJobMessage>,
    main_thread_receiver: Receiver<MainThreadMessage>,
) -> Result<()> {
    let thread_pool_size = {
        let album_locked = album.read();

        album_locked
            .euphony_configuration()
            .aggregated_library
            .transcode_threads
    };

    // TODO Missing verbose messages.

    let mut thread_pool =
        CancellableThreadPoolV2::new(thread_pool_size, worker_progress_sender);
    thread_pool.start()?;

    // Generate and queue all file jobs.
    let jobs = changes.generate_file_jobs(|file_type, file_path| {
        // Parse queue item details.
        let file_item_type = match file_type {
            FileType::Audio => FileQueueItemType::Audio,
            FileType::Data => FileQueueItemType::Data,
            FileType::Unknown => FileQueueItemType::Unknown,
        };

        let file_name =
            file_path.file_name().unwrap_or_default().to_string_lossy();

        // Instantiate `FileItem` and add to queue.
        let file_item = FileQueueItem::<'config>::new(
            album.clone(),
            file_item_type,
            file_name.to_string(),
        );

        let queued_file_item_id = terminal.queue_file_item_add(file_item)?;

        Ok(queued_file_item_id)
    })?;

    // Could flatten this into `generate_file_jobs`, but this is cleaner.
    for job in jobs {
        // This does not block! The thread pool has an internal job queue.
        thread_pool.queue_task(job);
    }

    // All jobs have been queued, now we wait for tasks to complete.
    while thread_pool.has_tasks_left() && thread_pool.is_running() {
        // Keep checking for a user exit message.
        let potential_main_thread_message =
            main_thread_receiver.recv_timeout(Duration::from_millis(20));

        match potential_main_thread_message {
            Ok(message) => match message {
                MainThreadMessage::StopProcessing => {
                    // Any exit from this while loop will mean the
                    // cancellation flag will be set to true, so a break is enough.
                    break;
                }
            },
            Err(error) => {
                if error == RecvTimeoutError::Disconnected {
                    panic!("Main thread receiver disconnected unexpectedly!?");
                }
            }
        }
    }

    // This point is reached on two occasions:
    // - thread pool jobs finished normally, in which case the following will barely block,
    // - main thread requested cancellation, in which case we're probably going to have to wait for the workers a bit.
    thread_pool
        .set_cancellation_and_join()
        .wrap_err_with(|| miette!("Thread pool exited abnormally."))?;

    Ok(())
}
