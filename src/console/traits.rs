use std::fmt::Display;

use miette::Result;

use crate::console::backends::{QueueItem, QueueItemID, QueueType};

pub trait TerminalBackend {
    /// Initialize the terminal backend.
    fn setup(&mut self) -> Result<()>;
    
    /// Clean up the terminal backend.
    fn destroy(self) -> Result<()>;
}

pub trait LogBackend {
    /// Print a new empty line into the log.
    fn log_newline(&mut self);
    
    /// Print a string into the log, followed by a new line.
    fn log_println<T: Display>(&mut self, content: T);
}

pub trait TranscodeBackend {
    /// Initialize the queue system. This should be called before any other `queue_*` methods.
    fn queue_begin(&mut self);
    
    /// Clean up the queue system.
    fn queue_end(&mut self);
    
    /// Add an item to the queue.
    fn queue_item_add<T: Display>(&mut self, item: T, item_type: QueueType) -> Result<QueueItemID>;
    
    /// Mark the item in queue as "in-progress".
    fn queue_item_start(&mut self, item_id: QueueItemID) -> Result<()>;
    
    /// Mark the item in queue as "finished", with additional result context provided by `was_ok`.
    fn queue_item_finish(&mut self, item_id: QueueItemID, was_ok: bool) -> Result<()>;
    
    /// Fetch a mutable reference to the given queue item, allowing you to modify its contents.
    /// This is done by providing a function that will take the mutable reference and modify it.
    fn queue_item_modify<F: FnOnce(&mut QueueItem)>(
        &mut self,
        item_id: QueueItemID,
        function: F,
    ) -> Result<()>;
    
    /// Remove the item from the queue.
    fn queue_item_remove(&mut self, item_id: QueueItemID) -> Result<()>;
    
    /// Clear the entire queue (of the given type).
    fn queue_clear(&mut self, queue_type: QueueType) -> Result<()>;
    
    
    fn progress_begin(&mut self);
    fn progress_end(&mut self);
    fn progress_set_total(&mut self, total: usize) -> Result<()>;
    fn progress_set_current(&mut self, finished: usize) -> Result<()>;
}
