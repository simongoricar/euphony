use std::fmt::Display;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::thread::Scope;

use crossterm::style::{Color, Stylize};
use miette::{miette, Context, IntoDiagnostic, Result};
use parking_lot::{Mutex, RwLock};
use tokio::sync::broadcast;

use crate::console::backends::shared::queue::{
    AlbumQueueItem,
    AlbumQueueItemFinishedResult,
    FileQueueItem,
    FileQueueItemFinishedResult,
    Queue,
    QueueItem,
    QueueItemID,
    RenderableQueueItem,
};
use crate::console::backends::shared::Progress;
use crate::console::traits::{
    LogToFileBackend,
    UserControllableBackend,
    ValidationBackend,
    ValidationErrorInfo,
};
use crate::console::{
    LogBackend,
    TerminalBackend,
    TranscodeBackend,
    UserControlMessage,
};

pub struct QueueAndProgressState<'config> {
    /// The album queue, when enabled.
    album_queue:
        Option<Queue<AlbumQueueItem<'config>, AlbumQueueItemFinishedResult>>,

    /// The file queue, when enabled.
    file_queue:
        Option<Queue<FileQueueItem<'config>, FileQueueItemFinishedResult>>,

    /// When the progress bar is active, this contains the progress bar state.
    progress: Option<Progress>,
}

impl<'config> QueueAndProgressState<'config> {
    pub fn new() -> Self {
        Self {
            album_queue: None,
            file_queue: None,
            progress: None,
        }
    }
}


/// A simple non-dynamic terminal backend implementation.
///
/// Any log output simply goes to stdout. More complex features, such as queues, are not displayed
/// dynamically as a UI, but with simple one-line status updates about the queue (e.g. "New item in queue: ...").
pub struct BareTerminalBackend<'config> {
    state: RwLock<QueueAndProgressState<'config>>,

    /// If log file output is enabled, this contains the mutex in front of the file writer.
    log_file_output: Mutex<Option<BufWriter<strip_ansi_escapes::Writer<File>>>>,

    broadcast_sender: Mutex<broadcast::Sender<UserControlMessage>>,
}

impl<'config> BareTerminalBackend<'config> {
    pub fn new() -> Self {
        let (broadcast_sender, _) = broadcast::channel(1);

        Self {
            state: RwLock::new(QueueAndProgressState::new()),
            log_file_output: Mutex::new(None),
            broadcast_sender: Mutex::new(broadcast_sender),
        }
    }
}

impl<'config, 'scope, 'scope_env: 'scope> TerminalBackend<'scope, 'scope_env>
    for BareTerminalBackend<'config>
{
    fn setup(&self, _scope: &'scope Scope<'scope, 'scope_env>) -> Result<()> {
        Ok(())
    }

    fn destroy(self) -> Result<()> {
        // If logging to file was enabled, we should disable it before this backend is dropped,
        // otherwise we risk failing to flush to file.
        self.disable_saving_logs_to_file()?;

        Ok(())
    }
}

impl<'config> LogBackend for BareTerminalBackend<'config> {
    fn log_newline(&self) {
        println!();

        if let Some(writer) = self.log_file_output.lock().as_mut() {
            writer
                .write_all("\n".as_bytes())
                .expect("Could not write to logfile.");
        }
    }

    fn log_println<D: Display>(&self, content: D) {
        let content_string = content.to_string();

        println!("{content_string}");

        if let Some(writer) = self.log_file_output.lock().as_mut() {
            writer
                .write_all(content_string.as_bytes())
                .expect("Could not write to logfile.");
            writer
                .write_all("\n".as_bytes())
                .expect("Could not write to logfile (newline).");
        }
    }
}

