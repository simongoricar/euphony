use std::cmp::min;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;
use std::{fs, thread};

use crossbeam::channel::Sender;
use miette::{miette, Context, IntoDiagnostic, Result};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use crate::commands::transcode::album_state::FileType;
use crate::commands::transcode::views::SharedAlbumView;
use crate::console::frontends::shared::queue::QueueItemID;
use crate::filesystem::get_path_extension_or_empty;
use crate::globals::is_verbose_enabled;

// How fast the thread pool's coordinator cleans up and creates new tasks ("ticks", if you will).
const THREAD_POOL_COORDINATOR_TICK_DURATION: Duration =
    Duration::from_millis(50);
const FFMPEG_TASK_CANCELLATION_CHECK_INTERVAL: Duration =
    Duration::from_millis(50);

pub struct CancellableTaskV2<C: Send> {
    #[allow(dead_code)]
    id: String,

    #[allow(clippy::type_complexity)]
    task_closure: Box<dyn FnOnce(&AtomicBool, &Sender<C>) + Send>,
}

impl<C: Send> CancellableTaskV2<C> {
    #[allow(clippy::type_complexity)]
    pub fn new(
        task_id: String,
        boxed_closure: Box<dyn FnOnce(&AtomicBool, &Sender<C>) + Send>,
    ) -> Self {
        Self {
            id: task_id,
            task_closure: boxed_closure,
        }
    }

    pub fn execute_task(
        self,
        cancellation_flag: &AtomicBool,
        message_sender: &Sender<C>,
    ) {
        (self.task_closure)(cancellation_flag, message_sender)
    }
}

pub trait IntoCancellableTaskV2<C: Send> {
    fn into_cancellable_task(self) -> CancellableTaskV2<C>;
}

#[derive(Debug)]
pub enum ThreadPoolV2StopReason {
    CancellationFlagSet,
}

/// This is an implementation of a cancellable thread pool.
/// There can be up to `max_num_threads` tasks running at once, each in its own thread.
/// New tasks are added from the queue automatically (once `start()` is called).
///
/// Each queued cancellable task receives two arguments:
/// - An AtomicBool with which it can check for task cancellation
///   (when `true` the task has been cancelled).
/// - A message sender (`Sender<C>`) that the worker can use to relay messages back to the main
///   thread. The message type is generic (`C`), but it must be `Send`.
pub struct CancellableThreadPoolV2 {
    /// Maximum amount of tasks (threads) that can be running concurrently.
    max_num_threads: usize,

    /// AtomicBool that is distributed across workers and acts as a cancellation flag.
    /// When the bool is true, threads *should* exit as soon as possible
    /// (how and when depends entirely on their implementation).
    task_cancellation_flag: Arc<AtomicBool>,

    /// A multi-producer single-consumer Sender. Distributed across workers who can send
    /// messages back to the user-provided channel's `Receiver`. The data sent can be anything
    /// that can be safely sent across threads (`Send`).
    worker_message_sender: Sender<FileJobMessage>,

    /// If `Some`, a handle to the pool coordinator (handles spawning and cleaning up tasks).
    pool_coordination_thread: Option<JoinHandle<Result<ThreadPoolV2StopReason>>>,

    /// A vector of pending tasks.
    pending_tasks: Arc<Mutex<Vec<CancellableTaskV2<FileJobMessage>>>>,

