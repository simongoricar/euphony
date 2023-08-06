use std::collections::{HashMap, HashSet};
use std::ops::Sub;
use std::time::{Duration, Instant};
use std::{fs, thread};

use crossbeam::channel;
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
use crossterm::style::Stylize;
use miette::{miette, Context, IntoDiagnostic, Result};

use crate::commands::transcode::album_state::changes::{
    AlbumFileChangesV2,
    FileType,
};
use crate::commands::transcode::album_state::transcoded::TranscodedAlbumState;
use crate::commands::transcode::jobs::common::FileJobMessage;
use crate::commands::transcode::jobs::{CancellableThreadPool, FileJobResult};
use crate::commands::transcode::library_state::{
    LibraryState,
    LibraryStateLoadError,
    TrackedAlbum,
    TrackedArtistAlbums,
    LIBRARY_STATE_FILE_NAME,
};
use crate::commands::transcode::views::library::LibraryViewError;
use crate::commands::transcode::views::{
    AlbumView,
    ArtistView,
    LibraryView,
    SharedAlbumView,
    SharedArtistView,
    SharedLibraryView,
};
use crate::configuration::Config;
use crate::console::frontends::shared::queue::{
    AlbumQueueItem,
    AlbumQueueItemFinishedResult,
    FileQueueItem,
    FileQueueItemErrorType,
    FileQueueItemFinishedResult,
    QueueItemID,
};
use crate::console::frontends::TranscodeTerminal;
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
mod library_state;
mod utilities;
pub mod views;


pub struct GlobalProgress {
    pub audio_files_currently_processing: usize,

    pub data_files_currently_processing: usize,

    pub audio_files_finished_ok: usize,

    pub data_files_finished_ok: usize,

    pub audio_files_errored: usize,

    pub data_files_errored: usize,
}