impl<'config> TranscodeBackend<'config> for BareTerminalBackend<'config> {
    /*
     * Album queue
     */
    fn queue_album_enable(&self) {
        self.log_println("Album queue enabled.");

        let mut locked_state = self.state.write();
        locked_state.album_queue = Some(Queue::new());
    }

    fn queue_album_disable(&self) {
        self.log_println("Album queue disabled.");

        let mut locked_state = self.state.write();
        locked_state.album_queue = None;
    }

    fn queue_album_clear(&self) -> Result<()> {
        self.log_println("Album queue cleared.");


        let mut locked_state = self.state.write();
        locked_state
            .album_queue
            .as_mut()
            .ok_or_else(|| miette!("Album queue is disabled, can't clear."))?
            .clear();

        Ok(())
    }

    fn queue_album_item_add(
        &self,
        item: AlbumQueueItem<'config>,
    ) -> Result<QueueItemID> {
        let item_id = item.get_id();

        self.log_println(format!(
            "Album queue item added: {}",
            item.render()
        ));

        let mut locked_state = self.state.write();
        locked_state
            .album_queue
            .as_mut()
            .ok_or_else(|| miette!("Album queue is disabled, can't clear."))?
            .queue_item(item)?;

        Ok(item_id)
    }

    fn queue_album_item_start(&self, item_id: QueueItemID) -> Result<()> {
        let mut locked_state = self.state.write();
        let album_queue =
            locked_state.album_queue.as_mut().ok_or_else(|| {
                miette!("Album queue is disabled, can't start item.")
            })?;


        album_queue.start_item(item_id)?;

        let item = album_queue
            .item(item_id)
            .ok_or_else(|| miette!("Invalid item_id, no such item."))?;
        let item_rendered = item.render();

        self.log_println(format!(
            "Album queue item started: {item_rendered}"
        ));

        Ok(())
    }

    fn queue_album_item_finish(
        &self,
        item_id: QueueItemID,
        result: AlbumQueueItemFinishedResult,
    ) -> Result<()> {
        let mut locked_state = self.state.write();
        let album_queue =
            locked_state.album_queue.as_mut().ok_or_else(|| {
                miette!("Album queue is disabled, can't finish item.")
            })?;

        album_queue.finish_item(item_id, result)?;

        let item = album_queue
            .item(item_id)
            .ok_or_else(|| miette!("Invalid item_id, no such item."))?;
        let item_rendered = item.render();

        self.log_println(format!(
            "Album queue item finished: {item_rendered} (result: {result:?})"
        ));

        Ok(())
    }

    fn queue_album_item_remove(
        &self,
        item_id: QueueItemID,
    ) -> Result<AlbumQueueItem<'config>> {
        let mut locked_state = self.state.write();
        let album_queue =
            locked_state.album_queue.as_mut().ok_or_else(|| {
                miette!("Album queue is disabled, can't remove item.")
            })?;

        album_queue.remove_item(item_id)
    }

    /*
     * File queue
     */
    fn queue_file_enable(&self) {
        self.log_println("File queue enabled.");

        let mut locked_state = self.state.write();
        locked_state.file_queue = Some(Queue::new());
    }

    fn queue_file_disable(&self) {
        self.log_println("File queue disabled.");

        let mut locked_state = self.state.write();
        locked_state.file_queue = None;
    }

    fn queue_file_clear(&self) -> Result<()> {
        self.log_println("File queue cleared.");

        let mut locked_state = self.state.write();
        locked_state
            .file_queue
            .as_mut()
            .ok_or_else(|| miette!("File queue is disabled, can't clear."))?
            .clear();

        Ok(())
    }

    fn queue_file_item_add(
        &self,
        item: FileQueueItem<'config>,
    ) -> Result<QueueItemID> {
        let item_id = item.get_id();

        self.log_println(format!(
            "File queue item added: {}",
            item.render()
        ));

        let mut locked_state = self.state.write();
        locked_state
            .file_queue
            .as_mut()
            .ok_or_else(|| miette!("File queue is disabled, can't add item."))?
            .queue_item(item)?;

        Ok(item_id)
    }

    fn queue_file_item_start(&self, item_id: QueueItemID) -> Result<()> {
        let mut locked_state = self.state.write();
        let file_queue = locked_state
            .file_queue
            .as_mut()
            .ok_or_else(|| miette!("File queue is disabled, can't add item."))?;


        file_queue.start_item(item_id)?;

        let item = file_queue
            .item(item_id)
            .ok_or_else(|| miette!("Invalid item_id, no such item."))?;
        let item_rendered = item.render();

        self.log_println(format!(
            "File queue item started: {item_rendered}"
        ));

        Ok(())
    }

    fn queue_file_item_finish(
        &self,
        item_id: QueueItemID,
        result: FileQueueItemFinishedResult,
    ) -> Result<()> {
        let mut locked_state = self.state.write();
        let file_queue = locked_state
            .file_queue
            .as_mut()
            .ok_or_else(|| miette!("File queue is disabled, can't add item."))?;


        let result_string = format!("{result:?}");

        file_queue.finish_item(item_id, result)?;

        let item = file_queue
            .item(item_id)
            .ok_or_else(|| miette!("Invalid item_id, no such item."))?;
        let item_rendered = item.render();

        self.log_println(format!(
            "File queue item finished: {item_rendered} (result: {result_string})"
        ));

        Ok(())
    }

    fn queue_file_item_remove(
        &self,
        item_id: QueueItemID,
    ) -> Result<FileQueueItem<'config>> {
        let mut locked_state = self.state.write();
        let file_queue = locked_state
            .file_queue
            .as_mut()
            .ok_or_else(|| miette!("File queue is disabled, can't add item."))?;

        file_queue.remove_item(item_id)
    }

    /*
     * Progress
     */
    fn progress_enable(&self) {
        println!("Progress bar enabled.");

        let mut locked_state = self.state.write();
        locked_state.progress = Some(Progress::default());
    }

    fn progress_disable(&self) {
        println!("Progress bar disabled.");

        let mut locked_state = self.state.write();
        locked_state.progress = None;
    }

    fn progress_set_total(&self, num_total: usize) -> Result<()> {
        let mut locked_state = self.state.write();

        match locked_state.progress.as_mut() {
            Some(progress) => {
                progress.total_files = num_total;
                Ok(())
            }
            None => Err(miette!(
                "Progress bar is disabled, can't set total."
            )),
        }
    }

    fn progress_set_audio_files_currently_processing(
        &self,
        num_audio_files_currently_processing: usize,
    ) -> Result<()> {
        let mut locked_state = self.state.write();

        match locked_state.progress.as_mut() {
            Some(progress) => {
                progress.audio_files_currently_processing =
                    num_audio_files_currently_processing;
                Ok(())
            }
            None => Err(miette!(
                "Progress bar is disabled, can't set currently processing audio files amount."
            )),
        }
    }

    fn progress_set_data_files_currently_processing(
        &self,
        num_data_files_currently_processing: usize,
    ) -> Result<()> {
        let mut locked_state = self.state.write();

        match locked_state.progress.as_mut() {
            Some(progress) => {
                progress.data_files_currently_processing =
                    num_data_files_currently_processing;
                Ok(())
            }
            None => Err(miette!(
                "Progress bar is disabled, can't set currently processing data files amount."
            )),
        }
    }

    fn progress_set_audio_files_finished_ok(
        &self,
        num_audio_files_finished_ok: usize,
    ) -> Result<()> {
        let mut locked_state = self.state.write();

        match locked_state.progress.as_mut() {
            Some(progress) => {
                progress.audio_files_finished_ok = num_audio_files_finished_ok;
                Ok(())
            }
            None => Err(miette!(
                "Progress bar is disabled, can't set audio finished ok."
            )),
        }
    }

    fn progress_set_data_files_finished_ok(
        &self,
        num_data_files_finished_ok: usize,
    ) -> Result<()> {
        let mut locked_state = self.state.write();

        match locked_state.progress.as_mut() {
            Some(progress) => {
                progress.data_files_finished_ok = num_data_files_finished_ok;
                Ok(())
            }
            None => Err(miette!(
                "Progress bar is disabled, can't set data finished ok."
            )),
        }
    }

    fn progress_set_audio_files_errored(
        &self,
        num_audio_files_errored: usize,
    ) -> Result<()> {
        let mut locked_state = self.state.write();

        match locked_state.progress.as_mut() {
            Some(progress) => {
                progress.audio_files_errored = num_audio_files_errored;
                Ok(())
            }
            None => Err(miette!(
                "Progress bar is disabled, can't set audio files errored."
            )),
        }
    }

    fn progress_set_data_files_errored(
        &self,
        num_data_files_errored: usize,
    ) -> Result<()> {
        let mut locked_state = self.state.write();

        match locked_state.progress.as_mut() {
            Some(progress) => {
                progress.data_files_errored = num_data_files_errored;
                Ok(())
            }
            None => Err(miette!(
                "Progress bar is disabled, can't set data files errored."
            )),
        }
    }
}

