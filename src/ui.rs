use std::{io::stdout, io::Result, rc::Rc};

use ratatui::{
    crossterm::{
        event::{self, KeyCode, KeyEventKind},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::{
    h5f::H5F,
    ui_tree_view::{compute_tree_view, TreeItem},
};
fn make_panels_rect(area: Rect) -> Rc<[Rect]> {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(area);
    chunks
}

pub fn init(h5f: &mut H5F) -> Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let mut help = false;
    let mut treeview = compute_tree_view(&h5f.root);

    loop {
        terminal.draw(|frame| {
            if !help {
                let areas = make_panels_rect(frame.area());
                let [tree, info] = areas.as_ref() else {
                    panic!("Could not get the areas for the panels");
                };
                render_tree(frame, tree, &mut treeview);
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
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

fn render_tree(f: &mut Frame, area: &Rect, treeview: &mut Vec<TreeItem>) {
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(format!("Tree"))
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(header_block, *area);

    let inner_area = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });

    let mut area = inner_area;

    for tree_item in treeview.iter() {
        let area_one_down = area.inner(Margin {
            horizontal: 0,
            vertical: 1,
        });
        let text = tree_item.text.clone();
        let p = Paragraph::new(text).wrap(Wrap { trim: true });
        f.render_widget(p, area_one_down);
        area = area_one_down;
    }
    // let p = Paragraph::new(texts)
    //     .block(Block::default().borders(Borders::NONE))
    //     .wrap(Wrap { trim: true });
    // f.render_widget(p, inner_area);
}

fn render_info(f: &mut Frame, area: &Rect) {
    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(format!("Info"))
        .title_style(Style::default().fg(Color::Yellow).bold())
        .title_alignment(Alignment::Center);
    f.render_widget(header_block, *area);
}