    /// A vector of currently-running tasks. Never larger than `max_num_threads`.
    running_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl CancellableThreadPoolV2 {
    /// Create a new cancellable thread pool.
    pub fn new(
        thread_pool_size: usize,
        worker_message_sender: Sender<FileJobMessage>,
    ) -> Self {
        Self {
            max_num_threads: thread_pool_size,
            task_cancellation_flag: Arc::new(AtomicBool::new(false)),
            worker_message_sender,
            pool_coordination_thread: None,
            pending_tasks: Arc::new(Mutex::new(Vec::new())),
            running_tasks: Arc::new(Mutex::new(Vec::with_capacity(
                thread_pool_size,
            ))),
        }
    }

    /// Initializes a new thread that consumes pending tasks of the pool and spawns worker threads
    /// (up to the limit) that will execute those tasks.
    pub fn start(&mut self) -> Result<()> {
        if self.pool_coordination_thread.is_some() {
            return Err(miette!("Threadpool is already running."));
        }

        let max_num_threads = self.max_num_threads;
        let cancellation_flag = self.task_cancellation_flag.clone();
        let worker_message_sender = self.worker_message_sender.clone();
        let pending_tasks_copy = self.pending_tasks.clone();
        let running_tasks_copy = self.running_tasks.clone();

        let coordinator_thread_handle = thread::spawn(move || {
            let out_of_loop_sender = worker_message_sender.clone();

            let coordinator_result = CancellableThreadPoolV2::run_coordinator(
                max_num_threads,
                cancellation_flag,
                worker_message_sender,
                pending_tasks_copy,
                running_tasks_copy,
            );

            if is_verbose_enabled() {
                out_of_loop_sender
                    .send(FileJobMessage::new_log(
                        "ThreadPool: coordinator thread has stopped.",
                    ))
                    .into_diagnostic()?;
            }

            coordinator_result
        });

        self.pool_coordination_thread = Some(coordinator_thread_handle);

        Ok(())
    }

    /// Enter the given cancellable task into the threadpool task queue.
    ///
    /// The cancellable task's message sender type must match the thread-pool's message sender.
    pub fn queue_task(
        &mut self,
        cancellable_task: CancellableTaskV2<FileJobMessage>,
    ) {
        let mut exclusive_queue_lock = self.get_locked_pending_tasks();
        exclusive_queue_lock.push(cancellable_task);
    }

    /// Checks whether there are any running or pending tasks in this thread pool.
    pub fn has_tasks_left(&self) -> bool {
        let (pending_vec_empty, running_vec_empty) = {
            let running_tasks = self.get_locked_running_tasks();
            let pending_tasks = self.get_locked_pending_tasks();

            (pending_tasks.is_empty(), running_tasks.is_empty())
        };

        !pending_vec_empty || !running_vec_empty
    }

    /// Returns `true` when the coordinator thread of the thread pool is running
    /// (i.e. when new tasks will be spawned).
    pub fn is_running(&self) -> bool {
        match self.pool_coordination_thread.as_ref() {
            None => false,
            Some(coordinator) => !coordinator.is_finished(),
        }
    }

    /// Get the cancellation flag of this thread pool. This is useful for
    /// triggering cancellation externally (by just setting this `AtomicBool` to `true`).
    #[allow(dead_code)]
    pub fn cancellation_flag(&self) -> Arc<AtomicBool> {
        self.task_cancellation_flag.clone()
    }

    /// This method will set the cancellation flag and wait for the thread pool to finish.
    pub fn set_cancellation_and_join(self) -> Result<ThreadPoolV2StopReason> {
        self.task_cancellation_flag.store(true, Ordering::SeqCst);
        self.join()
    }

    /// This method will wait for the thread pool to finish.
    /// Note that this method does **not** set the cancellation flag.
    pub fn join(self) -> Result<ThreadPoolV2StopReason> {
        if self.pool_coordination_thread.is_none() {
            return Err(miette!("Thread pool is not running."));
        }

        // Checked above.
        let coordinator_thread_handle = self.pool_coordination_thread.unwrap();

        coordinator_thread_handle.join().map_err(|error| {
            miette!(
                "ThreadPool coordinator thread exited abnormally. {:?}",
                error
            )
        })?
    }

    /// Lock and return the list of pending tasks.
    fn get_locked_pending_tasks(
        &self,
    ) -> MutexGuard<Vec<CancellableTaskV2<FileJobMessage>>> {
        self.pending_tasks
            .lock()
            .expect("pending_tasks job queue lock has been poisoned!")
    }

    /// Lock and return the list of currently-running task handles.
    fn get_locked_running_tasks(&self) -> MutexGuard<Vec<JoinHandle<()>>> {
        self.running_tasks
            .lock()
            .expect("active_tasks thread lock has been poisoned!")
    }

    /// This method is the coordinator function. It should be executed in its own thread.
    ///
    /// The goal of this method is to manage pending and active threads by cleaning up finished
    /// threads and spawning new pending tasks in their place. This process happens every tick,
    /// see `THREAD_POOL_COORDINATOR_TICK_DURATION`.
    fn run_coordinator(
        max_num_threads: usize,
        cancellation_flag: Arc<AtomicBool>,
        worker_message_sender: Sender<FileJobMessage>,
        pending_tasks: Arc<Mutex<Vec<CancellableTaskV2<FileJobMessage>>>>,
        running_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
    ) -> Result<ThreadPoolV2StopReason> {
        loop {
            let cancellation_flag_value =
                cancellation_flag.load(Ordering::SeqCst);
            if cancellation_flag_value {
                // Cancellation flag is set, we should exit!
                // We should wait for all active threads first though - the threads will, if
                // properly implemented, soon see the cancellation flag and exit accordingly.

                if is_verbose_enabled() {
                    worker_message_sender.send(
                        FileJobMessage::new_log("ThreadPool: cancellation flag set, waiting for active workers, clearing pending tasks and joining.")
                    )
                        .into_diagnostic()?;
                }

                let mut running_tasks_locked = running_tasks
                    .lock()
                    .expect("running_tasks mutex lock has been poisoned!");
                for task in running_tasks_locked.drain(..) {
                    task.join().expect("Thread pool worker panicked!");
                }

                let mut pending_tasks_locked = pending_tasks
                    .lock()
                    .expect("pending_tasks mutex lock has been poisoned!");
                pending_tasks_locked.clear();

                if is_verbose_enabled() {
                    worker_message_sender
                        .send(FileJobMessage::new_log(
                            "ThreadPool: exiting coordinator thread.",
                        ))
                        .into_diagnostic()?;
                }


                return Ok(ThreadPoolV2StopReason::CancellationFlagSet);
            }

            // No cancellation yet, so tick normally:
            // - check for any finished tasks and clean up after them,
            // - create fresh tasks if there is space for them.
            {
                let mut running_tasks_locked = running_tasks
                    .lock()
                    .expect("running_tasks mutex lock has been poisoned!");

                let mut finished_tasks_indices: Vec<usize> =
                    Vec::with_capacity(running_tasks_locked.len());
                for (index, task) in running_tasks_locked.iter().enumerate() {
                    if task.is_finished() {
                        finished_tasks_indices.push(index);
                    }
                }

                if !finished_tasks_indices.is_empty() && is_verbose_enabled() {
                    worker_message_sender
                        .send(FileJobMessage::new_log(format!(
                            "ThreadPool: {} tasks finished since last tick.",
                            finished_tasks_indices.len()
                        )))
                        .into_diagnostic()?;
                }

                if !finished_tasks_indices.is_empty() {
                    // We now drain the running task vector by given indices.
                    // This can be done directly using indices without problems because
                    // the vector is reversed.
                    finished_tasks_indices.iter().rev().for_each(|index| {
                        let finished_task = running_tasks_locked.remove(*index);
                        finished_task
                            .join()
                            .expect("Thread pool worker panicked!");
                    });
                }

                // Fill with new tasks (if we cleared any tasks this tick).
                let threads_to_limit =
                    max_num_threads - running_tasks_locked.len();
                if threads_to_limit > 0 {
                    let tasks_to_run: Vec<CancellableTaskV2<FileJobMessage>> = {
                        let mut pending_tasks_locked =
                            pending_tasks.lock().expect(
                                "pending_tasks mutex lock has been poisoned!",
                            );

                        let pending_tasks_num = pending_tasks_locked.len();

                        pending_tasks_locked
                            .drain(0..min(pending_tasks_num, threads_to_limit))
                            .collect()
                    };

                    // Create new threads for each new task.
                    for new_task in tasks_to_run {
                        let cancellation_flag_copy = cancellation_flag.clone();
                        let message_sender_copy = worker_message_sender.clone();

                        let task_thread_handle = thread::spawn(move || {
                            new_task.execute_task(
                                &cancellation_flag_copy,
                                &message_sender_copy,
                            )
                        });

                        running_tasks_locked.push(task_thread_handle);
                    }
                } else if !finished_tasks_indices.is_empty()
                    && is_verbose_enabled()
                {
                    worker_message_sender
                        .send(FileJobMessage::new_log(
                            "ThreadPool: no pending tasks to spawn right now",
                        ))
                        .into_diagnostic()?;
                }
            }

            thread::sleep(THREAD_POOL_COORDINATOR_TICK_DURATION);
        }
    }
}

/*
 * Specific job implementations
 */

/// Task state for completed `FileJob`s.
#[derive(Debug)]
pub enum FileJobResult {
    Okay {
        verbose_info: Option<String>,
    },
    Errored {
        error: String,
        verbose_info: Option<String>,
    },
}

/// Message enum that file job workers send back to the main thread.
pub enum FileJobMessage {
    Starting {
        queue_item: QueueItemID,
        file_type: FileType,
        file_path: String,
    },
    // TODO Some sort of progress message?
    Finished {
        queue_item: QueueItemID,
        file_type: FileType,
        file_path: String,
        processing_result: FileJobResult,
    },
    Cancelled {
        queue_item: QueueItemID,
        file_type: FileType,
        file_path: String,
    },
    Log {
        content: String,
    },
}

impl FileJobMessage {
    pub fn new_starting<P: Into<String>>(
        queue_item: QueueItemID,
        file_type: FileType,
        file_path: P,
    ) -> Self {
        Self::Starting {
            queue_item,
            file_type,
            file_path: file_path.into(),
        }
    }