impl<'config> ValidationBackend for BareTerminalBackend<'config> {
    fn validation_add_error(&self, error: ValidationErrorInfo) {
        self.log_newline();
        self.log_newline();

        let formatted_header = format!(
            "{} {}",
            "#".bold().with(Color::AnsiValue(142)), // Gold3 (#afaf00)
            error.header.bold()
        );
        let formatted_attributes = error
            .attributes
            .iter()
            .map(|(name, value)| {
                format!("{}: {}", name.to_string().dark_yellow(), value)
            })
            .collect::<Vec<String>>()
            .join("\n");

        self.log_println(Box::new(format!(
            "{formatted_header}\n{formatted_attributes}",
        )));
    }
}

impl<'config> UserControllableBackend for BareTerminalBackend<'config> {
    fn get_user_control_receiver(
        &self,
    ) -> Result<broadcast::Receiver<UserControlMessage>> {
        Ok(self.broadcast_sender.lock().subscribe())
    }
}

impl<'config, 'scope, 'scope_env: 'scope> LogToFileBackend<'scope, 'scope_env>
    for BareTerminalBackend<'config>
{
    fn enable_saving_logs_to_file<P: AsRef<Path>>(
        &self,
        log_file_path: P,
        _scope: &'scope Scope<'scope, 'scope_env>,
    ) -> Result<()> {
        let file = File::create(log_file_path).into_diagnostic()?;
        let ansi_escaped_writer = strip_ansi_escapes::Writer::new(file);
        let buf_writer = BufWriter::with_capacity(1024, ansi_escaped_writer);

        let mut locked_log_output = self.log_file_output.lock();
        *locked_log_output = Some(buf_writer);

        Ok(())
    }

    fn disable_saving_logs_to_file(&self) -> Result<()> {
        let mut locked_log_output = self.log_file_output.lock();

        if let Some(writer) = locked_log_output.take() {
            let mut inner_writer = writer
                .into_inner()
                .map_err(|_| miette!("Failed to unwrap the BufWriter."))?
                .into_inner()
                .map_err(|_| {
                    miette!("Failed to unwrap the ansi escape writer.")
                })?;

            inner_writer.flush().into_diagnostic().wrap_err_with(|| {
                miette!("Failed to perform final flush on the File.")
            })?;
        }

        Ok(())
    }
}
