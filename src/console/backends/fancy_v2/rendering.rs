use std::io::Stdout;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ansi_to_tui::IntoText;
use crossterm::event::{Event, KeyCode};
use miette::Result;
use miette::{miette, IntoDiagnostic, WrapErr};
use parking_lot::{Mutex, RwLock};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block,
    BorderType,
    Borders,
    Clear,
    LineGauge,
    Padding,
    Paragraph,
};
use ratatui::{Frame, Terminal};
use tokio::sync::broadcast;

use crate::cancellation::CancellationToken;
use crate::configuration::TranscodingUIConfig;
use crate::console::backends::fancy_v2::queue_display::generate_smart_collapsible_queue;
use crate::console::backends::fancy_v2::state::{LogState, UIPage, UIState};
use crate::console::colours::{
    X061_SLATE_BLUE3,
    X064_CHARTREUSE4,
    X104_MEDIUM_PURPLE,
    X136_DARK_GOLDENROD,
    X160_RED3,
    X172_ORANGE3,
    X173_LIGHT_SALMON3,
    X189_LIGHT_STEEL_BLUE1,
    X216_LIGHT_SALMON1,
    X242_GREY42,
    X244_GREY50,
    X245_GREY54,
};
use crate::console::UserControlMessage;

const EUPHONY_VERSION: &str = env!("CARGO_PKG_VERSION");

const MUTED_BORDER_STYLE: Style = X242_GREY42;
const MUTED_TEXT_STYLE: Style = MUTED_BORDER_STYLE;

const TRANSCODING_TAB_BORDER_STYLE: Style = X173_LIGHT_SALMON3;
const TRANSCODING_TAB_TITLE_STYLE: Style = X216_LIGHT_SALMON1;

const LOGS_TAB_BORDER_STYLE: Style = X104_MEDIUM_PURPLE;
const LOGS_TAB_TITLE_STYLE: Style = X189_LIGHT_STEEL_BLUE1;

const LOGS_TAB_LOG_TIME_STYLE: Style = X244_GREY50;

const HEADER_TRANSCODING_TAB_TEXT_STYLE: Style = TRANSCODING_TAB_TITLE_STYLE;
const HEADER_LOGS_TAB_TEXT_STYLE: Style = LOGS_TAB_TITLE_STYLE;

const PROGRESS_BAR_BLOCK_BORDER_STYLE: Style = X136_DARK_GOLDENROD;
const PROGRESS_BAR_BLOCK_TITLE_STYLE: Style = X172_ORANGE3;
const PROGRESS_GAUGE_STYLE: Style = X172_ORANGE3;



const PROGRESS_DESCRIPTION_PENDING_FILES_VALUES_STYLE: Style = X245_GREY54;
const PROGRESS_DESCRIPTION_PROCESSING_FILES_VALUES_STYLE: Style =
    X061_SLATE_BLUE3;
const PROGRESS_DESCRIPTION_OK_FILES_VALUES_STYLE: Style = X064_CHARTREUSE4;
const PROGRESS_DESCRIPTION_ERRORED_FILES_VALUES_STYLE: Style = X160_RED3;



