use std::cmp::min;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam::channel::Sender;
use miette::{miette, IntoDiagnostic, Result};
use parking_lot::{Mutex, MutexGuard};

use crate::commands::transcode::jobs::{CancellableTask, FileJobMessage};
use crate::globals::is_verbose_enabled;

// How fast the thread pool's coordinator cleans up and creates new tasks ("ticks", if you will).
const THREAD_POOL_COORDINATOR_TICK_DURATION: Duration =
    Duration::from_millis(50);

#[derive(Debug)]
pub enum ThreadPoolStopReason {
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
pub struct CancellableThreadPool {
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
    pool_coordination_thread: Option<JoinHandle<Result<ThreadPoolStopReason>>>,

    /// A vector of pending tasks.
    pending_tasks: Arc<Mutex<Vec<CancellableTask<FileJobMessage>>>>,

    /// A vector of currently-running tasks. Never larger than `max_num_threads`.
    running_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl CancellableThreadPool {
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
            return Err(miette!("Thread pool is already running."));
        }

        let max_num_threads = self.max_num_threads;
        let cancellation_flag = self.task_cancellation_flag.clone();
        let worker_message_sender = self.worker_message_sender.clone();
        let pending_tasks_copy = self.pending_tasks.clone();
        let running_tasks_copy = self.running_tasks.clone();

        let coordinator_thread_handle = thread::spawn(move || {
            let out_of_loop_sender = worker_message_sender.clone();

            let coordinator_result = CancellableThreadPool::run_coordinator(
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

    /// Enter the given cancellable task into the thread-pool task queue.
    ///
    /// The cancellable task's message sender type must match the thread-pool's message sender.
    pub fn queue_task(
        &mut self,
        cancellable_task: CancellableTask<FileJobMessage>,
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
    pub fn set_cancellation_and_join(self) -> Result<ThreadPoolStopReason> {
        self.task_cancellation_flag.store(true, Ordering::SeqCst);
        self.join()
    }

    /// This method will wait for the thread pool to finish.
    /// Note that this method does **not** set the cancellation flag.
    pub fn join(self) -> Result<ThreadPoolStopReason> {
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
    ) -> MutexGuard<Vec<CancellableTask<FileJobMessage>>> {
        self.pending_tasks.lock()
    }

    /// Lock and return the list of currently-running task handles.
    fn get_locked_running_tasks(&self) -> MutexGuard<Vec<JoinHandle<()>>> {
        self.running_tasks.lock()
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
        pending_tasks: Arc<Mutex<Vec<CancellableTask<FileJobMessage>>>>,
        running_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
    ) -> Result<ThreadPoolStopReason> {
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

                let mut running_tasks_locked = running_tasks.lock();
                for task in running_tasks_locked.drain(..) {
                    task.join().expect("Thread pool worker panicked!");
                }

                let mut pending_tasks_locked = pending_tasks.lock();
                pending_tasks_locked.clear();

                if is_verbose_enabled() {
                    worker_message_sender
                        .send(FileJobMessage::new_log(
                            "ThreadPool: exiting coordinator thread.",
                        ))
                        .into_diagnostic()?;
                }


                return Ok(ThreadPoolStopReason::CancellationFlagSet);
            }

            // No cancellation yet, so tick normally:
            // - check for any finished tasks and clean up after them,
            // - create fresh tasks if there is space for them.
            {
                let mut running_tasks_locked = running_tasks.lock();

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
                    let tasks_to_run: Vec<CancellableTask<FileJobMessage>> = {
                        let mut pending_tasks_locked = pending_tasks.lock();

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
