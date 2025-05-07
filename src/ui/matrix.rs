use std::{cell::RefCell, rc::Rc};

use ratatui::{layout::Rect, widgets::Paragraph, Frame};

use crate::{error::AppError, h5f::H5FNode};

use super::state::AppState;

pub fn render_matrix(
    f: &mut Frame,
    area: &Rect,
    _selected_node: &Rc<RefCell<H5FNode>>,
    _state: &mut AppState,
) -> Result<(), AppError> {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let para = Paragraph::new("matrix");
    f.render_widget(para, area_inner);
    Ok(())
}
