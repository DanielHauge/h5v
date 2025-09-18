use std::{
    cell::RefCell,
    io::stdout,
    rc::Rc,
    sync::mpsc::{channel, Sender},
    thread,
};

use cli_clipboard::{ClipboardContext, ClipboardProvider};
use ratatui::{
    crossterm::{
        event::{self},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    layout::{Alignment, Constraint, Layout, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::picker::Picker;

use crate::{
    error::AppError,
    h5f,
    search::Searcher,
    ui::{input::EventResult, mchart::MultiChartState},
};

use super::{
    command::{Command, CommandState},
    command_view::render_command_dialog,
    image_preview::{
        handle_image_load, handle_image_resize, handle_imagefs_load, handle_imagefsvlen_load,
        ImageLoadedResult, ImageResizeResult,
    },
    input::handle_input_event,
    main_display::render_main_display,
    state::{
        self, AppState, AttributeCursor, ContentShowMode, Focus, ImgState, LastFocused,
        MatrixViewState, Mode,
    },
    tree_view::render_tree,
};

fn make_panels_rect(area: Rect, mode: Mode) -> Rc<[Rect]> {
    if let Mode::Search = mode {
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(100), Constraint::Percentage(0)].as_ref())
            .split(area);
        chunks
    } else {
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
            .split(area);
        chunks
    }
}

type Result<T> = std::result::Result<T, AppError>;

pub struct IntendedMainLoopBreak {}

pub fn init(filename: String) -> Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let mut last_message = None;

    loop {
        match main_recover_loop(&mut terminal, filename.clone()) {
            Ok(_) => break,
            Err(e) => match e {
                AppError::Io(error) => {
                    last_message = Some(format!("IO Error: - {error}"));
                }
                AppError::Hdf5(error) => match error {
                    hdf5_metno::Error::HDF5(_) => {
                        last_message = Some("HDF5 Error".to_string());
                        break;
                    }
                    hdf5_metno::Error::Internal(e) => {
                        last_message = Some(format!("HDF5 Internal: - {e}"));
                        break;
                    }
                },
                AppError::ChannelError(c) => last_message = Some(format!("Channel Error: - {c}")),
                AppError::ClipboardError(msg) => {
                    last_message = Some(format!("Clipboard Error: - {msg}"));
                    break;
                }
                AppError::InvalidCommand(cmd) => {
                    last_message = Some(format!("Invalid Command: - {cmd}"));
                    break;
                }
            },
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    if let Some(message) = last_message {
        eprintln!("Unrecoverable AppError: {}", message);
    }
    Ok(())
}

fn main_recover_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    filename: String,
) -> Result<IntendedMainLoopBreak> {
    let h5f = h5f::H5F::open(filename).map_err(|e| {
        AppError::Hdf5(hdf5_metno::Error::from(format!(
            "Failed to open HDF5 file: {}",
            e
        )))
    })?;

    let (tx_events, rx_events) = channel();
    let picker = Picker::from_query_stdio().unwrap_or(Picker::from_fontsize((7, 14)));
    let tx_events_2 = tx_events.clone();
    let tx_load_img = handle_image_resize(tx_events_2);
    let tx_load_imgfs = handle_imagefs_load(tx_events.clone(), tx_load_img.clone(), picker.clone());
    let tx_load_imgfsvlen =
        handle_imagefsvlen_load(tx_events.clone(), tx_load_img.clone(), picker.clone());
    let tx_load_img = handle_image_load(tx_events.clone(), tx_load_img.clone(), picker.clone());

    let img_state = ImgState {
        protocol: None,
        tx_load_imgfs,
        tx_load_imgfsvlen,
        tx_load_img,
        ds: None,
        idx_to_load: 0,
        idx_loaded: -1,
        error: None,
        picker: picker.clone(),
    };

    let matrix_view_state = MatrixViewState {
        col_offset: 0,
        row_offset: 0,
        rows_currently_available: 0,
        cols_currently_available: 0,
    };
    let clipboard = ClipboardContext::new().expect("Failed to create clipboard context");

    let segment_state = state::SegmentState {
        idx: 0,
        segment_count: 0,
        segumented: state::SegmentType::NoSegment,
    };

    let command_state = CommandState {
        command_buffer: String::new(),
        last_command: Command::Noop,
        cursor: 0,
    };

    let mut state = AppState {
        root: h5f.root.clone(),
        multi_chart: MultiChartState::new(picker.clone()),
        segment_state,
        command_state,
        treeview: vec![],
        tree_view_cursor: 0,
        attributes_view_cursor: AttributeCursor {
            attribute_index: 0,
            attribute_view_selection: state::AttributeViewSelection::Name,
        },
        focus: Focus::Tree(LastFocused::Attributes),
        clipboard,
        mode: Mode::Normal,
        copying: false,
        indexed: false,
        searcher: None,
        show_tree_view: true,
        content_mode: ContentShowMode::Preview,
        img_state,
        matrix_view_state,
    };

    state.compute_tree_view();

    let draw_closure = |frame: &mut Frame, state: &mut AppState| {
        if let Mode::Help = state.mode {
            return render_help(frame);
        }
        if let Mode::MultiChart = state.mode {
            return state.multi_chart.render(frame);
        }

        let show_tree_view = state.show_tree_view;

        let main_display_area = match show_tree_view {
            true => {
                let areas = make_panels_rect(frame.area(), state.mode.clone());
                let (tree_area, main_display_area) = (areas[0], areas[1]);
                render_tree(frame, tree_area, state);
                main_display_area
            }
            false => frame.area(),
        };

        match state.mode {
            Mode::Search => {}
            Mode::Command => render_command_dialog(frame, state),
            Mode::Normal => {
                let selected_node = state.treeview[state.tree_view_cursor].node.clone();
                match render_main_display(frame, &main_display_area, &selected_node, state) {
                    Ok(()) => {}
                    Err(e) => render_error(frame, &format!("Error: {}", e)),
                }
            }
            Mode::Help => {}       // already handled above,
            Mode::MultiChart => {} // already handled above,
        }
    };

    // First time draw nice state
    terminal.draw(|f| draw_closure(f, &mut state))?;

    handle_term_events(tx_events);

    loop {
        let event = rx_events.recv().expect("Oh no, something horrible went wrong. Could no longer receive events from events channel.");
        match event {
            AppEvent::TermEvent(event) => match handle_input_event(&mut state, event)? {
                EventResult::Quit => break,
                EventResult::Continue => {}
                EventResult::Redraw => {
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
                EventResult::Copying => {
                    state.copying = true;
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                    // sleep for 50 ms
                    state.copying = false;
                    thread::sleep(std::time::Duration::from_millis(100));
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
                EventResult::Error(e) => {
                    terminal.draw(|f| {
                        render_error(f, &e);
                    })?;
                    thread::sleep(std::time::Duration::from_secs(2));
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            },
            AppEvent::ImageResized(resize_response) => match resize_response {
                ImageResizeResult::Success(resize_response) => {
                    if let Some(ref mut img_thread_protocol) = state.img_state.protocol {
                        let _ = img_thread_protocol.update_resized_protocol(resize_response);
                        terminal.draw(|f| {
                            draw_closure(f, &mut state);
                        })?;
                    }
                }
                ImageResizeResult::Error(e) => {
                    state.img_state.error = Some(format!("Error resizing image: {}", e));
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            },
            AppEvent::ImageLoad(img_load) => match img_load {
                ImageLoadedResult::Success(thread_protocol) => {
                    state.img_state.protocol = Some(thread_protocol);
                    state.img_state.error = None;
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
                ImageLoadedResult::Failure(f) => {
                    state.img_state.protocol = None;
                    state.img_state.error = Some(f);

                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            },
        }
    }
    Ok(IntendedMainLoopBreak {})
}

pub enum AppEvent {
    TermEvent(event::Event),
    ImageResized(ImageResizeResult),
    ImageLoad(ImageLoadedResult),
}

fn handle_term_events(tx_events: Sender<AppEvent>) {
    thread::spawn(move || loop {
        if event::poll(std::time::Duration::from_millis(16)).is_ok() {
            if let Ok(event) = event::read() {
                match tx_events.send(AppEvent::TermEvent(event)) {
                    Ok(_) => {}
                    Err(_e) => {} // TODO: log to a file maybe
                }
            }
        }
    });
}

fn render_error(frame: &mut Frame<'_>, error: &str) {
    let error_text = Text::from(error);
    let error_paragraph = Paragraph::new(error_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title("Error")
                .title_style(Style::default().fg(Color::Yellow).bold())
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(error_paragraph, frame.area());
}

fn render_help(frame: &mut Frame<'_>) {
    let help_text = Text::from("Press 'q' to quit");
    let help_paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::LightGreen))
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title("Help")
                .title_style(Style::default().fg(Color::Yellow).bold())
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(help_paragraph, frame.area());
}
