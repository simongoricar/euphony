use std::fmt::Display;
use std::path::PathBuf;

use crossbeam::channel::Receiver;
use miette::Result;

use crate::console::backends::shared::queue_v2::{
    AlbumItem,
    AlbumItemFinishedResult,
    FileItem,
    FileItemFinishedResult,
    QueueItemID,
};

/// The way multiple UI backends are done in euphony is via a set of terminal backend traits.
/// **This is the base. All terminal backends must implement this.**
///
/// For further information details see `src/console/backends/mod.rs`.
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
pub trait TranscodeBackend<'a> {
    /*
     * Album queue
     */
    /// Initialize the album queue system.
    /// This should be called before any other `queue_album_*` methods.
    fn queue_album_enable(&mut self);

    /// Clean up and disable the album queue system.
    fn queue_album_disable(&mut self);

    /// Clear the whole album queue.
    fn queue_album_clear(&mut self) -> Result<()>;

    /// Add an album to the album queue. This will give it the `AlbumItemState::Queued` state.
    fn queue_album_item_add(
        &mut self,
        item: AlbumItem<'a>,
    ) -> Result<QueueItemID>;

    /// Mark the given album in the album queue as "in-progress".
    /// This will give it the `AlbumItemState::InProgress` state.
    fn queue_album_item_start(&mut self, item_id: QueueItemID) -> Result<()>;

    /// Mark the given album in the album queue as "finished".
    /// This will give it the `AlbumItemState:Finished` state and the given `result`.
    fn queue_album_item_finish(
        &mut self,
        item_id: QueueItemID,
        result: AlbumItemFinishedResult,
    ) -> Result<()>;

    /// Remove an album from the album queue.
    fn queue_album_item_remove(
        &mut self,
        item_id: QueueItemID,
    ) -> Result<AlbumItem<'a>>;

    /*
     * File queue
     */
    /// Initialize the file queue system.
    /// This should be called before any other `queue_file_*` methods.
    fn queue_file_enable(&mut self);

    /// Clean up and disable the file queue system.
    fn queue_file_disable(&mut self);

    /// Clear the whole file queue.
    fn queue_file_clear(&mut self) -> Result<()>;

    /// Add a file to the file queue. This will give it the `FileItemState::Queued` state.
    fn queue_file_item_add(&mut self, item: FileItem<'a>)
        -> Result<QueueItemID>;

    /// Mark the given file in the file queue as "in-progress".
    /// This will give it the `FileItemState::InProgress` state.
    fn queue_file_item_start(&mut self, item_id: QueueItemID) -> Result<()>;

    /// Mark the given file in the file queue as "finished".
    /// This will give it the `FileItemState:Finished` state and the given `result`.
    fn queue_file_item_finish(
        &mut self,
        item_id: QueueItemID,
        result: FileItemFinishedResult,
    ) -> Result<()>;

    /// Remove a file from the file queue.
    fn queue_file_item_remove(
        &mut self,
        item_id: QueueItemID,
    ) -> Result<FileItem<'a>>;

    /*
     * Progress bar
     */
    /// Enable the progress bar. This must be called before any other `progress_*` methods.
    fn progress_enable(&mut self);

    /// Disable the progress bar (potentially represented in the implementor as hiding the bar or greying it out).
    fn progress_disable(&mut self);

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