fn process_album<'config>(
    queued_album: QueuedAlbum<'config>,
    progress: &mut GlobalProgress,
    terminal: &TranscodeTerminal<'config, '_>,
    terminal_user_input_receiver: &mut tokio::sync::broadcast::Receiver<
        UserControlMessage,
    >,
) -> Result<()> {
    // TODO A percentage of storage saved after each file finishes would be cool.
    let time_album_start = Instant::now();

    let (album_artist_name, album_title, album_library_name) = {
        let album_view = queued_album.album.read();
        let artist_view = album_view.read_lock_artist();
        let library_view = artist_view.read_lock_library();

        (
            artist_view.name.clone(),
            album_view.title.clone(),
            library_view.name(),
        )
    };

    terminal.queue_album_item_start(queued_album.queue_id)?;
    terminal.log_println(format!(
        "↳ Transcoding album \"{album_artist_name} - {album_title}\" (library: {album_library_name})"
    ));

    if is_verbose_enabled() {
        terminal.log_println(format!(
            "Album changes: {:?}",
            queued_album.changes
        ));
    }

    let (worker_tx, worker_rx) = channel::unbounded::<FileJobMessage>();
    let (processing_control_tx, processing_control_rx) =
        channel::unbounded::<MainThreadMessage>();

    let mut user_requested_cancellation = false;

    thread::scope::<'_, _, Result<()>>(|scope| {
        // Spawn a thread that will manage the following:
        // - initialize the thread pool
        // - spawn workers that will work on jobs generated from `queued_album.changes`,
        // - fill up the file queue on the terminal frontend.
        let processing_thread_handle = scope.spawn(|| {
            process_changes(
                &queued_album.changes,
                queued_album.album.clone(),
                terminal,
                worker_tx,
                processing_control_rx,
            )
        });

        // We didn't block on `process_album_changes` above because we want to be able to receive two things:
        // - user input messages through `terminal_user_input_receiver` and
        // - album transcode/copy/delete progress through `worker_rx`.
        // Not blocking above means we keep doing the above things until the processing thread says
        // it's done with the album or until some other fail state (such as cancellation).
        loop {
            // Check and handle job progress.
            let worker_job_message =
                worker_rx.recv_timeout(Duration::from_millis(1));

            match worker_job_message {
                Ok(job_message) => match job_message {
                    FileJobMessage::Starting {
                        queue_item,
                        file_type,
                        file_path,
                    } => {
                        if is_verbose_enabled() {
                            terminal.log_println(format!(
                                "File starting: {file_path} ({file_type:?})"
                            ));
                        }

                        terminal.queue_file_item_start(queue_item)?;

                        match file_type {
                            FileType::Audio => {
                                progress.audio_files_currently_processing += 1;
                            }
                            FileType::Data | FileType::Unknown => {
                                progress.data_files_currently_processing += 1;
                            }
                        }

                        terminal.progress_set_audio_files_currently_processing(
                            progress.audio_files_currently_processing,
                        )?;
                        terminal.progress_set_data_files_currently_processing(
                            progress.data_files_currently_processing,
                        )?;
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

                        match file_type {
                            FileType::Audio => {
                                progress.audio_files_currently_processing -= 1;
                            }
                            FileType::Data | FileType::Unknown => {
                                progress.data_files_currently_processing -= 1;
                            }
                        }

                        terminal.progress_set_audio_files_currently_processing(
                            progress.audio_files_currently_processing,
                        )?;
                        terminal.progress_set_data_files_currently_processing(
                            progress.data_files_currently_processing,
                        )?;

                        let item_result = match processing_result {
                            FileJobResult::Okay { verbose_info } => {
                                if let Some(verbose_info) = verbose_info {
                                    if is_verbose_enabled() {
                                        terminal.log_println(verbose_info);
                                    }
                                }

                                match file_type {
                                    FileType::Audio => {
                                        progress.audio_files_finished_ok += 1;
                                        terminal.progress_set_audio_files_finished_ok(progress.audio_files_finished_ok)?;
                                    }
                                    FileType::Data => {
                                        progress.data_files_finished_ok += 1;
                                        terminal
                                            .progress_set_data_files_finished_ok(
                                                progress.data_files_finished_ok,
                                            )?;
                                    }
                                    FileType::Unknown => {
                                        terminal.log_println("REPORT THIS BUG: Unexpected OK FileType::Unknown!");
                                    }
                                }

                                FileQueueItemFinishedResult::Ok
                            }
                            FileJobResult::Errored {
                                error,
                                verbose_info,
                            } => {
                                if let Some(verbose_info) = verbose_info {
                                    if is_verbose_enabled() {
                                        terminal.log_println(verbose_info);
                                    }
                                }

                                match file_type {
                                    FileType::Audio => {
                                        progress.audio_files_errored += 1;
                                        terminal
                                            .progress_set_audio_files_errored(
                                                progress.audio_files_errored,
                                            )?;
                                    }
                                    FileType::Data => {
                                        progress.data_files_errored += 1;
                                        terminal
                                            .progress_set_data_files_errored(
                                                progress.data_files_errored,
                                            )?;
                                    }
                                    FileType::Unknown => {
                                        terminal.log_println("REPORT THIS BUG: Unexpected ERR FileType::Unknown!");
                                    }
                                };

                                FileQueueItemFinishedResult::Failed(
                                    FileQueueItemErrorType::Errored { error },
                                )
                            }
                        };

                        // TODO How should I handle errored files? Previous implementation
                        //      returned an `Err`, but that's a bit extreme, no?
                        terminal
                            .queue_file_item_finish(queue_item, item_result)?;
                    }
                    FileJobMessage::Cancelled { queue_item, .. } => {
                        let item_result = FileQueueItemFinishedResult::Failed(
                            FileQueueItemErrorType::Cancelled,
                        );

                        terminal
                            .queue_file_item_finish(queue_item, item_result)?;
                    }
                    FileJobMessage::Log { content } => {
                        terminal.log_println(content);
                    }
                },
                Err(error) => {
                    if error == RecvTimeoutError::Disconnected {
                        // This happens when the sender (i.e. the processing thread) drops the
                        // sender handle (i.e. stops).
                        // This error indicates that processing has (or is about to) finish.
                        break;
                    }
                }
            }


            // Check and handle user input from the terminal frontend.
            let user_input = terminal_user_input_receiver.try_recv();

            if let Ok(user_input) = user_input {
                match user_input {
                    UserControlMessage::Exit if !user_requested_cancellation => {
                        user_requested_cancellation = true;

                        terminal.log_println(
                            "User wants to exit, cancelling transcode.",
                        );

                        processing_control_tx
                            .send(MainThreadMessage::StopProcessing)
                            .into_diagnostic()?;
                    }
                    _ => {}
                }
            }


            // Finally, if the processing thread has finished, we should stop.
            if processing_thread_handle.is_finished() {
                break;
            }
        }


        // Wait for processing thread to fully finish.
        processing_thread_handle
            .join()
            .expect("Album processing thread panicked.")
    })?;


    if user_requested_cancellation {
        // TODO Implement deletion of partial transcodes and similar rollback mechanisms.

        let album_view = queued_album.album.read();

        terminal.log_println(format!(
            "{} A partially-transcoded album ({} - {}) has been potentially left behind \
            in the transcoded library - clean up any remains before running again \
            (reason: deletion of partial transcodes is not yet implemented).",
            "WARNING:".red(),
            album_view.read_lock_artist().name,
            album_view.title,
        ));

        return Err(miette!("User aborted transcoding."));
    }


    // There are now two possibilities:
    // - if the album was being processed normally, we should save the states (see below - `.album.source-state.euphony`, ...)
    // - but if the transcoded album was being deleted (e.g. when the source album is fully deleted),
    //   we need to remove those state files and possibly delete the empty directory that has now been left behind

    if queued_album.job_type == QueuedAlbumJobType::NormalProcessing {
        // The entire album is not up-to-date, so we generate two state structs that are then
        // saved as JSON:
        // - `.album.source-state.euphony` is saved in the source album directory
        //   and contains all the tracked source files' metadata.
        // - `.album.transcode-state.euphony` is saved in the transcoded album directory
        //   and contains a mapping from transcoded files back to their originals
        //   as well as metadata of the tracked *transcoded* files.

        let source_album_state =
            queued_album.changes.generate_source_album_state()?;
        let transcoded_album_state =
            queued_album.changes.generate_transcoded_album_state()?;

        {
            let album_view = queued_album.album.read();

            source_album_state.save_to_directory(
                album_view.album_directory_in_source_library(),
                true,
            )?;

            transcoded_album_state.save_to_directory(
                album_view.album_directory_in_transcoded_library(),
                true,
            )?;
        }

        // Mark the album as finished in the album queue and clear the file queue.
        terminal.queue_album_item_finish(
            queued_album.queue_id,
            AlbumQueueItemFinishedResult::new_ok(),
        )?;
        terminal.queue_file_clear()?;

        let time_album_elapsed = time_album_start.elapsed().as_secs_f64();
        terminal.log_println(format!(
            "  Album transcoded in {time_album_elapsed:.2} seconds."
        ));
    } else if queued_album.job_type == QueuedAlbumJobType::FullyRemoving {
        // The transcoded album was fully deleted, meaning we need to delete the state (`.*.euphony`) files
        // and potentially remove the now-empty album directory.

        let album_view = queued_album.album.read();
        let album_transcoded_directory_path =
            album_view.album_directory_in_transcoded_library();

        let transcoded_album_state_file_path =
            TranscodedAlbumState::get_state_file_path_for_directory(
                &album_transcoded_directory_path,
            );

        if transcoded_album_state_file_path.exists()
            && transcoded_album_state_file_path.is_file()
        {
            fs::remove_file(&transcoded_album_state_file_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!(
                        "Failed to remove transcoded state at {:?}.",
                        transcoded_album_state_file_path
                    )
                })?;

            if is_verbose_enabled() {
                terminal.log_println(format!(
                    "Removed transcoded state file at {:?}.",
                    transcoded_album_state_file_path
                ));
            }
        }

        // Now remove the album directory if it is empty.
        // `std::fs::remove_dir` already guarantees that it will only remove empty directories.
        if album_transcoded_directory_path
            .read_dir()
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!(
                    "Failed to read directory contents of {:?}",
                    album_transcoded_directory_path
                )
            })?
            .next()
            .is_none()
        {
            fs::remove_dir(&album_transcoded_directory_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!(
                        "Failed to remove empty directory at {:?}",
                        album_transcoded_directory_path
                    )
                })?;

            if is_verbose_enabled() {
                terminal.log_println(format!(
                    "Removed empty album directory at {:?}.",
                    transcoded_album_state_file_path
                ));
            }
        }
    }

    Ok(())
}

