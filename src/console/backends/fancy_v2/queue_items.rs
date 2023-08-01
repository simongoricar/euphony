use std::time::Duration;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::console::backends::shared::queue::{
    AlbumQueueItem,
    AlbumQueueItemFinishedResult,
    AlbumQueueItemState,
    FileQueueItem,
    FileQueueItemFinishedResult,
    FileQueueItemState,
    FileQueueItemType,
    GenericQueueItemState,
    QueueItem,
    RenderableQueueItem,
};
use crate::console::backends::shared::{AnimatedSpinner, SpinnerStyle};
use crate::console::colours::{
    X009_RED,
    X060_MEDIUM_PURPLE4,
    X064_CHARTREUSE4,
    X065_DARK_SEA_GREEN4,
    X106_YELLOW4,
    X107_DARK_OLIVE_GREEN3,
    X147_LIGHT_STEEL_BLUE,
    X188_GREY84,
    X209_SALMON1,
    X242_GREY42,
    X244_GREY50,
    X245_GREY54,
    X248_GREY66,
};


pub struct FancyAlbumQueueItem<'config> {
    pub item: AlbumQueueItem<'config>,

    pub spinner: Option<AnimatedSpinner>,
}

impl<'config> FancyAlbumQueueItem<'config> {
    pub fn new(queue_item: AlbumQueueItem<'config>) -> Self {
        Self {
            item: queue_item,
            spinner: None,
        }
    }

    pub fn enable_spinner(
        &mut self,
        style: SpinnerStyle,
        speed_override: Option<Duration>,
    ) {
        self.spinner = Some(AnimatedSpinner::new(style, speed_override));
    }

    pub fn disable_spinner(&mut self) {
        self.spinner = None;
    }
}

impl<'config> QueueItem<AlbumQueueItemFinishedResult>
    for FancyAlbumQueueItem<'config>
{
    #[inline]
    fn get_id(&self) -> crate::console::backends::shared::queue::QueueItemID {
        self.item.get_id()
    }

    #[inline]
    fn get_state(&self) -> GenericQueueItemState {
        self.item.get_state()
    }

    #[inline]
    fn on_item_enqueued(&mut self) {
        self.item.on_item_enqueued();
    }

    fn on_item_started(&mut self) {
        self.item.on_item_started();
        self.enable_spinner(SpinnerStyle::Arc, None);
    }

    fn on_item_finished(&mut self, result: AlbumQueueItemFinishedResult) {
        self.item.on_item_finished(result);
        self.disable_spinner();
    }
}

const ALBUM_ITEM_HEADER_PENDING_STYLE: Style = X248_GREY66;
const ALBUM_ITEM_PREFIX_PENDING_STYLE: Style = ALBUM_ITEM_HEADER_PENDING_STYLE;
const ALBUM_ITEM_CHANGES_PENDING_STYLE: Style = X245_GREY54;

const ALBUM_ITEM_HEADER_IN_PROGRESS_STYLE: Style = X147_LIGHT_STEEL_BLUE;
const ALBUM_ITEM_PREFIX_IN_PROGRESS_STYLE: Style =
    ALBUM_ITEM_HEADER_IN_PROGRESS_STYLE;
const ALBUM_ITEM_CHANGES_IN_PROGRESS_STYLE: Style = X060_MEDIUM_PURPLE4;

const ALBUM_ITEM_HEADER_FINISHED_STYLE: Style = X064_CHARTREUSE4;
const ALBUM_ITEM_PREFIX_FINISHED_STYLE: Style = ALBUM_ITEM_HEADER_FINISHED_STYLE;
const ALBUM_ITEM_CHANGES_FINISHED_STYLE: Style = X065_DARK_SEA_GREEN4;


