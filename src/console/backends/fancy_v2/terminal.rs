use std::fmt::Display;
use std::fs::File;
use std::io::{stdout, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::thread::Scope;
use std::time::Duration;

use crossterm::ExecutableCommand;
use miette::{miette, Context, IntoDiagnostic, Result};
use parking_lot::{Mutex, RwLock};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use tokio::sync::broadcast;

use crate::cancellation::CancellationToken;
use crate::configuration::Config;
use crate::console::backends::fancy_v2::queue_items::{
    FancyAlbumQueueItem,
    FancyFileQueueItem,
};
use crate::console::backends::fancy_v2::rendering;
use crate::console::backends::fancy_v2::state::{
    LogOutputMode,
    LogState,
    TerminalState,
    UIPage,
    UIState,
};
use crate::console::backends::shared::queue::{
    AlbumQueueItem,
    AlbumQueueItemFinishedResult,
    FileQueueItem,
    FileQueueItemFinishedResult,
    Queue,
    QueueItem,
    QueueItemID,
};
use crate::console::backends::shared::Progress;
use crate::console::{
    LogBackend,
    LogToFileBackend,
    TerminalBackend,
    TranscodeBackend,
    UserControlMessage,
    UserControllableBackend,
};

const LOG_FILE_OUTPUT_FLUSHING_INTERVAL: Duration = Duration::from_secs(8);
const FLUSHING_THREAD_CANCELLATION_CHECK_INTERVAL: Duration =
    Duration::from_millis(100);


fn run_log_output_flushing_loop(
    log_file_writer: Arc<Mutex<BufWriter<strip_ansi_escapes::Writer<File>>>>,
    cancellation_token: CancellationToken,
) -> Result<()> {
    let mut time_accumulator = Duration::from_secs(0);

    loop {
        thread::sleep(FLUSHING_THREAD_CANCELLATION_CHECK_INTERVAL);
        time_accumulator += FLUSHING_THREAD_CANCELLATION_CHECK_INTERVAL;

        if cancellation_token.is_cancelled()
            || Arc::strong_count(&log_file_writer) == 1
        {
            // Either flushing was stopped or the original writer reference has been dropped
            // (and we have the only strong reference left).
            log_file_writer
                .lock()
                .flush()
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!("Failed to perform final log file writer flush.")
                })?;

            return Ok(());
        }

        if time_accumulator >= LOG_FILE_OUTPUT_FLUSHING_INTERVAL {
            log_file_writer
                .lock()
                .flush()
                .into_diagnostic()
                .wrap_err_with(|| {
                    miette!("Failed to perform log file writer flush.")
                })?;

            time_accumulator = Duration::from_secs(0);
        }
    }
}


pub struct FancyTerminalBackend<'thread_scope, 'config> {
    terminal_state: Arc<Mutex<Option<TerminalState<'thread_scope>>>>,

    log_state: Arc<Mutex<LogState<'thread_scope>>>,

    ui_state: Arc<RwLock<UIState<'config>>>,

    config: &'config Config,
}

impl<'thread_scope, 'config> FancyTerminalBackend<'thread_scope, 'config> {
    pub fn new(config: &'config Config) -> Result<Self> {
        let terminal_state = Arc::new(Mutex::new(None));
        let log_state = Arc::new(Mutex::new(LogState::new()));
        let ui_state = Arc::new(RwLock::new(UIState::new()));

        Ok(Self {
            terminal_state,
            log_state,
            ui_state,
            config,
        })
    }
}