fn process_library<'config>(
    queued_library: QueuedLibrary<'config>,
    progress: &mut GlobalProgress,
    terminal: &TranscodeTerminal<'config, '_>,
    terminal_user_input_receiver: &mut tokio::sync::broadcast::Receiver<
        UserControlMessage,
    >,
) -> Result<()> {
    for album in queued_library.queued_albums {
        process_album(
            album,
            progress,
            terminal,
            terminal_user_input_receiver,
        )?;
    }


    // There might be some artists whose transcoded albums we just deleted (because they were
    // completely removed from the source library). In that case, it's a good idea to check
    // whether the artist directory is now empty - in that case we should delete the now-empty artist directory
    // (in the transcoded directory; we never touch the source directory).
    for fully_removed_artist in queued_library.fully_removed_artists {
        let artist_transcoded_directory_path = fully_removed_artist
            .read()
            .artist_directory_in_transcoded_library();

        // Now remove the artist directory if it is empty.
        // `std::fs::remove_dir` already guarantees that it will only remove empty directories.
        if artist_transcoded_directory_path
            .read_dir()
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!(
                    "Failed to read directory contents of {:?}",
                    artist_transcoded_directory_path
                )
            })?
            .next()
            .is_none()
        {
            fs::remove_dir(&artist_transcoded_directory_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!(
                        "Failed to remove artist directory at {:?}",
                        artist_transcoded_directory_path
                    )
                })?;

            if is_verbose_enabled() {
                terminal.log_println(format!(
                    "Removed empty artist directory at {:?}.",
                    artist_transcoded_directory_path
                ));
            }
        }
    }


    let library_view = queued_library.library.read();
    let library_directory = library_view.root_directory_in_source_library();

    queued_library
        .fresh_artist_album_list_state
        .save_to_directory(library_directory, true)?;

    if is_verbose_enabled() {
        terminal.log_println(format!(
            "Saved library state into {} for library {} ({:?})",
            LIBRARY_STATE_FILE_NAME,
            library_view.name(),
            library_view.root_directory_in_source_library()
        ));
    }

    Ok(())
}

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
    // The terminal UI backend for example implements the "q" keybind that sends UserControlMessage::Exit.
    let mut terminal_user_input = terminal.get_user_control_receiver()?;


    let libraries = collect_sorted_libraries(configuration, terminal)?;

    let fresh_library_states = collect_full_library_states(&libraries)?;
    let libraries_with_changes =
        collect_changes(&fresh_library_states, terminal)?;

    // It is possible that no changes have been detected, in which case we should just exit.
    if libraries_with_changes.is_empty() {
        terminal.log_println(
            "All albums are up to date, no transcoding needed."
                .green()
                .bold(),
        );
        return Ok(());
    }

    let num_total_changed_files = libraries_with_changes
        .iter()
        .flat_map(|library| &library.sorted_changed_artists)
        .map(|artist| {
            let num_files_a = artist
                .sorted_changed_albums
                .iter()
                .map(|album| album.changes.number_of_changed_files())
                .sum::<usize>();

            let num_files_b = artist
                .sorted_removed_albums
                .iter()
                .map(|album| album.changes.number_of_changed_files())
                .sum::<usize>();

            num_files_a + num_files_b
        })
        .sum::<usize>();

    terminal.log_println(format!(
        "{} files are new, have changed or otherwise need to be processed.",
        num_total_changed_files.to_string().bold()
    ));


    // Queue the entire workload - this way we'll generate `QueueItemID`s
    // for each item, enabling us to interact with the terminal backend
    // and display individual album and file progress.
    terminal.queue_album_enable();
    terminal.queue_file_enable();
    terminal.progress_enable();

    let queued_libraries =
        queue_all_changed_albums(terminal, libraries_with_changes)?;

    // Set up progress bar tracking.
    let mut global_progress = GlobalProgress {
        audio_files_currently_processing: 0,
        data_files_currently_processing: 0,
        audio_files_finished_ok: 0,
        data_files_finished_ok: 0,
        audio_files_errored: 0,
        data_files_errored: 0,
    };

    terminal.progress_set_audio_files_currently_processing(
        global_progress.audio_files_currently_processing,
    )?;
    terminal.progress_set_data_files_currently_processing(
        global_progress.data_files_currently_processing,
    )?;
    terminal.progress_set_audio_files_finished_ok(
        global_progress.audio_files_finished_ok,
    )?;
    terminal.progress_set_data_files_finished_ok(
        global_progress.data_files_finished_ok,
    )?;
    terminal
        .progress_set_audio_files_errored(global_progress.audio_files_errored)?;
    terminal
        .progress_set_data_files_errored(global_progress.data_files_errored)?;

    terminal.progress_set_total(num_total_changed_files)?;


    for queued_library in queued_libraries {
        process_library(
            queued_library,
            &mut global_progress,
            terminal,
            &mut terminal_user_input,
        )?;
    }

    let time_full_processing_elapsed =
        time_full_processing_start.elapsed().as_secs_f64();

    terminal.log_println(format!(
        "All changes successfully processed in {time_full_processing_elapsed:.2} seconds."
    ));

    Ok(())
}


