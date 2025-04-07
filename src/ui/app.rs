use std::{
    cell::RefCell,
    io::{stdout, Error},
    rc::Rc,
};

use ratatui::{
    crossterm::{
        event::{self},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    layout::{Alignment, Constraint, Layout, Margin, Offset, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::{
    color_consts,
    h5f::{H5FNode, H5F},
    ui::attributes::render_info_attributes,
    ui::input::{tree::handle_normal_tree_event, EventResult},
    ui::tree_view::TreeItem,
};

use super::input::handle_input_event;

fn make_panels_rect(area: Rect) -> Rc<[Rect]> {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(area);
    chunks
}

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Hdf5(hdf5_metno::Error),
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<hdf5_metno::Error> for AppError {
    fn from(err: hdf5_metno::Error) -> Self {
        AppError::Hdf5(err)
    }
}

type Result<T> = std::result::Result<T, AppError>;

pub enum Focus {
    Tree,
    Attributes,
}

pub enum Mode {
    Normal,
    Search,
    Help,
}

pub struct AppState<'a> {
    pub root: Rc<RefCell<H5FNode>>,
    pub treeview: Vec<TreeItem<'a>>,
    pub tree_view_cursor: usize,
    pub help: bool,
    pub focus: Focus,
    pub mode: Mode,
}

pub fn init(file: H5F) -> Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let state = Rc::new(RefCell::new(AppState {
        root: file.root.clone(),
        treeview: vec![],
        tree_view_cursor: 0,
        help: false,
        focus: Focus::Tree,
        mode: Mode::Normal,
    }));
    state.borrow_mut().compute_tree_view();

    let draw_closure = |frame: &mut Frame| {
        if !state.borrow().help {
            let areas = make_panels_rect(frame.area());
            let [tree, info] = areas.as_ref() else {
                panic!("Could not get the areas for the panels");
            };
            render_tree(frame, tree, state.clone());
            let selected_node = &state.borrow().treeview[state.borrow().tree_view_cursor].node;
            match render_info(frame, info, selected_node) {
                Ok(_) => {}
                Err(e) => {
                    let error_text = Text::from(format!("Error: {}", e));
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
                    frame.render_widget(error_paragraph, *info);
                }
            }
        } else {
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
    };

    // First time draw
    terminal.draw(draw_closure)?;

    loop {
        // Interaction to modify state -> Move to eventual ux module
        if event::poll(std::time::Duration::from_millis(16))? {
            let event = event::read()?;
            match handle_input_event(&state, event)? {
                EventResult::Quit => break,
                EventResult::Continue => {}
                EventResult::Redraw => {
                    terminal.draw(draw_closure)?;
                }
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

fn render_app(f: &mut Frame, area: &Rect, state: Rc<RefCell<AppState>>) {
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(format!("App"))
        .bg(color_consts::BG_COLOR)
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(header_block, *area);

    let inner_area = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let mut area = inner_area;
    let state = state.borrow_mut();
    let mut tree_view_skip_offset = 0;
    let mut highlight_index = state.tree_view_cursor;
    if area.height <= state.tree_view_cursor as u16 {
        let half = area.height / 2;
        tree_view_skip_offset = state.tree_view_cursor as u16 - half;
        highlight_index = half as usize;
    }

    let treeview = &state.treeview;

    for (i, tree_item) in treeview
        .iter()
        .skip(tree_view_skip_offset as usize)
        .enumerate()
    {
        if i >= area.height as usize {
            break;
        }
        let text = tree_item.line.clone();
        if highlight_index == i {
            f.render_widget(text.bg(color_consts::HIGHLIGHT_BG_COLOR), area);
        } else {
            f.render_widget(text, area);
        }
        area = area.offset(Offset { x: 0, y: 1 });
    }
}

fn render_tree(f: &mut Frame, area: &Rect, state: Rc<RefCell<AppState>>) {
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(format!("Tree"))
        .bg(color_consts::BG_COLOR)
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(header_block, *area);

    let inner_area = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let mut area = inner_area;
    let state = state.borrow_mut();
    match state.mode {
        Mode::Normal => {
            let mut tree_view_skip_offset = 0;
            let mut highlight_index = state.tree_view_cursor;
            if area.height <= state.tree_view_cursor as u16 {
                let half = area.height / 2;
                tree_view_skip_offset = state.tree_view_cursor as u16 - half;
                highlight_index = half as usize;
            }

            let treeview = &state.treeview;

            for (i, tree_item) in treeview
                .iter()
                .skip(tree_view_skip_offset as usize)
                .enumerate()
            {
                if i >= area.height as usize {
                    break;
                }
                let text = tree_item.line.clone();
                if highlight_index == i {
                    f.render_widget(text.bg(color_consts::HIGHLIGHT_BG_COLOR), area);
                } else {
                    f.render_widget(text, area);
                }
                area = area.offset(Offset { x: 0, y: 1 });
            }
        }
        Mode::Search => {}
        Mode::Help => unreachable!(),
    }
}

fn render_info(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
) -> std::result::Result<(), hdf5_metno::Error> {
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(format!("Info"))
        .bg(color_consts::BG2_COLOR)
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(header_block, *area);
    render_info_attributes(f, area, selected_node)?;
    Ok(())
}
