use std::{cell::RefCell, f64, io::BufReader, rc::Rc};

use hdf5_metno::{
    types::{VarLenAscii, VarLenUnicode},
    Error,
};
use image::{ImageFormat, ImageReader};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    symbols,
    text::{Span, Text},
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{
    picker::{self, Picker},
    StatefulImage,
};

use crate::{
    color_consts,
    data::{PreviewSelection, Previewable, SliceSelection},
    h5f::{Encoding, H5FNode, ImageType, Node},
};

use super::app::AppState;

pub fn render_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
    state: &mut AppState,
) -> Result<(), Error> {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 0,
    });
    let node = &selected_node.borrow().node;

    match node {
        Node::Dataset(_, attr) => match &attr.image {
            Some(image_type) => match image_type {
                ImageType::JPEG => render_jpeg(f, &area_inner, &node),
                ImageType::PNG => todo!(),
                ImageType::GRAYSCALE => todo!(),
                ImageType::BITMAP => todo!(),
                ImageType::TRUECOLOR => todo!(),
                ImageType::INDEXED => todo!(),
            },
            None => {
                if !attr.numerical {
                    render_string_preview(f, &area_inner, node)
                } else {
                    render_chart_preview(f, &area_inner, node, state)
                }
            }
        },
        _ => {
            return Ok(());
        }
    }
}

fn render_jpeg(f: &mut Frame, area: &Rect, selected_node: &Node) -> Result<(), Error> {
    let (ds, _) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => return Ok(()),
    };

    let inner_area = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });

    let ds_reader = ds.as_byte_reader().unwrap();
    let ds_buffered = BufReader::new(ds_reader);
    let dyn_img = image::load(ds_buffered, ImageFormat::Jpeg).unwrap();
    let picker = Picker::from_query_stdio().unwrap();
    // let picker = Picker::from_fontsize((20, 9));
    let mut image = picker.new_resize_protocol(dyn_img);
    let image_widget = StatefulImage::default();
    f.render_stateful_widget(image_widget, inner_area, &mut image);
    Ok(())
}

fn render_dim_selector(f: &mut Frame, area: &Rect, state: &mut AppState) -> Result<(), Error> {
    let p = Paragraph::new("TODO: X axis").style(Style::default().fg(color_consts::COLOR_WHITE));
    f.render_widget(p, *area);
    Ok(())
}

fn render_chart_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Node,
    state: &mut AppState,
) -> Result<(), Error> {
    let (ds, _) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => return Ok(()),
    };

    let shape = ds.shape();
    let total_dims = shape.len();
    let x_selectable_dims: Vec<usize> = shape
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 1)
        .map(|(i, _)| i)
        .collect();

    let selected_indexe_length = state.selected_indexes.len();
    for i in 0..selected_indexe_length {
        if !x_selectable_dims.contains(&i) {
            state.selected_indexes[i] = 0;
        }
    }

    if !x_selectable_dims.contains(&state.selected_x_dim) {
        state.selected_x_dim = x_selectable_dims[0];
    }

    let chart_area = if x_selectable_dims.len() > 1 {
        let areas_split =
            Layout::vertical(vec![Constraint::Length(3), Constraint::Min(1)]).split(*area);
        render_dim_selector(f, &areas_split[0], state)?;
        areas_split[1].inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 1,
        })
    } else {
        area.inner(ratatui::layout::Margin {
            horizontal: 0,
            vertical: 1,
        })
    };

    if shape[state.selected_x_dim] > 250000 {
        let paragrapth = Paragraph::new("TODO: To many data points to show")
            .style(Style::default().fg(color_consts::COLOR_WHITE));
        f.render_widget(paragrapth, *area);
        return Ok(());
    }

    // make slice of state.selected_indexes[0..total_dims]
    let data_preview = ds.preview(PreviewSelection {
        x: state.selected_x_dim,
        index: state.selected_indexes[0..total_dims - 1].to_vec(),
        slice: SliceSelection::All,
    })?;

    let x_label_count = match chart_area.width {
        0 => 0,
        _ => chart_area.width / 8,
    };
    let x_labels = (0..=x_label_count)
        .map(|i| {
            let x = (data_preview.length as f64) * (i as f64) / (x_label_count as f64);
            Span::styled(format!("{:.1}", x), color_consts::COLOR_WHITE)
        })
        .collect::<Vec<_>>();

    let y_label_count = match chart_area.height {
        0 => 0,
        _ => chart_area.height / 4,
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
    f.render_widget(chart, chart_area);

    Ok(())
}

fn render_string_preview(f: &mut Frame, area: &Rect, selected_node: &Node) -> Result<(), Error> {
    let (dataset, meta) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => panic!("Expected a string dataset to preview string data"),
    };

    match meta.encoding {
        Encoding::Unknown => panic!("Unknown encoding not supported for string data"),
        Encoding::LittleEndian => panic!("LittleEndian not supported for string data"),
        Encoding::ASCII => match dataset.read_scalar::<VarLenAscii>() {
            Ok(x) => {
                let string = x.to_string();
                let string = string.lines().collect::<Vec<_>>().join("\n");
                let string = Span::styled(string, color_consts::COLOR_WHITE);
                let string = Text::from(string);
                let string = Paragraph::new(string).wrap(Wrap { trim: true });
                f.render_widget(string, *area);
            }
            Err(e) => {
                f.render_widget(
                    Paragraph::new(format!("Error: {}", e))
                        .style(Style::default().fg(color_consts::ERROR_COLOR)),
                    *area,
                );
            }
        },
        Encoding::UTF8 => match dataset.read_scalar::<VarLenUnicode>() {
            Ok(x) => {
                let string = x.to_string();
                let string = string.lines().collect::<Vec<_>>().join("\n");
                let string = Span::styled(string, color_consts::COLOR_WHITE);
                let string = Text::from(string);
                let string = Paragraph::new(string).wrap(Wrap { trim: true });
                f.render_widget(string, *area);
            }
            Err(e) => {
                f.render_widget(
                    Paragraph::new(format!("Error: {}", e))
                        .style(Style::default().fg(color_consts::ERROR_COLOR)),
                    *area,
                );
            }
        },
    }
    Ok(())
}
