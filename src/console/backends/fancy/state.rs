use std::collections::VecDeque;
use crate::console::backends::fancy::terminal::LOG_JOURNAL_MAX_LINES;
use crate::console::backends::shared::{ProgressState, QueueState};

/// Container of entire fancy terminal UI state, and precisely the state required
/// for a render pass.
pub struct TerminalUIState {
    /// Items in library/album/file queue, if any (`None` if disabled).
    pub queue_state: Option<QueueState>,
    
    /// When the progress bar is enabled, this contains the progress bar state.
    pub progress: Option<ProgressState>,
    
    /// Logs to be shown in their own terminal sub-window (oldest to newest).
    pub log_journal: VecDeque<String>,
}

impl TerminalUIState {
    pub fn new() -> Self {
        Self {
            queue_state: None,
            progress: None,
            log_journal: VecDeque::with_capacity(LOG_JOURNAL_MAX_LINES),
        }
    }
}
