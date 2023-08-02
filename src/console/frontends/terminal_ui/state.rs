use std::collections::vec_deque::Iter;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Stdout};
use std::sync::Arc;
use std::thread::ScopedJoinHandle;

use chrono::{DateTime, Local};
use miette::Result;
use parking_lot::Mutex;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::broadcast;

use crate::cancellation::CancellationToken;
use crate::console::frontends::shared::queue::{
    AlbumQueueItemFinishedResult,
    FileQueueItemFinishedResult,
    Queue,
};
use crate::console::frontends::shared::Progress;
use crate::console::frontends::terminal_ui::queue_items::{
    FancyAlbumQueueItem,
    FancyFileQueueItem,
};
use crate::console::UserControlMessage;


const LOG_JOURNAL_DEFAULT_MAXIMUM_HISTORY: usize = 40;


pub struct TerminalState<'thread_scope> {
    pub terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,

    pub cursor_end_position: (u16, u16),

    pub user_control_sender: broadcast::Sender<UserControlMessage>,

    pub render_thread_join_handle: ScopedJoinHandle<'thread_scope, Result<()>>,

    pub render_thread_cancellation_token: CancellationToken,
}



pub enum LogOutputMode<'thread_scope> {
    None,
    ToFile {
        buf_writer: Arc<Mutex<BufWriter<strip_ansi_escapes::Writer<File>>>>,
        writer_flushing_thread_handle:
            ScopedJoinHandle<'thread_scope, Result<()>>,
        writer_flushing_thread_cancellation_token: CancellationToken,
    },
}

impl<'thread_scope> Default for LogOutputMode<'thread_scope> {
    fn default() -> Self {
        Self::None
    }
}

pub struct LogJournal {
    journal: VecDeque<(String, DateTime<Local>)>,
    maximum_history: usize,
}

impl LogJournal {
    pub fn new(maximum_history: usize) -> Self {
        Self {
            journal: VecDeque::with_capacity(maximum_history),
            maximum_history,
        }
    }

    pub fn insert_entry<S: Into<String>>(&mut self, entry: S) {
        // We pop first if full because otherwise the deque would have to
        // pointlessly allocate more space.
        if self.journal.len() == self.maximum_history {
            self.journal.pop_back();
        }

        self.journal.push_front((entry.into(), Local::now()));
    }

    pub fn iter_most_recent_first(&self) -> Iter<'_, (String, DateTime<Local>)> {
        self.journal.iter()
    }
}

pub struct LogState<'thread_scope> {
    pub log_output: LogOutputMode<'thread_scope>,

    pub log_journal: LogJournal,
}

impl<'thread_scope> LogState<'thread_scope> {
    pub fn new() -> Self {
        Self {
            log_output: LogOutputMode::default(),
            log_journal: LogJournal::new(LOG_JOURNAL_DEFAULT_MAXIMUM_HISTORY),
        }
    }
}



#[derive(Eq, PartialEq, Copy, Clone)]
pub enum UIPage {
    Transcoding,
    Logs,
}

pub struct UIState<'config> {
    pub album_queue: Option<
        Queue<FancyAlbumQueueItem<'config>, AlbumQueueItemFinishedResult>,
    >,

    pub file_queue:
        Option<Queue<FancyFileQueueItem<'config>, FileQueueItemFinishedResult>>,

    pub progress: Option<Progress>,

    pub current_page: UIPage,
}

impl<'config> UIState<'config> {
    pub fn new() -> Self {
        Self {
            album_queue: None,
            file_queue: None,
            progress: None,
            current_page: UIPage::Logs,
        }
    }
}
