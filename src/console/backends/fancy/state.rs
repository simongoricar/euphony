use std::collections::VecDeque;
use crate::console::backends::shared::{ProgressState, QueueState};


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
            log_journal: VecDeque::default(),
        }
    }
}
