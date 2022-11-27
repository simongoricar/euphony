use std::fmt::Display;

use miette::{miette, Result};

use crate::console::{LogBackend, LogTerminalBackend, TerminalBackend, TranscodeBackend};
use crate::console::backends::shared::{ProgressState, QueueItem, QueueItemFinishedState, QueueItemID, QueueState, QueueType};
use crate::console::traits::TranscodeLogTerminalBackend;

pub struct BareConsoleBackend {
    queue: Option<QueueState>,
    
    progress: Option<ProgressState>,
}

impl BareConsoleBackend {
    pub fn new() -> Self {
        Self {
            queue: None,
            progress: None,
        }
    }
}

impl TerminalBackend for BareConsoleBackend {
    fn setup(&mut self) -> Result<()> {
        Ok(())
    }
    
    fn destroy(&mut self) -> Result<()> {
        Ok(())
    }
}

impl LogBackend for BareConsoleBackend {
    fn log_newline(&mut self) {
        println!();
    }
    
    fn log_println(&mut self, content: Box<dyn Display>) {
        println!("{}", content)
    }
}

impl TranscodeBackend for BareConsoleBackend {
    fn queue_begin(&mut self) {
        println!("Queue starting.");
        self.queue = Some(QueueState::default());
    }
    
    fn queue_end(&mut self) {
        println!("Queue finished.");
        self.queue = None;
    }
    
    fn queue_item_add(&mut self, item: String, item_type: QueueType) -> Result<QueueItemID> {
        if let Some(queue) = &mut self.queue {
            let queue_item = QueueItem::new(item, item_type);
            let queue_item_id = queue_item.id;
            
            println!(
                "New item in queue ({:?}): {}",
                item_type, queue_item.content,
            );
            
            match item_type {
                QueueType::Library => queue.library_items.push(queue_item),
                QueueType::Album => queue.album_items.push(queue_item),
                QueueType::File => queue.file_items.push(queue_item),
            }
    
            Ok(queue_item_id)
        } else {
            Err(miette!("Queue is currently disabled, can't add to the queue."))
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
            Err(miette!("Queue is currently disabled, can't start item."))
        }
    }
    
    fn queue_item_finish(&mut self, item_id: QueueItemID, was_ok: bool) -> Result<()> {
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
            Err(miette!("Queue is currently disabled, can't finish item."))
        }
    }
    
    fn queue_item_modify(
        &mut self,
        item_id: QueueItemID,
        function: Box<dyn FnOnce(&mut QueueItem)>,
    ) -> Result<()>
        where Self: Sized
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
            Err(miette!("Queue is currently disabled, can't modify item."))
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
            Err(miette!("Queue is currently disabled, can't remove item."))
        }
    }
    
    fn queue_clear(&mut self, queue_type: QueueType) -> Result<()> {
        if let Some(queue) = &mut self.queue {
            queue.clear_queue_by_type(queue_type);
            
            println!("Queue {:?} has been cleared.", queue_type);
            
            Ok(())
        } else {
            Err(miette!("Queue is currently disabled, can't clear."))
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
            Err(miette!("Progress bar is currently disabled, can't set total."))
        }
    }
    
    fn progress_set_current(&mut self, current: usize) -> Result<()> {
        if let Some(progress) = &mut self.progress {
            progress.current = current;
            Ok(())
        } else {
            Err(miette!("Progress bar is currently disabled, can't set current."))
        }
    }
}

impl LogTerminalBackend for BareConsoleBackend {}
impl TranscodeLogTerminalBackend for BareConsoleBackend {}
