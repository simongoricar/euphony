use std::collections::VecDeque;
use std::fmt::Display;
use std::io::{Stdout, stdout};
use std::sync::{Arc, mpsc, Mutex, MutexGuard};
use std::sync::mpsc::{Sender, TryRecvError};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossterm::ExecutableCommand;
use crossterm::style::Print;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use miette::{IntoDiagnostic, miette, Result, WrapErr};
use tui::{Frame, Terminal};
use tui::backend::CrosstermBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Gauge, List, ListItem};

use crate::console::{LogBackend, QueueItem, QueueItemID};
use crate::console::traits::{TerminalBackend, TranscodeBackend};

const LOG_JOURNAL_MAX_LINES: usize = 20;
const TERMINAL_REFRESH_RATE_SECONDS: f64 = 0.1;




struct TUITerminalBackendState {
    /// Items in queue, ordered from oldest to newest (if `Some`).
    queue_items: Option<Vec<QueueItem>>,
    
    /// If `Some`, a percentage (0 to 100) of the queued items.
    progress_percent: Option<u16>,
    
    /// Logs to be shown in their own terminal sub-window (oldest to newest).
    log_journal: VecDeque<String>,
}

impl TUITerminalBackendState {
    pub fn new() -> Self {
        Self {
            queue_items: None,
            progress_percent: None,
            log_journal: VecDeque::default(),
        }
    }
}


pub struct TUITerminalBackend {
    /// `tui::Terminal`, which is how we interact with the terminal and build a terminal UI.
    terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    
    /// Whether `setup()` has been called, meaning that appropriate terminal setup has been done
    /// and that the render thread is active.
    has_been_set_up: bool,
    
    /// When `has_been_set_up` is true, `render_thread` contains a handle to the render thread.
    render_thread: Option<JoinHandle<Result<()>>>,
    
    /// When `has_been_set_up` is true, `render_thread_channel` contains a sender with which to
    /// signal to the render thread that it should stop.
    render_thread_channel: Option<Sender<()>>,
    
    /// Houses non-terminal-organisation related data - this is precisely
    /// the data required for a render pass.
    state: Arc<Mutex<TUITerminalBackendState>>,
}

impl TUITerminalBackend {
    pub fn new() -> Result<Self> {
        let terminal = Terminal::new(CrosstermBackend::new(stdout()))
            .into_diagnostic()?;
        
        Ok(Self {
            terminal: Arc::new(Mutex::new(terminal)),
            has_been_set_up: false,
            render_thread: None,
            render_thread_channel: None,
            state: Arc::new(Mutex::new(TUITerminalBackendState::new()))
        })
    }
    
    fn lock_state(&self) -> MutexGuard<TUITerminalBackendState> {
        self.state.lock().unwrap()
    }
    
    /// If the current log journal exceeds the set limit of lines, this method drops the oldest
    /// logs in order to shrink the log back down.
    fn trim_log_journal(&mut self) {
        let mut state = self.lock_state();
        
        let current_log_count = state.log_journal.len();
        if current_log_count > LOG_JOURNAL_MAX_LINES {
            let logs_to_prune = current_log_count - LOG_JOURNAL_MAX_LINES;
            state.log_journal.drain(0..logs_to_prune - 1);
        }
    }
    