impl<'scope, 'scope_env: 'scope, 'config: 'scope>
    TerminalBackend<'scope, 'scope_env>
    for FancyTerminalBackend<'scope, 'config>
{
    fn setup(&self, scope: &'scope Scope<'scope, 'scope_env>) -> Result<()> {
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))
            .into_diagnostic()
            .wrap_err_with(|| miette!("Failed to initialize terminal."))?;

        let (user_control_sender, _) =
            broadcast::channel::<UserControlMessage>(512);

        // Enable raw mode, make space in terminal for rendering (without obscuring the previous content)
        // and save the final cursor position (so we can restore it).
        crossterm::terminal::enable_raw_mode()
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Failed to enable raw mode for terminal window.")
            })?;

        let terminal_size = terminal
            .size()
            .into_diagnostic()
            .wrap_err_with(|| miette!("Failed to get terminal size."))?;

        let backend = terminal.backend_mut();

        backend
            .execute(crossterm::style::Print(
                "\n".repeat(terminal_size.height as usize),
            ))
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Failed to prepare terminal for rendering.")
            })?;

        let cursor_end_position =
            backend.get_cursor().into_diagnostic().wrap_err_with(|| {
                miette!("Failed to get terminal cursor position.")
            })?;

        let terminal_arc_mutex = Arc::new(Mutex::new(terminal));


        // Set up terminal rendering thread.
        let render_cancellation_token = CancellationToken::new();

        let terminal_arc_mutex_clone = terminal_arc_mutex.clone();
        let log_state_arc_clone = self.log_state.clone();
        let ui_state_arc_clone = self.ui_state.clone();
        let user_control_sender_clone = user_control_sender.clone();
        let render_cancellation_token_clone = render_cancellation_token.clone();

        let transcoding_ui_config = self.config.ui.transcoding.clone();

        let render_thread_join_handle = scope.spawn(move || {
            rendering::run_render_loop(
                terminal_arc_mutex_clone,
                transcoding_ui_config,
                log_state_arc_clone,
                ui_state_arc_clone,
                &user_control_sender_clone,
                render_cancellation_token_clone,
            )
        });


        let terminal_state = TerminalState {
            terminal: terminal_arc_mutex,
            cursor_end_position,
            user_control_sender,
            render_thread_join_handle,
            render_thread_cancellation_token: render_cancellation_token,
        };

        let mut locked_terminal_state = self.terminal_state.lock();
        *locked_terminal_state = Some(terminal_state);

        Ok(())
    }

    fn destroy(self) -> Result<()> {
        let terminal_state =
            self.terminal_state.lock().take().ok_or_else(|| {
                miette!("Cannot destroy terminal, hasn't been set up yet.")
            })?;

        // Wait for render thread to stop.
        terminal_state.render_thread_cancellation_token.cancel();
        terminal_state
            .render_thread_join_handle
            .join()
            .map_err(|_| miette!("Render thread panicked!?"))??;

        // Destroy the terminal UI.
        let mut terminal = Arc::into_inner(terminal_state.terminal)
            .expect("BUG: Something is still holding a strong reference to the terminal.")
            .into_inner();

        terminal
            .backend_mut()
            .set_cursor(
                terminal_state.cursor_end_position.0,
                terminal_state.cursor_end_position.1,
            )
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Failed to set cursor to end position.")
            })?;

        drop(terminal);

        crossterm::terminal::disable_raw_mode()
            .into_diagnostic()
            .wrap_err_with(|| miette!("Failed to disable terminal raw mode."))?;


        // Join log output flushing thread.
        let log_state = self.log_state.lock();
        if matches!(log_state.log_output, LogOutputMode::ToFile { .. }) {
            drop(log_state);
            self.disable_saving_logs_to_file()
                .wrap_err_with(|| miette!("Failed to disable log output."))?;
        }

        Ok(())
    }
}

impl<'thread_scope, 'config> LogBackend
    for FancyTerminalBackend<'thread_scope, 'config>
{
    fn log_newline(&self) {
        let mut state = self.log_state.lock();

        // If enabled, write newline into the log file (its BufWriter, to be precise).
        match &state.log_output {
            LogOutputMode::ToFile { buf_writer, .. } => {
                let mut locked_buf_writer = buf_writer.lock();

                locked_buf_writer
                    .write_all("\n".as_bytes())
                    .expect("Failed to write newline to log file output.");
            }
            LogOutputMode::None => {}
        }

        // Add newline to log journal.
        state.log_journal.insert_entry("\n");
    }

    fn log_println<D: Display>(&self, content: D) {
        let message = content.to_string();
        let mut state = self.log_state.lock();

        // If enabled, write message into the log file (its BufWriter, to be precise).
        match &state.log_output {
            LogOutputMode::ToFile { buf_writer, .. } => {
                let mut locked_buf_writer = buf_writer.lock();

                locked_buf_writer.write_all(message.as_bytes()).expect(
                    "Failed to write println contents to log file output.",
                );
                locked_buf_writer
                    .write_all("\n".as_bytes())
                    .expect("Failed to write newline to log file output.");
            }
            LogOutputMode::None => {}
        }

        // Add message to log journal.
        state.log_journal.insert_entry(message);
    }
}

