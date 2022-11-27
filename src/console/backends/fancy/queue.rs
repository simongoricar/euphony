use std::ops::Deref;

use miette::{miette, Result};
use tui::style::{Modifier, Style};
use tui::text::Span;
use tui::widgets::ListItem;
use crate::console::backends::PixelSpinner;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum QueueType {
    Library,
    Album,
    File,
}


#[derive(Copy, Clone, Eq, PartialEq)]
pub struct QueueItemID(pub u32);

impl Deref for QueueItemID {
    type Target = u32;
    
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}


#[derive(Clone, Eq, PartialEq)]
pub struct QueueItemFinishedState {
    pub is_ok: bool,
}


#[derive(Clone, Eq, PartialEq)]
pub struct QueueItem {
    pub prefix: Option<String>,
    
    pub content: String,
    
    pub suffix: Option<String>,
    
    pub item_type: QueueType,
    
    pub id: QueueItemID,
    
    pub is_active: bool,
    
    pub finished_state: Option<QueueItemFinishedState>,
    
    pub spinner: Option<PixelSpinner>,
    
    pub spaces_when_spinner_is_disabled: bool,
}

impl QueueItem {
    pub fn new<S: Into<String>>(
        content: S,
        item_type: QueueType,
    ) -> Self {
        let random_id = QueueItemID(rand::random::<u32>());
        
        Self {
            prefix: None,
            content: content.into(),
            suffix: None,
            item_type,
            id: random_id,
            is_active: false,
            finished_state: None,
            spinner: None,
            spaces_when_spinner_is_disabled: true,
        }
    }
    
    pub fn set_finished_state(&mut self, finished_state: QueueItemFinishedState) {
        self.finished_state = Some(finished_state);
    }
    
    pub fn enable_spinner(&mut self) {
        self.spinner = Some(PixelSpinner::new(None));
    }
    
    pub fn disable_spinner(&mut self) {
        self.spinner = None;
    }
    
    pub fn set_prefix<S: Into<String>>(&mut self, prefix: S) {
        self.prefix = Some(prefix.into());
    }
    
    pub fn clear_prefix(&mut self) {
        self.prefix = None;
    }
    
    pub fn set_suffix<S: Into<String>>(&mut self, suffix: S) {
        self.suffix = Some(suffix.into());
    }
    
    pub fn clear_suffix(&mut self) {
        self.suffix = None;
    }
}

#[derive(Clone)]
pub struct ListItemStyleRules {
    pub item_pending_style: Style,
    pub item_in_progress_style: Style,
    pub item_finished_ok_style: Style,
    pub item_finished_not_ok_style: Style,
    
    pub leading_completed_items_style: Style,
    pub trailing_hidden_pending_items_style: Style,
}

pub fn generate_dynamic_list_from_queue_items(
    full_queue: &Vec<QueueItem>,
    list_style_rules: ListItemStyleRules,
    max_lines: usize,
) -> Result<Vec<ListItem>> {
    let total_queue_size = full_queue.len();
    let mut dynamic_list: Vec<ListItem> = Vec::with_capacity(max_lines);
    
    let mut leading_items_completed_count = full_queue
        .iter()
        .take_while(|item| item.finished_state.is_some())
        .count();
    
    let trailing_items_pending_count = full_queue
        .iter()
        .rev()
        .take_while(|item| !item.is_active && item.finished_state.is_none())
        .count();
    
    // Generate dynamic list.
    let mut lines_used: usize = 0;
    let mut current_queue_offset: usize = 0;
    
    // Additionally, don't generate the leading and trailing lines if
    // the amount of tasks fits in the window anyway.
    
    // Generates leading "... (12 completed) ..." if there are
    // at least two completed items at the top of the queue.
    if leading_items_completed_count >= 2
        && total_queue_size > max_lines
    {
        dynamic_list.push(
            ListItem::new(
                Span::styled(
                    format!("  ... ({} completed) ...", leading_items_completed_count),
                    Style::default().add_modifier(Modifier::ITALIC)
                )
            )
                .style(list_style_rules.leading_completed_items_style)
        );
        current_queue_offset += leading_items_completed_count;
        lines_used += 1;
    }
    
    // Preparation for trailing "... (23 remaining) ..." line.
    // This trailing message is shown only if there are more items that are hidden.
    let add_trailing = total_queue_size - current_queue_offset + 1 > max_lines - lines_used;
    if add_trailing {
        lines_used += 1;
    }
    
    // Generates as many "normal" lines as possible.
    while lines_used < max_lines
        && current_queue_offset < total_queue_size
    {
        let next_item = full_queue.get(current_queue_offset)
            .ok_or_else(|| miette!("Could not get queue item."))?;
        
        let item_style = if let Some(finished) = &next_item.finished_state {
            if finished.is_ok {
                list_style_rules.item_finished_ok_style
            } else {
                list_style_rules.item_finished_not_ok_style
            }
        } else if next_item.is_active {
            list_style_rules.item_in_progress_style
        } else {
            list_style_rules.item_pending_style
        };
        
        dynamic_list.push(
            ListItem::new(format!(
                "{}{}{}{}",
                if let Some(spinner) = &next_item.spinner {
                    format!(" {} ", spinner.get_current_phase())
                } else {
                    match next_item.spaces_when_spinner_is_disabled {
                        true => "   ".into(),
                        false => "".into()
                    }
                },
                if let Some(prefix) = &next_item.prefix {
                    prefix
                } else {
                    ""
                },
                next_item.content,
                if let Some(suffix) = &next_item.suffix {
                    suffix
                } else {
                    ""
                }
            ))
                .style(item_style)
        );
        
        current_queue_offset += 1;
        lines_used += 1;
    }
    
    // If there are still (overflowing) trailing pending tasks, we show them in the last line
    // as "... (54 remaining) ...".
    if add_trailing {
        let hidden_pending_item_count = total_queue_size - current_queue_offset + 2;
        
        dynamic_list.pop();
        dynamic_list.pop();
        dynamic_list.push(
            ListItem::new(
                Span::styled(
                    format!("  ... ({} remaining) ...", hidden_pending_item_count),
                    Style::default().add_modifier(Modifier::ITALIC)
                )
            )
                .style(list_style_rules.trailing_hidden_pending_items_style)
        );
    }
    
    Ok(dynamic_list)
}
