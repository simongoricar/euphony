use std::fmt::Display;
use std::path::PathBuf;

use crossbeam::channel::Receiver;
use miette::Result;

use crate::console::backends::shared::{QueueItem, QueueItemID, QueueType};

/// The way multiple UI backends are done in euphony is via a set of terminal backend traits.
/// This is the base. All terminal backends must implement this.
pub trait TerminalBackend {
    /// Initialize the terminal backend.
    fn setup(&mut self) -> Result<()>;

    /// Clean up the terminal backend.
    fn destroy(&mut self) -> Result<()>;
}

/// Allows backends to print out content and newlines.
pub trait LogBackend {
    /// Print a new empty line into the log.
    fn log_newline(&self);

    /// Print a string into the log, followed by a new line.
    fn log_println<D: Display>(&self, content: D);
}

/// Allows backends to be used in transcoding process. This means the implementor
/// must maintain some form of (purely visual) queue system and a way of monitoring progress.
pub trait TranscodeBackend {
    /// Initialize the queue system. This should be called before any other `queue_*` methods.
    fn queue_begin(&mut self);

    /// Clean up the queue system.
    fn queue_end(&mut self);

    /// Add an item to the queue.
    fn queue_item_add(
        &mut self,
        item: String,
        item_type: QueueType,
    ) -> Result<QueueItemID>;

    /// Mark the item in queue as "in-progress".
    fn queue_item_start(&mut self, item_id: QueueItemID) -> Result<()>;

    /// Mark the item in queue as "finished", with additional result context provided by `was_ok`.
    fn queue_item_finish(
        &mut self,
        item_id: QueueItemID,
        was_ok: bool,
    ) -> Result<()>;

    /// Fetch a mutable reference to the given queue item, allowing you to modify its contents.
    /// This is done by providing a closure in the second argument that will take the mutable reference and modify it.
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

    /// Disable the progress bar (potentially represented in the implementor as hiding the bar or greying it out).
    fn progress_end(&mut self);

    /// Set the total number of tasks to show in the progress bar.
    fn progress_set_total(&mut self, num_total: usize) -> Result<()>;

    /// Set the currently completed number of tasks to show in the progress bar (should be less or equal to total).
    fn progress_set_current(&mut self, num_finished: usize) -> Result<()>;
}

/// Shared format for validation errors.
/// Consists of:
/// - a header that describes the general validation error and
/// - a set of key-value attributes that further explain the details of this error.
///
/// For example, the header might be "Invalid file found in the album directory." and
/// we could potentially have the following attributes: \[("Library": "Standard", "File": "./some/filepath.wav")]
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

/// Allows backends to be used for displaying collection validation results.
pub trait ValidationBackend {
    fn validation_add_error(&self, error: ValidationErrorInfo);
}

/// Describes all the possible user inputs that can be received from the backend.
#[derive(Copy, Clone)]
pub enum UserControlMessage {
    Exit,
}

/// Allows user input (whatever that means for the implementor - generally a key press)
/// in the form of a `UserControlMessage` that describes the user's action.
/// It is up to the backend to parse and kind of raw user input and parse it into a `UserControlMessage`.
pub trait UserControllableBackend {
    fn get_user_control_receiver(
        &mut self,
    ) -> Result<Receiver<UserControlMessage>>;
}

/// Allows saving `LogBackend`'s log output to file (usually in addition to the terminal or whatever).
pub trait LogToFileBackend {
    fn enable_saving_logs_to_file(
        &mut self,
        log_file_path: PathBuf,
    ) -> Result<()>;
    fn disable_saving_logs_to_file(&mut self) -> Result<()>;
}