impl<'scope, 'scope_env: 'scope, 'config: 'scope>
    LogToFileBackend<'scope, 'scope_env>
    for FancyTerminalBackend<'scope, 'config>
{
    fn enable_saving_logs_to_file<P: AsRef<Path>>(
        &self,
        log_output_file_path: P,
        scope: &'scope Scope<'scope, 'scope_env>,
    ) -> Result<()> {
        let output_file = File::create(log_output_file_path)
            .into_diagnostic()
            .wrap_err_with(|| {
            miette!("Failed to create log output file.")
        })?;

        let ansi_escaping_writer = strip_ansi_escapes::Writer::new(output_file);
        let buf_writer = BufWriter::with_capacity(1024, ansi_escaping_writer);

        let buf_writer_arc_mutex = Arc::new(Mutex::new(buf_writer));
        let buf_writer_arc_mutex_clone = buf_writer_arc_mutex.clone();


        let flushing_thread_cancellation_token = CancellationToken::new();
        let flushing_thread_cancellation_token_clone =
            flushing_thread_cancellation_token.clone();


        let flushing_thread_handle = scope.spawn(move || {
            run_log_output_flushing_loop(
                buf_writer_arc_mutex_clone,
                flushing_thread_cancellation_token_clone,
            )
        });

        {
            let mut locked_state = self.log_state.lock();

            locked_state.log_output = LogOutputMode::ToFile {
                buf_writer: buf_writer_arc_mutex,
                writer_flushing_thread_handle: flushing_thread_handle,
                writer_flushing_thread_cancellation_token:
                    flushing_thread_cancellation_token,
            }
        }

        Ok(())
    }

    fn disable_saving_logs_to_file(&self) -> Result<()> {
        let mut locked_state = self.log_state.lock();

        let (
            writer_flushing_thread_handle,
            writer_flushing_thread_cancellation_token,
        ) = match std::mem::replace(
            &mut locked_state.log_output,
            LogOutputMode::None,
        ) {
            LogOutputMode::ToFile {
                writer_flushing_thread_handle,
                writer_flushing_thread_cancellation_token,
                ..
            } => (
                writer_flushing_thread_handle,
                writer_flushing_thread_cancellation_token,
            ),
            LogOutputMode::None => {
                return Err(miette!("Log file output is already disabled."));
            }
        };

        writer_flushing_thread_cancellation_token.cancel();
        writer_flushing_thread_handle
            .join()
            .map_err(|_| miette!("Log file output flushing thread panicked!"))?
            .wrap_err_with(|| {
                miette!("Failed to run log file output flushing thread.")
            })?;

        Ok(())
    }
}

