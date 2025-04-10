use std::{cell::RefCell, f64, rc::Rc};

use hdf5_metno::{Error, Selection};
use ratatui::{
    layout::Rect,
    style::Style,
    symbols,
    text::Span,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
    Frame,
};

use crate::{
    color_consts,
    h5f::{H5FNode, HasName, Node},
};

pub fn render_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
) -> Result<(), Error> {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let node = &selected_node.borrow().node;
    match node {
        Node::Dataset(ds, attr) => {

            // ds.read_slice()?;
        }
        _ => return Ok(()),
    }
    // Generate a long list of sinus data points
    let mut data: Vec<(f64, f64)> = Vec::new();
    for i in 0..1000 {
        let x = i as f64 / 10.0;
        let y = (x * 2.0).sin();
        data.push((x, y));
    }

    // into a &'a [(f64, f64)]
    let data: &[(f64, f64)] = data.as_slice();
    let ds = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .data(data);
    let chart = Chart::new(vec![ds])
        .style(Style::default().bg(color_consts::BG2_COLOR))
        .x_axis(
            Axis::default()
                .title("X axis")
                .style(Style::default().fg(ratatui::style::Color::White))
                .labels([
                    Span::styled("0", color_consts::COLOR_WHITE),
                    Span::styled("10", color_consts::COLOR_WHITE),
                ])
                .bounds((0.0, 500.0).into()),
        )
        .y_axis(
            Axis::default()
                .title("Y axis")
                .style(Style::default().fg(ratatui::style::Color::White))
                .labels([
                    Span::styled("1", color_consts::COLOR_WHITE),
                    Span::styled("0", color_consts::COLOR_WHITE),
                    Span::styled("-1", color_consts::COLOR_WHITE),
                ])
                .bounds((-2.0, 2.0).into()),
        );
    f.render_widget(chart, area_inner);

    Ok(())
}
