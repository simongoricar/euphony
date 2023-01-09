use std::fmt::Display;
use std::path::PathBuf;
use crossbeam::channel::Receiver;

use miette::Result;

use crate::console::backends::shared::{QueueItem, QueueItemID, QueueType};

pub trait TerminalBackend {
    /// Initialize the terminal backend.
    fn setup(&mut self) -> Result<()>;
    
    /// Clean up the terminal backend.
    fn destroy(&mut self) -> Result<()>;
}

pub trait LogBackend {
    /// Print a new empty line into the log.
    fn log_newline(&self);
    
    /// Print a string into the log, followed by a new line.
    fn log_println(&self, content: Box<dyn Display>);
}

pub trait TranscodeBackend {
    /// Initialize the queue system. This should be called before any other `queue_*` methods.
    fn queue_begin(&mut self);
    
    /// Clean up the queue system.
    fn queue_end(&mut self);
    
    /// Add an item to the queue.
    fn queue_item_add(&mut self, item: String, item_type: QueueType) -> Result<QueueItemID>;
    
    /// Mark the item in queue as "in-progress".
    fn queue_item_start(&mut self, item_id: QueueItemID) -> Result<()>;
    
    /// Mark the item in queue as "finished", with additional result context provided by `was_ok`.
    fn queue_item_finish(&mut self, item_id: QueueItemID, was_ok: bool) -> Result<()>;
    
    /// Fetch a mutable reference to the given queue item, allowing you to modify its contents.
    /// This is done by providing a function that will take the mutable reference and modify it.
    fn queue_item_modify(
        &mut self,
        item_id: QueueItemID,
        function: Box<dyn FnOnce(&mut QueueItem)>,
    ) -> Result<()>;
    
    /// Remove the item from the queue.
    fn queue_item_remove(&mut self, item_id: QueueItemID) -> Result<()>;
    
    /// Clear the entire queue (of the given type).
    fn queue_clear(&mut self, queue_type: QueueType) -> Result<()>;
    
    /// Enable the progress bar. This must be called before any other progress bar-related methods.
    fn progress_begin(&mut self);
    
    /// Disable the progress bar.
    fn progress_end(&mut self);
    
    /// Set the total number of tasks to show in the progress bar.
    fn progress_set_total(&mut self, total: usize) -> Result<()>;
    
    /// Set the currently completed number of tasks to show in the progress bar (should be less or equal to total).
    fn progress_set_current(&mut self, finished: usize) -> Result<()>;
}

pub struct ValidationErrorInfo {
    pub header: String,
    pub attributes: Vec<(String, String)>,
}

impl ValidationErrorInfo {
    pub fn new<H: Into<String>>(
        header: H,
        attributes: Vec<(String, String)>,
    ) -> Self {
        Self {
            header: header.into(),
            attributes,
        }
    }
}

pub trait ValidationBackend {
    fn validation_add_error(&self, error: ValidationErrorInfo);
}

#[derive(Copy, Clone)]
pub enum UserControlMessage {
    Exit,
}

pub trait UserControllableBackend {
    fn get_user_control_receiver(&mut self) -> Result<Receiver<UserControlMessage>>;
}

pub trait LogToFileBackend {
    fn enable_saving_logs_to_file(&mut self, log_file_path: PathBuf) -> Result<()>;
    fn disable_saving_logs_to_file(&mut self) -> Result<()>;
}

/// Terminal backends that implement this only allow for basic logging and saving logs to file.
pub trait SimpleTerminalBackend: TerminalBackend + LogBackend + LogToFileBackend {}

/// Terminal backends that implement this allow for basic logging, saving logs to file and validation actions.
pub trait FullValidationBackend: TerminalBackend + LogBackend + ValidationBackend + LogToFileBackend {}

/// Terminal backends that implement this allow for basic logging, transcoding actions, are user-controllable and allow saving logs to file.
pub trait AdvancedTranscodeTerminalBackend: TerminalBackend + LogBackend + TranscodeBackend + UserControllableBackend + LogToFileBackend {}
