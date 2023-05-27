use std::cmp::min;
use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use miette::{miette, Result};

const THREAD_POOL_COORDINATOR_TICK_DURATION: Duration = Duration::from_millis(7);

fn run_threadpool_coordinator(
    max_num_threads: usize,
    cancellation_flag: Arc<AtomicBool>,
    pending_tasks: Arc<Mutex<Vec<CancellableTask>>>,
    active_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
) -> ThreadPoolStopReason {
    loop {
        let cancellation_value = cancellation_flag.load(Ordering::SeqCst);
        if cancellation_value {
            // Cancellation flag is set, we should exit!
            // But before that, we should wait for all active threads - they should all
            // be seeing the cancellation flag set to true, and if their closures properly
            // check for cancellation, we shouldn't have to wait too long to `join` the threads.
            let mut active_tasks_locked = active_tasks
                .lock()
                .expect("active_tasks Mutex lock has been poisoned!");
            for active_task in active_tasks_locked.drain(..) {
                active_task.join().expect("Worker thread panicked!");
            }

            let mut pending_tasks_locked = pending_tasks
                .lock()
                .expect("pending_tasks Mutex lock has been posioned!");
            pending_tasks_locked.clear();

            return ThreadPoolStopReason::CancellationFlagSet;
        }


        // Check for any finished theads.
        {
            let mut active_tasks_locked = active_tasks
                .lock()
                .expect("active_tasks Mutex lock has been poisoned.");

            let mut finished_task_indices: Vec<usize> =
                Vec::with_capacity(active_tasks_locked.len());
            for (index, task) in active_tasks_locked.iter().enumerate() {
                if task.is_finished() {
                    finished_task_indices.push(index);
                }
            }

            if !finished_task_indices.is_empty() {
                // Now drain the active task vector by given indices (sorted to descnending to avoid index shift).
                let finished_tasks: Vec<JoinHandle<()>> = finished_task_indices
                    .iter()
                    .rev()
                    .map(|index| active_tasks_locked.remove(*index))
                    .collect();

                // Finally, `join` all finished handles.
                for task in finished_tasks {
                    let _ = task.join();
                }
            }

            // Check for any free spots and spawn new tasks up to the thread limit (up to `max_num_threads`).
            let threads_until_limit =
                max_num_threads - active_tasks_locked.len();
            if threads_until_limit > 0 {
                let tasks_to_run: Vec<CancellableTask> = {
                    let mut pending_tasks_locked = pending_tasks
                        .lock()
                        .expect("pending_tasks Mutex lock has been poisoned.");
                    let pending_tasks_num = pending_tasks_locked.len();

                    pending_tasks_locked
                        .drain(0..min(threads_until_limit, pending_tasks_num))
                        .collect()
                };

                // Delegate each task to a new thread, storing its join handle.
                for new_task in tasks_to_run {
                    let thread_handle = thread::spawn(move || {
                        new_task.execute_task();
                    });

                    active_tasks_locked.push(thread_handle);
                }
            }
        }

        thread::sleep(THREAD_POOL_COORDINATOR_TICK_DURATION);
    }
}

/// Reprsents a single cancellable task - a closure that takes a single argument: a reference
/// to an `AtomicBool` that acts as a cancellation flag (`true` means task has been cancelled).
///
/// **NOTE: It is completely up to the specific implementation inside the task closure to
/// willingly quit when the cancellation flag is set.** This mechanism allows the task to
/// complete gracefully.
pub struct CancellableTask {
    name: Option<String>,

    task: Box<dyn FnOnce(&AtomicBool) + Send>,

    cancellation_flag: Arc<AtomicBool>,
}

impl CancellableTask {
    /// Construct a new `CancellableTask` with the given boxed closure and cancellation flag.
    pub fn new(
        boxed_task_closure: Box<dyn FnOnce(&AtomicBool) + Send>,
        cancellation_flag: Arc<AtomicBool>,
        task_name: Option<String>,
    ) -> Self {
        Self {
            name: task_name,
            task: boxed_task_closure,
            cancellation_flag,
        }
    }

    /// Execute the task. This blocks until the task completes.
    pub fn execute_task(self) {
        let cancellation_flag_ref = self.cancellation_flag.as_ref();

        (self.task)(cancellation_flag_ref);
    }
}

impl Debug for CancellableTask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<CancellableTask name={}>",
            match self.name.as_ref() {
                None => String::from(""),
                Some(name) => name.clone(),
            }
        )
    }
}

#[derive(Debug)]
pub enum ThreadPoolStopReason {
    CancellationFlagSet,
}

/// A basic thread pool implementation with a cancellation mechanism.
///
/// The cancellation flag is shared among all the worker threads, who are themselves essentialy
/// just `CancellableTask`s.
///
/// **It is up to the implementation of each task to read the cancellation flag and quit
/// accordingly.** `CancellableThreadPool`'s job is only thread pool organization
/// (task and flag distribution).
pub struct CancellableThreadPool {
    has_started: bool,