    pub fn new_finished<P: Into<String>>(
        queue_item: QueueItemID,
        file_type: FileType,
        file_path: P,
        result: FileJobResult,
    ) -> Self {
        Self::Finished {
            queue_item,
            file_type,
            file_path: file_path.into(),
            processing_result: result,
        }
    }

    pub fn new_cancelled<P: Into<String>>(
        queue_item: QueueItemID,
        file_type: FileType,
        file_path: P,
    ) -> Self {
        Self::Cancelled {
            queue_item,
            file_type,
            file_path: file_path.into(),
        }
    }

    pub fn new_log<S: Into<String>>(log_string: S) -> Self {
        Self::Log {
            content: log_string.into(),
        }
    }
}


/// A simple file job abstraction.
///
/// All implementors must have a `run` method that will execute the task.
trait FileJob {
    fn run(
        &mut self,
        cancellation_flag: &AtomicBool,
        message_sender: &Sender<FileJobMessage>,
    ) -> Result<()>;
}

/// Blanket implementation of the `into_cancellable_task` method for all `FileJob`s.
/// The generated `task_id` is 8 random ASCII characters.
impl<Job> IntoCancellableTaskV2<FileJobMessage> for Job
where
    Job: FileJob + Send + 'static,
{
    fn into_cancellable_task(mut self) -> CancellableTaskV2<FileJobMessage> {
        // Random 8-character ASCII id.
        let random_task_id = thread_rng()
            .sample_iter(Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();

        CancellableTaskV2::new(
            random_task_id,
            Box::new(move |cancellation_flag, sender| {
                self.run(cancellation_flag, sender)
                    .expect("Task errored while running.");
            }),
        )
    }
}


/// One of multiple file jobs.
///
/// `TranscodeAudioFileJob` uses ffmpeg to transcode an audio file. The resulting file location
/// is in the album directory of the aggregated library.
pub struct TranscodeAudioFileJob {
    /// Path to the target file's directory (for missing directory creation purposes).
    target_file_directory_path: PathBuf,

    /// Path to the target file that will be created.
    target_file_path: PathBuf,

    /// Path to the ffmpeg binary.
    ffmpeg_binary_path: String,

    /// List of arguments to ffmpeg that will transcode the audio as configured.
    ffmpeg_arguments: Vec<String>,

    /// `QueueItemID` this job belongs to.
    queue_item: QueueItemID,
}

impl TranscodeAudioFileJob {
    /// Initialize a new `TranscodeAudioFileJob`.
    pub fn new(
        album: SharedAlbumView,
        source_file_path: PathBuf,
        target_file_path: PathBuf,
        queue_item: QueueItemID,
    ) -> Result<Self> {
        let album_locked = album.read();

        let config = album_locked.euphony_configuration();

        /*
         * 1. Sanity and error checking before we begin, as these jobs should not operate on
         *    unusual cases that are not matching the configuration.
         */
        let transcoding_config =
            &album_locked.library_configuration().transcoding;
        let ffmpeg_config = &config.tools.ffmpeg;

        if !transcoding_config
            .is_path_audio_file_by_extension(&source_file_path)?
        {
            return Err(miette!(
                "Invalid source file extension \"{}\": \
                expected a tracked audio extension for this library (one of \"{:?}\").",
                get_path_extension_or_empty(source_file_path)?,
                transcoding_config.audio_file_extensions,
            ));
        }

        if !ffmpeg_config
            .is_path_transcoding_output_by_extension(&target_file_path)?
        {
            let ffmpeg_output_extension =
                &config.tools.ffmpeg.audio_transcoding_output_extension;

            return Err(miette!(
                "Invalid ffmpeg output file extension \"{}\": expected \"{}\".",
                get_path_extension_or_empty(target_file_path)?,
                ffmpeg_output_extension
            ));
        };

        let target_file_directory = target_file_path
            .parent()
            .ok_or_else(|| miette!("Could not get target file directory."))?;

        let source_file_path_str = source_file_path
            .to_str()
            .ok_or_else(|| miette!("Source file path is not valid UTF-8."))?;
        let target_file_path_str = target_file_path
            .to_str()
            .ok_or_else(|| miette!("Target file path is not valid UTF-8."))?;

        let ffmpeg_arguments: Vec<String> = config
            .tools
            .ffmpeg
            .audio_transcoding_args
            .iter()
            .map(|arg| {
                arg.replace("{INPUT_FILE}", source_file_path_str)
                    .replace("{OUTPUT_FILE}", target_file_path_str)
            })
            .collect();


        // We have owned versions of data here because we want to be able to send this
        // job across threads easily.
        Ok(Self {
            target_file_directory_path: target_file_directory.to_path_buf(),
            target_file_path: PathBuf::from(target_file_path_str),
            ffmpeg_binary_path: config.tools.ffmpeg.binary.clone(),
            ffmpeg_arguments,
            queue_item,
        })
    }
}

impl FileJob for TranscodeAudioFileJob {
    fn run(
        &mut self,
        cancellation_flag: &AtomicBool,
        message_sender: &Sender<FileJobMessage>,
    ) -> Result<()> {
        message_sender
            .send(FileJobMessage::new_starting(
                self.queue_item,
                FileType::Audio,
                self.target_file_path.to_string_lossy(),
            ))
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not send FileJobMessage::Starting.")
            })?;

        /*
         * Step 1: create missing directories
         */
        let create_dir_result =
            fs::create_dir_all(&self.target_file_directory_path);

        if let Err(error) = create_dir_result {
            let verbose_info = is_verbose_enabled()
                .then(|| format!("fs::create_dir_all error: {error}"));

            message_sender.send(FileJobMessage::new_finished(self.queue_item, FileType::Audio, self.target_file_path.to_string_lossy(), FileJobResult::Errored {
                error: "Could not create target file's missing parent directory.".to_string(),
                verbose_info
            }))
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not send FileJobMessage::Finished"))?;

            return Ok(());
        }

        /*
         * Step 2: run ffmpeg (transcodes audio)
         */
        let mut ffmpeg_child_process = Command::new(&self.ffmpeg_binary_path)
            .args(&self.ffmpeg_arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not spawn ffmpeg for transcoding.")
            })?;

