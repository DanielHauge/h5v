use hdf5_metno::Error;
use ratatui::{layout::Rect, style::Style, widgets::Paragraph, Frame};

use crate::color_consts;

use super::state::AppState;

pub fn render_dim_selector(
    f: &mut Frame,
    area: &Rect,
    state: &mut AppState,
    shape: &Vec<usize>,
) -> Result<(), Error> {
    let p = Paragraph::new("TODO: X axis").style(Style::default().fg(color_consts::COLOR_WHITE));
    f.render_widget(p, *area);
    Ok(())
}
