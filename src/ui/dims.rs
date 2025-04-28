use hdf5_metno::Error;
use ndarray::{s, IxDyn, Slice, SliceInfo, SliceInfoElem};
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

const MAX_PAGE_SIZE: usize = 250000;

pub fn generate_selector_slice(x: usize, selections: Vec<isize>, page: usize) {
    let mut slice = Vec::new();
    let mut selection_idx = 0;
    let total_dims = selections.len();
    #[allow(clippy::explicit_counter_loop)]
    for dim in 0..total_dims {
        if x == dim {
            slice.push(SliceInfoElem::Slice {
                start: 0,
                end: None,
                step: 1,
            });
        } else {
            slice.push(SliceInfoElem::Index(selections[selection_idx]));
        }
        selection_idx += 1;
    }
}
