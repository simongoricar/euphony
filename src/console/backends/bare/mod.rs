use std::fmt::Display;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use crossbeam::channel::{never, Receiver};
use crossbeam::thread::Scope;
use crossterm::style::{Color, Stylize};
use miette::{miette, IntoDiagnostic, Result};
use strip_ansi_escapes::Writer;

use crate::console::backends::shared::queue_v2::{
    AlbumItem,
    AlbumItemFinishedResult,
    FileItem,
    FileItemFinishedResult,
    Queue,
    QueueItem,
    QueueItemID,
    RenderableQueueItem,
};
use crate::console::backends::shared::ProgressState;
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

/// A simple non-dynamic terminal backend implementation.
///
/// Any log output simply goes to stdout. More complex features, such as queues, are not displayed
/// dynamically as a UI, but with simple one-line status updates about the queue (e.g. "New item in queue: ...").
pub struct BareTerminalBackend<'config> {
    /// The album queue, when enabled.
    album_queue: Option<Queue<AlbumItem<'config>, AlbumItemFinishedResult>>,

    /// The file queue, when enabled.
    file_queue: Option<Queue<FileItem<'config>, FileItemFinishedResult>>,

    /// When the progress bar is active, this contains the progress bar state.
    progress: Option<ProgressState>,

    /// If log file output is enabled, this contains the mutex in front of the file writer.
    log_file_output: Option<Mutex<BufWriter<Writer<File>>>>,
}

impl<'config> BareTerminalBackend<'config> {
    pub fn new() -> Self {
        Self {
            album_queue: None,
            file_queue: None,
            progress: None,
            log_file_output: None,
        }
    }
}

impl<'config: 'scope, 'scope> TerminalBackend<'scope>
    for BareTerminalBackend<'config>
{
    fn setup(&mut self, _thread_scope: &'scope Scope<'scope>) -> Result<()> {
        Ok(())
    }

    fn destroy(mut self) -> Result<()> {
        // If logging to file was enabled, we should disable it before this backend is dropped,
        // otherwise we risk failing to flush to file.
        self.disable_saving_logs_to_file()?;

        Ok(())
    }
}

impl<'config> LogBackend for BareTerminalBackend<'config> {
    fn log_newline(&self) {
        println!();

        if let Some(writer) = self.log_file_output.as_ref() {
            let mut writer_locked =
                writer.lock().expect("writer lock has been poisoned!");

            writer_locked
                .write_all("\n".as_bytes())
                .expect("Could not write to logfile.");
        }
    }

    fn log_println<D: Display>(&self, content: D) {
        let content_string = content.to_string();

        println!("{content_string}");

        if let Some(writer) = self.log_file_output.as_ref() {
            let mut writer_locked =
                writer.lock().expect("writer lock has been poisoned!");

            writer_locked
                .write_all(content_string.as_bytes())
                .expect("Could not write to logfile.");
            writer_locked
                .write_all("\n".as_bytes())
                .expect("Could not write to logfile (newline).");
        }
    }
}