    max_num_threads: usize,

    pending_tasks: Arc<Mutex<Vec<CancellableTask>>>,

    active_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,

    cancellation_flag: Arc<AtomicBool>,

    coordinator_thread: Option<JoinHandle<ThreadPoolStopReason>>,
}

impl CancellableThreadPool {
    /// Initialize a new `CancellableThreadPool` with the given user cancellation flag
    /// (`AtomicBool` wrapped in an `Arc`). If `start` is true, the `CancellableThreadPool::start`
    /// method is immediately called, activating the thread pool.
    pub fn new_with_user_flag(
        num_threads: usize,
        user_cancellation_flag: Arc<AtomicBool>,
        start: bool,
    ) -> Self {
        let mut pool = Self {
            has_started: false,
            max_num_threads: num_threads,
            active_tasks: Arc::new(Mutex::new(Vec::with_capacity(num_threads))),
            cancellation_flag: user_cancellation_flag,
            pending_tasks: Arc::new(Mutex::new(Vec::with_capacity(num_threads))),
            coordinator_thread: None,
        };

        if start {
            pool.start();
        }

        pool
    }

    /// This is a one-off method that initializes a coordinator thread which oversees all
    /// task delegation. Essentially, you can call `queue_task` before or after calling this method,
    /// but the threads will only be created and start executing after you call this method.
    fn start(&mut self) {
        if self.has_started {
            return;
        }

        let max_num_threads = self.max_num_threads;
        let cancellation_flag_copy = self.cancellation_flag.clone();
        let pending_tasks_copy = self.pending_tasks.clone();
        let active_tasks_copy = self.active_tasks.clone();

        // A single coordinator thread is spawned, which handles everything regarding task spawning
        // and eventual thread `join`s.
        let coordinator_thread_handle = thread::spawn(move || {
            run_threadpool_coordinator(
                max_num_threads,
                cancellation_flag_copy,
                pending_tasks_copy,
                active_tasks_copy,
            )
        });

        self.coordinator_thread = Some(coordinator_thread_handle);
        self.has_started = true;
    }

    /// Check whether the thread pool has any tasks left (be it active or pending).
    pub fn has_tasks_left(&self) -> bool {
        let (pending_tasks_empty, active_tasks_empty) = {
            let active_tasks = self.get_locked_active_tasks();
            let pending_tasks = self.get_locked_pending_tasks();

            (pending_tasks.is_empty(), active_tasks.is_empty())
        };

        !pending_tasks_empty || !active_tasks_empty
    }

    pub fn is_stopped(&self) -> bool {
        match &self.coordinator_thread {
            None => true,
            Some(coordinator_thread) => coordinator_thread.is_finished(),
        }
    }

    /// Block until all tasks are finished, returning `Err` if the coordinator thread exited abnormally.
    pub fn join(mut self) -> Result<ThreadPoolStopReason> {
        let coordinator_thread_handle =
            self.coordinator_thread.take().ok_or_else(|| {
                miette!("call to join was performed before initialization.")
            })?;

        coordinator_thread_handle.join().map_err(|error| {
            miette!(
                "Threadpool coordinator exited abnormally: {:?}",
                error
            )
        })
    }

    /// Queue a new task by providing a closure that takes a single argument: an `AtomicBool`
    /// reference. This `AtomicBool` is called a *cancellation flag* - when it is true
    /// (see `AtomicBool::load`), it means that the owner of the thread pool has requested
    /// all workers to stop working and exit.
    ///
    /// **NOTE: It is up to the implementation in your closure to check this flag
    /// and exit accordingly (potentially after some cleanup or other code). If the closure does
    /// not respond to the flag, the thread pool will NOT stop the worker itself.**
    pub fn queue_task<F, S: Into<String>>(
        &mut self,
        title: Option<S>,
        cancellable_task_closure: F,
    ) where
        F: FnOnce(&AtomicBool) + Send + 'static,
    {
        let boxed_closure: Box<dyn FnOnce(&AtomicBool) + Send> =
            Box::new(cancellable_task_closure);
        let task = CancellableTask::new(
            boxed_closure,
            self.cancellation_flag.clone(),
            title.map(|name| name.into()),
        );

        let mut exclusive_queue_lock = self.get_locked_pending_tasks();
        exclusive_queue_lock.push(task);
    }

    fn get_locked_pending_tasks(&self) -> MutexGuard<Vec<CancellableTask>> {
        self.pending_tasks
            .lock()
            .expect("pending_tasks job queue lock has been poisoned!")
    }

    fn get_locked_active_tasks(&self) -> MutexGuard<Vec<JoinHandle<()>>> {
        self.active_tasks
            .lock()
            .expect("active_tasks thread lock has been poisoned!")
    }
}
