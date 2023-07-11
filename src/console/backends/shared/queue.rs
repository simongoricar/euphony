use std::fmt::Debug;

use miette::{miette, Result};
use tui::style::{Modifier, Style};
use tui::text::{Span, Text};
use tui::widgets::ListItem;

use crate::console::backends::shared::queue_v2::{
    QueueItem,
    QueueItemGenericState,
    QueueItemStateQuery,
    RenderableQueueItem,
};


/// Represents a set of `tui::style::Style` rules to apply when rendering a dynamic item queue.
#[derive(Clone)]
pub struct ListItemStyleRules {
    /// Style to apply to items that are pending.
    pub item_pending_style: Style,

    /// Style to apply to items that are in progress.
    pub item_in_progress_style: Style,

    /// Style to apply to items that have finished successfully.
    pub item_finished_ok_style: Style,

    /// Style to apply to items that have finished unsuccessfully.
    pub item_finished_not_ok_style: Style,

    /// Style to apply to the potential leading "... (N completed) ..." message at the top of the queue.
    pub leading_completed_items_style: Style,

    /// Style to apply to the potential trailing "... (N remaining) ..." message at the bottom of the queue.
    pub trailing_hidden_pending_items_style: Style,
}

/// Given a full list of `QueueItem`s, rules for styling the items and the maximum lines
/// to generate, this function will generate a dynamic queue (a `Vec<ListItem>`).
///
/// The idea behind the dynamic queue is as follows:
/// - `QueueItem`s can be pending, in-progress or completed,
/// - as such we should hide the leading group of completed items, shifting the "live" view of the queue
///   to a more relevant part where items are in-progress and pending.
/// - additionally, to give a sense of progress, we should also add two special list items:
///    - the leading "... (N completed) ..." when N leading items have been completed (when N >= 2)
///    - the trailing "... (N hidden) ..." when there are N trailing items that don't fit in the given view.
///
///
/// ### Example
/// `task_queue` contains items ITEM #1 though ITEM #17 (#1 to #12 are completed), `max_list_lines = 5`
/// ```
/// ... (12 completed) ...
/// ITEM #13
/// ITEM #14
/// ITEM #15
/// ... (2 hidden) ...
/// ```
pub fn generate_dynamic_task_list<
    'a,
    Item: QueueItem<R> + RenderableQueueItem<RenderOutput> + 'a,
    R: Debug,
    RenderOutput: Into<Text<'a>>,
    ItemIterator: ExactSizeIterator<Item = &'a Item>,
>(
    task_queue: ItemIterator,
    list_style_rules: ListItemStyleRules,
    max_list_lines: usize,
) -> Result<Vec<ListItem<'a>>> {
    let task_queue: Vec<&'a Item> = task_queue.collect();

    let queue_size = task_queue.len();
    let mut dynamic_task_list: Vec<ListItem> =
        Vec::with_capacity(max_list_lines);


    let mut lines_used: usize = 0;
    let mut current_queue_offset: usize = 0;

    // Generates leading text "... (N completed) ..." if there are at least
    // two completed items at the top of the queue that we can squash and if the total length
    // of the task queue is longer than we can display. This means we don't squash completed items
    // if we can display them all.
    let leading_items_completed_count = task_queue
        .iter()
        .take_while(|item| item.is_finished())
        .count();

    if leading_items_completed_count >= 2 && queue_size > max_list_lines {
        let leading_completed_text = Span::styled(
            format!(
                "... ({} completed) ...",
                leading_items_completed_count
            ),
            Style::default().add_modifier(Modifier::ITALIC),
        );

        dynamic_task_list.push(
            ListItem::new(leading_completed_text)
                .style(list_style_rules.leading_completed_items_style),
        );

        current_queue_offset += leading_items_completed_count;
        lines_used += 1;
    }

    // Before we add normal lines we calculate if we will need a trailing "... (N hidden) ..." entry.
    // This is only shown when the remaining items would overflow.
    let should_add_trailing =
        (queue_size - current_queue_offset + 1) > (max_list_lines - lines_used);
    if should_add_trailing {
        lines_used += 1;
    }

    // Add as many normal lines as possible.
    while lines_used < max_list_lines && current_queue_offset < queue_size {
        let next_item = task_queue
            .get(current_queue_offset)
            .ok_or_else(|| miette!("BUG: Could not get queue item."))?;

        let wanted_item_style = match next_item.get_state() {
            QueueItemGenericState::Pending => {
                list_style_rules.item_pending_style
            }
            QueueItemGenericState::Queued => list_style_rules.item_pending_style,
            QueueItemGenericState::InProgress => {
                list_style_rules.item_in_progress_style
            }
            QueueItemGenericState::Finished { ok } => match ok {
                true => list_style_rules.item_finished_ok_style,
                false => list_style_rules.item_finished_not_ok_style,
            },
        };

        let item = ListItem::new(next_item.render()).style(wanted_item_style);

        dynamic_task_list.push(item);

        current_queue_offset += 1;
        lines_used += 1;
    }

    if should_add_trailing {
        let hidden_item_count = queue_size - current_queue_offset + 2;

        // FIXME Figure out why I had to insert this and fix the source of the problem.
        dynamic_task_list.pop();
        dynamic_task_list.pop();

        let trailing_hidden_text = Span::styled(
            format!("... ({} hidden) ...", hidden_item_count),
            Style::default().add_modifier(Modifier::ITALIC),
        );

        dynamic_task_list.push(
            ListItem::new(trailing_hidden_text)
                .style(list_style_rules.trailing_hidden_pending_items_style),
        );
    }

    Ok(dynamic_task_list)
}

