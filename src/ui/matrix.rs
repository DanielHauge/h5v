use std::{cell::RefCell, f64, rc::Rc};

use hdf5_metno::{
    types::{VarLenAscii, VarLenUnicode},
    Error,
};
use ratatui::{
    layout::{Constraint, Layout, Offset, Rect},
    style::Style,
    symbols,
    text::{Span, Text},
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Wrap},
    Frame,
};

use crate::{
    color_consts,
    data::{PreviewSelection, Previewable, SliceSelection},
    error::AppError,
    h5f::{Encoding, H5FNode, Node},
};

use super::{dims::render_dim_selector, image_preview::render_img, state::AppState};

pub fn render_matrix(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
    state: &mut AppState,
) -> Result<(), AppError> {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let para = Paragraph::new("matrix");
    f.render_widget(para, area_inner);
    Ok(())
}
