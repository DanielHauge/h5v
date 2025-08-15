use std::{cell::RefCell, f64, rc::Rc};

use hdf5_metno::{
    types::{FixedAscii, FixedUnicode, VarLenAscii, VarLenUnicode},
    Error,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    symbols,
    text::{Span, Text},
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Wrap},
    Frame,
};

use super::{
    dims::render_dim_selector, image_preview::render_img, segment_scroll::render_segment_scroll,
    state::AppState,
};
use crate::{
    color_consts,
    data::{PreviewSelection, Previewable, SliceSelection},
    error::AppError,
    h5f::{Encoding, H5FNode, Node},
    ui::state::SegmentType,
};

pub fn render_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Rc<RefCell<H5FNode>>,
    state: &mut AppState,
) {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let node = &selected_node.borrow().node;

    if let Node::Dataset(_, attr) = node {
        match &attr.image {
            Some(image_type) => match render_img(image_type, f, &area_inner, node, state) {
                Ok(()) => {}
                Err(e) => {
                    render_error(f, &area_inner, format!("Render img error: {}", e));
                }
            },
            None => {
                if attr.matrixable.is_none() {
                    match render_string_preview(f, &area_inner, node) {
                        Ok(()) => {}
                        Err(e) => {
                            render_error(f, &area_inner, format!("Render string error: {}", e));
                        }
                    }
                } else {
                    match render_chart_preview(f, &area_inner, node, state) {
                        Ok(()) => {}
                        Err(e) => {
                            render_error(f, &area_inner, format!("Render chart error: {}", e));
                        }
                    }
                }
            }
        }
    }
}

