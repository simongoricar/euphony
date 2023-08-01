use std::fmt::Debug;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{List, ListItem};

use crate::console::backends::shared;
use crate::console::backends::shared::queue::{
    QueueItem,
    QueueItemStateQuery,
    RenderableQueueItem,
};
use crate::console::colours::X242_GREY42;

const LEADING_HIDDEN_ITEMS_EXPLAINER_STYLE: Style = X242_GREY42;
const TRAILING_HIDDEN_ITEMS_EXPLAINER_STYLE: Style = X242_GREY42;



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
    let mut included_items: Vec<(ListItem, usize, bool)> =
        Vec::with_capacity(available_height);

    let mut used_up_height = 0;

    let mut leading_explainer: Option<usize> = None;
    let mut has_already_stripped_leading_finished_items: bool = false;
    let mut trailing_explainer: Option<usize> = None;

    let queue_iterator = queue.items().enumerate();
    let queue_size = queue_iterator.len();

    for (item_index, (_, item)) in queue_iterator {
        let rendered_item = item.render().into();
        let rendered_item_lines = rendered_item.lines.len();

        let mut fixed_trailing_line_cost = 0;
        if leading_explainer.is_some() {
            fixed_trailing_line_cost += 1;
        }
        if trailing_explainer.is_some() {
            fixed_trailing_line_cost += 1;
        }

        if (item_index + 1) == queue_size
            && (used_up_height + rendered_item_lines)
                <= (available_height - fixed_trailing_line_cost)
        {
            // This is the last element and we can fill up the available height without having
            // to pre-/append explainers about remaining items above and below.

            // This also respects having less space if e.g. the leading explainer has already been confirmed.

            included_items.push((
                ListItem::new(rendered_item),
                rendered_item_lines,
                item.is_finished(),
            ));

            // No need to track `used_up_height` anymore.
            // used_up_height += rendered_item_lines;

            break;
        } else if (used_up_height + rendered_item_lines)
            <= (available_height - 2)
        {
            // This is *not* the last element, and there is enough space to fit this queue element
            // on the screen, even if we'll have to add an explainer at the top and on the bottom
            // in the future.
            included_items.push((
                ListItem::new(rendered_item),
                rendered_item_lines,
                item.is_finished(),
            ));

            used_up_height += rendered_item_lines;
        } else {
            // Displaying this element would put us over the available height limit.

            // Don't attempt to strip leading finished items multiple times.
            if has_already_stripped_leading_finished_items {
                // Calculate the trailing explainer, then stop rendering.
                let remaining_items = queue_size - item_index;
                trailing_explainer = Some(remaining_items);

                break;
            }

            // We first remove all leading finished items to potentially clear up some space.
            // This means we'll have to display a leading explainer, but we could very well
            // save a lot of space when the queue has finished a lot of the first items.

            let mut index_to_cut_at: Option<usize> =
                Some(included_items.len() - 1);
            let mut sum_of_freed_lines: usize = 0;
            for (
                incl_item_index,
                (_, incl_rendered_lines_count, incl_has_finished),
            ) in included_items.iter().enumerate()
            {
                if !incl_has_finished {
                    if incl_item_index == 0 {
                        index_to_cut_at = None;
                    } else {
                        index_to_cut_at = Some(incl_item_index);
                    }
                    break;
                }

                sum_of_freed_lines += incl_rendered_lines_count;
            }

            if let Some(cut_index) = index_to_cut_at {
                included_items.drain(0..cut_index);
                used_up_height -= sum_of_freed_lines;

                leading_explainer = Some(cut_index);
            } else {
                // There are no leading finished items to remove. This also means we
                // can't render anything anymore and that we'll need a trailing explainer
                // (which is taken care of above, since we set the `has_stripped_leading_finished` flag).
                leading_explainer = None;
            }

            // If the stripped items are ALL of the current items, we want to be able to
            // run this bit of code again (e.g. in a queue with 100 finished items and a few in-progress items at the end).
            has_already_stripped_leading_finished_items =
                !included_items.is_empty();
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
        )])))
    }

    final_list_items.extend(included_items.into_iter().map(|(item, _, _)| item));

    if let Some(trailing_hidden_items_count) = trailing_explainer {
        let trailing_explainer_contents = format!(
            " ... {} invisible below ... ",
            trailing_hidden_items_count
        );

        let leading_spaces_for_centering = " "
            .repeat((available_width - trailing_explainer_contents.len()) / 2);

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
