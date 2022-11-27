use miette::{miette, Result};
use crate::console::backends::shared::{QueueItem, QueueItemID, QueueType};

#[derive(Default)]
pub struct QueueState {
    pub library_items: Vec<QueueItem>,
    pub album_items: Vec<QueueItem>,
    pub file_items: Vec<QueueItem>,
}

impl QueueState {
    pub fn find_item_by_id(
        &mut self,
        item_id: QueueItemID,
    ) -> Option<&mut QueueItem> {
        self.library_items
            .iter_mut()
            .chain(self.album_items.iter_mut())
            .chain(self.file_items.iter_mut())
            .find(|item| item.id == item_id)
    }
    
    pub fn remove_item_by_id(
        &mut self,
        item_id: QueueItemID,
    ) -> Result<()> {
        // Look at `library_items`.
        let library_items_pos = self.library_items
            .iter()
            .position(|item| item.id == item_id);
        if let Some(position) = library_items_pos {
            self.library_items.remove(position);
            return Ok(());
        }
        
        // Look at `album_items`.
        let album_items_pos = self.album_items
            .iter()
            .position(|item| item.id == item_id);
        if let Some(position) = album_items_pos {
            self.album_items.remove(position);
            return Ok(());
        }
        
        // Look at `file_items`.
        let file_items_pos = self.file_items
            .iter()
            .position(|item| item.id == item_id);
        if let Some(position) = file_items_pos {
            self.file_items.remove(position);
            return Ok(());
        }
        
        // No match in any of the queues, no such item.
        Err(miette!("No such queue item."))
    }
    
    pub fn clear_queue_by_type(&mut self, queue_type: QueueType) {
        match queue_type {
            QueueType::Library => self.library_items.clear(),
            QueueType::Album => self.album_items.clear(),
            QueueType::File => self.file_items.clear(),
        }
    }
}


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