impl<'config> TranscodeBackend<'config> for BareTerminalBackend<'config> {
    /*
     * Album queue
     */
    fn queue_album_enable(&mut self) {
        self.log_println("Album queue enabled.");
        self.album_queue = Some(Queue::new());
    }

    fn queue_album_disable(&mut self) {
        self.log_println("Album queue disabled.");
        self.album_queue = None;
    }

    fn queue_album_clear(&mut self) -> Result<()> {
        self.log_println("Album queue cleared.");

        let queue = self
            .album_queue
            .as_mut()
            .ok_or_else(|| miette!("Album queue is disabled, can't clear."))?;
        queue.clear();

        Ok(())
    }

    fn queue_album_item_add(
        &mut self,
        item: AlbumItem<'config>,
    ) -> Result<QueueItemID> {
        let item_id = item.get_id();

        self.log_println(format!(
            "Album queue item added: {}",
            item.render()
        ));

        let queue = self.album_queue.as_mut().ok_or_else(|| {
            miette!("Album queue is disabled, can't add item.")
        })?;
        queue.add_item(item)?;

        Ok(item_id)
    }

    fn queue_album_item_start(&mut self, item_id: QueueItemID) -> Result<()> {
        // TODO Fix .as_mut, then &reference use
        let album_queue_locked = self.album_queue.as_mut().ok_or_else(|| {
            miette!("Album queue is disabled, can't start item.")
        })?;

        album_queue_locked.start_item(item_id)?;

        let item = album_queue_locked.item(item_id)?;
        let item_rendered = item.render();

        self.log_println(format!(
            "Album queue item started: {item_rendered}"
        ));

        Ok(())
    }

    fn queue_album_item_finish(
        &mut self,
        item_id: QueueItemID,
        result: AlbumItemFinishedResult,
    ) -> Result<()> {
        let queue = self.album_queue.as_mut().ok_or_else(|| {
            miette!("Album queue is disabled, can't finish item.")
        })?;

        queue.finish_item(item_id, result)?;

        let item = queue.item(item_id)?;
        let item_rendered = item.render();

        self.log_println(format!(
            "Album queue item finished: {item_rendered} (result: {result:?})"
        ));

        Ok(())
    }

    fn queue_album_item_remove(
        &mut self,
        item_id: QueueItemID,
    ) -> Result<AlbumItem<'config>> {
        let queue = self.album_queue.as_mut().ok_or_else(|| {
            miette!("Album queue is disabled, can't remove item.")
        })?;

        queue.remove_item(item_id)
    }

    /*
     * File queue
     */
    fn queue_file_enable(&mut self) {
        self.log_println("File queue enabled.");
        self.file_queue = Some(Queue::new());
    }

    fn queue_file_disable(&mut self) {
        self.log_println("File queue disabled.");
        self.file_queue = None;
    }

    fn queue_file_clear(&mut self) -> Result<()> {
        let queue = self
            .file_queue
            .as_mut()
            .ok_or_else(|| miette!("File queue is disabled, can't clear."))?;

        queue.clear();
        self.log_println("File queue cleared.");

        Ok(())
    }

    fn queue_file_item_add(
        &mut self,
        item: FileItem<'config>,
    ) -> Result<QueueItemID> {
        let queue = self
            .file_queue
            .as_mut()
            .ok_or_else(|| miette!("File queue is disabled, can't add item."))?;

        let item_id = item.get_id();

        queue.add_item(item)?;

        let added_item = queue.item(item_id)?;
        let added_item_rendered = added_item.render();

        self.log_println(format!(
            "File queue item added: {added_item_rendered}"
        ));

        Ok(item_id)
    }

    fn queue_file_item_start(&mut self, item_id: QueueItemID) -> Result<()> {
        let queue = self.file_queue.as_mut().ok_or_else(|| {
            miette!("File queue is disabled, can't start item.")
        })?;

        queue.start_item(item_id)?;

        let item = queue.item(item_id)?;
        let item_rendered = item.render();

        self.log_println(format!(
            "File queue item started: {item_rendered}"
        ));

        Ok(())
    }

    fn queue_file_item_finish(
        &mut self,
        item_id: QueueItemID,
        result: FileItemFinishedResult,
    ) -> Result<()> {
        let queue = self.file_queue.as_mut().ok_or_else(|| {
            miette!("File queue is disabled, can't finish item.")
        })?;

        let result_string = format!("{result:?}");

        queue.finish_item(item_id, result)?;

        let item = queue.item(item_id)?;
        let item_rendered = item.render();

        self.log_println(format!(
            "File queue item finished: {item_rendered} (result: {result_string})"
        ));

        Ok(())
    }

    fn queue_file_item_remove(
        &mut self,
        item_id: QueueItemID,
    ) -> Result<FileItem<'config>> {
        let queue = self.file_queue.as_mut().ok_or_else(|| {
            miette!("File queue is disabled, can't remove item.")
        })?;

        queue.remove_item(item_id)
    }

    /*
     * Progress
     */
    fn progress_enable(&mut self) {
        println!("Progress bar enabled.");
        self.progress = Some(ProgressState::default());
    }

    fn progress_disable(&mut self) {
        println!("Progress bar disabled.");
        self.progress = None;
    }

    fn progress_set_total(&mut self, total: usize) -> Result<()> {
        if let Some(progress) = &mut self.progress {
            progress.total = total;
            Ok(())
        } else {
            Err(miette!(
                "Progress bar is currently disabled, can't set total."
            ))
        }
    }

    fn progress_set_current(&mut self, current: usize) -> Result<()> {
        if let Some(progress) = &mut self.progress {
            progress.current = current;
            Ok(())
        } else {
            Err(miette!(
                "Progress bar is currently disabled, can't set current."
            ))
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
        &mut self,
    ) -> Result<Receiver<UserControlMessage>> {
        Ok(never::<UserControlMessage>())
    }
}

impl<'config> LogToFileBackend for BareTerminalBackend<'config> {
    fn enable_saving_logs_to_file(
        &mut self,
        log_file_path: PathBuf,
    ) -> Result<()> {
        let file = File::create(log_file_path).into_diagnostic()?;

        let ansi_escaped_file_writer = Writer::new(file);

        let buf_writer =
            BufWriter::with_capacity(1024, ansi_escaped_file_writer);
        self.log_file_output = Some(Mutex::new(buf_writer));

        Ok(())
    }

    fn disable_saving_logs_to_file(&mut self) -> Result<()> {
        if let Some(buf_writer) = self.log_file_output.take() {
            let mut buf_writer = buf_writer.into_inner().into_diagnostic()?;

            buf_writer.flush().into_diagnostic()?;
        }

        Ok(())
    }
}
