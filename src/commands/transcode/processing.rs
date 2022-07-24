use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::sleep;
use std::time::Duration;
use indicatif::ProgressBar;
use rayon::{ThreadPool, ThreadPoolBuilder};
use crate::commands::transcode::packets::file::{FileWorkPacket, ProcessingResult};
use crate::Config;
use crate::globals::verbose_enabled;

/// Builds a ThreadPool using the `transcode_threads` configuration value.
pub fn build_transcode_thread_pool(config: &Config) -> ThreadPool {
    ThreadPoolBuilder::new()
        .num_threads(config.aggregated_library.transcode_threads as usize)
        .build()
        .unwrap()
}

pub struct ThreadPoolWorkResult {
    pub results: Vec<ProcessingResult>,
}

impl ThreadPoolWorkResult {
    pub fn new_empty() -> Self {
        ThreadPoolWorkResult {
            results: Vec::new(),
        }
    }

    pub fn from_results(results: Vec<ProcessingResult>) -> Self {
        ThreadPoolWorkResult {
            results,
        }
    }

    pub fn has_errors(&self) -> bool {
        for result in &self.results {
            if !result.is_ok() {
                return true;
            }
        }

        false
    }

    pub fn get_errored_results(&self) -> Vec<ProcessingResult> {
        let mut errored_results: Vec<ProcessingResult> = Vec::new();

        for result in &self.results {
            if !result.is_ok() {
                errored_results.push(result.clone());
            }
        }

        errored_results
    }
}


/// Processes all given `FileWorkPacket`s in parallel as allowed by the given ThreadPool.
/// Updates the progress bar after each successful step.
pub fn process_file_packets_in_threadpool<F: Fn(&str, &MutexGuard<ProgressBar>) + Send + Clone>(
    config: &Config,
    thread_pool: &ThreadPool,
    file_packets: Vec<FileWorkPacket>,
    file_progress_bar_arc: &Arc<Mutex<ProgressBar>>,
    file_progress_bar_set_fn: F,
) -> ThreadPoolWorkResult {
    if file_packets.len() == 0 {
        return ThreadPoolWorkResult::new_empty();
    }

    let (tx, rx): (Sender<ProcessingResult>, Receiver<ProcessingResult>) = channel();

    thread_pool.scope(move |s| {
        for file_packet in file_packets {
            let thread_tx = tx.clone();
            let thread_progress_bar = file_progress_bar_arc.clone();
            let thread_pbc = file_progress_bar_set_fn.clone();

            let file_name = match file_packet.get_file_name() {
                Ok(name) => name,
                Err(error) => {
                    thread_tx.send(
                        ProcessingResult::Error {
                            error: error.to_string(),
                            verbose_info: if verbose_enabled() {
                                Some(
                                    format!(
                                        "Error on get_file_name for FileWorkPacket: {:?}",
                                        file_packet,
                                    )
                                )
                            } else {
                                None
                            },
                        }
                    )
                        .expect("Work thread could not send file name error to main thread.");
                    return;
                }
            };

            s.spawn(move |_| {
                let mut current_retries = 0;
                let max_retries = config.aggregated_library.max_processing_retries;

                // TODO Show better errors.

                let mut last_work_result: Option<ProcessingResult> = None;
                let mut last_work_result_is_ok: bool = false;

                // Retry at least once, until success and up to `max_retries` times.
                while last_work_result.is_none()
                    || !last_work_result_is_ok
                    || current_retries > max_retries
                {
                    let work_result = file_packet.process(config);

                    let thread_progress_bar_lock = thread_progress_bar.lock().unwrap();
                    thread_progress_bar_lock.inc(1);
                    thread_pbc(
                        &file_name,
                        &thread_progress_bar_lock,
                    );

                    last_work_result_is_ok = work_result.is_ok();
                    last_work_result = Some(work_result);

                    current_retries = current_retries + 1;
                    sleep(
                        Duration::from_secs(config.aggregated_library.processing_retry_delay_seconds as u64),
                    );
                }

                // At this point, the ProcessingResult might be either Ok or Err, but the main thread will check that.
                let last_work_result = last_work_result.unwrap();
                thread_tx.send(last_work_result)
                    .expect("Worker thread could not send ProcessingResult to main thread.");
            });
        }
    });

    let collected_thread_results: Vec<ProcessingResult> = Vec::from_iter(rx.try_iter());
    ThreadPoolWorkResult::from_results(collected_thread_results)
}
