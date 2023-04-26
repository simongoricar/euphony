use std::collections::VecDeque;

use crate::console::backends::fancy::queue::{
    FancyAlbumQueueItem,
    FancyFileQueueItem,
};
use crate::console::backends::fancy::terminal::LOG_JOURNAL_MAX_LINES;
use crate::console::backends::shared::queue_v2::{
    AlbumItemFinishedResult,
    FileItemFinishedResult,
    Queue,
};
use crate::console::backends::shared::ProgressState;

/// Container of entire fancy terminal UI state, and precisely the state required
/// for a render pass.
pub struct TerminalUIState<'a> {
    /// Album queue, if any (`None` if disabled).
    pub album_queue:
        Option<Queue<FancyAlbumQueueItem<'a>, AlbumItemFinishedResult>>,

    /// File queue, if any (`None` if disabled).
    pub file_queue:
        Option<Queue<FancyFileQueueItem<'a>, FileItemFinishedResult>>,

    /// When the progress bar is enabled, this contains the progress bar state.
    pub progress: Option<ProgressState>,

    /// Logs to be shown in their own terminal sub-window (oldest to newest).
    pub log_journal: VecDeque<String>,
}

impl<'a> TerminalUIState<'a> {
    pub fn new() -> Self {
        Self {
            album_queue: None,
            file_queue: None,
            progress: None,
            log_journal: VecDeque::with_capacity(LOG_JOURNAL_MAX_LINES),
        }
    }
}
