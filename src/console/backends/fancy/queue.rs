use std::time::Duration;

use ratatui::text::{Line, Span, Text};

use crate::console::backends::shared::queue::{
    AlbumQueueItem,
    AlbumQueueItemFinishedResult,
    FileQueueItem,
    FileQueueItemFinishedResult,
    FileQueueItemType,
    GenericQueueItemState,
    QueueItem,
    QueueItemID,
    RenderableQueueItem,
};
use crate::console::backends::shared::{AnimatedSpinner, SpinnerStyle};

/*
 * ALBUM QUEUE ITEM implementation (fancy backend-specific)
 */
pub struct FancyAlbumQueueItem<'config> {
    pub item: AlbumQueueItem<'config>,

    pub spinner: Option<AnimatedSpinner>,

    pub pad_leading_space_when_spinner_is_disabled: bool,
}

impl<'a> FancyAlbumQueueItem<'a> {
    pub fn new(queue_item: AlbumQueueItem<'a>) -> Self {
        Self {
            item: queue_item,
            spinner: None,
            pad_leading_space_when_spinner_is_disabled: true,
        }
    }

    pub fn enable_spinner(
        &mut self,
        style: SpinnerStyle,
        speed: Option<Duration>,
    ) {
        self.spinner = Some(AnimatedSpinner::new(style, speed));
    }

    pub fn disable_spinner(&mut self) {
        self.spinner = None;
    }
}

impl<'a> QueueItem<AlbumQueueItemFinishedResult> for FancyAlbumQueueItem<'a> {
    #[inline]
    fn get_id(&self) -> QueueItemID {
        self.item.get_id()
    }

    #[inline]
    fn get_state(&self) -> GenericQueueItemState {
        self.item.get_state()
    }

    fn on_item_enqueued(&mut self) {
        self.item.on_item_enqueued();
    }

    fn on_item_started(&mut self) {
        self.item.on_item_started();

        self.enable_spinner(SpinnerStyle::Pixel, None);
    }

    fn on_item_finished(&mut self, result: AlbumQueueItemFinishedResult) {
        self.item.on_item_finished(result);

        self.disable_spinner();
    }
}

impl<'a, 'b> RenderableQueueItem<Text<'b>> for FancyAlbumQueueItem<'a> {
    fn render(&self) -> Text<'b> {
        let prefix = if let Some(spinner) = &self.spinner {
            format!(" {} ", spinner.get_current_phase())
        } else if self.pad_leading_space_when_spinner_is_disabled {
            "  ".into()
        } else {
            "".into()
        };

        // TODO Add colouring based on completion.
        let rendered_spans: Vec<Span> = {
            let album_locked = self.item.album_view.read();

            vec![
                Span::raw(prefix),
                Span::raw(self.item.num_changed_files.to_string()),
                Span::raw(format!(
                    "{} - {}",
                    album_locked.read_lock_artist().name,
                    album_locked.title
                )),
            ]
        };

        Text {
            lines: vec![Line::from(rendered_spans)],
        }
    }
}


/*
 * FILE QUEUE ITEM implementation (fancy backend-specific)
 */
pub struct FancyFileQueueItem<'item> {
    pub item: FileQueueItem<'item>,

    pub spinner: Option<AnimatedSpinner>,

    pub pad_leading_space_when_spinner_is_disabled: bool,
}

impl<'a> FancyFileQueueItem<'a> {
    pub fn new(queue_item: FileQueueItem<'a>) -> Self {
        Self {
            item: queue_item,
            spinner: None,
            pad_leading_space_when_spinner_is_disabled: true,
        }
    }

    pub fn enable_spinner(
        &mut self,
        style: SpinnerStyle,
        speed: Option<Duration>,
    ) {
        self.spinner = Some(AnimatedSpinner::new(style, speed));
    }

    pub fn disable_spinner(&mut self) {
        self.spinner = None;
    }
}

impl<'a> QueueItem<FileQueueItemFinishedResult> for FancyFileQueueItem<'a> {
    #[inline]
    fn get_id(&self) -> QueueItemID {
        self.item.get_id()
    }

    #[inline]
    fn get_state(&self) -> GenericQueueItemState {
        self.item.get_state()
    }

    fn on_item_enqueued(&mut self) {
        self.item.on_item_enqueued();
    }

    fn on_item_started(&mut self) {
        self.item.on_item_started();

        self.enable_spinner(SpinnerStyle::Square, None);
    }

    fn on_item_finished(&mut self, result: FileQueueItemFinishedResult) {
        self.item.on_item_finished(result);

        self.disable_spinner();
    }
}

impl<'a, 'b> RenderableQueueItem<Text<'b>> for FancyFileQueueItem<'a> {
    fn render(&self) -> Text<'b> {
        let prefix = if let Some(spinner) = &self.spinner {
            format!(" {} ", spinner.get_current_phase())
        } else if self.pad_leading_space_when_spinner_is_disabled {
            "  ".into()
        } else {
            "".into()
        };

        let file_type_str = match self.item.file_type {
            FileQueueItemType::Audio => "[audio]",
            FileQueueItemType::Data => " [data]",
            FileQueueItemType::Unknown => "   [??]",
        };

        // TODO Add colouring based on completion.
        let rendered_spans: Vec<Span> = vec![
            Span::raw(prefix),
            Span::raw(file_type_str),
            Span::raw(" "),
            Span::raw(self.item.file_name.clone()),
        ];

        Text {
            lines: vec![Line::from(rendered_spans)],
        }
    }
}
