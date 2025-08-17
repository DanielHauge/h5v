use std::fmt::Pointer;

use image::{DynamicImage, ImageBuffer, Rgb};
use plotters::{
    chart::ChartBuilder,
    prelude::{BitMapBackend, IntoDrawingArea},
    style::{Color, IntoFont},
};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    symbols::Marker,
    text::Span,
    widgets::{Axis, Chart, Dataset, GraphType},
    Frame,
};
use ratatui_image::StatefulImage;

use crate::{
    color_consts,
    data::{DatasetPlotingData, PreviewSelection, Previewable, SliceSelection},
    error::AppError,
    h5f::{H5FNode, Node},
    ui::{
        dims::render_dim_selector,
        preview::render_string_preview,
        segment_scroll::render_segment_scroll,
        state::SegmentType,
        std_comp_render::{render_error, render_string, render_unsupported_rendering},
    },
    utils::image_capable_terminal,
};

use super::state::AppState;

pub fn render_chart_preview(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    state: &mut AppState,
) -> Result<(), AppError> {
    let selected_node = &node.node;
    let (ds, ds_meta) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => return Ok(()),
    };
    let ds = ds.clone();

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
                    );
                    return Ok(());
                }
                crate::sprint_typedesc::MatrixRenderType::Strings => {
                    render_string_preview(f, area, node)?;
                    return Ok(());
                }
            },
            None => {
                render_unsupported_rendering(
                    f,
                    area,
                    selected_node,
                    "Not enough data for selectable dimensions for x-axis",
                );
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

    if !x_selectable_dims.contains(&node.selected_x) {
        node.selected_x = x_selectable_dims[0];
    }
    if node.selected_dim == node.selected_x {
        node.selected_dim = x_selectable_dims
            .iter()
            .find(|&&x| x != node.selected_x)
            .cloned()
            .unwrap_or(0);
    }

    let chart_area = if x_selectable_dims.len() > 1 {
        let areas_split =
            Layout::vertical(vec![Constraint::Length(4), Constraint::Min(1)]).split(*area);
        render_dim_selector(f, &areas_split[0], node, state, &shape, false)?;
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
    let (chart_area, data_preview) = if shape[node.selected_x] > MAX_SEGMENT_SIZE {
        state.segment_state.segumented = SegmentType::Chart;
        state.segment_state.segment_count =
            (shape[node.selected_x] as f64 / MAX_SEGMENT_SIZE as f64).ceil() as i32;
        let areas_split =
            Layout::vertical(vec![Constraint::Length(2), Constraint::Min(1)]).split(*area);
        render_segment_scroll(f, &areas_split[0], state)?;

        let data_preview = ds.plot(PreviewSelection {
            x: node.selected_x,
            index: state.selected_indexes[0..total_dims - 1].to_vec(),
            slice: SliceSelection::FromTo(
                MAX_SEGMENT_SIZE * state.segment_state.idx as usize,
                MAX_SEGMENT_SIZE * (state.segment_state.idx + 1) as usize,
            ),
        })?;
        (areas_split[1], data_preview)
    } else {
        let data_preview = ds.plot(PreviewSelection {
            x: node.selected_x,
            index: state.selected_indexes[0..total_dims - 1].to_vec(),
            slice: SliceSelection::All,
        })?;
        (chart_area, data_preview)
    };

    if image_capable_terminal() {
        // TODO: Optimize this, store the image buffer in the node, and just reuse it.
        // TODO: Optimize this, store the image buffer in the node .
        let (x, y) = state.img_state.picker.font_size();
        let height = chart_area.height as u32 * y as u32;
        let width = chart_area.width as u32 * x as u32;
        let mut buffer = vec![0; (height * width * 3) as usize];
        render_image_chart(&mut buffer, width, height, data_preview)?;
        let image = ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, buffer)
            .expect("buffer size mismatch");
        let image_widget = StatefulImage::default();
        let dyn_img = DynamicImage::ImageRgb8(image);
        let mut stateful_protocol = state.img_state.picker.new_resize_protocol(dyn_img);
        f.render_stateful_widget(image_widget, chart_area, &mut stateful_protocol);
    } else {
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
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .data(data);
        let bg = match (&state.focus, &state.mode) {
            (super::state::Focus::Content, super::state::Mode::Normal) => {
                color_consts::FOCUS_BG_COLOR
            }
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
    }

    Ok(())
}

fn render_image_chart(
    buffer: &mut [u8],
    width: u32,
    height: u32,
    data_preview: DatasetPlotingData,
) -> Result<(), AppError> {
    let root = BitMapBackend::with_buffer(buffer, (width, height)).into_drawing_area();
    root.margin(10, 10, 10, 10);
    root.fill(&plotters::prelude::WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(
            0.0..data_preview.length as f64,
            data_preview.min..data_preview.max,
        )
        .unwrap();

    // Draw the mesh (grid lines)
    chart
        .configure_mesh()
        .x_label_style(("sans-serif", 18).into_font())
        .y_label_style(("sans-serif", 18).into_font())
        .draw()
        .unwrap();

    let data = data_preview.data.iter().map(|(x, y)| (*x, *y));
    let line_series = plotters::prelude::LineSeries::new(data, plotters::prelude::BLUE);
    chart.draw_series(line_series).unwrap();
    // chart
    //     .configure_series_labels()
    //     .label_font(("sans-serif", 15).into_font())
    //     .background_style(plotters::prelude::WHITE.mix(0.8))
    //     .draw()
    //     .unwrap();
    root.present().unwrap();
    Ok(())
}