impl<'config, 'text> RenderableQueueItem<Text<'text>>
    for FancyAlbumQueueItem<'config>
{
    fn render(&self) -> Text<'text> {
        let potential_spinner_prefix = match &self.spinner {
            Some(spinner) => {
                format!(" {} ", spinner.get_current_phase())
            }
            None => match self.item.state {
                AlbumQueueItemState::Pending => "   ",
                AlbumQueueItemState::Queued => "   ",
                AlbumQueueItemState::InProgress => " R ",
                AlbumQueueItemState::Finished { .. } => " F ",
            }
            .to_string(),
        };

        let locked_album_view = self.item.album_view.read();
        let locked_artist_view = locked_album_view.read_lock_artist();

        let prefix_style = match self.item.state {
            AlbumQueueItemState::Pending => ALBUM_ITEM_PREFIX_PENDING_STYLE,
            AlbumQueueItemState::Queued => ALBUM_ITEM_PREFIX_PENDING_STYLE,
            AlbumQueueItemState::InProgress => {
                ALBUM_ITEM_PREFIX_IN_PROGRESS_STYLE
            }
            AlbumQueueItemState::Finished { .. } => {
                ALBUM_ITEM_PREFIX_FINISHED_STYLE
            }
        };

        let header_style = match self.item.state {
            AlbumQueueItemState::Pending => ALBUM_ITEM_HEADER_PENDING_STYLE,
            AlbumQueueItemState::Queued => ALBUM_ITEM_HEADER_PENDING_STYLE,
            AlbumQueueItemState::InProgress => {
                ALBUM_ITEM_HEADER_IN_PROGRESS_STYLE
            }
            AlbumQueueItemState::Finished { .. } => {
                ALBUM_ITEM_HEADER_FINISHED_STYLE
            }
        };

        let changes_text_style = match self.item.state {
            AlbumQueueItemState::Pending => ALBUM_ITEM_CHANGES_PENDING_STYLE,
            AlbumQueueItemState::Queued => ALBUM_ITEM_CHANGES_PENDING_STYLE,
            AlbumQueueItemState::InProgress => {
                ALBUM_ITEM_CHANGES_IN_PROGRESS_STYLE
            }
            AlbumQueueItemState::Finished { .. } => {
                ALBUM_ITEM_CHANGES_FINISHED_STYLE
            }
        };


        // TODO Rewrite the automatic scrolling that hides the completed albums/files when
        //      there is not enough space to display the entire list, then integrate that scrolling
        //      here and in the file queue.
        Text::from(vec![
            Line::from(vec![
                Span::styled(potential_spinner_prefix, prefix_style),
                // TODO Wrap onto newline automatically (with maximum wrap of two lines)
                Span::styled(locked_artist_view.name.to_string(), header_style),
                Span::styled(" - ", header_style),
                Span::styled(
                    locked_album_view.title.to_string(),
                    header_style.add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![Span::styled(
                format!(
                    "    ↳ changes: {} audio and {} data files",
                    self.item.num_changed_audio_files,
                    self.item.num_changed_data_files,
                ),
                changes_text_style,
            )]),
        ])
    }
}



pub struct FancyFileQueueItem<'config> {
    pub item: FileQueueItem<'config>,

    pub spinner: Option<AnimatedSpinner>,
}

impl<'config> FancyFileQueueItem<'config> {
    pub fn new(queue_item: FileQueueItem<'config>) -> Self {
        Self {
            item: queue_item,
            spinner: None,
        }
    }

    pub fn enable_spinner(
        &mut self,
        style: SpinnerStyle,
        speed_override: Option<Duration>,
    ) {
        self.spinner = Some(AnimatedSpinner::new(style, speed_override));
    }

    pub fn disable_spinner(&mut self) {
        self.spinner = None;
    }
}

impl<'config> QueueItem<FileQueueItemFinishedResult>
    for FancyFileQueueItem<'config>
{
    #[inline]
    fn get_id(&self) -> crate::console::backends::shared::queue::QueueItemID {
        self.item.get_id()
    }

    #[inline]
    fn get_state(&self) -> GenericQueueItemState {
        self.item.get_state()
    }

    fn on_item_enqueued(&mut self) {
        self.item.on_item_enqueued()
    }

    fn on_item_started(&mut self) {
        self.item.on_item_started();
        self.enable_spinner(SpinnerStyle::Pixel, None);
    }

    fn on_item_finished(&mut self, result: FileQueueItemFinishedResult) {
        self.item.on_item_finished(result);
        self.disable_spinner();
    }
}


const FILE_ITEM_CONTENT_PENDING_STYLE: Style = X244_GREY50;
const FILE_ITEM_PREFIX_PENDING_STYLE: Style = X242_GREY42;
const FILE_ITEM_TYPE_PENDING_STYLE: Style = X242_GREY42;