/*
 * Utility functions
 */

fn collect_sorted_libraries<'config>(
    configuration: &'config Config,
    terminal: &TranscodeTerminal<'config, '_>,
) -> Result<Vec<SharedLibraryView<'config>>> {
    // `LibraryView` is the root abstraction here - we use it to discover artists and their albums.
    let mut libraries = configuration
        .libraries
        .values()
        .map(|library| {
            LibraryView::from_library_configuration(configuration, library)
        })
        .collect::<Result<Vec<SharedLibraryView>, LibraryViewError>>()?;

    libraries.sort_unstable_by_key(|library| library.read().name());

    if is_verbose_enabled() {
        terminal.log_println(format!(
            "Collected libraries: {:?}",
            libraries
                .iter()
                .map(|library| library.read().name())
                .collect::<Vec<String>>()
        ));
    }

    Ok(libraries)
}


fn collect_full_library_states<'config>(
    sorted_libraries: &[SharedLibraryView<'config>],
) -> Result<Vec<(SharedLibraryView<'config>, LibraryState)>> {
    sorted_libraries
        .iter()
        .map(|library| {
            let library = library.clone();

            let tracked_artists_and_albums = library
                .read()
                .artists()?
                .iter()
                .map(|(artist_name, artist_view)| {
                    let mut tracked_albums = Vec::new();
                    for (album_title, album_view) in
                        artist_view.read().albums()?
                    {
                        let album_path = album_view
                            .read()
                            .directory_path_relative_to_library_root();

                        tracked_albums.push(TrackedAlbum {
                            album_title,
                            album_source_relative_path: dunce::simplified(
                                &album_path,
                            )
                            .to_string_lossy()
                            .to_string(),
                        })
                    }

                    Ok((
                        artist_name.clone(),
                        TrackedArtistAlbums { tracked_albums },
                    ))
                })
                .collect::<Result<HashMap<String, TrackedArtistAlbums>>>()?;

            Ok((
                library,
                LibraryState::new(tracked_artists_and_albums),
            ))
        })
        .collect()
}