        // Keep checking for cancellation
        while ffmpeg_child_process
            .try_wait()
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not wait or get process exit code.")
            })?
            .is_none()
        {
            let cancellation_flag_value =
                cancellation_flag.load(Ordering::SeqCst);
            if cancellation_flag_value {
                // Cancellation flag is set to true, we should kill ffmpeg and exit as soon as possible.
                ffmpeg_child_process
                    .kill()
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        miette!("Could not kill ffmpeg process.")
                    })?;
                break;
            }

            thread::sleep(FFMPEG_TASK_CANCELLATION_CHECK_INTERVAL);
        }

        // ffmpeg process is finished at this point, we should just check what the reason was.
        let final_cancellation_flag = cancellation_flag.load(Ordering::SeqCst);
        if final_cancellation_flag {
            // Process was killed because of cancellation.
            message_sender
                .send(FileJobMessage::new_cancelled(
                    self.queue_item,
                    FileType::Audio,
                    self.target_file_path.to_string_lossy(),
                ))
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!("Could not send FileJobMessage::Cancelled.")
                })?;

            Ok(())
        } else {
            // Everything was normal.
            let ffmpeg_output = ffmpeg_child_process
                .wait_with_output()
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not get ffmpeg output."))?;

            let ffmpeg_exit_code = ffmpeg_output
                .status
                .code()
                .ok_or_else(|| miette!("No ffmpeg exit code?!"))?;

            // Extract ffmpeg stdout/stderr/exit code if necessary.
            let processing_result = if ffmpeg_exit_code == 0 {
                let verbose_info: Option<String> = is_verbose_enabled()
                    .then(|| {
                        format!(
                            "ffmpeg exited (exit code 0). Binary={:?} Arguments={:?}",
                            &self.ffmpeg_binary_path, &self.ffmpeg_arguments
                        )
                    });

                FileJobResult::Okay { verbose_info }
            } else {
                let ffmpeg_stdout = String::from_utf8(ffmpeg_output.stdout)
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        miette!("Could not parse ffmpeg stdout.")
                    })?;

                let ffmpeg_stderr = String::from_utf8(ffmpeg_output.stderr)
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        miette!("could not parse ffmpeg stderr.")
                    })?;

                let error = format!(
                    "ffmpeg exited with non-zero exit code.\nStdout: {}\nStderr: {}",
                    ffmpeg_stdout, ffmpeg_stderr
                );

                let verbose_info: Option<String> = is_verbose_enabled()
                    .then(|| {
                        format!(
                            "ffmpeg exited (exit code {}). Binary={:?} Arguments={:?}",
                            ffmpeg_exit_code,
                            &self.ffmpeg_binary_path, &self.ffmpeg_arguments
                        )
                    });

                FileJobResult::Errored {
                    error,
                    verbose_info,
                }
            };

            message_sender
                .send(FileJobMessage::new_finished(
                    self.queue_item,
                    FileType::Audio,
                    self.target_file_path.to_string_lossy(),
                    processing_result,
                ))
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!("Could not send FileJobMessage::Finished.")
                })?;

            Ok(())
        }
    }
}

