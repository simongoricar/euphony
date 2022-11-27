use std::cmp::min;
use std::fmt::Display;
use std::io::{Stdout, stdout};
use std::sync::{Arc, mpsc, Mutex, MutexGuard};
use std::sync::mpsc::{Sender, TryRecvError};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use ansi_to_tui::IntoText;

use crossterm::ExecutableCommand;
use crossterm::style::Print;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use miette::{IntoDiagnostic, miette, Result, WrapErr};
use tui::{Frame, Terminal};
use tui::backend::{Backend, CrosstermBackend};
use tui::layout::{Alignment, Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::Span;
use tui::widgets::{Block, Borders, Gauge, List, ListItem};

use crate::console::backends::{QueueItem, QueueItemID, QueueType};
use crate::console::backends::fancy::{ProgressState, QueueState};
use crate::console::backends::fancy::queue::{generate_dynamic_list_from_queue_items, ListItemStyleRules, QueueItemFinishedState};
use crate::console::backends::fancy::state::TerminalUIState;
use crate::console::LogBackend;
use crate::console::traits::{TerminalBackend, TranscodeBackend};

const LOG_JOURNAL_MAX_LINES: usize = 20;
const TERMINAL_REFRESH_RATE_SECONDS: f64 = 0.05;


pub struct TUITerminalBackend {
    /// `tui::Terminal`, which is how we interact with the terminal and build a terminal UI.
    terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    
    /// An end cursor position we save in setup - this allows us to restore the
    /// ending cursor position when the backend is destroyed.
    terminal_end_cursor_position: Option<(u16, u16)>,
    
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
    state: Arc<Mutex<TerminalUIState>>,
}

impl TUITerminalBackend {
    pub fn new() -> Result<Self> {
        let terminal = Terminal::new(CrosstermBackend::new(stdout()))
            .into_diagnostic()?;
        
        Ok(Self {
            terminal: Arc::new(Mutex::new(terminal)),
            terminal_end_cursor_position: None,
            has_been_set_up: false,
            render_thread: None,
            render_thread_channel: None,
            state: Arc::new(Mutex::new(TerminalUIState::new()))
        })
    }
    
    fn lock_state(&self) -> MutexGuard<TerminalUIState> {
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
    
    /// Perform a full render of all terminal UI widgets.
    /// `state` is a mutex guard with the locked terminal state behind it,
    /// `frame` is the `tui` terminal frame to draw on and `frame_size_height_offset` is an
    /// optional argument that can be used to increase or decrease the height of the drawing area
    /// (this is used in the last render pass).
    fn perform_render(
        state: MutexGuard<TerminalUIState>,
        frame: &mut Frame<CrosstermBackend<Stdout>>,
        frame_size_height_offset: Option<isize>,
    ) {
        // Render entire terminal UI based on the current state.
        let mut frame_size = frame.size();
        if let Some(offset) = frame_size_height_offset {
            let adjusted_height = (frame_size.height as isize) + offset;
            if adjusted_height < 0 {
                panic!("Given frame size height offset would result in subzero height.");
            }
    
            frame_size.height = adjusted_height as u16;
        }
        
        // Dynamically constrain the layout, hiding some UI elements when they are disabled.
        let layout_constraints: Vec<Constraint> = vec![
            if state.queue_state.is_some() {
                Constraint::Percentage(65)
            } else {
                Constraint::Length(0)
            },
            if state.progress.is_some() {
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
            .split(frame_size);
        
        let queue_horizontal_layout = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints(
                [
                    Constraint::Percentage(45),
                    Constraint::Percentage(55),
                ].as_ref()
            )
            .split(multi_block_layout[0]);
        
        let queue_left_vertical_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(
                [
                    Constraint::Percentage(35),
                    Constraint::Percentage(65),
                ].as_ref()
            )
            .split(queue_horizontal_layout[0]);
        
        let area_queue_top_left = queue_left_vertical_layout[0];
        let area_queue_bottom_left = queue_left_vertical_layout[1];
        let area_queue_right = queue_horizontal_layout[1];
        let area_progress_bar = multi_block_layout[1];
        let area_logs = multi_block_layout[2];
        
        // Most of the implementation below depends on whether the specific functionality has been enabled
        // (`queue_begin_processing`, `progress_begin`, ...).
        // If it is currently disabled a simple placeholder `tui::widgets::Block` is shown in most cases.
        
        
        // 1. Queue (three queues)
        
        // Constant styles that are applied when generating dynamic lists for each queue.
        let queue_libraries_styles = ListItemStyleRules {
            item_pending_style: Style::default().fg(Color::DarkGray),
            item_in_progress_style: Style::default().fg(Color::LightBlue),
            item_finished_ok_style: Style::default().fg(Color::Indexed(65)), // DarkSeaGreen4 (#5f875f)
            item_finished_not_ok_style: Style::default().fg(Color::Indexed(65)), // DarkSeaGreen4 (#5f875f)
            leading_completed_items_style: Style::default().fg(Color::DarkGray),
            trailing_hidden_pending_items_style: Style::default().fg(Color::DarkGray),
        };
        let queue_albums_styles = ListItemStyleRules {
            item_pending_style: Style::default().fg(Color::DarkGray),
            item_in_progress_style: Style::default().fg(Color::LightBlue),
            item_finished_ok_style: Style::default().fg(Color::Indexed(65)), // DarkSeaGreen4 (#5f875f)
            item_finished_not_ok_style: Style::default().fg(Color::Indexed(65)), // DarkSeaGreen4 (#5f875f)
            leading_completed_items_style: Style::default().fg(Color::DarkGray),
            trailing_hidden_pending_items_style: Style::default().fg(Color::DarkGray),
        };
        let queue_files_styles = ListItemStyleRules {
            item_pending_style: Style::default().fg(Color::DarkGray),
            item_in_progress_style: Style::default().fg(Color::LightYellow),
            item_finished_ok_style: Style::default().fg(Color::Green),
            item_finished_not_ok_style: Style::default().fg(Color::Indexed(172)), // Orange3 (#d78700)
            leading_completed_items_style: Style::default().fg(Color::DarkGray),
            trailing_hidden_pending_items_style: Style::default().fg(Color::DarkGray),
        };
        
        if let Some(queue) = &state.queue_state {
            // 1.1 Top Left Queue (libraries)
            let library_dynamic_queue_items = generate_dynamic_list_from_queue_items(
                &queue.library_items,
                queue_libraries_styles,
                area_queue_top_left.height as usize,
            ).expect("Could not generate dynamic list for library queue.");
            
            let library_queue = List::new(library_dynamic_queue_items)
                .block(
                    Block::default()
                        .title(
                            Span::styled(
                                "Libraries",
                                Style::default()
                                    .fg(Color::Indexed(104)) //  MediumPurple (#8787d7)
                                    .add_modifier(Modifier::BOLD)
                            )
                        )
                        .borders(Borders::ALL)
                        .title_alignment(Alignment::Left)
                );
            
            frame.render_widget(library_queue, area_queue_top_left);
            
            
            // 1.2 Bottom Left Queue (albums)
            let album_dynamic_queue_items = generate_dynamic_list_from_queue_items(
                &queue.album_items,
                queue_albums_styles,
                area_queue_bottom_left.height as usize,
            ).expect("Could not generate dynamic list for album queue.");
            
            let album_queue = List::new(album_dynamic_queue_items)
                .block(
                    Block::default()
                        .title(
                            Span::styled(
                                "Albums",
                                Style::default()
                                    .fg(Color::Indexed(117)) // SkyBlue1 (#87d7ff)
                                    .add_modifier(Modifier::BOLD)
                            )
                        )
                        .borders(Borders::ALL)
                        .title_alignment(Alignment::Left)
                );
            
            frame.render_widget(album_queue, area_queue_bottom_left);
            
            
            // 1.3 Right Queue (items/files)
            let file_dynamic_queue_items = generate_dynamic_list_from_queue_items(
                &queue.file_items,
                queue_files_styles,
                area_queue_right.height as usize,
            ).expect("Could not generate dynamic list for file queue.");
            
            let file_queue = List::new(file_dynamic_queue_items)
                .block(
                    Block::default()
                        .title(
                            Span::styled(
                                "Files",
                                Style::default()
                                    .fg(Color::Indexed(139)) // Grey63 (#af87af)
                                    .add_modifier(Modifier::BOLD)
                            )
                        )
                        .borders(Borders::ALL)
                        .title_alignment(Alignment::Left)
                );
            
            frame.render_widget(file_queue, area_queue_right);
        }
        
        // 2. Progress Bar
        if let Some(progress) = &state.progress {
            let progress_bar = Gauge::default()
                .block(
                    Block::default()
                        .title(
                            format!(
                                "Progress ({}/{})",
                                progress.current,
                                progress.total,
                            )
                        )
                        .borders(Borders::ALL)
                        .title_alignment(Alignment::Left)
                )
                .gauge_style(
                    Style::default()
                        .fg(Color::Indexed(61)) // SlateBlue3 (#5f5faf)
                        .bg(Color::Reset)
                )
                .percent(progress.get_percent());
            
            frame.render_widget(progress_bar, area_progress_bar);
            
        } else {
            let empty_progress_bar = Block::default()
                .title(
                    Span::styled(
                        "Progress (inactive)",
                        Style::default()
                            .add_modifier(Modifier::ITALIC)
                    )
                )
                .borders(Borders::ALL)
                .title_alignment(Alignment::Left);
            
            frame.render_widget(empty_progress_bar, area_progress_bar);
        }
        
        
        // 3. Logs
        // TODO Figure out why this doesn't always update in time.
        let log_lines_visible_count = min(area_logs.height as usize, state.log_journal.len());
        
        let mut logs_list_items: Vec<ListItem> = Vec::with_capacity(log_lines_visible_count);
        for log in state.log_journal.range(state.log_journal.len() - log_lines_visible_count..) {
            logs_list_items.push(
                ListItem::new(
                    log
                        .into_text()
                        .expect("Could not convert str into tui::Text.")
                )
            );
        }
        
        let logs = List::new(logs_list_items)
            .block(
                Block::default()
                    .title(
                        Span::styled(
                            "Logs",
                            Style::default()
                                .add_modifier(Modifier::ITALIC)
                        )
                    )
                    .borders(Borders::ALL)
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
        
        let cursor_end_position = terminal.backend_mut()
            .get_cursor()
            .into_diagnostic()?;
        self.terminal_end_cursor_position = Some(cursor_end_position);
        
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
                        break;
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
                                TUITerminalBackend::perform_render(state, f, None)
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
            
            // One last draw call before exiting.
            // IMPORTANT: In this last render we manually decrease the height of the UI by 1, so
            // after the program exits and a normal terminal prompt is shown, the entire UI is still in view.
            {
                let mut terminal = terminal_render_thread_clone.lock().unwrap();
                let state = state_render_thread_clone.lock().unwrap();
                
                terminal
                    .draw(
                        |f|
                            TUITerminalBackend::perform_render(state, f, Some(-1))
                    )
                    .into_diagnostic()?;
            }
            
            Ok(())
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
        
        // The program will exit soon - make sure the next prompt doesn't start somewhere in the
        // middle of the screen, where the UI was - reset cursor and print a newline to make it look
        // like a sane and normal terminal application.
        let mut terminal = self.terminal.lock().unwrap();
        
        let original_cursor_position = self.terminal_end_cursor_position
            .expect("has_been_set_up is true, but no original cursor position?!");
        
        terminal.backend_mut()
            .set_cursor(original_cursor_position.0, original_cursor_position.1)
            .into_diagnostic()?;
        
        // No need for additional newline, as our last render pass lowers the height by 1 so
        // the entire UI can fit on screen in addition to the new console prompt
        // (see last render in `setup`'s rendering thread).
        
        // terminal.backend_mut()
        //     .execute(Print("\n"))
        //     .into_diagnostic()?;
        
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
        state.queue_state = Some(QueueState::default());
    }
    
    fn queue_end(&mut self) {
        let mut state = self.lock_state();
        state.queue_state = None;
    }
    
    fn queue_item_add<T: Display>(
        &mut self,
        item: T,
        item_type: QueueType,
    ) -> Result<QueueItemID> {
        let mut state = self.lock_state();
        
        let queue = state.queue_state
            .as_mut()
            .ok_or_else(|| miette!("Queue is currently disabled, can't add item."))?;
        
        let queue_item = QueueItem::new(item.to_string(), item_type);
        let queue_item_id = queue_item.id;
        
        match item_type {
            QueueType::Library => queue.library_items.push(queue_item),
            QueueType::Album => queue.album_items.push(queue_item),
            QueueType::File => queue.file_items.push(queue_item),
        }
        
        Ok(queue_item_id)
    }
    
    fn queue_item_start(&mut self, item_id: QueueItemID) -> Result<()> {
        let mut state = self.lock_state();
        
        let queue = state.queue_state
            .as_mut()
            .ok_or_else(|| miette!("Queue is currently disabled, can't set item as active."))?;
        
        let target_item = queue.find_item_by_id(item_id);
        
        if let Some(item) = target_item {
            item.is_active = true;
            
            Ok(())
        } else {
            Err(miette!("No such queue item."))
        }
    }
    
    fn queue_item_finish(&mut self, item_id: QueueItemID, was_ok: bool) -> Result<()> {
        let mut state = self.lock_state();
        
        let queue = state.queue_state
            .as_mut()
            .ok_or_else(|| miette!("Queue is currently disabled, can't set item as active."))?;
        
        let target_item = queue.find_item_by_id(item_id);
        
        if let Some(item) = target_item {
            item.is_active = false;
            item.set_finished_state(QueueItemFinishedState {
                is_ok: was_ok,
            });
            
            Ok(())
        } else {
            Err(miette!("No such queue item."))
        }
    }
    
    fn queue_item_modify<F: FnOnce(&mut QueueItem)>(
        &mut self,
        item_id: QueueItemID,
        function: F,
    ) -> Result<()> {
        let mut state = self.lock_state();
    
        let queue = state.queue_state
            .as_mut()
            .ok_or_else(|| miette!("Queue is currently disabled, can't set item as active."))?;
    
        let queue_item = queue.find_item_by_id(item_id)
            .ok_or_else(|| miette!("No such queue item."))?;
        
        function(queue_item);
        Ok(())
    }
    
    fn queue_item_remove(&mut self, item_id: QueueItemID) -> Result<()> {
        let mut state = self.lock_state();
        
        let queue = state.queue_state
            .as_mut()
            .ok_or_else(|| miette!("Queue is currently disabled, can't set item as active."))?;
        
        queue.remove_item_by_id(item_id)
    }
    
    fn queue_clear(&mut self, queue_type: QueueType) -> Result<()> {
        let mut state = self.lock_state();
        
        if let Some(queue) = &mut state.queue_state {
            queue.clear_queue_by_type(queue_type);
            Ok(())
        } else {
            Err(miette!("Queue is currently disabled, can't clear."))
        }
    }
    
    fn progress_begin(&mut self) {
        let mut state = self.lock_state();
        state.progress = Some(ProgressState::default());
    }
    
    fn progress_end(&mut self) {
        let mut state = self.lock_state();
        state.progress = None;
    }
    
    fn progress_set_total(&mut self, total: usize) -> Result<()> {
        let mut state = self.lock_state();
        
        let mut progress = state.progress
            .as_mut()
            .ok_or_else(|| miette!("Progress bar is currently disabled, can't set total."))?;
        
        progress.total = total;
        Ok(())
    }
    
    fn progress_set_current(&mut self, current: usize) -> Result<()> {
        let mut state = self.lock_state();
        
        let mut progress = state.progress
            .as_mut()
            .ok_or_else(|| miette!("Progress bar is currently disabled, can't set current."))?;
        
        progress.current = current;
        Ok(())
    }
}
