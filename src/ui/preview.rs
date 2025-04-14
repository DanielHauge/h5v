use std::{cell::RefCell, f64, rc::Rc};

use hdf5_metno::{
    types::{VarLenAscii, VarLenUnicode},
    Error,
};
use ratatui::{
    layout::Rect,
    style::Style,
    symbols,
    text::Span,
    widgets::{Axis, Chart, Dataset, GraphType},
    Frame,
};

use crate::{
    color_consts,
    data::{PreviewSelection, Previewable, Slice},
    h5f::{H5FNode, Node},
};

use super::app::AppState;

pub fn render_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
    state: &Rc<RefCell<AppState>>,
) -> Result<(), Error> {
    // Make a break line with a heading to indicater we render a preview
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 0,
    });
    let node = &selected_node.borrow().node;

    match node {
        Node::Dataset(_, attr) => {
            if !attr.numerical {
                render_string_preview(f, &area_inner, node)
            } else {
                render_chart_preview(f, &area_inner, node, state)
            }
        }
        _ => {
            return Ok(());
        }
    }
}

fn render_chart_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Node,
    _state: &Rc<RefCell<AppState>>,
) -> Result<(), Error> {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 0,
        vertical: 1,
    });
    let data_preview = match &selected_node {
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

fn render_string_preview(f: &mut Frame, area: &Rect, selected_node: &Node) -> Result<(), Error> {
    let (dataset, meta) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => panic!("Expected a string dataset to preview string data"),
    };

    match meta.encoding {
        crate::h5f::Encoding::Unknown => panic!("Unknown encoding not supported for string data"),
        crate::h5f::Encoding::LittleEndian => panic!("LittleEndian not supported for string data"),
        crate::h5f::Encoding::ASCII => match dataset.read_scalar::<VarLenAscii>() {
            Ok(x) => {
                let string = x.to_string();
                let string = string.lines().collect::<Vec<_>>().join("\n");
                let string = Span::styled(string, color_consts::COLOR_WHITE);
                let string = ratatui::text::Text::from(string);
                let string = ratatui::widgets::Paragraph::new(string)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                f.render_widget(string, *area);
            }
            Err(e) => {
                f.render_widget(
                    ratatui::widgets::Paragraph::new(format!("Error: {}", e))
                        .style(Style::default().fg(color_consts::ERROR_COLOR)),
                    *area,
                );
            }
        },
        crate::h5f::Encoding::UTF8 => match dataset.read_scalar::<VarLenUnicode>() {
            Ok(x) => {
                let string = x.to_string();
                let string = string.lines().collect::<Vec<_>>().join("\n");
                let string = Span::styled(string, color_consts::COLOR_WHITE);
                let string = ratatui::text::Text::from(string);
                let string = ratatui::widgets::Paragraph::new(string)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                f.render_widget(string, *area);
            }
            Err(e) => {
                f.render_widget(
                    ratatui::widgets::Paragraph::new(format!("Error: {}", e))
                        .style(Style::default().fg(color_consts::ERROR_COLOR)),
                    *area,
                );
            }
        },
    }
    Ok(())
}
