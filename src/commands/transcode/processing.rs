use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use rayon::{ThreadPool, ThreadPoolBuilder};
use crate::commands::transcode::packets::file::{FileProcessingResult, FileWorkPacket};
use crate::Config;
use crate::globals::verbose_enabled;
use crate::observer::Observer;

pub struct ProcessingObserver {
    event_callback: Box<dyn Fn(FileProcessingResult) + Send + Sync>,
}

impl ProcessingObserver {
    pub fn new(event_callback: Box<dyn Fn(FileProcessingResult) + Send + Sync>) -> Self {
        ProcessingObserver {
            event_callback
        }
    }
}

impl Observer<FileProcessingResult> for ProcessingObserver {
    fn emit(&self, event: FileProcessingResult) {
        (*self.event_callback)(event);
    }
}


/// Builds a ThreadPool using the `transcode_threads` configuration value.
pub fn build_transcode_thread_pool(config: &Config) -> ThreadPool {
    ThreadPoolBuilder::new()
        .num_threads(config.aggregated_library.transcode_threads as usize)
        .build()
        .unwrap()
}


/// Processes all given `FileWorkPacket`s in parallel as allowed by the given ThreadPool.
/// The returned boolean indicates whether there were any errors (true is okay, false means at least one error).
pub fn process_file_packets_in_threadpool(
    config: &Config,
    thread_pool: &ThreadPool,
    file_packets: Vec<FileWorkPacket>,
    observer: &ProcessingObserver,
) -> bool {
    if file_packets.len() == 0 {
        return true;
    }

    let has_errored: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    let threadpool_has_errored = has_errored.clone();
    thread_pool.scope(move |s| {
        for file_packet in file_packets {
            let thread_has_errored = threadpool_has_errored.clone();

            s.spawn(move |_| {
                let mut current_retries = 0;
                let max_retries = config.aggregated_library.max_processing_retries;

                let mut last_work_result: Option<FileProcessingResult> = None;
                let mut last_work_result_is_ok: bool = false;

                // Retry at least once, until success and up to `max_retries` times.
                while (last_work_result.is_none() || !last_work_result_is_ok) && current_retries <= max_retries
                {
                    let work_result = file_packet.process(config);

                    last_work_result_is_ok = work_result.is_ok();
                    if !last_work_result_is_ok && verbose_enabled() {
                        observer.emit(work_result.clone_as_non_final());
                    }

                    last_work_result = Some(work_result);
                    current_retries = current_retries + 1;

                    sleep(
                        Duration::from_secs(config.aggregated_library.processing_retry_delay_seconds as u64),
                    );
                }

                // At this point, the ProcessingResult might be either Ok or Err, but the main thread will check that.
                let last_work_result = last_work_result.unwrap();
                if !last_work_result.is_ok() {
                    let mut locked_has_errored = thread_has_errored.lock().unwrap();
                    *locked_has_errored = true;
                }

                observer.emit(last_work_result);
            });
        }
    });

    let locked_has_errored = has_errored.lock().unwrap();
    !(*locked_has_errored)
}