pub struct ChangedAlbum<'view> {
    pub album: SharedAlbumView<'view>,

    pub album_title: String,

    pub changes: AlbumFileChangesV2<'view>,
}

pub struct FullyRemovedAlbum<'view> {
    pub album_title: String,

    pub changes: AlbumFileChangesV2<'view>,
}

pub struct ArtistWithChanges<'view> {
    pub artist: SharedArtistView<'view>,

    pub artist_name: String,

    pub sorted_changed_albums: Vec<ChangedAlbum<'view>>,

    pub sorted_removed_albums: Vec<FullyRemovedAlbum<'view>>,
}

pub struct LibraryWithChanges<'view> {
    pub library: SharedLibraryView<'view>,

    pub library_name: String,

    pub fresh_artist_album_list_state: LibraryState,

    pub sorted_changed_artists: Vec<ArtistWithChanges<'view>>,

    pub fully_removed_artists: Vec<SharedArtistView<'view>>,
}


fn collect_artist_changes<'config>(
    artist: SharedArtistView<'config>,
    saved_tracked_album_list: Option<&TrackedArtistAlbums>,
    fresh_tracked_album_list: &TrackedArtistAlbums,
    terminal: &TranscodeTerminal<'config, '_>,
) -> Result<Option<ArtistWithChanges<'config>>> {
    let artist_locked = artist.read();

    let mut changed_albums: Vec<ChangedAlbum> = artist_locked
        .scan_for_albums_with_changes()?
        .into_iter()
        .map(
            |(album_title, (album_view, album_changes))| ChangedAlbum {
                album: album_view,
                album_title,
                changes: album_changes,
            },
        )
        .collect::<Vec<ChangedAlbum>>();

    if is_verbose_enabled() {
        terminal.log_println(format!(
            "Changes for artist {}:\n{}",
            artist_locked.name,
            changed_albums
                .iter()
                .map(|album| format!(
                    "album_title={},changes={:?}",
                    album.album_title, album.changes
                ))
                .collect::<Vec<String>>()
                .join("\n"),
        ));
    }

    let mut removed_albums = if let Some(saved_album_list) =
        saved_tracked_album_list
    {
        let saved_album_set: HashSet<&TrackedAlbum> =
            HashSet::from_iter(saved_album_list.tracked_albums.iter());
        let fresh_album_set: HashSet<&TrackedAlbum> =
            HashSet::from_iter(fresh_tracked_album_list.tracked_albums.iter());

        let fully_removed_album_set = saved_album_set.sub(&fresh_album_set);

        if is_verbose_enabled() && !fully_removed_album_set.is_empty() {
            terminal.log_println(format!(
                "Some source albums have been removed since last transcode: {:?}",
                fully_removed_album_set
            ));
        }

        fully_removed_album_set
            .into_iter()
            .map(|album| {
                let album_view = AlbumView::new(
                    artist.clone(),
                    album.album_title.clone(),
                    true,
                )?;

                let changes = AlbumFileChangesV2::generate_entire_transcoded_album_deletion(
                    album_view,
                    &album.album_source_relative_path
                )?;

                Ok(FullyRemovedAlbum {
                    album_title: album.album_title.clone(),
                    changes,
                })
            })
            .collect::<Result<Vec<FullyRemovedAlbum>>>()?
    } else {
        Vec::new()
    };

    if !changed_albums.is_empty() || !removed_albums.is_empty() {
        changed_albums.sort_unstable_by(|first, second| {
            first.album_title.cmp(&second.album_title)
        });
        removed_albums.sort_unstable_by(|first, second| {
            first.album_title.cmp(&second.album_title)
        });

        Ok(Some(ArtistWithChanges {
            artist: artist.clone(),
            artist_name: artist_locked.name.clone(),
            sorted_changed_albums: changed_albums,
            sorted_removed_albums: removed_albums,
        }))
    } else {
        Ok(None)
    }
}

