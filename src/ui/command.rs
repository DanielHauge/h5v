use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders},
    Frame,
};

use crate::{color_consts, error::AppError};

use super::state::AppState;

fn popup_area(area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(3)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(60)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub fn render_command_dialog(f: &mut Frame, state: &mut AppState) -> Result<(), AppError> {
    let popup_area = popup_area(f.area());

    let popup_block = Block::default()
        .title("Command")
        .title_style(Style::default().fg(Color::Yellow))
        .borders(Borders::ALL)
        .style(Style::default().bg(color_consts::FOCUS_BG_COLOR))
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(Color::Green));

    let command_text = format!(">: {}", state.command_state.command);
    // pad command_test with spaces to fill the popup area
    let command_text = format!(
        "{:width$}",
        command_text,
        width = popup_area.width as usize - 2
    );
    let command_text_widget = ratatui::widgets::Paragraph::new(command_text)
        .block(popup_block)
        .style(Style::default().fg(Color::White));

    f.render_widget(command_text_widget, popup_area);

    Ok(())
}