/*
/// Given a full list of `QueueItem`s, rules for styling the items and the maximum lines to generate,
/// this function will generate a dynamic queue (a `Vec<ListItem>`).
///
/// The idea behind the dynamic queue is as follows:
/// - `QueueItem`s can be pending, in-progress or completed,
/// - as such we should hide the leading group of completed items, shifting the "live" view of the queue
///   to a more relevant part where items are in-progress and pending.
/// - additionally, to give a sense of progress, we should also add two special list items:
///    - the leading "... (N completed) ..." when N leading items have been completed (when N >= 2)
///    - the trailing "... (N remaining) ..." when there are N trailing items that don't fit in the given view.
///
/// Example: `full_queue` contains items ITEM #1 though ITEM #17 (#1 to #12 are completed), `max_lines = 5`
/// ```
/// ... (12 completed) ...
/// ITEM #13
/// ITEM #14
/// ITEM #15
/// ... (2 remaining) ...
/// ```
// TODO Pending rewrite. When finished, move into queue_v2.rs
pub fn generate_dynamic_list_from_queue_items<Item: QueueItem<R>, R: Debug>(
    full_queue: &Vec<Item>,
    list_style_rules: ListItemStyleRules,
    max_lines: usize,
) -> Result<Vec<ListItem>> {
    let total_queue_size = full_queue.len();
    let mut dynamic_list: Vec<ListItem> = Vec::with_capacity(max_lines);

    let leading_items_completed_count = full_queue
        .iter()
        .take_while(|item| item.finished_state.is_some())
        .count();

    // Generate dynamic list.
    let mut lines_used: usize = 0;
    let mut current_queue_offset: usize = 0;

    // Additionally, don't generate the leading and trailing lines if
    // the amount of tasks fits in the window anyway.

    // Generates leading "... (12 completed) ..." if there are
    // at least two completed items at the top of the queue.
    if leading_items_completed_count >= 2 && total_queue_size > max_lines {
        dynamic_list.push(
            ListItem::new(Span::styled(
                format!(
                    "  ... ({leading_items_completed_count} completed) ...",
                ),
                Style::default().add_modifier(Modifier::ITALIC),
            ))
            .style(list_style_rules.leading_completed_items_style),
        );
        current_queue_offset += leading_items_completed_count;
        lines_used += 1;
    }

    // Preparation for trailing "... (23 remaining) ..." line.
    // This trailing message is shown only if there are more items that are hidden.
    let add_trailing =
        total_queue_size - current_queue_offset + 1 > max_lines - lines_used;
    if add_trailing {
        lines_used += 1;
    }

    // Generates as many "normal" lines as possible.
    while lines_used < max_lines && current_queue_offset < total_queue_size {
        let next_item = full_queue
            .get(current_queue_offset)
            .ok_or_else(|| miette!("Could not get queue item."))?;

        let item_style = match next_item.get_state() {
            QueueItemState::Pending => list_style_rules.item_pending_style,
            QueueItemState::InProgress => {
                list_style_rules.item_in_progress_style
            }
            QueueItemState::Finished => {
                match next_item
                    .finished_state
                    .as_ref()
                    .expect(
                        "QueueItemState::Finished, but finished state is None?!",
                    )
                    .is_ok
                {
                    true => list_style_rules.item_finished_ok_style,
                    false => list_style_rules.item_finished_not_ok_style,
                }
            }
        };

        dynamic_list.push(
            ListItem::new(Spans(vec![
                Span::raw(
                    if let Some(spinner) = &next_item.spinner {
                        format!(" {} ", spinner.get_current_phase())
                    } else {
                        match next_item.spaces_when_spinner_is_disabled {
                            true => "   ".into(),
                            false => "".into(),
                        }
                    },
                ),
                Span::styled(
                    if let Some(prefix) = &next_item.prefix {
                        prefix
                    } else {
                        ""
                    },
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(next_item.content.clone()),
                Span::styled(
                    if let Some(suffix) = &next_item.suffix {
                        suffix
                    } else {
                        ""
                    },
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]))
            .style(item_style),
        );

        current_queue_offset += 1;
        lines_used += 1;
    }

    // If there are still (overflowing) trailing pending tasks, we show them in the last line
    // as "... (54 remaining) ...".
    if add_trailing {
        let hidden_pending_item_count =
            total_queue_size - current_queue_offset + 2;

        dynamic_list.pop();
        dynamic_list.pop();
        dynamic_list.push(
            ListItem::new(Span::styled(
                format!("  ... ({hidden_pending_item_count} remaining) ...",),
                Style::default().add_modifier(Modifier::ITALIC),
            ))
            .style(list_style_rules.trailing_hidden_pending_items_style),
        );
    }

    Ok(dynamic_list)
}
*/