fn collect_changes<'config>(
    sorted_libraries_with_fresh_states: &Vec<(
        SharedLibraryView<'config>,
        LibraryState,
    )>,
    terminal: &TranscodeTerminal<'config, '_>,
) -> Result<Vec<LibraryWithChanges<'config>>> {
    // We perform a scan on each library: for each artist in the library, we scan each
    // of their albums for changes (this includes untranscoded albums in addition to
    // albums changed since last transcode).
    //
    // This is a relatively expensive step (a lot of disk accesses),
    // but at the end we'll have all the work we need to perform.

    let mut libraries_with_changes: Vec<LibraryWithChanges> =
        Vec::with_capacity(sorted_libraries_with_fresh_states.len());

    for (library_view, fresh_tracked_artist_album_list) in
        sorted_libraries_with_fresh_states
    {
        let library = library_view.read();

        if is_verbose_enabled() {
            terminal.log_println(format!(
                "Scanning changes in library: {}",
                library.name(),
            ));
        }

        let saved_tracked_artist_album_list =
            match LibraryState::load_from_directory(
                library.root_directory_in_source_library(),
            ) {
                Ok(state) => Some(state),
                Err(error) => match error {
                    LibraryStateLoadError::NotFound => None,
                    LibraryStateLoadError::SchemaVersionMismatch(_) => None,
                    _ => return Err(error.into()),
                },
            };

        if is_verbose_enabled() {
            terminal.log_println(format!(
                "Saved library state (artist album list): {:?}",
                saved_tracked_artist_album_list
            ));
        }

        let mut remaining_saved_tracked_artists: HashSet<&String> =
            if let Some(saved_tracked_artist_album_list) =
                &saved_tracked_artist_album_list
            {
                HashSet::from_iter(
                    saved_tracked_artist_album_list.tracked_artists.keys(),
                )
            } else {
                HashSet::new()
            };

        let mut artists_with_changes: Vec<ArtistWithChanges> = Vec::new();
        for (artist_name, artist_view) in library.artists()? {
            let saved_artist_album_list = match &saved_tracked_artist_album_list
            {
                Some(saved_state) => {
                    match saved_state.tracked_artists.get(&artist_name) {
                        Some(album_list) => {
                            remaining_saved_tracked_artists.remove(&artist_name);
                            Some(album_list)
                        }
                        None => None,
                    }
                }
                None => None,
            };

            let fresh_artist_album_list = fresh_tracked_artist_album_list
                .tracked_artists
                .get(&artist_name)
                .ok_or_else(|| {
                    miette!(
                        "BUG: Missing fresh tracked artist state: {}",
                        artist_name
                    )
                })?;

            let changes = collect_artist_changes(
                artist_view.clone(),
                saved_artist_album_list,
                fresh_artist_album_list,
                terminal,
            )?;

            if let Some(changes) = changes {
                artists_with_changes.push(changes);
            }
        }

        // Any artists left in `remaining_saved_tracked_artists` are those that were entirely removed
        // since the last transcode, meaning we should remove all transcodes of their albums.
        let mut fully_removed_artists: Vec<SharedArtistView> =
            Vec::with_capacity(remaining_saved_tracked_artists.len());

        for fully_removed_artist in remaining_saved_tracked_artists {
            let artist_view = ArtistView::new(
                library_view.clone(),
                fully_removed_artist.clone(),
                true,
            )?;

            let saved_tracked_artist_album_list = saved_tracked_artist_album_list.as_ref().expect("BUG: remaining_saved_tracked_artists was non-empty even though saved_tracked_artist_album_list was None.");

            let artist_albums = saved_tracked_artist_album_list.tracked_artists.get(fully_removed_artist)
                .expect("BUG: Artist is missing even though the set was generated from it.");

            let sorted_removed_albums = artist_albums
                .tracked_albums
                .iter()
                .map(|album| {
                    let album_view = AlbumView::new(
                        artist_view.clone(),
                        album.album_title.clone(),
                        true,
                    )?;

                    let album_changes = AlbumFileChangesV2::generate_entire_transcoded_album_deletion(
                        album_view,
                        &album.album_source_relative_path
                    )?;

                    Ok(FullyRemovedAlbum {
                        album_title: album.album_title.clone(),
                        changes: album_changes,
                    })
                })
                .collect::<Result<Vec<FullyRemovedAlbum>>>()?;


            if is_verbose_enabled() {
                terminal.log_println(format!(
                    "Artist {} has been fully removed since last transcode, removing all the albums: {:?}",
                    fully_removed_artist,
                    sorted_removed_albums
                        .iter().map(|album| &album.album_title).collect::<Vec<&String>>()
                ));
            }


            let artist_with_changes = ArtistWithChanges {
                artist_name: fully_removed_artist.clone(),
                artist: artist_view.clone(),
                sorted_changed_albums: Vec::new(),
                sorted_removed_albums,
            };

            fully_removed_artists.push(artist_view);
            artists_with_changes.push(artist_with_changes);
        }

        artists_with_changes.sort_unstable_by(|first, second| {
            first.artist_name.cmp(&second.artist_name)
        });

        libraries_with_changes.push(LibraryWithChanges {
            library: library_view.clone(),
            library_name: library.name(),
            fresh_artist_album_list_state: fresh_tracked_artist_album_list
                .clone(),
            sorted_changed_artists: artists_with_changes,
            fully_removed_artists,
        })
    }

    libraries_with_changes.sort_unstable_by(|first, second| {
        first.library_name.cmp(&second.library_name)
    });

    Ok(libraries_with_changes)
}


