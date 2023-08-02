use std::sync::atomic::AtomicBool;

use crossbeam::channel::Sender;
use miette::Result;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use crate::commands::transcode::album_state::changes::FileType;
use crate::console::frontends::shared::queue::QueueItemID;

pub struct CancellableTask<C: Send> {
    #[allow(dead_code)]
    id: String,

    #[allow(clippy::type_complexity)]
    task_closure: Box<dyn FnOnce(&AtomicBool, &Sender<C>) + Send>,
}

impl<C: Send> CancellableTask<C> {
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

pub trait IntoCancellableTask<C: Send> {
    fn into_cancellable_task(self) -> CancellableTask<C>;
}
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
pub trait FileJob {
    fn run(
        &mut self,
        cancellation_flag: &AtomicBool,
        message_sender: &Sender<FileJobMessage>,
    ) -> Result<()>;
}

/// Blanket implementation of the `into_cancellable_task` method for all `FileJob`s.
/// The generated `task_id` is 8 random ASCII characters.
impl<Job> IntoCancellableTask<FileJobMessage> for Job
where
    Job: FileJob + Send + 'static,
{
    fn into_cancellable_task(mut self) -> CancellableTask<FileJobMessage> {
        // Random 8-character ASCII id.
        let random_task_id = thread_rng()
            .sample_iter(Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();

        CancellableTask::new(
            random_task_id,
            Box::new(move |cancellation_flag, sender| {
                self.run(cancellation_flag, sender)
                    .expect("Task errored while running.");
            }),
        )
    }
}
