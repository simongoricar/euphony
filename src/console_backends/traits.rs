use std::fmt::Display;
use std::ops::Deref;
use miette::Result;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct QueueItemID(pub u32);

impl Deref for QueueItemID {
    type Target = u32;
    
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub trait TerminalBackend {
    fn setup(&mut self) -> Result<()>;
    fn destroy(self) -> Result<()>;
}

pub trait LogBackend {
    fn log_newline(&mut self);
    fn log_println<T: Display>(&mut self, content: T);
}

pub trait TranscodeBackend {
    fn queue_begin(&mut self);
    fn queue_end(&mut self);
    fn queue_item_add<T: Display>(&mut self, item: T) -> Result<QueueItemID>;
    fn queue_item_start(&mut self, item_id: QueueItemID) -> Result<()>;
    fn queue_item_finish(&mut self, item_id: QueueItemID, was_ok: bool) -> Result<()>;
    fn queue_item_remove(&mut self, item_id: QueueItemID) -> Result<()>;
    fn queue_clear(&mut self) -> Result<()>;
    
    fn progress_begin(&mut self);
    fn progress_end(&mut self);
    fn progress_set_percent(&mut self, percent: u16);
}