    fn perform_render(state: MutexGuard<TUITerminalBackendState>, frame: &mut Frame<CrosstermBackend<Stdout>>) {
        // Render entire terminal UI based on the current state.
        
        // Dynamically constrain the layout, hiding some UI elements when they are disabled.
        let layout_constraints: Vec<Constraint> = vec![
            if state.queue_items.is_some() {
                Constraint::Percentage(50)
            } else {
                Constraint::Length(0)
            },
            if state.progress_percent.is_some() {
                Constraint::Length(3)
            } else {
                Constraint::Length(0)
            },
            Constraint::Min(8),
        ];
        
        let multi_block_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(layout_constraints.as_ref())
            .split(frame.size());
        
        let area_queue = multi_block_layout[0];
        let area_progress_bar = multi_block_layout[1];
        let area_logs = multi_block_layout[2];
        
        // Most of the implementation below depends on whether the specific functionality has been enabled
        // (`queue_begin_processing`, `progress_begin`, ...).
        // If it is currently disabled a simple placeholder `tui::widgets::Block` is shown in most cases.
        
        // TODO Implement multiple queues - two vertically on the left side
        //      (libraries and albums underneath), one on the right (file queue for current album).
        
        // 1. Queue
        if let Some(queue_items) = &state.queue_items {
            let mut queue_list_items: Vec<ListItem> = Vec::with_capacity(area_queue.height as usize);
            for item in queue_items.iter().take(area_queue.height as usize) {
                let item_style = if item.is_active {
                    Style::default()
                        .fg(Color::Green)
                } else {
                    Style::default()
                        .fg(Color::DarkGray)
                };
        
                queue_list_items.push(
                    ListItem::new(item.content.clone())
                        .style(item_style)
                );
            }
    
            let queue = List::new(queue_list_items)
                .block(
                    Block::default()
                        .title("Processing queue")
                        .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
                        .title_alignment(Alignment::Left)
                );
    
            frame.render_widget(queue, area_queue);
            
        } else {
            let empty_queue = Block::default()
                .title("Processing queue (inactive)")
                .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
                .title_alignment(Alignment::Left);
            
            frame.render_widget(empty_queue, area_queue);
        }
        
        
        // 2. Progress Bar
        if let Some(percent) = state.progress_percent {
            let progress_bar = Gauge::default()
                .block(
                    Block::default()
                        .title("Progress")
                        .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                        .title_alignment(Alignment::Left)
                )
                .gauge_style(
                    Style::default()
                        .fg(Color::LightBlue)
                )
                .percent(percent);
            
            frame.render_widget(progress_bar, area_progress_bar);
            
        } else {
            let empty_progress_bar = Block::default()
                .title("Progress (inactive)")
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .title_alignment(Alignment::Left);
            
            frame.render_widget(empty_progress_bar, area_progress_bar);
        }
        
        
        // 3. Logs
        let mut logs_list_items: Vec<ListItem> = Vec::with_capacity(area_logs.height as usize);
        for log in state.log_journal.iter().take(area_logs.height as usize) {
            logs_list_items.push(
                ListItem::new(log.to_string())
            );
        }
        
        let logs = List::new(logs_list_items)
            .block(
                Block::default()
                    .title("Logs")
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .title_alignment(Alignment::Left)
            );
        
        frame.render_widget(logs, area_logs);
    }
}

impl TerminalBackend for TUITerminalBackend {
    fn setup(&mut self) -> Result<()> {
        enable_raw_mode()
            .into_diagnostic()?;
        
        let mut terminal = self.terminal.lock().unwrap();
    
        // Prepare space for terminal UI (without drawing over previous content).
        let size = terminal.size()
            .into_diagnostic()?;
        
        terminal.backend_mut()
            .execute(Print("\n".repeat(size.height as usize)))
            .into_diagnostic()?;
        terminal.clear()
            .into_diagnostic()?;
        
        // We create a simple one-way channel that we can now use to signal to the render thread
        // to stop rendering and exit.
        let (tx, rx) = mpsc::channel::<()>();
        
        let terminal_render_thread_clone = self.terminal.clone();
        let state_render_thread_clone = self.state.clone();
        
        let render_thread: JoinHandle<Result<()>> = thread::spawn(move || {
            // Continiously render terminal UI (until stop signal is received via channel).
            loop {
                // We might get a signal (via a multiproducer-singleconsumer channel) to stop rendering,
                // which is why we check our Receiver every iteration. If there is a message, we stop rendering
                // and exit the thread.
                match rx.try_recv() {
                    Ok(_) => {
                        // Main thread signaled us to stop, exit by returning Ok(()).
                        return Ok(())
                    },
                    Err(error) => match error {
                        TryRecvError::Empty => {
                            // Nothing should be done - main thread simply hasn't sent us a request to stop.
                        }
                        TryRecvError::Disconnected => {
                            // Something went very wrong, panic (main thread somehow died or dropped Sender).
                            panic!("Main thread has disconnected.");
                        }
                    }
                }
                
                // Perform drawing and thread sleeping.
                // (subtracts drawing time from tick rate to preserve a consistent update rate)
                let time_pre_draw = Instant::now();
    
                {
                    let mut terminal = terminal_render_thread_clone.lock().unwrap();
                    let state = state_render_thread_clone.lock().unwrap();
    
                    terminal
                        .draw(
                            |f|
                                TUITerminalBackend::perform_render(state, f)
                        )
                        .into_diagnostic()?;
                }
                
                let time_draw_delta = time_pre_draw.elapsed().as_secs_f64();
                let adjusted_sleep_time = if time_draw_delta >= TERMINAL_REFRESH_RATE_SECONDS {
                    0.0
                } else {
                    TERMINAL_REFRESH_RATE_SECONDS - time_draw_delta
                };
                
                thread::sleep(Duration::from_secs_f64(adjusted_sleep_time));
            }
        });
        
        self.render_thread = Some(render_thread);
        self.render_thread_channel = Some(tx);
        self.has_been_set_up = true;
        
        Ok(())
    }
    
