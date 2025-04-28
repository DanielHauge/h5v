use hdf5_metno::{Error, Hyperslab, Selection, SliceOrIndex};
use ratatui::{layout::Rect, style::Style, widgets::Paragraph, Frame};

use crate::color_consts;

use super::state::AppState;

pub fn render_dim_selector(
    f: &mut Frame,
    area: &Rect,
    _state: &mut AppState,
    _shape: &[usize],
) -> Result<(), Error> {
    let p = Paragraph::new("TODO: X axis").style(Style::default().fg(color_consts::COLOR_WHITE));
    f.render_widget(p, *area);
    Ok(())
}

const MAX_PAGE_SIZE: usize = 250000;

pub trait HasSelection {
    fn get_selection(&self) -> Selection;
}

impl HasSelection for AppState<'_> {
    fn get_selection(&self) -> Selection {
        let x = self.selected_x_dim;
        let sels = self.selected_indexes;
        let page = self.page;
        generate_selector_slice(x, &sels, page)
    }
}

fn generate_selector_slice(x: usize, selections: &[usize], page: usize) -> Selection {
    let mut slice: Vec<SliceOrIndex> = Vec::new();
    let total_dims = selections.len();
    let start = page * MAX_PAGE_SIZE;
    let end = start + MAX_PAGE_SIZE;
    (0..total_dims).for_each(|dim| {
        if x == dim {
            slice.push(SliceOrIndex::SliceTo {
                start,
                end,
                step: 1,
                block: 1,
            });
        } else {
            slice.push(SliceOrIndex::Index(selections[dim]));
        }
    });

    Selection::Hyperslab(Hyperslab::from(slice))
}
