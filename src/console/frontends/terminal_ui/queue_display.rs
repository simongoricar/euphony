use std::fmt::Debug;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{List, ListItem};

use crate::console::colours::X242_GREY42;
use crate::console::frontends::shared;
use crate::console::frontends::shared::queue::{
    QueueItem,
    QueueItemStateQuery,
    RenderableQueueItem,
};

const LEADING_HIDDEN_ITEMS_EXPLAINER_STYLE: Style = X242_GREY42;
const TRAILING_HIDDEN_ITEMS_EXPLAINER_STYLE: Style = X242_GREY42;


struct IncludedItem<'text> {
    pub list_item: ListItem<'text>,
    pub item_height: usize,
    pub is_a_finished_item: bool,
}


pub fn generate_smart_collapsible_queue<
    'text,
    Content: Into<Text<'text>>,
    ItemResult: Debug,
    Item: QueueItem<ItemResult> + RenderableQueueItem<Content>,
>(
    queue: &shared::queue::Queue<Item, ItemResult>,
    available_height: usize,
    available_width: usize,
) -> List<'text> {
    let mut included_items: Vec<IncludedItem<'text>> =
        Vec::with_capacity(available_height);
    let mut is_first_item_in_finished_state: bool = false;

    let mut used_height = 0;

    let mut leading_explainer: Option<usize> = None;
    let mut trailing_explainer: Option<usize> = None;

    let queue_iterator = queue.items().enumerate();
    let queue_size = queue_iterator.len();

    for (item_index, (_, item)) in queue_iterator {
        let rendered_item = item.render().into();
        let rendered_item_lines = rendered_item.lines.len();

        let mut current_available_height_offset: usize = 0;
        if leading_explainer.is_some() {
            current_available_height_offset += 1;
        }
        if trailing_explainer.is_some() {
            current_available_height_offset += 1;
        }

        let mut max_potential_available_height_offset = 2;
        if !is_first_item_in_finished_state && leading_explainer.is_none() {
            // The first element isn't finished and we don't have a leading explainer,
            // meaning there can never be a leading explainer.
            max_potential_available_height_offset -= 1;
        }

        if (item_index + 1) == queue_size
            && ((used_height
                + current_available_height_offset
                + rendered_item_lines)
                <= available_height)
        {
            // This is the last queue element.
            // We can fill up the available height. This also respects having less space
            // if e.g. the leading explainer has already been added.

            if included_items.is_empty() {
                is_first_item_in_finished_state = item.is_finished();
            }

            included_items.push(IncludedItem {
                list_item: ListItem::new(rendered_item),
                item_height: rendered_item_lines,
                is_a_finished_item: item.is_finished(),
            });

            used_height += rendered_item_lines;
        } else if (item_index + 1) != queue_size
            && (used_height
                + max_potential_available_height_offset
                + rendered_item_lines)
                <= available_height
        {
            // This is *not* the last queue element and there is enough space to fit this queue
            // element on the screen, even if we'll have to potentially add some explainers later.

            let is_finished = item.is_finished();

            if included_items.is_empty() {
                is_first_item_in_finished_state = is_finished;
            }

            included_items.push(IncludedItem {
                list_item: ListItem::new(rendered_item),
                item_height: rendered_item_lines,
                is_a_finished_item: is_finished,
            });

            used_height += rendered_item_lines;
        } else {
            // Displaying this element would put us over the available height limit
            // (can be any element, even the last one).

            // We can no longer shorten the list, meaning we need to add a trailing explainer and stop.
            if !is_first_item_in_finished_state {
                let remaining_items = queue_size - item_index;
                trailing_explainer = Some(remaining_items);

                break;
            }

            // We first remove all leading finished items to potentially clear up some space.
            // This means we'll have to display a leading explainer, but we could very well
            // save a lot of space when the queue has finished a lot of the first items.
            let mut first_non_finished_item_index: Option<usize> = None;
            let mut sum_of_about_to_be_freed_lines: usize = 0;

            for (item_index, item) in included_items.iter().enumerate() {
                // Stop at first unfinished item.
                if !item.is_a_finished_item {
                    first_non_finished_item_index = Some(item_index);
                    break;
                }

                sum_of_about_to_be_freed_lines += item.item_height;
            }

            let first_non_finished_item_index =
                if let Some(first_non_finished_item_index) =
                    first_non_finished_item_index
                {
                    first_non_finished_item_index
                } else {
                    included_items.len()
                };

            if first_non_finished_item_index > 0 {
                included_items.drain(0..first_non_finished_item_index);
                used_height -= sum_of_about_to_be_freed_lines;

                is_first_item_in_finished_state = match included_items.first() {
                    Some(item) => item.is_a_finished_item,
                    None => false,
                };
            }

            if let Some(number_of_hidden_finished_items) = leading_explainer {
                leading_explainer = Some(
                    number_of_hidden_finished_items
                        + first_non_finished_item_index,
                );
            } else {
                leading_explainer = Some(first_non_finished_item_index);
            }
        }
    }

    // Pre-/Append explainers if we need to.
    let mut final_list_items: Vec<ListItem> =
        Vec::with_capacity(included_items.len() + 2);

    if let Some(leading_finished_items_count) = leading_explainer {
        let leading_explainer_contents = format!(
            "... {} finished and hidden ...",
            leading_finished_items_count
        );

        let leading_spaces_for_centering =
            " ".repeat((available_width - leading_explainer_contents.len()) / 2);

        final_list_items.push(ListItem::new(Line::from(vec![Span::styled(
            format!(
                "{}{}",
                leading_spaces_for_centering, leading_explainer_contents
            ),
            LEADING_HIDDEN_ITEMS_EXPLAINER_STYLE.add_modifier(Modifier::ITALIC),
        )])));

        used_height += 1;
    }

    final_list_items
        .extend(included_items.into_iter().map(|item| item.list_item));

    if let Some(trailing_hidden_items_count) = trailing_explainer {
        let trailing_explainer_contents = format!(
            " ... {} invisible below ... ",
            trailing_hidden_items_count
        );

        let leading_spaces_for_centering = " "
            .repeat((available_width - trailing_explainer_contents.len()) / 2);

        // Make sure we position this (vertically) at the bottom.
        let num_empty_lines_for_bottom_vertical_positioning =
            available_height - used_height - 1;
        final_list_items.extend(
            std::iter::repeat(ListItem::new(Line::default()))
                .take(num_empty_lines_for_bottom_vertical_positioning),
        );

        // Finally, add the last line - the trailing explainer.
        final_list_items.push(ListItem::new(Line::from(vec![Span::styled(
            format!(
                "{}{}",
                leading_spaces_for_centering, trailing_explainer_contents
            ),
            TRAILING_HIDDEN_ITEMS_EXPLAINER_STYLE.add_modifier(Modifier::ITALIC),
        )])))
    }


    List::new(final_list_items)
}
