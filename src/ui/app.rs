use std::{
    cell::RefCell,
    io::{stdout, BufReader},
    rc::Rc,
    sync::mpsc::{channel, Sender},
    thread,
};

use hdf5_metno::{ByteReader, Dataset};
use image::ImageFormat;
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
use ratatui_image::thread::{ResizeResponse, ThreadProtocol};

use crate::{
    error::AppError,
    h5f::{self, H5FNode, ImageType, Node},
    search::Searcher,
    ui::{input::EventResult, tree_view::TreeItem},
};

use super::{
    image_preview::{handle_image_load, handle_image_resize, handle_imagefs_load},
    input::handle_input_event,
    main_display::render_main_display,
    tree_view::render_tree,
};

fn make_panels_rect(area: Rect) -> Rc<[Rect]> {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(area);
    chunks
}

type Result<T> = std::result::Result<T, AppError>;

pub enum Focus {
    Tree,
    Attributes,
}

#[derive(Debug, Clone)]
pub enum Mode {
    Normal,
    Search,
    Help,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ContentShowMode {
    Preview,
    Matrix,
    Heatmap,
}

pub struct ImgState {
    pub protocol: Option<ThreadProtocol>,
    pub tx_load_imgfs: Sender<(BufReader<ByteReader>, ImageFormat)>,
    pub tx_load_img: Sender<(Dataset, ImageType)>,
    pub ds: Option<String>,
}

impl ImgState {
    pub fn is_from_ds(&self, node: &Node) -> bool {
        if self.ds.is_none() {
            return false;
        }
        match node {
            Node::Dataset(ds, _) => {
                let name = &ds.name();
                let ds_name_str = match &self.ds {
                    Some(ds_name) => ds_name.as_str(),
                    None => {
                        return false;
                    }
                };
                if *name == ds_name_str {
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

pub struct AppState<'a> {
    pub root: Rc<RefCell<H5FNode>>,
    pub treeview: Vec<TreeItem<'a>>,
    pub tree_view_cursor: usize,
    pub help: bool,
    pub focus: Focus,
    pub mode: Mode,
    pub indexed: bool,
    pub searcher: Rc<RefCell<Searcher>>,
    pub show_tree_view: bool,
    pub content_mode: ContentShowMode,
    pub selected_x_dim: usize,
    // pub selected_y_dim: usize,
    pub selected_indexes: [usize; 15], // WARN: Will we ever need more than 15 dimensions?
    pub img_state: ImgState,
}

impl AppState<'_> {
    pub fn index(&mut self) -> Result<()> {
        let mut root = self.root.borrow_mut();
        root.index(true)?;
        self.indexed = true;
        Ok(())
    }
}

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
                    last_message = Some(format!("IO Error: - {}", error));
                }
                AppError::Hdf5(error) => match error {
                    hdf5_metno::Error::HDF5(_) => {
                        last_message = Some("HDF5 Error".to_string());
                        break;
                    }
                    hdf5_metno::Error::Internal(e) => {
                        last_message = Some(format!("HDF5 Internal: - {}", e));
                        break;
                    }
                },
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
    let searcher = Rc::new(RefCell::new(Searcher::new()));

    let h5f = h5f::H5F::open(filename, searcher.clone()).map_err(|e| {
        AppError::Hdf5(hdf5_metno::Error::from(format!(
            "Failed to open HDF5 file: {}",
            e
        )))
    })?;

    let (tx_events, rx_events) = channel();
    let tx_events_2 = tx_events.clone();
    let tx_load_img = handle_image_resize(tx_events_2);
    let tx_load_imgfs = handle_imagefs_load(tx_events.clone(), tx_load_img.clone());
    let tx_load_img = handle_image_load(tx_events.clone(), tx_load_img.clone());

    let img_state = ImgState {
        protocol: None,
        tx_load_imgfs,
        tx_load_img,
        ds: None,
    };

    let mut state = AppState {
        root: h5f.root.clone(),
        treeview: vec![],
        tree_view_cursor: 0,
        help: false,
        focus: Focus::Tree,
        mode: Mode::Normal,
        indexed: false,
        searcher,
        show_tree_view: true,
        content_mode: ContentShowMode::Preview,
        selected_x_dim: 0,
        // selected_y_dim: 0,
        selected_indexes: [0; 15],
        img_state,
    };

    state.compute_tree_view();

    let draw_closure = |frame: &mut Frame, state: &mut AppState| {
        if state.help {
            return render_help(frame);
        }

        let show_tree_view = state.show_tree_view;

        let main_display_area = match show_tree_view {
            true => {
                let areas = make_panels_rect(frame.area());
                let (tree_area, main_display_area) = (areas[0], areas[1]);
                render_tree(frame, tree_area, state);
                main_display_area
            }
            false => frame.area(),
        };

        let selected_node = state.treeview[state.tree_view_cursor].node.clone();
        match render_main_display(frame, &main_display_area, &selected_node, state) {
            Ok(()) => {}
            Err(e) => render_error(frame, &format!("Error: {}", e)),
        }
    };

    // First time draw nice state
    terminal.draw(|f| draw_closure(f, &mut state))?;

    handle_term_events(tx_events);

    loop {
        let event = rx_events.recv().unwrap();
        match event {
            AppEvent::TermEvent(event) => match handle_input_event(&mut state, event)? {
                EventResult::Quit => break,
                EventResult::Continue => {}
                EventResult::Redraw => {
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            },
            AppEvent::ImageResized(resize_response) => {
                if let Some(ref mut img_thread_protocol) = state.img_state.protocol {
                    let _ = img_thread_protocol.update_resized_protocol(resize_response);
                    terminal.draw(|f| {
                        draw_closure(f, &mut state);
                    })?;
                }
            }
            AppEvent::ImageLoaded(thread_protocol) => {
                state.img_state.protocol = Some(thread_protocol);
                terminal.draw(|f| {
                    draw_closure(f, &mut state);
                })?;
            }
        }
    }
    Ok(IntendedMainLoopBreak {})
}

pub enum AppEvent {
    TermEvent(event::Event),
    ImageResized(ResizeResponse),
    ImageLoaded(ThreadProtocol),
}

fn handle_term_events(tx_events: Sender<AppEvent>) {
    thread::spawn(move || loop {
        if event::poll(std::time::Duration::from_millis(16)).is_ok() {
            if let Ok(event) = event::read() {
                tx_events.send(AppEvent::TermEvent(event)).unwrap();
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