    fn destroy(self) -> Result<()> {
        if !self.has_been_set_up {
            return Ok(());
        }
        
        let render_thread_stop_sender = self.render_thread_channel
            .expect("has_been_set_up is true, but no render thread Sender?!");
        render_thread_stop_sender.send(())
            .into_diagnostic()
            .wrap_err("Could not send stop signal to render thread.")?;
        
        let render_thread = self.render_thread
            .expect("has_been_set_up is true, but no render thread?!");
        render_thread.join()
            .expect("Render thread panicked!")?;
        
        disable_raw_mode()
            .into_diagnostic()?;
        
        Ok(())
    }
}

impl LogBackend for TUITerminalBackend {
    fn log_newline(&mut self) {
        {
            let mut state = self.lock_state();
            state.log_journal.push_back("\n".to_string());
        }
        
        self.trim_log_journal();
    }
    
    fn log_println<T: Display>(&mut self, content: T) {
        {
            let terminal = self.terminal.lock().unwrap();
            let mut state = self.lock_state();
    
            let terminal_width = terminal.size()
                .expect("Could not get terminal width.")
                .width as usize;
    
            for line in content.to_string().split('\n') {
                if line.len() > terminal_width {
                    // Will require a manual line break (possibly multiple).
            
                    // An elegant solution that works on multi-byte characters:
                    // https://users.rust-lang.org/t/solved-how-to-split-string-into-multiple-sub-strings-with-given-length/10542/12
                    let mut characters = line.chars();
                    let chunks = (0..)
                        .map(|_| characters.by_ref().take(terminal_width).collect::<String>())
                        .take_while(|str| !str.is_empty())
                        .collect::<Vec<String>>();
            
                    for chunk in chunks {
                        state.log_journal.push_back(chunk);
                    }
                } else {
                    state.log_journal.push_back(line.to_string());
                }
            };
        }
        
        self.trim_log_journal();
    }
}

impl TranscodeBackend for TUITerminalBackend {
    fn queue_begin(&mut self) {
        let mut state = self.lock_state();
        state.queue_items = Some(Vec::new());
    }
    
    fn queue_end(&mut self) {
        let mut state = self.lock_state();
        state.queue_items = None;
    }
    
    fn queue_item_add<T: Display>(&mut self, item: T) -> Result<QueueItemID> {
        let mut state = self.lock_state();
        
        let queue = state.queue_items
            .as_mut()
            .ok_or_else(|| miette!("Queue is currently disabled, can't add item."))?;
        
        let queue_item = QueueItem::new(item.to_string());
        let queue_item_id = queue_item.id;
        
        queue.push(queue_item);
        
        Ok(queue_item_id)
    }
    
    fn queue_item_start(&mut self, item_id: QueueItemID) -> Result<()> {
        let mut state = self.lock_state();
        
        let queue = state.queue_items
            .as_mut()
            .ok_or_else(|| miette!("Queue is currently disabled, can't set item as active."))?;
        
        let target_item = queue
            .iter_mut()
            .find(|item| item.id == item_id);
        
        if let Some(item) = target_item {
            item.is_active = true;
            Ok(())
        } else {
            Err(miette!("No such queue item."))
        }
    }
    
    fn queue_item_finish(&mut self, item_id: QueueItemID, was_ok: bool) -> Result<()> {
        let mut state = self.lock_state();
        
        let queue = state.queue_items
            .as_mut()
            .ok_or_else(|| miette!("Queue is currently disabled, can't set item as active."))?;
    
        let target_item = queue
            .iter_mut()
            .find(|item| item.id == item_id);
    
        if let Some(item) = target_item {
            item.is_active = false;
            item.is_ok = was_ok;
            Ok(())
        } else {
            Err(miette!("No such queue item."))
        }
    }
    
    fn queue_item_remove(&mut self, item_id: QueueItemID) -> Result<()> {
        let mut state = self.lock_state();
        
        let queue = state.queue_items
            .as_mut()
            .ok_or_else(|| miette!("Queue is currently disabled, can't set item as active."))?;
    
        let target_item_position = queue
            .iter()
            .position(|item| item.id == item_id);
        
        if let Some(item_position) = target_item_position {
            queue.remove(item_position);
            Ok(())
        } else {
            Err(miette!("No such queue item."))
        }
    }
    
    fn queue_clear(&mut self) -> Result<()> {
        let mut state = self.lock_state();
        
        if let Some(queue) = &mut state.queue_items {
            queue.clear();
            Ok(())
        } else {
            Err(miette!("Queue is currently disabled, can't clear."))
        }
    }
    
    fn progress_begin(&mut self) {
        let mut state = self.lock_state();
        state.progress_percent = Some(0);
    }
    
    fn progress_end(&mut self) {
        let mut state = self.lock_state();
        state.progress_percent = None;
    }
    
    fn progress_set_percent(&mut self, percent: u16) {
        let mut state = self.lock_state();
        state.progress_percent = Some(percent);
    }
}