fn render_header(
    terminal_frame: &mut Frame<CrosstermBackend<Stdout>>,
    header_rect: Rect,
    ui_state: &UIState,
) {
    let header_constraints =
        vec![Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)];

    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(header_constraints)
        .split(header_rect);


    // Tab selection section
    let tab_selection_block = Block::default()
        .title(Span::styled(" View ", MUTED_TEXT_STYLE))
        .title_alignment(Alignment::Left)
        .padding(Padding::horizontal(1))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(MUTED_BORDER_STYLE);

    let mut line_contents: Vec<Span> = Vec::new();

    {
        let queues_disabled =
            ui_state.album_queue.is_none() || ui_state.file_queue.is_none();

        let mut text_style = HEADER_TRANSCODING_TAB_TEXT_STYLE;
        if ui_state.current_page == UIPage::Transcoding {
            text_style = text_style.add_modifier(Modifier::BOLD);
        }
        if queues_disabled {
            text_style = text_style.add_modifier(Modifier::CROSSED_OUT);
        }

        line_contents.push(Span::styled(
            if ui_state.current_page == UIPage::Transcoding {
                "TRANSCODING <T>"
            } else {
                "transcoding <t>"
            },
            text_style,
        ));
    };

    line_contents.push(Span::styled(" | ", MUTED_TEXT_STYLE));

    {
        let mut text_style = HEADER_LOGS_TAB_TEXT_STYLE;
        if ui_state.current_page == UIPage::Logs {
            text_style = text_style.add_modifier(Modifier::BOLD);
        };

        line_contents.push(Span::styled(
            if ui_state.current_page == UIPage::Logs {
                "LOGS <L>"
            } else {
                "logs <l>"
            },
            text_style,
        ));
    };

    let tab_selection_paragraph = Paragraph::new(Line::from(line_contents))
        .block(tab_selection_block)
        .alignment(Alignment::Left);

    terminal_frame.render_widget(tab_selection_paragraph, header_layout[0]);


    // Help section
    let help_block = Block::default()
        .title(Span::styled(
            format!(" Help (euphony {EUPHONY_VERSION}) "),
            MUTED_TEXT_STYLE,
        ))
        .title_alignment(Alignment::Left)
        .padding(Padding::horizontal(1))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(MUTED_BORDER_STYLE);

    let help_paragraph = Paragraph::new(Line::from(vec![
        Span::styled(
            "quit",
            MUTED_TEXT_STYLE.add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            "<q>",
            MUTED_TEXT_STYLE.add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(help_block)
    .alignment(Alignment::Left);

    terminal_frame.render_widget(help_paragraph, header_layout[1]);
}


fn render_logs_tab(
    terminal_frame: &mut Frame<CrosstermBackend<Stdout>>,
    body_rect: Rect,
    log_state: &LogState,
) -> Result<()> {
    let logs_block = Block::default()
        .title(Span::styled(" Logs ", LOGS_TAB_TITLE_STYLE))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(LOGS_TAB_BORDER_STYLE);
    let logs_inner_rect = logs_block.inner(body_rect);

    let max_line_width = logs_inner_rect.width as usize;
    let max_lines = logs_inner_rect.height as usize;

    let mut log_lines: Vec<Line> = Vec::with_capacity(max_lines);

    let mut log_iterator = log_state.log_journal.iter_most_recent_first();
    while log_lines.len() < max_lines {
        let Some((log_content, log_time)) = log_iterator.next() else {
            break;
        };

        let log_content_length = log_content.len();

        let formatted_log_time = format!("{} ", log_time.format("%H:%M:%S"));
        let formatted_log_time_length = formatted_log_time.len();
        let formatted_log_time =
            Span::styled(formatted_log_time, LOGS_TAB_LOG_TIME_STYLE);


        if (formatted_log_time_length + log_content_length) <= max_line_width {
            // Log entry fits in one line, no need to wrap.
            let log_entry_as_text =
                log_content.as_bytes().into_text().into_diagnostic()?;
            let log_entry_as_first_line = log_entry_as_text
                .lines
                .get(0)
                .ok_or_else(|| miette!("BUG: No text generated."))?
                .clone();

            let mut formatted_line_spans: Vec<Span> = Vec::with_capacity(2);
            formatted_line_spans.push(formatted_log_time);
            formatted_line_spans.extend(log_entry_as_first_line.spans);

            log_lines.insert(0, Line::from(formatted_line_spans));
        } else {
            let wrapped_text = textwrap::wrap(
                log_content.as_str(),
                textwrap::Options::new(
                    max_line_width - formatted_log_time_length,
                )
                .break_words(false),
            );

            assert!(wrapped_text.len() >= 2);

            if wrapped_text.len() + log_lines.len() > max_lines {
                break;
            }

            // Add first line.
            let first_line =
                wrapped_text[0].as_bytes().into_text().into_diagnostic()?;

            let mut first_formatted_line_spans: Vec<Span> =
                Vec::with_capacity(2);
            first_formatted_line_spans.push(formatted_log_time);
            first_formatted_line_spans.extend(first_line.lines[0].spans.clone());

            log_lines.insert(0, Line::from(first_formatted_line_spans));


            // Add the rest of the lines.
            for (index, line) in wrapped_text.iter().enumerate().skip(1) {
                let styled_line =
                    line.as_bytes().into_text().into_diagnostic()?;

                let mut next_formatted_line_spans: Vec<Span> =
                    Vec::with_capacity(2);
                next_formatted_line_spans
                    .push(Span::raw(" ".repeat(formatted_log_time_length)));
                next_formatted_line_spans
                    .extend(styled_line.lines[0].spans.clone());

                log_lines.insert(index, Line::from(next_formatted_line_spans));
            }
        }
    }

    // Fill any potential remaining space at the top with empty lines.
    let num_empty_lines_needed = max_lines - log_lines.len();
    for _ in 0..num_empty_lines_needed {
        log_lines.insert(0, Line::default());
    }

    let logs_paragraph = Paragraph::new(log_lines);

    terminal_frame.render_widget(logs_block, body_rect);
    terminal_frame.render_widget(logs_paragraph, logs_inner_rect);

    Ok(())
}


fn render_transcoding_tab(
    terminal_frame: &mut Frame<CrosstermBackend<Stdout>>,
    body_rect: Rect,
    ui_state: &UIState,
) {
    if ui_state.album_queue.is_none() || ui_state.file_queue.is_none() {
        // This if statement shouldn't ever trigger, but if it does for some reason, we
        // should clear the rectangle that is reserved for it as we shouldn't display anything here.
        terminal_frame.render_widget(Clear, body_rect);
        return;
    }

    let album_queue = ui_state
        .album_queue
        .as_ref()
        .expect("BUG: Just checked that album queue is Some?!");
    let file_queue = ui_state
        .file_queue
        .as_ref()
        .expect("BUG: Just checked that album queue is Some?!");


    let transcoding_tab_constraints =
        vec![Constraint::Ratio(2, 5), Constraint::Ratio(3, 5)];

    let transcoding_tab_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(transcoding_tab_constraints)
        .split(body_rect);


    // Album queue
    let albums_queue_block = Block::default()
        .title(Span::styled(
            " Album list ",
            TRANSCODING_TAB_TITLE_STYLE,
        ))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(TRANSCODING_TAB_BORDER_STYLE);

    let albums_queue_inner_rect =
        albums_queue_block.inner(transcoding_tab_layout[0]);

    let albums_queue_list = generate_smart_collapsible_queue(
        album_queue,
        albums_queue_inner_rect.height as usize,
        albums_queue_inner_rect.width as usize,
    )
    .block(albums_queue_block);

    terminal_frame.render_widget(albums_queue_list, transcoding_tab_layout[0]);

    // File queue
    let files_queue_block = Block::default()
        .title(Span::styled(
            " Current album ",
            TRANSCODING_TAB_TITLE_STYLE,
        ))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(TRANSCODING_TAB_BORDER_STYLE);

    let files_queue_inner_rect =
        files_queue_block.inner(transcoding_tab_layout[1]);

    let files_queue_list = generate_smart_collapsible_queue(
        file_queue,
        files_queue_inner_rect.height as usize,
        files_queue_inner_rect.width as usize,
    )
    .block(files_queue_block);

    terminal_frame.render_widget(files_queue_list, transcoding_tab_layout[1]);
}


fn render_progress_footer(
    terminal_frame: &mut Frame<CrosstermBackend<Stdout>>,
    footer_rect: Rect,
    ui_state: &UIState,
) {
    if ui_state.progress.is_none() {
        // This if statement shouldn't ever trigger, but if it does for some reason, we
        // should clear the rectangle that is reserved for it as we shouldn't display anything here.
        terminal_frame.render_widget(Clear, footer_rect);
        return;
    }

    let progress = ui_state.progress.expect("BUG: progress shouldn't be None.");

    let footer_block = Block::default()
        .title(Span::styled(
            format!(
                " Overall file progress ({:.1}%) ",
                progress.completion_ratio() * 100f64
            ),
            PROGRESS_BAR_BLOCK_TITLE_STYLE,
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(PROGRESS_BAR_BLOCK_BORDER_STYLE);
    let footer_inner_rect = footer_block.inner(footer_rect);

    terminal_frame.render_widget(footer_block, footer_rect);


    let footer_constraints =
        vec![Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)];

    let footer_inner_layout = Layout::default()
        .constraints(footer_constraints)
        .split(footer_inner_rect);


    // Progress bar line
    let progress_bar_gauge = LineGauge::default()
        .gauge_style(PROGRESS_GAUGE_STYLE)
        .line_set(ratatui::symbols::line::THICK)
        .ratio(progress.completion_ratio())
        .label(Span::raw(""));

    terminal_frame.render_widget(progress_bar_gauge, footer_inner_layout[0]);

    // General PENDING/IN PROGRESS/FINISHED/FAILED status line
    let general_ok_err_status_line = Paragraph::new(Line::from(vec![
        Span::styled("PENDING: ", MUTED_TEXT_STYLE),
        Span::styled(
            format!("{} files", progress.total_pending()),
            PROGRESS_DESCRIPTION_PENDING_FILES_VALUES_STYLE,
        ),
        Span::styled(" => ", MUTED_TEXT_STYLE),
        Span::styled("IN PROGRESS: ", MUTED_TEXT_STYLE),
        Span::styled(
            format!(
                "{} audio, {} data",
                progress.audio_files_currently_processing,
                progress.data_files_currently_processing
            ),
            PROGRESS_DESCRIPTION_PROCESSING_FILES_VALUES_STYLE,
        ),
        Span::styled(" => ", MUTED_TEXT_STYLE),
        Span::styled("FINISHED: ", MUTED_TEXT_STYLE),
        Span::styled(
            format!(
                "{} audio, {} data",
                progress.audio_files_finished_ok,
                progress.data_files_finished_ok
            ),
            if progress.audio_files_finished_ok > 0
                || progress.data_files_finished_ok > 0
            {
                PROGRESS_DESCRIPTION_OK_FILES_VALUES_STYLE
            } else {
                MUTED_TEXT_STYLE
            },
        ),
        Span::styled(" / FAILED: ", MUTED_TEXT_STYLE),
        Span::styled(
            format!(
                "{} audio, {} data",
                progress.audio_files_errored, progress.data_files_errored
            ),
            if progress.audio_files_errored > 0
                || progress.data_files_errored > 0
            {
                PROGRESS_DESCRIPTION_ERRORED_FILES_VALUES_STYLE
            } else {
                MUTED_TEXT_STYLE
            },
        ),
    ]))
    .alignment(Alignment::Center);

    terminal_frame
        .render_widget(general_ok_err_status_line, footer_inner_layout[1]);
}


fn render_ui(
    log_state: &LogState,
    ui_state: &UIState,
    terminal_frame: &mut Frame<CrosstermBackend<Stdout>>,
    is_final_render: bool,
) -> Result<()> {
    // # Interface layout (approximately)
    //
    // ## Log page:
    // /------- euphony 2.0.0 ----------- help ---------------------------\
    // | transcoding <t> | LOGS <L>     | quit <q>                        |
    // |------------------------------------------------------------------|
    // |- Logs -----------------------------------------------------------|
    // | lorem ipsum dolor sir amet                                       |
    // | ... (logs go downwards)                                          |
    // |                                                                  |
    // |                                                                  |
    // |                                                                  |
    // |                                                                  |
    // |                                                                  |
    // |                                                                  |
    // |                                                                  |
    // |                                                                  |
    // |                                                                  |
    // |- Transcoding ----------------------------------------------------|
    // | █ █ █ █ █ █ █ █ █ █ █  (22 / 91 files) 56%                       |
    // |   OK files (audio/data): 19/1 | FAILED files (audio/data): 1/1   |
    // |------------------------------------------------------------------|
    //
    //
    // Transcoding page:
    // /------- euphony 2.0.0 --------------- help ---------------------------\
    // | transcoding <T> | logs <l>         | quit transcoding <q>            |
    // |----------------------------------------------------------------------|
    // |- Albums ------------------- Files -----------------------------------|
    // | ◰ Peppsen - RimWorld      |                                          |
    // |   20 audio / 1 data file  |                                          |
    // |   Wintergatan - Tornado   |                                          |
    // |   1 audio / 1 data file   |                                          |
    // |   ...                     |                                          |
    // |                           |                                          |
    // |                           |                                          |
    // |                           |                                          |
    // |                           |                                          |
    // |                           |                                          |
    // |                           |                                          |
    // |                           |                                          |
    // |                           |                                          |
    // |- Transcoding --------------------------------------------------------|
    // | █ █ █ █ █ █ █ █ █ █ █    (22 / 91 files) 56%                         |
    // |     OK files (audio/data): 19/1 | FAILED files (audio/data): 1/1     |
    // |----------------------------------------------------------------------|
    //

    let frame_size = {
        let mut size = terminal_frame.size();
        if is_final_render {
            size.height -= 1;
        }

        size
    };

    let main_constraints = vec![
        // Header (contains left and right subheader)
        Constraint::Length(3),
        // Body of the app (either transcoding queue or log view)
        Constraint::Min(5),
        // Footer containing the progress bar and additional info.
        if ui_state.progress.is_some() {
            Constraint::Length(4)
        } else {
            Constraint::Length(0)
        },
    ];

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(main_constraints)
        .split(frame_size);

    render_header(terminal_frame, main_layout[0], ui_state);

    // If any of the queues are disabled, always render the log view instead.
    if ui_state.file_queue.is_none() || ui_state.album_queue.is_none() {
        render_logs_tab(terminal_frame, main_layout[1], log_state)?;
    } else {
        match ui_state.current_page {
            UIPage::Transcoding => {
                render_transcoding_tab(terminal_frame, main_layout[1], ui_state);
            }
            UIPage::Logs => {
                render_logs_tab(terminal_frame, main_layout[1], log_state)?;
            }
        };
    }

    // Prevents the function from being called when the progress bar is disabled
    // (the Rect will have 0 height anyway).
    if ui_state.progress.is_some() {
        render_progress_footer(terminal_frame, main_layout[2], ui_state);
    }

    Ok(())
}

const TERMINAL_REFRESH_INTERVAL_IN_SECONDS: f64 = 1f64 / 30f64;

pub fn run_render_loop(
    terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    transcoding_ui_config: TranscodingUIConfig,
    log_state: Arc<Mutex<LogState>>,
    ui_state: Arc<RwLock<UIState>>,
    user_control_sender: &broadcast::Sender<UserControlMessage>,
    cancellation_token: CancellationToken,
) -> Result<()> {
    // Continuously render the terminal UI.
    // Stop when the cancellation token is set.

    loop {
        let render_time_start = Instant::now();

        if cancellation_token.is_cancelled() {
            // Main thread signalled us to stop, simply exit by returning early.
            break;
        }

        // Perform one draw.
        {
            let mut locked_terminal = terminal.lock();
            let locked_ui_state = ui_state.read();
            let locked_log_state = log_state.lock();

            locked_terminal
                .draw(|frame| {
                    render_ui(&locked_log_state, &locked_ui_state, frame, false)
                        .expect("Failed to render terminal UI.")
                })
                .into_diagnostic()
                .wrap_err_with(|| miette!("Failed to render terminal UI."))?;
        }


        // Effectively sleep the rest of the iteration (while checking for user input).
        loop {
            if cancellation_token.is_cancelled() {
                // Main thread signalled us to stop, simply exit by returning early.
                break;
            }

            let since_last_render_start =
                render_time_start.elapsed().as_secs_f64();
            if since_last_render_start >= TERMINAL_REFRESH_INTERVAL_IN_SECONDS {
                break;
            }

            let time_to_poll_for =
                TERMINAL_REFRESH_INTERVAL_IN_SECONDS - since_last_render_start;

            // If less than 0.1 ms away from next render, don't bother with keybind polling.
            if time_to_poll_for < 0.0001 {
                continue;
            }

            // We still have time left until the next render, so let's poll for keyboard inputs.
            if crossterm::event::poll(Duration::from_secs_f64(time_to_poll_for))
                .into_diagnostic()
                .wrap_err_with(|| miette!("Failed to poll keyboard events."))?
            {
                if let Event::Key(key) =
                    crossterm::event::read().into_diagnostic().wrap_err_with(
                        || miette!("Failed to read keyboard event."),
                    )?
                {
                    if let KeyCode::Char(char) = key.code {
                        if char == 'q' {
                            if transcoding_ui_config.show_logs_tab_on_exit {
                                let mut locked_ui_state = ui_state.write();
                                locked_ui_state.current_page = UIPage::Logs;
                            }

                            let _ = user_control_sender
                                .send(UserControlMessage::Exit);
                        } else if char == 't' {
                            let mut locked_ui_state = ui_state.write();
                            if locked_ui_state.album_queue.is_some()
                                && locked_ui_state.file_queue.is_some()
                            {
                                locked_ui_state.current_page =
                                    UIPage::Transcoding;
                            }
                        } else if char == 'l' {
                            let mut locked_ui_state = ui_state.write();
                            locked_ui_state.current_page = UIPage::Logs;
                        }
                    }
                }
            }
        }
    }

    // Perform last render pass.
    // In this one, we manually decrease the viewport height by one, so there will be no
    // jarring UI movement when the app exits.
    {
        let mut locked_terminal = terminal.lock();
        let locked_ui_state = ui_state.read();
        let locked_log_state = log_state.lock();

        locked_terminal
            .draw(|frame| {
                render_ui(&locked_log_state, &locked_ui_state, frame, true)
                    .expect("Failed to render terminal UI.")
            })
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!("Failed to render finale frame of terminal UI.")
            })?;
    }

    Ok(())
}
