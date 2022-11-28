use miette::{miette, Result};
use crate::console::backends::shared::{QueueItem, QueueItemID, QueueType};




#[derive(Default)]
pub struct ProgressState {
    pub current: usize,
    pub total: usize,
}

impl ProgressState {
    pub fn get_percent(&self) -> u16 {
        if self.total == 0 {
            return 0;
        } else {
            (self.current as f32 / self.total as f32 * 100.0) as u16
        }
    }
}