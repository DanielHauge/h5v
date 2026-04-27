use hdf5_metno::types::TypeDescriptor;
use plotters::{
    chart::ChartBuilder,
    prelude::{BitMapBackend, IntoDrawingArea},
    style::IntoFont,
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
    h5f::{H5FNode, HasPath, Node},
    ui::{
        dims::render_dim_selector,
        matrix::{EnumRenderer, RenderIntercept},
        preview::render_string_preview,
        segment_scroll::render_segment_scroll,
        state::{ChartPreviewLoadRequest, IsFromDs, SegmentType},
        std_comp_render::{render_error, render_string, render_unsupported_rendering},
    },
    utils::image_capable_terminal,
};

use super::state::AppState;

pub const MAX_SEGMENT_SIZE: usize = 250000;

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
        match ds_meta.matrixable {
            Some(t) => match t {
                crate::sprint_typedesc::MatrixRenderType::Float64 => {
                    let ds = ds.read_scalar::<f64>();
                    let ds = match ds {
                        Ok(ds) => ds,
                        Err(e) => {
                            render_error(f, area, format!("Error reading scalar: {}", e));
                            return Ok(());
                        }
                    };
                    render_string(f, area, node, ds, None);
                }
                crate::sprint_typedesc::MatrixRenderType::Uint64 => {
                    let ds = ds.read_scalar::<u64>();
                    let ds = match ds {
                        Ok(ds) => ds,
                        Err(e) => {
                            render_error(f, area, format!("Error reading scalar: {}", e));
                            return Ok(());
                        }
                    };
                    render_string(f, area, node, ds, None);
                }
                crate::sprint_typedesc::MatrixRenderType::Int64 => {
                    let ds = ds.read_scalar::<i64>();
                    let ds = match ds {
                        Ok(ds) => ds,
                        Err(e) => {
                            render_error(f, area, format!("Error reading scalar: {}", e));
                            return Ok(());
                        }
                    };
                    render_string(f, area, node, ds, None);
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
                crate::sprint_typedesc::MatrixRenderType::Enum => {
                    let TypeDescriptor::Enum(et) = ds.dtype()?.to_descriptor()? else {
                        unreachable!("MatrixRenderType::Enum should only be set for enum types")
                    };
                    let enum_rendere = EnumRenderer::new(et);
                    let scalar_value = ds.read_scalar::<u64>()?;
                    let string = enum_rendere.render_as_line(&scalar_value);
                    f.render_widget(ratatui::widgets::Paragraph::new(string), *area);

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

    let selected_indexe_length = node.selected_indexes.len();
    for i in 0..selected_indexe_length {
        if !x_selectable_dims.contains(&i) {
            node.selected_indexes[i] = 0;
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
        render_dim_selector(f, &areas_split[0], node, &shape, false)?;
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

    let (chart_area, data_preview_selection) = if shape[node.selected_x] > MAX_SEGMENT_SIZE {
        state.segment_state.segumented = SegmentType::Chart;
        state.segment_state.segment_count =
            (shape[node.selected_x] as f64 / MAX_SEGMENT_SIZE as f64).ceil() as i32;
        let areas_split =
            Layout::horizontal(vec![Constraint::Min(1), Constraint::Length(2)]).split(*area);
        render_segment_scroll(f, &areas_split[1], state)?;

        let max_len = shape[node.selected_x];
        let data_preview_selection = PreviewSelection {
            x: node.selected_x,
            index: node.selected_indexes[0..total_dims].to_vec(),
            slice: SliceSelection::FromTo(
                MAX_SEGMENT_SIZE * state.segment_state.idx as usize,
                (MAX_SEGMENT_SIZE * (state.segment_state.idx + 1) as usize).min(max_len),
            ),
        };
        (areas_split[0], data_preview_selection)
    } else {
        let data_preview_selection = PreviewSelection {
            x: node.selected_x,
            index: node.selected_indexes[0..total_dims].to_vec(),
            slice: SliceSelection::All,
        };

        (chart_area, data_preview_selection)
    };

    let image_capable = image_capable_terminal();
    let loaded_preview_selection = state.chart_preview_state.ds_selection.clone();

    if image_capable
        && state.chart_preview_state.is_from_ds(&node.node)
        && loaded_preview_selection == Some(data_preview_selection.clone())
    {
        if let Some(ref error) = state.chart_preview_state.error {
            render_error(
                f,
                &chart_area,
                format!("Error loading chart preview: {}", error),
            );
            return Ok(());
        }
        if let Some(ref mut protocol) = state.chart_preview_state.protocol {
            f.render_stateful_widget(StatefulImage::default(), chart_area, protocol);
        } else {
            render_string(
                f,
                &chart_area,
                node,
                "Loading chart preview...".to_string(),
                None,
            );
        }
        return Ok(());
    }

    if image_capable {
        state.chart_preview_state.ds_loaded = Some(node.node.path());
        state.chart_preview_state.ds_selection = Some(data_preview_selection.clone());
        state.chart_preview_state.error = None;
        state.chart_preview_state.protocol = None;
        let chart_preview_load_request = ChartPreviewLoadRequest {
            ds,
            width: chart_area.width,
            height: chart_area.height,
            selection: data_preview_selection.clone(),
            segment_state: state.segment_state.clone(),
        };
        state
            .chart_preview_state
            .tx_load_chartpreview
            .send(chart_preview_load_request)
            .ok();

        render_string(
            f,
            &chart_area,
            node,
            "Loading chart preview...".to_string(),
            None,
        );
    } else {
        let data_preview = match ds.plot(&data_preview_selection) {
            Ok(dp) => dp,
            Err(e) => {
                render_error(f, &chart_area, format!("Error plotting data: {}", e));
                return Ok(());
            }
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

pub fn render_image_chart(
    buffer: &mut [u8],
    width: u32,
    height: u32,
    x_min: f64,
    data_preview: DatasetPlotingData,
) -> Result<(), AppError> {
    let root = BitMapBackend::with_buffer(buffer, (width, height)).into_drawing_area();
    root.margin(10, 10, 10, 10);
    root.fill(&plotters::prelude::WHITE)
        .map_err(|e| AppError::DrawingError(format!("Error filling background: {}", e)))?;
    let max = data_preview.max;
    let y_label_area_size = format!("{max:.4}").len() as u32 * 3 + 30;

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(y_label_area_size)
        .build_cartesian_2d(
            x_min..(x_min + data_preview.length as f64),
            data_preview.min..data_preview.max,
        )
        .map_err(|e| AppError::DrawingError(format!("Error building chart: {}", e)))?;

    // Draw the mesh (grid lines)
    chart
        .configure_mesh()
        .x_label_style(("sans-serif", 18).into_font())
        .y_label_style(("sans-serif", 18).into_font())
        .draw()
        .map_err(|e| AppError::DrawingError(format!("Error drawing mesh: {}", e)))?;

    let data = data_preview.data.iter().map(|(x, y)| (x_min + *x, *y));
    let line_series = plotters::prelude::LineSeries::new(data, plotters::prelude::BLUE);
    chart
        .draw_series(line_series)
        .map_err(|e| AppError::DrawingError(format!("Error drawing line series: {}", e)))?;
    root.present()
        .map_err(|e| AppError::DrawingError(format!("Error presenting chart: {}", e)))?;
    Ok(())
}
