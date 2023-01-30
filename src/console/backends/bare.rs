use std::fmt::Display;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use crossbeam::channel::{never, Receiver};
use crossterm::style::{Color, Stylize};
use miette::{miette, IntoDiagnostic, Result};
use strip_ansi_escapes::Writer;

use crate::console::backends::shared::{
    ProgressState,
    QueueItem,
    QueueItemFinishedState,
    QueueItemID,
    QueueState,
    QueueType,
};
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

pub struct BareTerminalBackend {
    queue: Option<QueueState>,

    progress: Option<ProgressState>,

    log_file_output: Option<Mutex<BufWriter<Writer<File>>>>,
}

impl BareTerminalBackend {
    pub fn new() -> Self {
        Self {
            queue: None,
            progress: None,
            log_file_output: None,
        }
    }
}

impl TerminalBackend for BareTerminalBackend {
    fn setup(&mut self) -> Result<()> {
        Ok(())
    }

    fn destroy(&mut self) -> Result<()> {
        // If logging to file was enabled, we should disable it before this backend is dropped,
        // otherwise we risk failing to flush to file.
        self.disable_saving_logs_to_file()?;

        Ok(())
    }
}

impl LogBackend for BareTerminalBackend {
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

        println!("{}", content_string);

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

impl TranscodeBackend for BareTerminalBackend {
    fn queue_begin(&mut self) {
        println!("Queue starting.");
        self.queue = Some(QueueState::default());
    }

    fn queue_end(&mut self) {
        println!("Queue finished.");
        self.queue = None;
    }

    fn queue_item_add(
        &mut self,
        item: String,
        item_type: QueueType,
    ) -> Result<QueueItemID> {
        if let Some(queue) = &mut self.queue {
            let queue_item = QueueItem::new(item, item_type);
            let queue_item_id = queue_item.id;

            println!(
                "New item in queue ({:?}): {}",
                item_type, queue_item.content,
            );

            queue.add_item(queue_item);

            Ok(queue_item_id)
        } else {
            Err(miette!(
                "Queue is currently disabled, can't add to the queue."
            ))
        }
    }

    fn queue_item_start(&mut self, item_id: QueueItemID) -> Result<()> {
        if let Some(queue) = &mut self.queue {
            let target_item = queue.find_item_by_id(item_id);

            if let Some(item) = target_item {
                item.is_active = true;

                println!(
                    "Queue item starting: {}{}{}",
                    item.prefix.clone().unwrap_or_default(),
                    item.content,
                    item.suffix.clone().unwrap_or_default(),
                );

                Ok(())
            } else {
                Err(miette!("No such queue item."))
            }
        } else {
            Err(miette!(
                "Queue is currently disabled, can't start item."
            ))
        }
    }

    fn queue_item_finish(
        &mut self,
        item_id: QueueItemID,
        was_ok: bool,
    ) -> Result<()> {
        if let Some(queue) = &mut self.queue {
            let target_item = queue.find_item_by_id(item_id);

            if let Some(item) = target_item {
                item.is_active = false;
                item.set_finished_state(QueueItemFinishedState {
                    is_ok: was_ok,
                });

                println!(
                    "Queue item finished: {}{}{}",
                    item.prefix.clone().unwrap_or_default(),
                    item.content,
                    item.suffix.clone().unwrap_or_default(),
                );

                Ok(())
            } else {
                Err(miette!("No such queue item."))
            }
        } else {
            Err(miette!(
                "Queue is currently disabled, can't finish item."
            ))
        }
    }

    fn queue_item_modify(
        &mut self,
        item_id: QueueItemID,
        function: Box<dyn FnOnce(&mut QueueItem)>,
    ) -> Result<()>
    where
        Self: Sized,
    {
        if let Some(queue) = &mut self.queue {
            let target_item = queue.find_item_by_id(item_id);

            if let Some(item) = target_item {
                function(item);

                println!(
                    "Queue item was modified: {}{}{}",
                    item.prefix.clone().unwrap_or_default(),
                    item.content,
                    item.suffix.clone().unwrap_or_default(),
                );

                Ok(())
            } else {
                Err(miette!("No such queue item."))
            }
        } else {
            Err(miette!(
                "Queue is currently disabled, can't modify item."
            ))
        }
    }

    fn queue_item_remove(&mut self, item_id: QueueItemID) -> Result<()> {
        if let Some(queue) = &mut self.queue {
            let target_item = queue.find_item_by_id(item_id);

            if let Some(item) = target_item {
                println!(
                    "Queue item removed: {}{}{}",
                    item.prefix.clone().unwrap_or_default(),
                    item.content,
                    item.suffix.clone().unwrap_or_default(),
                );

                queue.remove_item_by_id(item_id)
            } else {
                Err(miette!("No such queue item."))
            }
        } else {
            Err(miette!(
                "Queue is currently disabled, can't remove item."
            ))
        }
    }

    fn queue_clear(&mut self, queue_type: QueueType) -> Result<()> {
        if let Some(queue) = &mut self.queue {
            queue.clear_queue_by_type(queue_type);

            println!("Queue {:?} has been cleared.", queue_type);

            Ok(())
        } else {
            Err(miette!(
                "Queue is currently disabled, can't clear."
            ))
        }
    }

    fn progress_begin(&mut self) {
        println!("Progress bar enabled.");
        self.progress = Some(ProgressState::default());
    }

    fn progress_end(&mut self) {
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

impl ValidationBackend for BareTerminalBackend {
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
            "{}\n{}",
            formatted_header, formatted_attributes
        )));
    }
}

impl UserControllableBackend for BareTerminalBackend {
    fn get_user_control_receiver(
        &mut self,
    ) -> Result<Receiver<UserControlMessage>> {
        Ok(never::<UserControlMessage>())
    }
}

impl LogToFileBackend for BareTerminalBackend {
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
