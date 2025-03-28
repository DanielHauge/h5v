use std::{
    cell::RefCell,
    io::{stdout, Result},
    rc::Rc,
};

use ratatui::{
    crossterm::{
        event::{self, KeyCode, KeyEventKind},
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
    h5f::H5F,
    ui_tree_view::{compute_tree_view, expand_full_tree, TreeItem},
};
fn make_panels_rect(area: Rect) -> Rc<[Rect]> {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(area);
    chunks
}

pub fn init(h5f: H5F) -> Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let mut help = false;
    let h5frcrc = Rc::new(RefCell::new(h5f.root));
    let mut treeview = compute_tree_view(&h5frcrc); // Vec<TreeItem>
    let mut cursor = 0;

    loop {
        terminal.draw(|frame| {
            if !help {
                let areas = make_panels_rect(frame.area());
                let [tree, info] = areas.as_ref() else {
                    panic!("Could not get the areas for the panels");
                };
                render_tree(frame, tree, &mut treeview, Some(cursor));
                render_info(frame, info);
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
        })?;

        // Interaction to modify state -> Move to eventual ux module
        if event::poll(std::time::Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                if let (KeyEventKind::Press, KeyCode::Char('q')) = (key.kind, key.code) {
                    break;
                }
                if let (KeyEventKind::Press, KeyCode::Char('?')) = (key.kind, key.code) {
                    help = !help;
                }
                if let (KeyEventKind::Press, KeyCode::Up) = (key.kind, key.code) {
                    if cursor > 0 {
                        cursor -= 1;
                    }
                }
                if let (KeyEventKind::Press, KeyCode::Char('j')) = (key.kind, key.code) {
                    if cursor < treeview.len() - 1 {
                        cursor += 1;
                    }
                }
                if let (KeyEventKind::Press, KeyCode::Down) = (key.kind, key.code) {
                    if cursor < treeview.len() - 1 {
                        cursor += 1;
                    }
                }
                if let (KeyEventKind::Press, KeyCode::Char('k')) = (key.kind, key.code) {
                    if cursor > 0 {
                        cursor -= 1;
                    }
                }
                if let (KeyEventKind::Press, KeyCode::Enter) = (key.kind, key.code) {
                    let tree_item = &mut treeview[cursor];
                    tree_item.node.borrow_mut().expand_toggle().unwrap();
                    treeview = compute_tree_view(&h5frcrc);
                }
                if let (KeyEventKind::Press, KeyCode::Char(' ')) = (key.kind, key.code) {
                    let tree_item = &mut treeview[cursor];
                    tree_item.node.borrow_mut().expand_toggle().unwrap();
                    treeview = compute_tree_view(&h5frcrc);
                }
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

fn render_tree(f: &mut Frame, area: &Rect, treeview: &mut Vec<TreeItem>, cursor: Option<usize>) {
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

    for (i, tree_item) in treeview.iter().enumerate() {
        let text = tree_item.line.clone();
        if cursor == Some(i) {
            f.render_widget(text.bg(color_consts::HIGHLIGHT_BG_COLOR), area);
        } else {
            f.render_widget(text, area);
        }
        area = area.offset(Offset { x: 0, y: 1 });
    }
}

fn render_info(f: &mut Frame, area: &Rect) {
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(format!("Info"))
        .bg(color_consts::BG2_COLOR)
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(header_block, *area);
}