fn render_chart_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Node,
    state: &mut AppState,
) -> Result<(), AppError> {
    let (ds, ds_meta) = match selected_node {
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

    if x_selectable_dims.is_empty() {
        // TODO: Read scalar
        match ds_meta.matrixable {
            Some(t) => match t {
                crate::sprint_typedesc::MatrixRenderType::Float64 => {
                    let ds = ds.read_scalar::<f64>();
                    if let Err(e) = ds {
                        render_error(f, area, format!("Error reading scalar: {}", e));
                        return Ok(());
                    }
                    let ds = ds.unwrap();
                    render_string(f, area, ds);
                }
                crate::sprint_typedesc::MatrixRenderType::Uint64 => {
                    let ds = ds.read_scalar::<u64>();
                    if let Err(e) = ds {
                        render_error(f, area, format!("Error reading scalar: {}", e));
                        return Ok(());
                    }
                    let ds = ds.unwrap();
                    render_string(f, area, ds);
                }
                crate::sprint_typedesc::MatrixRenderType::Int64 => {
                    let ds = ds.read_scalar::<i64>();
                    if let Err(e) = ds {
                        render_error(f, area, format!("Error reading scalar: {}", e));
                        return Ok(());
                    }
                    let ds = ds.unwrap();
                    render_string(f, area, ds);
                }
                crate::sprint_typedesc::MatrixRenderType::Compound => {
                    render_unsupported_rendering(
                        f,
                        area,
                        selected_node,
                        "Compound types are not supported for chart preview",
                    )?;
                    return Ok(());
                }
                crate::sprint_typedesc::MatrixRenderType::Strings => {
                    render_string_preview(f, area, selected_node)?;
                    return Ok(());
                }
            },
            None => {
                render_unsupported_rendering(
                    f,
                    area,
                    selected_node,
                    "Not enough data for selectable dimensions for x-axis",
                )?;
            }
        }
        return Ok(());
    }

    let selected_indexe_length = state.selected_indexes.len();
    for i in 0..selected_indexe_length {
        if !x_selectable_dims.contains(&i) {
            state.selected_indexes[i] = 0;
        }
    }

    if !x_selectable_dims.contains(&state.selected_x) {
        state.selected_x = x_selectable_dims[0];
    }

    let chart_area = if x_selectable_dims.len() > 1 {
        let areas_split =
            Layout::vertical(vec![Constraint::Length(4), Constraint::Min(1)]).split(*area);
        render_dim_selector(f, &areas_split[0], state, &shape, false)?;
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

    const MAX_SEGMENT_SIZE: usize = 250000;
    let (chart_area, data_preview) = if shape[state.selected_x] > MAX_SEGMENT_SIZE {
        state.segment_state.segumented = SegmentType::Chart;
        state.segment_state.segment_count =
            (shape[state.selected_x] as f64 / MAX_SEGMENT_SIZE as f64).ceil() as i32;
        let areas_split =
            Layout::vertical(vec![Constraint::Length(2), Constraint::Min(1)]).split(*area);
        render_segment_scroll(f, &areas_split[0], state)?;

        let data_preview = ds.plot(PreviewSelection {
            x: state.selected_x,
            index: state.selected_indexes[0..total_dims - 1].to_vec(),
            slice: SliceSelection::FromTo(
                MAX_SEGMENT_SIZE * state.segment_state.idx as usize,
                MAX_SEGMENT_SIZE * (state.segment_state.idx + 1) as usize,
            ),
        })?;
        (areas_split[1], data_preview)
    } else {
        let data_preview = ds.plot(PreviewSelection {
            x: state.selected_x,
            index: state.selected_indexes[0..total_dims - 1].to_vec(),
            slice: SliceSelection::All,
        })?;
        (chart_area, data_preview)
    };

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
    let bg = match (&state.focus, &state.mode) {
        (super::state::Focus::Content, super::state::Mode::Normal) => color_consts::FOCUS_BG_COLOR,
        _ => color_consts::BG_COLOR,
    };
    let chart = Chart::new(vec![ds])
        .style(Style::default().bg(bg))
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

fn render_unsupported_rendering(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Node,
    desc: &str,
) -> Result<(), Error> {
    let (ds, _) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => return Ok(()),
    };

    let inner_area = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let unsupported_msg = format!("Unsupported preview for dataset: {}", ds.name());
    f.render_widget(unsupported_msg, inner_area);
    let why = format!("Reason: {}", desc);
    f.render_widget(
        why,
        inner_area.inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );
    Ok(())
}

fn render_string<T: ToString>(f: &mut Frame, area: &Rect, string: T) {
    let string = string.to_string();
    let string = string.lines().collect::<Vec<_>>().join("\n");
    let string = Span::styled(string, color_consts::COLOR_WHITE);
    let string = Text::from(string);
    let string = Paragraph::new(string).wrap(Wrap { trim: true });
    f.render_widget(string, *area);
}

fn render_error<T: ToString>(f: &mut Frame, area: &Rect, error: T) {
    f.render_widget(
        Paragraph::new(error.to_string()).style(Style::default().fg(color_consts::ERROR_COLOR)),
        *area,
    );
}

fn render_string_preview(f: &mut Frame, area: &Rect, selected_node: &Node) -> Result<(), AppError> {
    let (dataset, meta) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => panic!("Expected a string dataset to preview string data"),
    };

    match meta.encoding {
        Encoding::LittleEndian => {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "LittleEndian not supported for string data",
            )?;
        }
        Encoding::Unknown => {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Unknown encoding not supported for string data",
            )?;
        }
        Encoding::Ascii => match dataset.read_scalar::<VarLenAscii>() {
            Ok(x) => render_string(f, area, x),
            Err(e) => render_error(f, area, format!("Error: {}", e)),
        },
        Encoding::UTF8 => match dataset.read_scalar::<VarLenUnicode>() {
            Ok(x) => render_string(f, area, x),
            Err(e) => render_error(f, area, format!("Error: {}", e)),
        },
        Encoding::UTF8Fixed => match dataset.read_scalar::<FixedUnicode<32768>>() {
            Ok(x) => render_string(f, area, x.to_string()),
            Err(e) => render_error(f, area, format!("Error: {}", e)),
        },
        Encoding::AsciiFixed => match dataset.read_scalar::<FixedAscii<32768>>() {
            Ok(x) => render_string(f, area, x.to_string()),
            Err(e) => render_error(f, area, format!("Error: {}", e)),
        },
    }
    Ok(())
}