/// One of multiple file jobs.
///
/// `CopyFileJob` simply copies a file (usually data/other files, not audio files) into the
/// album directory in the aggregated library.
pub struct CopyFileJob {
    /// File to copy from.
    source_file_path: PathBuf,

    /// File to copy to.
    target_file_path: PathBuf,

    /// For missing directory creation purposes, the directory `target_file_path` is in.
    target_file_directory_path: PathBuf,

    /// `QueueItemID` this job belongs to.
    queue_item: QueueItemID,
}

impl CopyFileJob {
    /// Initialize a new `CopyFileJob`.
    pub fn new(
        album: SharedAlbumView,
        source_file_path: PathBuf,
        target_file_path: PathBuf,
        queue_item: QueueItemID,
    ) -> Result<Self> {
        let album_locked = album.read();

        let transcoding_config =
            &album_locked.library_configuration().transcoding;

        /*
         * 1. Sanity checks
         */
        if !transcoding_config
            .is_path_data_file_by_extension(&source_file_path)?
        {
            return Err(miette!(
                "Invalid source file extension: \"{}\": \
                expected a tracked data file extension for this library (one of \"{:?}\").",
                get_path_extension_or_empty(source_file_path)?,
                transcoding_config.audio_file_extensions,
            ));
        }


        let target_file_directory = target_file_path
            .parent()
            .ok_or_else(|| miette!("Could not get target file directory."))?;

        Ok(Self {
            target_file_directory_path: target_file_directory.to_path_buf(),
            source_file_path,
            target_file_path,
            queue_item,
        })
    }
}

