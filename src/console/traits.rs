use std::fmt::Display;

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
    fn log_newline(&mut self);
    
    /// Print a string into the log, followed by a new line.
    fn log_println(&mut self, content: Box<dyn Display>);
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


pub trait LogTerminalBackend: TerminalBackend + LogBackend {}
pub trait TranscodeLogTerminalBackend: TerminalBackend + LogBackend + TranscodeBackend {}