impl<'thread_scope, 'config> TranscodeBackend<'config>
    for FancyTerminalBackend<'thread_scope, 'config>
{
    /*
     * Album queue
     */

    fn queue_album_enable(&self) {
        let mut locked_state = self.ui_state.write();
        locked_state.album_queue = Some(Queue::new());
    }

    fn queue_album_disable(&self) {
        let mut locked_state = self.ui_state.write();
        locked_state.album_queue = None;
    }

    fn queue_album_clear(&self) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        match &mut locked_state.album_queue {
            Some(queue) => {
                queue.clear();
                Ok(())
            }
            None => Err(miette!(
                "Album queue is disabled, can't clear queue."
            )),
        }
    }

    fn queue_album_item_add(
        &self,
        item: AlbumQueueItem<'config>,
    ) -> Result<QueueItemID> {
        let wrapped_item = FancyAlbumQueueItem::new(item);
        let item_id = wrapped_item.get_id();

        let mut locked_state = self.ui_state.write();

        locked_state
            .album_queue
            .as_mut()
            .ok_or_else(|| miette!("Album queue is disabled, can't add item."))?
            .queue_item(wrapped_item)?;

        Ok(item_id)
    }

    fn queue_album_item_start(&self, item_id: QueueItemID) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .album_queue
            .as_mut()
            .ok_or_else(|| {
                miette!("Album queue is disabled, can't start item.")
            })?
            .start_item(item_id)
    }

    fn queue_album_item_finish(
        &self,
        item_id: QueueItemID,
        result: AlbumQueueItemFinishedResult,
    ) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .album_queue
            .as_mut()
            .ok_or_else(|| {
                miette!("Album queue is disabled, can't finish item.")
            })?
            .finish_item(item_id, result)
    }

    fn queue_album_item_remove(
        &self,
        item_id: QueueItemID,
    ) -> Result<AlbumQueueItem<'config>> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .album_queue
            .as_mut()
            .ok_or_else(|| {
                miette!("Album queue is disabled, can't remove item.")
            })?
            .remove_item(item_id)
            .map(|fancy_item| fancy_item.item)
    }


    /*
     * File queue
     */

    fn queue_file_enable(&self) {
        let mut locked_state = self.ui_state.write();
        locked_state.file_queue = Some(Queue::new());
        locked_state.current_page = UIPage::Transcoding;
    }

    fn queue_file_disable(&self) {
        let mut locked_state = self.ui_state.write();
        locked_state.file_queue = None;
        locked_state.current_page = UIPage::Logs;
    }

    fn queue_file_clear(&self) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        match &mut locked_state.file_queue {
            Some(queue) => {
                queue.clear();
                Ok(())
            }
            None => Err(miette!(
                "File queue is disabled, can't clear queue."
            )),
        }
    }

    fn queue_file_item_add(
        &self,
        item: FileQueueItem<'config>,
    ) -> Result<QueueItemID> {
        let wrapped_item = FancyFileQueueItem::new(item);
        let item_id = wrapped_item.get_id();

        let mut locked_state = self.ui_state.write();

        locked_state
            .file_queue
            .as_mut()
            .ok_or_else(|| miette!("File queue is disabled, can't queue item."))?
            .queue_item(wrapped_item)?;

        Ok(item_id)
    }

    fn queue_file_item_start(&self, item_id: QueueItemID) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .file_queue
            .as_mut()
            .ok_or_else(|| miette!("File queue is disabled, can't start item."))?
            .start_item(item_id)
    }

    fn queue_file_item_finish(
        &self,
        item_id: QueueItemID,
        result: FileQueueItemFinishedResult,
    ) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .file_queue
            .as_mut()
            .ok_or_else(|| {
                miette!("File queue is disabled, can't finish item.")
            })?
            .finish_item(item_id, result)
    }

    fn queue_file_item_remove(
        &self,
        item_id: QueueItemID,
    ) -> Result<FileQueueItem<'config>> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .file_queue
            .as_mut()
            .ok_or_else(|| {
                miette!("File queue is disabled, can't remove item.")
            })?
            .remove_item(item_id)
            .map(|fancy_item| fancy_item.item)
    }


    /*
     * Progress bar
     */

    fn progress_enable(&self) {
        let mut locked_state = self.ui_state.write();
        locked_state.progress = Some(Progress::default());
        locked_state.current_page = UIPage::Transcoding;
    }

    fn progress_disable(&self) {
        let mut locked_state = self.ui_state.write();
        locked_state.progress = None;
        locked_state.current_page = UIPage::Logs;
    }

    fn progress_set_total(&self, num_total: usize) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .progress
            .as_mut()
            .ok_or_else(|| {
                miette!("Progress bar is disabled, can't set total.")
            })?
            .total_files = num_total;

        Ok(())
    }

    fn progress_set_audio_files_currently_processing(
        &self,
        num_audio_files_currently_processing: usize,
    ) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .progress
            .as_mut()
            .ok_or_else(|| {
                miette!("Progress bar is disabled, can't set currently processing audio files amount.")
            })?
            .audio_files_currently_processing = num_audio_files_currently_processing;

        Ok(())
    }

    fn progress_set_data_files_currently_processing(
        &self,
        num_data_files_currently_processing: usize,
    ) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .progress
            .as_mut()
            .ok_or_else(|| {
                miette!("Progress bar is disabled, can't set currently processing data files amount.")
            })?
            .data_files_currently_processing = num_data_files_currently_processing;

        Ok(())
    }

    fn progress_set_audio_files_finished_ok(
        &self,
        num_audio_files_finished_ok: usize,
    ) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .progress
            .as_mut()
            .ok_or_else(|| {
                miette!("Progress bar is disabled, can't set ok audio files.")
            })?
            .audio_files_finished_ok = num_audio_files_finished_ok;

        Ok(())
    }

    fn progress_set_data_files_finished_ok(
        &self,
        num_data_files_finished_ok: usize,
    ) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .progress
            .as_mut()
            .ok_or_else(|| {
                miette!("Progress bar is disabled, can't set ok audio files.")
            })?
            .data_files_finished_ok = num_data_files_finished_ok;

        Ok(())
    }

    fn progress_set_audio_files_errored(
        &self,
        num_audio_files_errored: usize,
    ) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .progress
            .as_mut()
            .ok_or_else(|| {
                miette!("Progress bar is disabled, can't set ok audio files.")
            })?
            .audio_files_errored = num_audio_files_errored;

        Ok(())
    }

    fn progress_set_data_files_errored(
        &self,
        num_data_files_errored: usize,
    ) -> Result<()> {
        let mut locked_state = self.ui_state.write();

        locked_state
            .progress
            .as_mut()
            .ok_or_else(|| {
                miette!("Progress bar is disabled, can't set ok audio files.")
            })?
            .data_files_errored = num_data_files_errored;

        Ok(())
    }
}

impl<'thread_scope, 'config> UserControllableBackend
    for FancyTerminalBackend<'thread_scope, 'config>
{
    fn get_user_control_receiver(
        &self,
    ) -> Result<broadcast::Receiver<UserControlMessage>> {
        let locked_terminal = self.terminal_state.lock();

        match locked_terminal.as_ref() {
            Some(terminal_state) => {
                Ok(terminal_state.user_control_sender.subscribe())
            }
            None => Err(miette!(
                "Backend hasn't been set up, can't get user control receiver."
            )),
        }
    }
}