impl FileJob for CopyFileJob {
    fn run(
        &mut self,
        _cancellation_flag: &AtomicBool,
        message_sender: &Sender<FileJobMessage>,
    ) -> Result<()> {
        message_sender
            .send(FileJobMessage::new_starting(
                self.queue_item,
                FileType::Data,
                self.target_file_path.to_string_lossy(),
            ))
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not send FileJobMessage::Starting.")
            })?;

        /*
         * Step 1: create parent directories if missing.
         */
        let create_dir_result =
            fs::create_dir_all(&self.target_file_directory_path);

        if let Err(error) = create_dir_result {
            let verbose_info = is_verbose_enabled()
                .then(|| format!("fs::create_dir_all error: {error}"));

            message_sender.send(FileJobMessage::new_finished(self.queue_item, FileType::Data, self.target_file_path.to_string_lossy(), FileJobResult::Errored {
                error: "Could not create target file's missing parent directory.".to_string(),
                verbose_info
            }))
                .into_diagnostic()
                .wrap_err_with(|| miette!("Could not send FileJobMessage::Finished"))?;

            return Ok(());
        }

        /*
         * Step 2: copy the file.
         */
        // TODO Find out a way to create cancellable file copies.
        //      (Make sure to handle the half-copied edge-case - we should delete such a file)
        let copy_result =
            fs::copy(&self.source_file_path, &self.target_file_path);

        let processing_result = match copy_result {
            Ok(bytes_copied) => {
                let verbose_info = is_verbose_enabled().then(|| {
                    format!(
                        "Copy operation OK. Copied {} bytes.",
                        bytes_copied
                    )
                });

                FileJobResult::Okay { verbose_info }
            }
            Err(error) => {
                let verbose_info = is_verbose_enabled().then(|| {
                    format!(
                        "Copy operation from {:?} to {:?} failed.",
                        &self.source_file_path, &self.target_file_path
                    )
                });

                FileJobResult::Errored {
                    error: error.to_string(),
                    verbose_info,
                }
            }
        };

        message_sender
            .send(FileJobMessage::new_finished(
                self.queue_item,
                FileType::Data,
                self.target_file_path.to_string_lossy(),
                processing_result,
            ))
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not send FileJobMessage::Finished.")
            })?;

        Ok(())
    }
}


