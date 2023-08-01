use std::fmt::Display;
use std::path::Path;
use std::thread::Scope;

use miette::Result;
use tokio::sync::broadcast;

use crate::console::backends::shared::queue::{
    AlbumQueueItem,
    AlbumQueueItemFinishedResult,
    FileQueueItem,
    FileQueueItemFinishedResult,
    QueueItemID,
};

/// The way multiple UI backends are done in euphony is via a set of terminal backend traits.
/// **This is the base. All terminal backends must implement this.**
///
/// For further information details see `src/console/backends/mod.rs`.
pub trait TerminalBackend<'scope, 'scope_env: 'scope> {
    /// Initialize the terminal backend.
    fn setup(&self, scope: &'scope Scope<'scope, 'scope_env>) -> Result<()>;

    /// Clean up the terminal backend.
    fn destroy(self) -> Result<()>;
}

/// Allows backends to print out content and newlines.
pub trait LogBackend {
    /// Print a new empty line into the log.
    fn log_newline(&self);

    /// Print a string into the log, followed by a new line.
    fn log_println<D: Display>(&self, content: D);
}

/// Allows saving `LogBackend`'s log output to file (usually in addition to the terminal or whatever).
pub trait LogToFileBackend<'scope, 'scope_env: 'scope> {
    fn enable_saving_logs_to_file<P: AsRef<Path>>(
        &self,
        log_file_path: P,
        scope: &'scope Scope<'scope, 'scope_env>,
    ) -> Result<()>;
    fn disable_saving_logs_to_file(&self) -> Result<()>;
}

/// Allows backends to be used in transcoding process. This means the implementor
/// must maintain some form of (purely visual) queue system and a way of monitoring progress.
pub trait TranscodeBackend<'config> {
    /*
     * Album queue
     */
    /// Initialize the album queue system.
    /// This should be called before any other `queue_album_*` methods.
    fn queue_album_enable(&self);

    /// Clean up and disable the album queue system.
    fn queue_album_disable(&self);

    /// Clear the whole album queue.
    fn queue_album_clear(&self) -> Result<()>;

    /// Add an album to the album queue. This will give it the `AlbumItemState::Queued` state.
    fn queue_album_item_add(
        &self,
        item: AlbumQueueItem<'config>,
    ) -> Result<QueueItemID>;

    /// Mark the given album in the album queue as "in-progress".
    /// This will give it the `AlbumItemState::InProgress` state.
    fn queue_album_item_start(&self, item_id: QueueItemID) -> Result<()>;

    /// Mark the given album in the album queue as "finished".
    /// This will give it the `AlbumItemState:Finished` state and the given `result`.
    fn queue_album_item_finish(
        &self,
        item_id: QueueItemID,
        result: AlbumQueueItemFinishedResult,
    ) -> Result<()>;

    /// Remove an album from the album queue.
    fn queue_album_item_remove(
        &self,
        item_id: QueueItemID,
    ) -> Result<AlbumQueueItem<'config>>;

    /*
     * File queue
     */
    /// Initialize the file queue system.
    /// This should be called before any other `queue_file_*` methods.
    fn queue_file_enable(&self);

    /// Clean up and disable the file queue system.
    fn queue_file_disable(&self);

    /// Clear the whole file queue.
    fn queue_file_clear(&self) -> Result<()>;

    /// Add a file to the file queue. This will give it the `FileItemState::Queued` state.
    fn queue_file_item_add(
        &self,
        item: FileQueueItem<'config>,
    ) -> Result<QueueItemID>;

    /// Mark the given file in the file queue as "in-progress".
    /// This will give it the `FileItemState::InProgress` state.
    fn queue_file_item_start(&self, item_id: QueueItemID) -> Result<()>;

    /// Mark the given file in the file queue as "finished".
    /// This will give it the `FileItemState:Finished` state and the given `result`.
    fn queue_file_item_finish(
        &self,
        item_id: QueueItemID,
        result: FileQueueItemFinishedResult,
    ) -> Result<()>;

    /// Remove a file from the file queue.
    fn queue_file_item_remove(
        &self,
        item_id: QueueItemID,
    ) -> Result<FileQueueItem<'config>>;

    /*
     * Progress bar
     */
    /// Enable the progress bar. This must be called before any other `progress_*` methods.
    fn progress_enable(&self);

    /// Disable the progress bar (potentially represented in the implementor as hiding the bar or greying it out).
    fn progress_disable(&self);

    /// Set the total number of tasks to show in the progress bar.
    fn progress_set_total(&self, num_total: usize) -> Result<()>;

    fn progress_set_audio_files_currently_processing(
        &self,
        num_audio_files_currently_processing: usize,
    ) -> Result<()>;

    fn progress_set_data_files_currently_processing(
        &self,
        num_data_files_currently_processing: usize,
    ) -> Result<()>;

    fn progress_set_audio_files_finished_ok(
        &self,
        num_audio_files_finished_ok: usize,
    ) -> Result<()>;

    fn progress_set_data_files_finished_ok(
        &self,
        num_data_files_finished_ok: usize,
    ) -> Result<()>;

    fn progress_set_audio_files_errored(
        &self,
        num_audio_files_errored: usize,
    ) -> Result<()>;

    fn progress_set_data_files_errored(
        &self,
        num_data_files_errored: usize,
    ) -> Result<()>;
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
        &self,
    ) -> Result<broadcast::Receiver<UserControlMessage>>;
}
