use std::cmp::min;
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use miette::{miette, Result};

const THREAD_POOL_COORDINATOR_TICK_SLEEP_TIME: Duration = Duration::from_millis(7);


pub struct CancellableTask {
    task: Box<dyn FnOnce(&AtomicBool) + Send>,
    cancellation_flag: Arc<AtomicBool>,
}

impl CancellableTask {
    pub fn new(
        boxed_task_closure: Box<dyn FnOnce(&AtomicBool) + Send>,
        cancellation_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            task: boxed_task_closure,
            cancellation_flag,
        }
    }
    
    pub fn execute_task(self) {
        let cancellation_flag_ref = self.cancellation_flag.as_ref();
        
        (self.task)(cancellation_flag_ref);
    }
}


pub struct CancellableThreadPool {
    max_num_threads: usize,
    
    pending_tasks: Arc<Mutex<Vec<CancellableTask>>>,
    
    active_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
    
    cancellation_flag: Arc<AtomicBool>,
    
    coordinator_thread: Option<JoinHandle<()>>,
}

impl CancellableThreadPool {
    pub fn new_with_user_flag(
        num_threads: usize,
        user_cancellation_flag: Arc<AtomicBool>,
        start: bool,
    ) -> Self {
        let mut pool = Self {
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
    
    fn start(&mut self) {
        let max_num_threads = self.max_num_threads;
        let cancellation_flag_copy = self.cancellation_flag.clone();
        let pending_tasks_copy = self.pending_tasks.clone();
        let active_tasks_copy = self.active_tasks.clone();
        
        // A single coordinator thread is spawned, which handles everything regarding task spawning
        // and eventual thread `join`s.
        let coordinator_thread_handle = thread::spawn(move || {
            loop {
                let cancellation_value = cancellation_flag_copy.load(Ordering::Relaxed);
                if cancellation_value {
                    // Cancellation flag is set, we should exit!
                    // But before that, we should wait for all active threads - they should all
                    // be seeing the cancellation flag set to true, and if their closures properly
                    // check for cancellation, we shouldn't have to wait too long to `join` the threads.
                    for active_task in active_tasks_copy.lock()
                        .expect("active_tasks Mutex lock has been poisoned.")
                        .drain(..) {
                        active_task.join()
                            .expect("Worker thread panicked!");
                    }
                    
                    return;
                }
                
                
                // Check for any finished theads.
                let mut active_tasks_locked = active_tasks_copy.lock()
                    .expect("active_tasks Mutex lock has been poisoned.");
                
                let mut finished_task_indices: Vec<usize> = Vec::with_capacity(active_tasks_locked.len());
                for (index, task) in active_tasks_locked.iter().enumerate() {
                    if task.is_finished() {
                        finished_task_indices.push(index);
                    }
                }
                
                if !finished_task_indices.is_empty() {
                    // Now drain the active task vector by given indices (sorted to descnending to avoid index shift).
                    let finished_tasks: Vec<JoinHandle<()>> = finished_task_indices.iter()
                        .rev()
                        .map(|index| active_tasks_locked.remove(*index))
                        .collect();
    
                    // Finally, `join` all finished handles.
                    for task in finished_tasks {
                        let _ = task.join();
                    }
                }
                
                
                // Check for any free spots and spawn new tasks to fill up the thread count (up to `max_num_threads`).
                let threads_until_max = max_num_threads - active_tasks_locked.len();
                if threads_until_max > 0 {
                    let mut pending_tasks_locked = pending_tasks_copy.lock()
                        .expect("pending_tasks Mutex lock has been poisoned.");
                    let pending_tasks_num = pending_tasks_locked.len();
                    
                    let tasks_to_run: Vec<CancellableTask> = pending_tasks_locked
                        .drain(0..min(threads_until_max, pending_tasks_num))
                        .collect();
                    
                    // Delegate each task to a new thread, storing its join handle.
                    for new_task in tasks_to_run {
                        let thread_handle = thread::spawn(move || {
                            new_task.execute_task();
                        });
                        
                        active_tasks_locked.push(thread_handle);
                    }
                }
                
                // TODO Test thread spawning logic.
                
                thread::sleep(THREAD_POOL_COORDINATOR_TICK_SLEEP_TIME);
            }
        });
        
        self.coordinator_thread = Some(coordinator_thread_handle);
    }
    
    pub fn has_pending_tasks(&self) -> bool {
        let pending_tasks = self.get_locked_pending_tasks();
        !pending_tasks.is_empty()
    }
    
    pub fn join(mut self) -> Result<()> {
        let coordinator_thread_handle = self.coordinator_thread.take()
            .ok_or_else(|| miette!("call to join was performed before initialization."))?;
        
        coordinator_thread_handle.join()
            .map_err(|error| miette!("Thread exited abnormally: {:?}", error))
    }
    
    fn get_locked_pending_tasks(&self) -> MutexGuard<Vec<CancellableTask>> {
        self.pending_tasks.lock()
            .expect("pending_tasks job queue lock has been poisoned!")
    }
    
    pub fn queue_task<F>(
        &mut self,
        cancellable_task_closure: F,
    )
        where F: FnOnce(&AtomicBool) + Send + 'static
    {
        let boxed_closure: Box<dyn FnOnce(&AtomicBool) + Send> = Box::new(cancellable_task_closure);
        let task = CancellableTask::new(
            boxed_closure,
            self.cancellation_flag.clone(),
        );
        
        let mut exclusive_queue_lock = self.get_locked_pending_tasks();
        exclusive_queue_lock.push(task);
    }
}