/// One of multiple file jobs.
///
/// `DeleteProcessedFileJob` removes a transcoded audio file or copied data file
/// from the aggregated library.
pub struct DeleteProcessedFileJob {
    /// Path to the file to delete.
    target_file_path: PathBuf,

    file_type: FileType,

    /// If `true` we should ignore the error if `target_file_path` does not exist.
    ignore_if_missing: bool,

    /// `QueueItemID` this job belongs to.
    queue_item: QueueItemID,
}

impl DeleteProcessedFileJob {
    /// Initialize a new `DeleteProcessedFileJob` from the given target path to remove.
    /// If the file is missing
    pub fn new(
        target_file_path: PathBuf,
        file_type: FileType,
        ignore_if_missing: bool,
        queue_item: QueueItemID,
    ) -> Result<Self> {
        /*
         * 1. Sanity checks
         */
        if target_file_path.exists() && !target_file_path.is_file() {
            return Err(miette!("Given path exists, but is not a file!"));
        }

        if !target_file_path.exists() && !ignore_if_missing {
            return Err(miette!("Given path doesn't exist."));
        }

        Ok(Self {
            target_file_path,
            file_type,
            ignore_if_missing,
            queue_item,
        })
    }
}

impl FileJob for DeleteProcessedFileJob {
    fn run(
        &mut self,
        _cancellation_flag: &AtomicBool,
        message_sender: &Sender<FileJobMessage>,
    ) -> Result<()> {
        message_sender
            .send(FileJobMessage::new_starting(
                self.queue_item,
                self.file_type,
                self.target_file_path.to_string_lossy(),
            ))
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not send FileJobMessage::Starting.")
            })?;

        let processing_result = if !self.target_file_path.is_file() {
            if self.ignore_if_missing {
                let verbose_info = is_verbose_enabled().then(|| "File did not exist, but ignore_if_missing==true - skipping.".to_string());

                FileJobResult::Okay { verbose_info }
            } else {
                FileJobResult::Errored {
                    error: "File did not exist and ignore_if_missing != true!"
                        .to_string(),
                    verbose_info: None,
                }
            }
        } else {
            let removal_result = fs::remove_file(&self.target_file_path);

            match removal_result {
                Ok(_) => FileJobResult::Okay { verbose_info: None },
                Err(error) => FileJobResult::Errored {
                    error: error.to_string(),
                    verbose_info: None,
                },
            }
        };

        message_sender
            .send(FileJobMessage::new_finished(
                self.queue_item,
                self.file_type,
                self.target_file_path.to_string_lossy(),
                processing_result,
            ))
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Could not send FileJobMessage::Finished.")
            })?;

        Ok(())
    }
}
