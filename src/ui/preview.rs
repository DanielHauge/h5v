use std::{cell::RefCell, f64, rc::Rc};

use hdf5_metno::Error;
use ratatui::{
    layout::{Alignment, Offset, Rect},
    style::Style,
    symbols,
    text::Span,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
    Frame,
};

use crate::{
    color_consts,
    data::{PreviewSelection, Previewable, Slice},
    h5f::{H5FNode, Node},
};

pub fn render_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
) -> Result<(), Error> {
    // Make a break line with a heading to indicater we render a preview
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let height = area_inner.height;
    let width = area_inner.width;
    let title = format!("Preview, {}x{}", width, height);
    let break_line = Block::default()
        .title(title)
        .borders(ratatui::widgets::Borders::TOP)
        .border_style(Style::default().fg(color_consts::BREAK_COLOR))
        .title_alignment(Alignment::Center)
        .title_style(Style::default().fg(color_consts::TITLE))
        .style(Style::default().bg(color_consts::BG2_COLOR));
    f.render_widget(break_line, area_inner.offset(Offset { x: 0, y: -2 }));
    let node = &selected_node.borrow().node;
    let data_preview = match node {
        Node::Dataset(ds, attr) => {
            if ds.shape().len() == 1 && attr.data_type == "f64" {
                ds.preview(PreviewSelection::OneDim(Slice::All))
            } else {
                return Ok(());
            }
        }
        _ => return Ok(()),
    };

    let x_label_count = match area_inner.width {
        0 => 0,
        _ => area_inner.width / 8,
    };
    let x_labels = (0..=x_label_count)
        .map(|i| {
            let x = (data_preview.length as f64) * (i as f64) / (x_label_count as f64);
            Span::styled(format!("{:.1}", x), color_consts::COLOR_WHITE)
        })
        .collect::<Vec<_>>();

    let y_label_count = match area_inner.height {
        0 => 0,
        _ => area_inner.height / 4,
    };

    let y_labels = (0..=y_label_count)
        .map(|i| {
            let y = data_preview.min
                + (data_preview.max - data_preview.min) * (i as f64) / (y_label_count as f64);
            Span::styled(format!("{:.1}", y), color_consts::COLOR_WHITE)
        })
        .collect::<Vec<_>>();

    // into a &'a [(f64, f64)]
    let data: &[(f64, f64)] = &data_preview.data;
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
                .labels(x_labels)
                .bounds((0.0, data_preview.length as f64).into()),
        )
        .y_axis(
            Axis::default()
                .title("Y axis")
                .style(Style::default().fg(ratatui::style::Color::White))
                .labels(y_labels)
                .bounds((data_preview.min, data_preview.max).into()),
        );
    f.render_widget(chart, area_inner);

    Ok(())
}