#[derive(Copy, Clone, Eq, PartialEq)]
pub enum QueuedAlbumJobType {
    NormalProcessing,
    FullyRemoving,
}

pub struct QueuedAlbum<'view> {
    pub album: SharedAlbumView<'view>,

    pub queue_id: QueueItemID,

    pub changes: AlbumFileChangesV2<'view>,

    pub job_type: QueuedAlbumJobType,
}

pub struct QueuedLibrary<'view> {
    pub library: SharedLibraryView<'view>,

    pub fresh_artist_album_list_state: LibraryState,

    pub queued_albums: Vec<QueuedAlbum<'view>>,

    pub fully_removed_artists: Vec<SharedArtistView<'view>>,
}


fn queue_all_changed_albums<'config: 'scope, 'scope>(
    terminal: &TranscodeTerminal<'config, 'scope>,
    libraries_with_changes: Vec<LibraryWithChanges<'config>>,
) -> Result<Vec<QueuedLibrary<'config>>> {
    let mut queued_libraries: Vec<QueuedLibrary> =
        Vec::with_capacity(libraries_with_changes.len());

    // Process each library by queueing all the changed albums in it.
    for changed_library in libraries_with_changes {
        let num_changed_albums = changed_library
            .sorted_changed_artists
            .iter()
            .map(|artist| {
                artist.sorted_changed_albums.len()
                    + artist.sorted_removed_albums.len()
            })
            .sum::<usize>();

        let mut queued_albums: Vec<QueuedAlbum> =
            Vec::with_capacity(num_changed_albums);

        // Queue each album of each artist in this library.
        for artist in changed_library.sorted_changed_artists {
            for changed_album in artist.sorted_changed_albums {
                let album_queue_id =
                    terminal.queue_album_item_add(AlbumQueueItem::new(
                        changed_album.album.clone(),
                        changed_album.changes.number_of_changed_audio_files(),
                        changed_album.changes.number_of_changed_data_files(),
                    ))?;

                queued_albums.push(QueuedAlbum {
                    album: changed_album.album.clone(),
                    queue_id: album_queue_id,
                    changes: changed_album.changes,
                    job_type: QueuedAlbumJobType::NormalProcessing,
                })
            }

            for removed_album in artist.sorted_removed_albums {
                let removed_album_view = AlbumView::new(
                    artist.artist.clone(),
                    removed_album.album_title.clone(),
                    true,
                )?;

                let album_queue_id =
                    terminal.queue_album_item_add(AlbumQueueItem::new(
                        removed_album_view.clone(),
                        removed_album.changes.number_of_changed_audio_files(),
                        removed_album.changes.number_of_changed_data_files(),
                    ))?;

                queued_albums.push(QueuedAlbum {
                    album: removed_album_view,
                    queue_id: album_queue_id,
                    changes: removed_album.changes,
                    job_type: QueuedAlbumJobType::FullyRemoving,
                })
            }
        }

        queued_libraries.push(QueuedLibrary {
            library: changed_library.library,
            fresh_artist_album_list_state: changed_library
                .fresh_artist_album_list_state,
            queued_albums,
            fully_removed_artists: changed_library.fully_removed_artists,
        });
    }

    Ok(queued_libraries)
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
fn process_changes<'config>(
    album_changes: &AlbumFileChangesV2,
    album: SharedAlbumView<'config>,
    terminal: &TranscodeTerminal<'config, '_>,
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

    let mut thread_pool =
        CancellableThreadPool::new(thread_pool_size, worker_progress_sender);
    thread_pool.start()?;

    if is_verbose_enabled() {
        terminal.log_println(format!(
            "absolute_source_file_paths_to_transcoded_file_paths_map={:?}",
            album_changes
                .tracked_source_files
                .as_ref()
                .map(|files| files
                    .map_source_file_paths_to_transcoded_file_paths_absolute())
                .unwrap_or_default()
        ));
    }

    // Generate and queue all file jobs.
    let jobs = album_changes.generate_file_jobs(|context| {
        // Parse queue item details.
        let target_path = context.action.target_path();
        let file_name = target_path
            .file_name()
            .ok_or_else(|| {
                miette!("Invalid path: no file name: {:?}.", target_path)
            })?
            .to_string_lossy();

        // Instantiate `FileItem` and add to queue.
        let file_item = FileQueueItem::<'config>::new(
            album.clone(),
            file_name.to_string(),
            context,
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