const FILE_ITEM_CONTENT_IN_PROGRESS_STYLE: Style = X188_GREY84;
const FILE_ITEM_PREFIX_IN_PROGRESS_TEXT_STYLE: Style = X248_GREY66;
const FILE_ITEM_TYPE_IN_PROGRESS_STYLE: Style = X248_GREY66;


const FILE_ITEM_CONTENT_FINISHED_OK_STYLE: Style = X106_YELLOW4;
const FILE_ITEM_PREFIX_FINISHED_OK_STYLE: Style = X107_DARK_OLIVE_GREEN3;
const FILE_ITEM_TYPE_FINISHED_OK_STYLE: Style = X107_DARK_OLIVE_GREEN3;


const FILE_ITEM_CONTENT_FINISHED_ERROR_TEXT_STYLE: Style = X009_RED;
const FILE_ITEM_PREFIX_FINISHED_ERROR_TEXT_STYLE: Style = X209_SALMON1;
const FILE_ITEM_TYPE_FINISHED_ERROR_STYLE: Style = X209_SALMON1;


impl<'config, 'text> RenderableQueueItem<Line<'text>>
    for FancyFileQueueItem<'config>
{
    fn render(&self) -> Line<'text> {
        let potential_spinner_prefix = match &self.spinner {
            Some(spinner) => {
                format!(" {} ", spinner.get_current_phase())
            }
            None => match self.item.state {
                FileQueueItemState::Pending => "   ",
                FileQueueItemState::Queued => "   ",
                FileQueueItemState::InProgress => " R ",
                FileQueueItemState::Finished { ref result } => match result {
                    FileQueueItemFinishedResult::Ok => " F ",
                    FileQueueItemFinishedResult::Failed(_) => " E ",
                },
            }
            .to_string(),
        };

        let file_type_str: &'static str = match self.item.file_type {
            FileQueueItemType::Audio => "[a]",
            FileQueueItemType::Data => "[d]",
            FileQueueItemType::Unknown => "[?]",
        };

        let content_text_style = match self.item.state {
            FileQueueItemState::Pending => FILE_ITEM_CONTENT_PENDING_STYLE,
            FileQueueItemState::Queued => FILE_ITEM_CONTENT_PENDING_STYLE,
            FileQueueItemState::InProgress => {
                FILE_ITEM_CONTENT_IN_PROGRESS_STYLE
            }
            FileQueueItemState::Finished { ref result } => match result {
                FileQueueItemFinishedResult::Ok => {
                    FILE_ITEM_CONTENT_FINISHED_OK_STYLE
                }
                FileQueueItemFinishedResult::Failed(_) => {
                    FILE_ITEM_CONTENT_FINISHED_ERROR_TEXT_STYLE
                }
            },
        };

        let prefix_style = match self.item.state {
            FileQueueItemState::Pending => FILE_ITEM_PREFIX_PENDING_STYLE,
            FileQueueItemState::Queued => FILE_ITEM_PREFIX_PENDING_STYLE,
            FileQueueItemState::InProgress => {
                FILE_ITEM_PREFIX_IN_PROGRESS_TEXT_STYLE
            }
            FileQueueItemState::Finished { ref result } => match result {
                FileQueueItemFinishedResult::Ok => {
                    FILE_ITEM_PREFIX_FINISHED_OK_STYLE
                }
                FileQueueItemFinishedResult::Failed(_) => {
                    FILE_ITEM_PREFIX_FINISHED_ERROR_TEXT_STYLE
                }
            },
        };

        let type_style = match self.item.state {
            FileQueueItemState::Pending => FILE_ITEM_TYPE_PENDING_STYLE,
            FileQueueItemState::Queued => FILE_ITEM_TYPE_PENDING_STYLE,
            FileQueueItemState::InProgress => FILE_ITEM_TYPE_IN_PROGRESS_STYLE,
            FileQueueItemState::Finished { ref result } => match result {
                FileQueueItemFinishedResult::Ok => {
                    FILE_ITEM_TYPE_FINISHED_OK_STYLE
                }
                FileQueueItemFinishedResult::Failed(_) => {
                    FILE_ITEM_TYPE_FINISHED_ERROR_STYLE
                }
            },
        };

        Line::from(vec![
            Span::styled(potential_spinner_prefix, prefix_style),
            Span::styled(file_type_str, type_style),
            Span::raw(" "),
            Span::styled(
                format!("\"{}\"", self.item.file_name),
                content_text_style,
            ),
        ])
    }
}
