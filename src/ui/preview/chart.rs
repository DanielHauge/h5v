use hdf5_metno::types::TypeDescriptor;
use plotters::{
    chart::ChartBuilder,
    prelude::{BitMapBackend, IntoDrawingArea, IntoLogRange, Text},
    style::{
        text_anchor::{HPos, Pos, VPos},
        Color as _, IntoFont, RGBColor, ShapeStyle,
    },
};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    symbols::Marker,
    text::{Line, Span},
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Wrap},
    Frame,
};

use crate::{
    configure,
    data::{DatasetPlotingData, PreviewSelection, Previewable, SliceSelection},
    error::AppError,
    h5f::{
        plot_projected, read_projected_scalar, read_single_value_dataset, DatasetHandle,
        DatasetMetaState, H5FNode, HasPath, Node,
    },
    ui::{
        chart_math::{format_axis_number, normalized_log_axis_bounds, symlog, symlog_inverse},
        chart_stats::{box_plot_summary, histogram_summary, histogram_summary_with_scale},
        matrix::{EnumRenderer, RenderIntercept},
        mchart::ChartAxisScale,
        page_scroll::PageDisplayInfo,
        perf,
        preview::render_string_preview,
        render::MatrixRenderType,
        state::{
            AppState, ChartPreviewKey, ChartPreviewSource, Focus, Mode, PageType, PreviewChartMode,
            PreviewChartRoi, PreviewChartViewport,
        },
        std_comp_render::{render_error, render_string, render_unsupported_rendering},
    },
};

mod context;
mod protocol;

use context::{
    copy_page_display_info, preview_chart_layout, preview_chart_plot_area, preview_roi_range,
    preview_roi_x_bounds, preview_stats_info, preview_view_info, preview_visible_points,
    preview_x_axis_max, preview_x_min, render_preview_context_panel,
};
pub(crate) use context::{
    preview_chart_data_bounds, preview_effective_x_domain, preview_visible_index_window,
    preview_x_from_ratio, preview_x_ratio,
};
use protocol::{clear_active_chart_preview, queue_chart_preview_load};

pub const MAX_PAGE_SIZE: usize = 2_500_000;
const PREVIEW_POINT_MARKER_RADIUS: i32 = 5;
const PREVIEW_SELECTED_POINT_MARKER_RADIUS: i32 = 7;

fn valid_log_range(min: f64, max: f64) -> bool {
    min.is_finite() && max.is_finite() && min > 0.0 && max > min
}

fn clear_chart_preview_layout(state: &mut AppState<'_>) {
    state.chart_preview_state.set_chart_area(None);
    state.chart_preview_state.set_plot_area(None);
    state.ui_layout.preview_axis_scales.clear();
}

fn preview_axis_scale_supported(state: &AppState<'_>, x_axis: bool) -> bool {
    if x_axis {
        state.chart_preview_state.mode.supports_x_log_scale()
    } else {
        state.chart_preview_state.mode.supports_y_log_scale()
    }
}

fn scale_value(scale: ChartAxisScale, value: f64) -> f64 {
    (scale == ChartAxisScale::SymLog)
        .then(|| symlog(value))
        .unwrap_or(value)
}

fn axis_label(scale: ChartAxisScale, value: f64) -> String {
    format_axis_number(match scale {
        ChartAxisScale::SymLog => symlog_inverse(value),
        ChartAxisScale::Logarithmic => value.exp(),
        ChartAxisScale::Linear => value,
    })
}

fn transformed_scale_value(scale: ChartAxisScale, value: f64) -> f64 {
    match scale {
        ChartAxisScale::SymLog => symlog(value),
        ChartAxisScale::Logarithmic => value.ln(),
        ChartAxisScale::Linear => value,
    }
}

fn histogram_tick_indices(boundary_count: usize, max_labels: usize) -> Vec<usize> {
    if boundary_count <= max_labels {
        return (0..boundary_count).collect();
    }
    let step = (boundary_count - 1).div_ceil(max_labels.saturating_sub(1));
    (0..boundary_count)
        .step_by(step)
        .chain(std::iter::once(boundary_count - 1))
        .collect()
}

fn sync_direct_chart_preview(
    state: &mut AppState<'_>,
    chart_area: Rect,
    data_preview: &DatasetPlotingData,
    x_min: f64,
) {
    state
        .chart_preview_state
        .sync_data_bounds(preview_chart_data_bounds(data_preview, x_min));
    state
        .chart_preview_state
        .set_current_data(Some(data_preview.clone()));
    state.chart_preview_state.set_chart_area(Some(chart_area));
    state.chart_preview_state.set_plot_area(
        (state.chart_preview_state.mode.supports_roi()
            || state.chart_preview_state.mode == PreviewChartMode::Histogram)
            .then(|| preview_chart_plot_area(chart_area, state.image_cell_size, data_preview.max))
            .flatten(),
    );
}

fn preview_windowed_values(
    data_preview: &DatasetPlotingData,
    viewport: PreviewChartViewport,
    x_min: f64,
    histogram_range: Option<crate::ui::state::PreviewHistogramRange>,
) -> Vec<f64> {
    let Some((start, end)) = preview_visible_index_window(data_preview, viewport, x_min) else {
        return Vec::new();
    };
    data_preview.data[start..=end]
        .iter()
        .map(|(_, y)| *y)
        .filter(|value| value.is_finite())
        .filter(|value| {
            histogram_range.is_none_or(|range| *value >= range.min && *value <= range.max)
        })
        .collect()
}

pub(crate) fn select_histogram_bin_at(state: &mut AppState<'_>, column: u16, row: u16) -> bool {
    if state.chart_preview_state.mode != PreviewChartMode::Histogram {
        return false;
    }
    let Some(plot_area) = state.chart_preview_state.last_plot_area else {
        return false;
    };
    if column < plot_area.x
        || column >= plot_area.x.saturating_add(plot_area.width)
        || row < plot_area.y
        || row >= plot_area.y.saturating_add(plot_area.height)
    {
        return false;
    }
    let Some(data) = state.chart_preview_state.current_data.as_ref() else {
        return false;
    };
    let Some(viewport) = state.chart_preview_state.effective_viewport() else {
        return false;
    };
    let x_min = state.chart_preview_state.selection_x_min();
    let values = preview_windowed_values(
        data,
        viewport,
        x_min,
        state.chart_preview_state.histogram_range,
    );
    let scale = state
        .chart_preview_state
        .effective_axis_scale_for_renderer(true, state.image_protocol_enabled);
    let Some(summary) = histogram_summary_with_scale(&values, scale) else {
        return false;
    };
    let ratio = f64::from(column.saturating_sub(plot_area.x))
        / f64::from(plot_area.width.saturating_sub(1).max(1));
    let value = match scale {
        ChartAxisScale::Linear => {
            summary.value_min + (summary.value_max - summary.value_min) * ratio
        }
        ChartAxisScale::Logarithmic => (summary.value_min.ln()
            + (summary.value_max.ln() - summary.value_min.ln()) * ratio)
            .exp(),
        ChartAxisScale::SymLog => symlog_inverse(
            symlog(summary.value_min)
                + (symlog(summary.value_max) - symlog(summary.value_min)) * ratio,
        ),
    };
    let index = summary
        .bins
        .iter()
        .position(|bin| value >= bin.start && value <= bin.end)
        .unwrap_or(summary.bins.len().saturating_sub(1));
    let selection = match state.chart_preview_state.histogram_selection {
        None => Some(crate::ui::state::PreviewHistogramSelection {
            start: index,
            end: index,
            selection_count: 1,
        }),
        Some(selection) if selection.selection_count < 2 => {
            Some(crate::ui::state::PreviewHistogramSelection {
                start: selection.start.min(index),
                end: selection.end.max(index),
                selection_count: 2,
            })
        }
        Some(_) => Some(crate::ui::state::PreviewHistogramSelection {
            start: index,
            end: index,
            selection_count: 1,
        }),
    };
    state.chart_preview_state.set_histogram_selection(selection);
    true
}

pub(crate) fn zoom_histogram_selection(state: &mut AppState<'_>) -> bool {
    if state.chart_preview_state.mode != PreviewChartMode::Histogram {
        return false;
    }
    let Some(selection) = state
        .chart_preview_state
        .histogram_selection
        .filter(|selection| selection.selection_count == 2)
    else {
        return false;
    };
    let Some(data) = state.chart_preview_state.current_data.as_ref() else {
        return false;
    };
    let Some(viewport) = state.chart_preview_state.effective_viewport() else {
        return false;
    };
    let values = preview_windowed_values(
        data,
        viewport,
        state.chart_preview_state.selection_x_min(),
        state.chart_preview_state.histogram_range,
    );
    let scale = state
        .chart_preview_state
        .effective_axis_scale_for_renderer(true, state.image_protocol_enabled);
    let Some(summary) = histogram_summary_with_scale(&values, scale) else {
        return false;
    };
    let Some(start) = summary.bins.get(selection.start) else {
        return false;
    };
    let Some(end) = summary.bins.get(selection.end) else {
        return false;
    };
    let range = crate::ui::state::PreviewHistogramRange {
        min: start.start,
        max: end.end,
    };
    if state.chart_preview_state.histogram_range == Some(range) {
        return false;
    }
    state.chart_preview_state.set_histogram_range(range);
    state.chart_preview_state.set_histogram_selection(None);
    true
}

fn render_preview_summary_widget(
    f: &mut Frame,
    chart_area: &Rect,
    title: &str,
    lines: Vec<Line<'static>>,
) {
    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(configure::themed_color(|colors| colors.text.primary)))
        .alignment(ratatui::layout::Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(
            ratatui::widgets::Block::default()
                .title(title)
                .title_alignment(ratatui::layout::Alignment::Center),
        );
    f.render_widget(paragraph, *chart_area);
}

pub fn render_precomputed_chart_preview(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    state: &mut AppState,
    data_preview: DatasetPlotingData,
) -> Result<(), AppError> {
    let _chart_render_timer = perf::metrics().preview.chart_render.start();
    clear_chart_preview_layout(state);
    let chart_area = area.inner(ratatui::layout::Margin {
        horizontal: 0,
        vertical: 1,
    });
    let preview_selection = PreviewSelection {
        x: 0,
        index: vec![],
        slice: SliceSelection::All,
    };
    state
        .chart_preview_state
        .sync_selection_identity(&node.node.path(), &preview_selection);
    let x_min = preview_x_min(&state.page_state);
    if !state.image_protocol_enabled {
        clear_active_chart_preview(state);
        sync_direct_chart_preview(state, chart_area, &data_preview, x_min);
        perf::metrics().preview.direct_widget_renders.increment();
        render_chart_widget(f, &chart_area, state, data_preview, x_min);
        return Ok(());
    }
    state.chart_preview_state.set_chart_area(Some(chart_area));
    state.chart_preview_state.set_plot_area(
        if state.chart_preview_state.mode.supports_roi()
            || state.chart_preview_state.mode == PreviewChartMode::Histogram
        {
            preview_chart_plot_area(chart_area, state.image_cell_size, data_preview.max)
        } else {
            None
        },
    );

    let current_key = ChartPreviewKey {
        ds_path: node.node.path(),
        selection: preview_selection.clone(),
        mode: state.chart_preview_state.mode,
        x_axis_scale: state.chart_preview_state.x_axis_scale,
        y_axis_scale: state.chart_preview_state.y_axis_scale,
        viewport: state.chart_preview_state.viewport,
        roi: state.chart_preview_state.roi,
        histogram_selection: state.chart_preview_state.histogram_selection,
        histogram_range: state.chart_preview_state.histogram_range,
        width: chart_area.width,
        height: chart_area.height,
    };
    queue_chart_preview_load(
        f,
        chart_area,
        state,
        &node.node,
        current_key,
        ChartPreviewSource::Precomputed { data_preview },
    )
}

pub fn render_chart_preview(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    state: &mut AppState,
) -> Result<(), AppError> {
    let _chart_render_timer = perf::metrics().preview.chart_render.start();
    clear_chart_preview_layout(state);
    node.ensure_dataset_meta()?;
    let (ds, ds_meta) = match &node.node {
        Node::Dataset(DatasetHandle::Loaded(ds), DatasetMetaState::Loaded(attr)) => {
            (ds.clone(), attr.clone())
        }
        _ => return Ok(()),
    };
    if ds_meta.is_compound_leaf() && matches!(ds_meta.matrixable, Some(MatrixRenderType::Strings)) {
        let shape = ds.shape();
        if shape.iter().any(|len| *len > 1) {
            render_unsupported_rendering(
                f,
                area,
                &node.node,
                "Projected string fields are matrix-only; use Matrix mode for multi-value string previews",
            );
            return Ok(());
        }
    }
    if matches!(ds_meta.matrixable, Some(MatrixRenderType::ByteArray)) {
        render_unsupported_rendering(
            f,
            area,
            &node.node,
            "Preview is only supported for vlen byte arrays when image attributes are present; use Matrix mode to inspect values",
        );
        return Ok(());
    }
    if ds_meta.is_compound_leaf() {
        return render_projected_chart_preview(f, area, node, state, ds, ds_meta);
    }
    if matches!(ds_meta.matrixable, Some(MatrixRenderType::Strings)) {
        return render_string_preview(f, area, node, state);
    }

    let shape = ds.shape();
    let total_dims = shape.len();
    node.sync_selection_rank(total_dims);
    let selected_node = &node.node;
    let x_selectable_dims: Vec<usize> = shape
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 1)
        .map(|(i, _)| i)
        .collect();

    if x_selectable_dims.is_empty() {
        match ds_meta.matrixable {
            Some(t) => match t {
                MatrixRenderType::Float64 => {
                    let ds = read_single_value_dataset::<f64>(&ds);
                    let ds = match ds {
                        Ok(ds) => ds,
                        Err(e) => {
                            render_error(f, area, format!("Error reading scalar: {}", e));
                            return Ok(());
                        }
                    };
                    render_string(f, area, node, ds, None);
                }
                MatrixRenderType::Uint64 => {
                    let ds = read_single_value_dataset::<u64>(&ds);
                    let ds = match ds {
                        Ok(ds) => ds,
                        Err(e) => {
                            render_error(f, area, format!("Error reading scalar: {}", e));
                            return Ok(());
                        }
                    };
                    render_string(f, area, node, ds, None);
                }
                MatrixRenderType::Int64 => {
                    let ds = read_single_value_dataset::<i64>(&ds);
                    let ds = match ds {
                        Ok(ds) => ds,
                        Err(e) => {
                            render_error(f, area, format!("Error reading scalar: {}", e));
                            return Ok(());
                        }
                    };
                    render_string(f, area, node, ds, None);
                }
                MatrixRenderType::Opaque => {
                    render_string_preview(f, area, node, state)?;
                    return Ok(());
                }
                MatrixRenderType::Compound => {
                    render_unsupported_rendering(
                        f,
                        area,
                        selected_node,
                        "Compound types are not supported for chart preview",
                    );
                    return Ok(());
                }
                MatrixRenderType::Strings => {
                    render_string_preview(f, area, node, state)?;
                    return Ok(());
                }
                MatrixRenderType::ByteArray => {
                    render_unsupported_rendering(
                        f,
                        area,
                        selected_node,
                        "Preview is only supported for vlen byte arrays when image attributes are present; use Matrix mode to inspect values",
                    );
                    return Ok(());
                }
                MatrixRenderType::Enum => {
                    let TypeDescriptor::Enum(et) = ds.dtype()?.to_descriptor()? else {
                        render_error(
                            f,
                            area,
                            "Dataset preview enum metadata is inconsistent with the actual type"
                                .to_string(),
                        );
                        return Ok(());
                    };
                    let enum_rendere =
                        EnumRenderer::with_overrides(et, ds_meta.enum_render_overrides.as_ref());
                    let scalar_value = read_single_value_dataset::<u64>(&ds)?;
                    let string = enum_rendere.render_as_line(&scalar_value);
                    f.render_widget(
                        ratatui::widgets::Paragraph::new(string).style(
                            ratatui::style::Style::default()
                                .fg(crate::configure::themed_color(|colors| colors.text.primary)),
                        ),
                        *area,
                    );

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

    for (i, selected_index) in node.selected_indexes.iter_mut().enumerate() {
        if !x_selectable_dims.contains(&i) {
            *selected_index = 0;
        }
    }

    if !x_selectable_dims.contains(&node.selected_x) {
        let Some(first_selectable_dim) = x_selectable_dims.first().copied() else {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Not enough data for selectable dimensions for x-axis",
            );
            return Ok(());
        };
        node.selected_x = first_selectable_dim;
    }
    if node.selected_dim == node.selected_x {
        node.selected_dim = x_selectable_dims
            .iter()
            .find(|&&x| x != node.selected_x)
            .cloned()
            .unwrap_or(0);
    }

    let page_info = if shape[node.selected_x] > MAX_PAGE_SIZE {
        state.page_state.paged = PageType::Chart;
        state.page_state.page_count =
            (shape[node.selected_x] as f64 / MAX_PAGE_SIZE as f64).ceil() as i32;
        let max_len = shape[node.selected_x];
        let range_start = MAX_PAGE_SIZE * state.page_state.idx as usize;
        let range_end = (MAX_PAGE_SIZE * (state.page_state.idx + 1) as usize).min(max_len);
        Some(PageDisplayInfo {
            title: "Page",
            current: state.page_state.idx.max(0) as usize,
            total: state.page_state.page_count.max(0) as usize,
            range_start,
            range_end,
            total_items: max_len,
            unit: "pts",
        })
    } else {
        state.page_state.paged = PageType::Unpaged;
        state.page_state.page_count = 0;
        state.page_state.idx = 0;
        None
    };

    let selector_info = preview_view_info(state, shape[node.selected_x])
        .or_else(|| page_info.as_ref().map(copy_page_display_info));
    let stats_info = preview_stats_info(state);
    let areas_split =
        Layout::vertical(vec![Constraint::Length(4), Constraint::Min(1)]).split(*area);
    let chart_area = areas_split[1].inner(ratatui::layout::Margin {
        horizontal: 0,
        vertical: 1,
    });

    let Some(selection_indexes) = node
        .selected_indexes
        .get(0..total_dims)
        .map(|indexes| indexes.to_vec())
    else {
        render_error(
            f,
            area,
            "Preview selection rank no longer matches the dataset rank".to_string(),
        );
        return Ok(());
    };
    let (chart_area, data_preview_selection) = if let Some(page_info) = page_info.as_ref() {
        let data_preview_selection = PreviewSelection {
            x: node.selected_x,
            index: selection_indexes.clone(),
            slice: SliceSelection::FromTo(page_info.range_start, page_info.range_end),
        };
        (chart_area, data_preview_selection)
    } else {
        let data_preview_selection = PreviewSelection {
            x: node.selected_x,
            index: selection_indexes,
            slice: SliceSelection::All,
        };

        (chart_area, data_preview_selection)
    };

    state
        .chart_preview_state
        .sync_selection_identity(&node.node.path(), &data_preview_selection);
    let x_min = preview_x_min(&state.page_state);
    let current_key = ChartPreviewKey {
        ds_path: node.node.path(),
        selection: data_preview_selection.clone(),
        mode: state.chart_preview_state.mode,
        x_axis_scale: state.chart_preview_state.x_axis_scale,
        y_axis_scale: state.chart_preview_state.y_axis_scale,
        viewport: state.chart_preview_state.viewport,
        roi: state.chart_preview_state.roi,
        histogram_selection: state.chart_preview_state.histogram_selection,
        histogram_range: state.chart_preview_state.histogram_range,
        width: chart_area.width,
        height: chart_area.height,
    };
    if !state.image_protocol_enabled {
        clear_active_chart_preview(state);
        let data_preview = match ds.plot(&data_preview_selection) {
            Ok(dp) => dp,
            Err(e) => {
                render_error(f, &chart_area, format!("Error plotting data: {}", e));
                return Ok(());
            }
        };
        sync_direct_chart_preview(state, chart_area, &data_preview, x_min);
        perf::metrics().preview.direct_widget_renders.increment();
        render_chart_widget(f, &chart_area, state, data_preview, x_min);
    } else {
        state.chart_preview_state.set_chart_area(Some(chart_area));
        state.chart_preview_state.set_plot_area(
            state
                .chart_preview_state
                .current_data
                .as_ref()
                .and_then(|data| {
                    preview_chart_plot_area(chart_area, state.image_cell_size, data.max)
                }),
        );
        queue_chart_preview_load(
            f,
            chart_area,
            state,
            &node.node,
            current_key,
            ChartPreviewSource::Dataset {
                ds,
                selection: data_preview_selection,
            },
        )?;
    }
    render_preview_context_panel(
        f,
        &areas_split[0],
        node,
        &shape,
        state,
        selector_info.as_ref(),
        stats_info.as_deref(),
        state.chart_preview_state.pending_key.is_some(),
    );

    Ok(())
}

fn render_projected_chart_preview(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    state: &mut AppState,
    ds: hdf5_metno::Dataset,
    ds_meta: crate::h5f::DatasetMeta,
) -> Result<(), AppError> {
    let _chart_render_timer = perf::metrics().preview.chart_render.start();
    let shape = ds.shape();
    let total_dims = shape.len();
    node.sync_selection_rank(total_dims);
    let selected_node = &node.node;
    let x_selectable_dims: Vec<usize> = shape
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 1)
        .map(|(i, _)| i)
        .collect();

    if x_selectable_dims.is_empty() {
        match ds_meta.matrixable {
            Some(MatrixRenderType::Float64) => {
                render_string(
                    f,
                    area,
                    node,
                    read_projected_scalar::<f64>(&ds, &ds_meta)?,
                    None,
                );
            }
            Some(MatrixRenderType::Uint64) => {
                render_string(
                    f,
                    area,
                    node,
                    read_projected_scalar::<u64>(&ds, &ds_meta)?,
                    None,
                );
            }
            Some(MatrixRenderType::Int64) => {
                render_string(
                    f,
                    area,
                    node,
                    read_projected_scalar::<i64>(&ds, &ds_meta)?,
                    None,
                );
            }
            Some(MatrixRenderType::Opaque) => {
                render_string_preview(f, area, node, state)?;
            }
            Some(MatrixRenderType::Enum) => {
                let hdf5_metno::types::TypeDescriptor::Enum(et) = &ds_meta.type_descriptor else {
                    render_error(
                        f,
                        area,
                        "Projected preview enum metadata is inconsistent with the field type"
                            .to_string(),
                    );
                    return Ok(());
                };
                let enum_renderer = EnumRenderer::with_overrides(
                    et.clone(),
                    ds_meta.enum_render_overrides.as_ref(),
                );
                let scalar_value = read_projected_scalar::<u64>(&ds, &ds_meta)?;
                let string = enum_renderer.render_as_line(&scalar_value);
                f.render_widget(
                    ratatui::widgets::Paragraph::new(string).style(
                        ratatui::style::Style::default()
                            .fg(crate::configure::themed_color(|colors| colors.text.primary)),
                    ),
                    *area,
                );
            }
            Some(MatrixRenderType::Strings) => {
                match read_projected_scalar::<String>(&ds, &ds_meta) {
                    Ok(value) => render_string(f, area, node, value, None),
                    Err(e) => render_error(f, area, format!("Error reading scalar string: {e}")),
                };
            }
            Some(MatrixRenderType::ByteArray) => render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Preview is only supported for vlen byte arrays when image attributes are present; use Matrix mode to inspect values",
            ),
            Some(MatrixRenderType::Compound) => render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Compound field containers are not previewable",
            ),
            None => render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Projected field is not previewable",
            ),
        }
        return Ok(());
    }

    for (i, selected_index) in node.selected_indexes.iter_mut().enumerate() {
        if !x_selectable_dims.contains(&i) {
            *selected_index = 0;
        }
    }

    if !x_selectable_dims.contains(&node.selected_x) {
        let Some(first_selectable_dim) = x_selectable_dims.first().copied() else {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Projected field is not previewable",
            );
            return Ok(());
        };
        node.selected_x = first_selectable_dim;
    }
    if node.selected_dim == node.selected_x {
        node.selected_dim = x_selectable_dims
            .iter()
            .find(|&&x| x != node.selected_x)
            .cloned()
            .unwrap_or(0);
    }

    let page_info = if shape[node.selected_x] > MAX_PAGE_SIZE {
        state.page_state.paged = PageType::Chart;
        state.page_state.page_count =
            (shape[node.selected_x] as f64 / MAX_PAGE_SIZE as f64).ceil() as i32;
        let max_len = shape[node.selected_x];
        let range_start = MAX_PAGE_SIZE * state.page_state.idx as usize;
        let range_end = (MAX_PAGE_SIZE * (state.page_state.idx + 1) as usize).min(max_len);
        Some(PageDisplayInfo {
            title: "Page",
            current: state.page_state.idx.max(0) as usize,
            total: state.page_state.page_count.max(0) as usize,
            range_start,
            range_end,
            total_items: max_len,
            unit: "pts",
        })
    } else {
        state.page_state.paged = PageType::Unpaged;
        state.page_state.page_count = 0;
        state.page_state.idx = 0;
        None
    };

    let selector_info = preview_view_info(state, shape[node.selected_x])
        .or_else(|| page_info.as_ref().map(copy_page_display_info));
    let stats_info = preview_stats_info(state);
    let areas_split =
        Layout::vertical(vec![Constraint::Length(4), Constraint::Min(1)]).split(*area);
    let chart_area = areas_split[1].inner(ratatui::layout::Margin {
        horizontal: 0,
        vertical: 1,
    });

    let Some(selection_indexes) = node
        .selected_indexes
        .get(0..total_dims)
        .map(|indexes| indexes.to_vec())
    else {
        render_error(
            f,
            area,
            "Projected preview selection rank no longer matches the dataset rank".to_string(),
        );
        return Ok(());
    };
    let (chart_area, data_preview_selection) = if let Some(page_info) = page_info.as_ref() {
        let data_preview_selection = PreviewSelection {
            x: node.selected_x,
            index: selection_indexes.clone(),
            slice: SliceSelection::FromTo(page_info.range_start, page_info.range_end),
        };
        (chart_area, data_preview_selection)
    } else {
        let data_preview_selection = PreviewSelection {
            x: node.selected_x,
            index: selection_indexes,
            slice: SliceSelection::All,
        };
        (chart_area, data_preview_selection)
    };
    state
        .chart_preview_state
        .sync_selection_identity(&node.node.path(), &data_preview_selection);
    let x_min = preview_x_min(&state.page_state);
    let current_key = ChartPreviewKey {
        ds_path: node.node.path(),
        selection: data_preview_selection.clone(),
        mode: state.chart_preview_state.mode,
        x_axis_scale: state.chart_preview_state.x_axis_scale,
        y_axis_scale: state.chart_preview_state.y_axis_scale,
        viewport: state.chart_preview_state.viewport,
        roi: state.chart_preview_state.roi,
        histogram_selection: state.chart_preview_state.histogram_selection,
        histogram_range: state.chart_preview_state.histogram_range,
        width: chart_area.width,
        height: chart_area.height,
    };
    if !state.image_protocol_enabled {
        clear_active_chart_preview(state);
        let data_preview = match plot_projected(&ds, &ds_meta, &data_preview_selection) {
            Ok(data_preview) => data_preview,
            Err(e) => {
                render_error(
                    f,
                    &chart_area,
                    format!("Error plotting projected field: {e}"),
                );
                return Ok(());
            }
        };
        sync_direct_chart_preview(state, chart_area, &data_preview, x_min);
        perf::metrics().preview.direct_widget_renders.increment();
        render_chart_widget(f, &chart_area, state, data_preview, x_min);
    } else {
        state.chart_preview_state.set_chart_area(Some(chart_area));
        state.chart_preview_state.set_plot_area(
            if state.chart_preview_state.mode.supports_roi()
                || state.chart_preview_state.mode == PreviewChartMode::Histogram
            {
                state
                    .chart_preview_state
                    .current_data
                    .as_ref()
                    .and_then(|data| {
                        preview_chart_plot_area(chart_area, state.image_cell_size, data.max)
                    })
            } else {
                None
            },
        );
        queue_chart_preview_load(
            f,
            chart_area,
            state,
            &node.node,
            current_key,
            ChartPreviewSource::ProjectedDataset {
                ds,
                meta: Box::new(ds_meta),
                selection: data_preview_selection,
            },
        )?;
    }
    render_preview_context_panel(
        f,
        &areas_split[0],
        node,
        &shape,
        state,
        selector_info.as_ref(),
        stats_info.as_deref(),
        state.chart_preview_state.pending_key.is_some(),
    );
    Ok(())
}

fn render_chart_widget(
    f: &mut Frame,
    chart_area: &Rect,
    state: &AppState,
    data_preview: DatasetPlotingData,
    x_min: f64,
) {
    let _widget_render_timer = perf::metrics().preview.chart_widget_render.start();
    let bounds = preview_chart_data_bounds(&data_preview, x_min);
    let viewport = state
        .chart_preview_state
        .effective_viewport()
        .or(bounds)
        .unwrap_or(PreviewChartViewport {
            x_min,
            x_max: x_min + preview_x_axis_max(&data_preview),
            y_min: data_preview.min,
            y_max: data_preview.max,
        });
    let mode = state.chart_preview_state.mode;
    if matches!(
        mode,
        PreviewChartMode::Histogram | PreviewChartMode::BoxPlot
    ) {
        let values = preview_windowed_values(
            &data_preview,
            viewport,
            x_min,
            state.chart_preview_state.histogram_range,
        );
        match mode {
            PreviewChartMode::Histogram => {
                let Some(summary) = histogram_summary(&values) else {
                    render_preview_summary_widget(
                        f,
                        chart_area,
                        "Histogram",
                        vec![Line::from("No finite values in the visible window.")],
                    );
                    return;
                };
                render_preview_summary_widget(
                    f,
                    chart_area,
                    "Histogram",
                    vec![
                        Line::from(format!(
                            "visible distribution: {} values, {} bins",
                            values.len(),
                            summary.bin_count
                        )),
                        Line::from(format!(
                            "range {:.4}..{:.4}",
                            summary.value_min, summary.value_max
                        )),
                        Line::from(format!("max bin count {:.0}", summary.count_max)),
                        Line::from("Image protocol renders the plotted histogram."),
                    ],
                );
            }
            PreviewChartMode::BoxPlot => {
                let Some(summary) = box_plot_summary(&values) else {
                    render_preview_summary_widget(
                        f,
                        chart_area,
                        "Box plot",
                        vec![Line::from("No finite values in the visible window.")],
                    );
                    return;
                };
                render_preview_summary_widget(
                    f,
                    chart_area,
                    "Box plot",
                    vec![
                        Line::from(format!("visible values {}", values.len())),
                        Line::from(format!(
                            "q1 {:.4}  median {:.4}  q3 {:.4}",
                            summary.q1, summary.median, summary.q3
                        )),
                        Line::from(format!(
                            "whiskers {:.4}..{:.4}  outliers {}",
                            summary.whisker_low,
                            summary.whisker_high,
                            summary.outliers.len()
                        )),
                        Line::from("Image protocol renders the plotted box plot."),
                    ],
                );
            }
            _ => {}
        }
        return;
    }
    let x_label_count = match chart_area.width {
        0..=7 => 1,
        _ => chart_area.width / 8,
    };
    let x_labels = (0..=x_label_count)
        .map(|i| {
            let x = viewport.x_min
                + (viewport.x_max - viewport.x_min) * (i as f64) / (x_label_count as f64);
            Span::styled(
                format_axis_number(x),
                configure::themed_color(|colors| colors.chart.label),
            )
        })
        .collect::<Vec<_>>();

    let y_label_count = match chart_area.height {
        0 => 0,
        _ => chart_area.height / 4,
    };

    let y_labels = (0..=y_label_count)
        .map(|i| {
            let y = viewport.y_min
                + (viewport.y_max - viewport.y_min) * (i as f64) / (y_label_count as f64);
            Span::styled(
                format_axis_number(y),
                configure::themed_color(|colors| colors.chart.label),
            )
        })
        .collect::<Vec<_>>();

    let data = data_preview
        .data
        .iter()
        .map(|(x, y)| (x_min + *x, *y))
        .collect::<Vec<_>>();
    let visible_points = preview_visible_points(&data_preview, viewport, x_min);
    let mut datasets = Vec::new();
    if matches!(mode, PreviewChartMode::Line) {
        datasets.push(
            Dataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.chart.preview_line))
                        .bold(),
                )
                .data(&data),
        );
    }
    if matches!(mode, PreviewChartMode::Scatter) {
        datasets.push(
            Dataset::default()
                .marker(Marker::Dot)
                .graph_type(GraphType::Scatter)
                .style(
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.chart.preview_line))
                        .bold(),
                )
                .data(&data),
        );
    } else if let Some(points) = visible_points.as_ref() {
        datasets.push(
            Dataset::default()
                .marker(Marker::Block)
                .graph_type(GraphType::Scatter)
                .style(
                    Style::default()
                        .fg(configure::themed_color(|colors| colors.chart.preview_line))
                        .bold(),
                )
                .data(points),
        );
    }
    let roi_storage = state.chart_preview_state.roi.and_then(|roi| {
        preview_roi_range(&data_preview, roi, x_min)
            .map(|(start, end)| (roi, data[start..=end].to_vec()))
    });
    if let Some((roi, roi_data)) = roi_storage.as_ref() {
        if roi.selection_count >= 2 && matches!(mode, PreviewChartMode::Line) {
            datasets.push(
                Dataset::default()
                    .marker(if roi.precise {
                        Marker::Dot
                    } else {
                        Marker::Braille
                    })
                    .graph_type(GraphType::Line)
                    .style(
                        Style::default()
                            .fg(configure::themed_color(|colors| {
                                colors.accent.selected_index
                            }))
                            .bold(),
                    )
                    .data(roi_data),
            );
        }
        if visible_points.is_some() || matches!(mode, PreviewChartMode::Scatter) {
            datasets.push(
                Dataset::default()
                    .marker(if matches!(mode, PreviewChartMode::Scatter) {
                        Marker::Dot
                    } else {
                        Marker::Block
                    })
                    .graph_type(GraphType::Scatter)
                    .style(
                        Style::default()
                            .fg(configure::themed_color(|colors| {
                                colors.accent.selected_index
                            }))
                            .bold(),
                    )
                    .data(roi_data),
            );
        }
    }
    let bg = match (&state.focus, &state.mode) {
        (
            Focus::Content,
            Mode::Normal
            | Mode::AttributeCreateDialog
            | Mode::AttributeDeleteDialog
            | Mode::FixedStringOverflowDialog
            | Mode::FixedStringResizeDialog,
        ) => configure::themed_color(|colors| colors.surface.focus_bg),
        _ => configure::themed_color(|colors| colors.surface.bg),
    };
    let chart = Chart::new(datasets)
        .style(Style::default().bg(bg))
        .x_axis(
            Axis::default()
                .title("X axis")
                .style(Style::default().fg(configure::themed_color(|colors| colors.chart.axis)))
                .labels(x_labels)
                .bounds((viewport.x_min, viewport.x_max).into()),
        )
        .y_axis(
            Axis::default()
                .title("Y axis")
                .style(Style::default().fg(configure::themed_color(|colors| colors.chart.axis)))
                .labels(y_labels)
                .bounds((viewport.y_min, viewport.y_max).into()),
        );
    f.render_widget(chart, *chart_area);
}

pub fn render_image_chart(
    buffer: &mut [u8],
    width: u32,
    height: u32,
    x_min: f64,
    data_preview: DatasetPlotingData,
    mode: PreviewChartMode,
    x_axis_scale: ChartAxisScale,
    y_axis_scale: ChartAxisScale,
    viewport: Option<PreviewChartViewport>,
    roi: Option<PreviewChartRoi>,
    histogram_selection: Option<crate::ui::state::PreviewHistogramSelection>,
    histogram_range: Option<crate::ui::state::PreviewHistogramRange>,
) -> Result<(), AppError> {
    let _image_render_timer = perf::metrics().preview.chart_image_render.start();
    let (bg_r, bg_g, bg_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.chart.plot_bg));
    let (grid_r, grid_g, grid_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.chart.grid));
    let (axis_r, axis_g, axis_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.chart.axis));
    let (line_r, line_g, line_b) =
        configure::rgb_channels(configure::themed_color(|colors| colors.chart.preview_line));
    let (selected_r, selected_g, selected_b) =
        configure::rgb_channels(configure::themed_color(|colors| {
            colors.accent.selected_index
        }));
    let plot_bg = RGBColor(bg_r, bg_g, bg_b);
    let grid = RGBColor(grid_r, grid_g, grid_b);
    let axis = RGBColor(axis_r, axis_g, axis_b);
    let line = RGBColor(line_r, line_g, line_b);
    let selected = RGBColor(selected_r, selected_g, selected_b);
    let roi_fill = line.mix(0.12);
    let roi_line = selected.mix(0.9);

    let bounds = preview_chart_data_bounds(&data_preview, x_min);
    let viewport = viewport.or(bounds).unwrap_or(PreviewChartViewport {
        x_min,
        x_max: x_min + preview_x_axis_max(&data_preview),
        y_min: data_preview.min,
        y_max: data_preview.max,
    });
    if matches!(mode, PreviewChartMode::Histogram) {
        let values = preview_windowed_values(&data_preview, viewport, x_min, histogram_range);
        let histogram_scale = if x_axis_scale == ChartAxisScale::Logarithmic
            && !values.iter().all(|value| *value > 0.0)
        {
            ChartAxisScale::Linear
        } else {
            x_axis_scale
        };
        let Some(summary) = histogram_summary_with_scale(&values, histogram_scale) else {
            return Err(AppError::DrawingError(
                "No finite values available for histogram preview".to_string(),
            ));
        };
        let root = BitMapBackend::with_buffer(buffer, (width, height)).into_drawing_area();
        root.fill(&plot_bg)
            .map_err(|e| AppError::DrawingError(format!("Error filling background: {}", e)))?;
        let layout = preview_chart_layout(width, height, summary.count_max);
        macro_rules! draw_histogram {
            ($x:expr) => {
                let mut chart = ChartBuilder::on(&root)
                    .margin(layout.margin)
                    .x_label_area_size(layout.x_label_area_size)
                    .y_label_area_size(layout.y_label_area_size)
                    .build_cartesian_2d($x, 0.0..summary.count_max)
                    .map_err(|e| AppError::DrawingError(format!("Error building chart: {}", e)))?;
                let boundaries = summary
                    .bins
                    .iter()
                    .map(|bin| bin.start)
                    .chain(summary.bins.last().map(|bin| bin.end))
                    .collect::<Vec<_>>();
                let tick_positions =
                    histogram_tick_indices(boundaries.len(), (width / 110).clamp(2, 7) as usize)
                        .into_iter()
                        .map(|index| {
                            (
                                chart.backend_coord(&(
                                    scale_value(histogram_scale, boundaries[index]),
                                    0.0,
                                )),
                                axis_label(
                                    histogram_scale,
                                    transformed_scale_value(histogram_scale, boundaries[index]),
                                ),
                            )
                        })
                        .collect::<Vec<_>>();
                chart
                    .configure_mesh()
                    .x_desc(format!("visible distribution ({} bins)", summary.bin_count))
                    .y_desc("count")
                    .disable_x_mesh()
                    .x_labels(0)
                    .y_label_formatter(&|value| format_axis_number(*value))
                    .x_label_style(
                        ("sans-serif", layout.x_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .y_label_style(
                        ("sans-serif", layout.y_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .axis_style(ShapeStyle::from(&axis).stroke_width(2))
                    .light_line_style(grid.mix(0.35))
                    .bold_line_style(grid.mix(0.55))
                    .draw()
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing histogram mesh: {}", e))
                    })?;
                chart
                    .draw_series(summary.bins.iter().enumerate().map(|(index, bin)| {
                        plotters::prelude::Rectangle::new(
                            [
                                (scale_value(histogram_scale, bin.start), 0.0),
                                (scale_value(histogram_scale, bin.end), bin.count),
                            ],
                            if let Some(selection) = histogram_selection {
                                if index >= selection.start && index <= selection.end {
                                    selected.mix(0.7).filled()
                                } else {
                                    line.mix(0.45).filled()
                                }
                            } else {
                                line.mix(0.45).filled()
                            },
                        )
                    }))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing histogram bars: {}", e))
                    })?;
                chart
                    .draw_series(summary.bins.iter().map(|bin| {
                        plotters::prelude::Rectangle::new(
                            [
                                (scale_value(histogram_scale, bin.start), 0.0),
                                (scale_value(histogram_scale, bin.end), bin.count),
                            ],
                            ShapeStyle::from(&line.mix(0.9)).stroke_width(1),
                        )
                    }))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error outlining histogram bars: {}", e))
                    })?;
                drop(chart);
                for ((x, y), label) in tick_positions {
                    root.draw(&plotters::element::PathElement::new(
                        vec![(x, y), (x, y + 5)],
                        ShapeStyle::from(&axis).stroke_width(2),
                    ))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing histogram tick mark: {}", e))
                    })?;
                    root.draw(&Text::new(
                        label,
                        (x, y + 5),
                        ("sans-serif", layout.x_label_font_size)
                            .into_font()
                            .color(&axis)
                            .pos(Pos::new(HPos::Center, VPos::Top)),
                    ))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing histogram ticks: {}", e))
                    })?;
                }
            };
        }
        if histogram_scale == ChartAxisScale::SymLog {
            draw_histogram!(symlog(summary.value_min)..symlog(summary.value_max));
        } else if histogram_scale == ChartAxisScale::Logarithmic
            && valid_log_range(summary.value_min, summary.value_max)
        {
            draw_histogram!((summary.value_min..summary.value_max).log_scale());
        } else {
            draw_histogram!(summary.value_min..summary.value_max);
        }
        root.present()
            .map_err(|e| AppError::DrawingError(format!("Error presenting chart: {}", e)))?;
        return Ok(());
    }
    if matches!(mode, PreviewChartMode::BoxPlot) {
        let values = preview_windowed_values(&data_preview, viewport, x_min, None);
        let Some(summary) = box_plot_summary(&values) else {
            return Err(AppError::DrawingError(
                "No finite values available for box-plot preview".to_string(),
            ));
        };
        let root = BitMapBackend::with_buffer(buffer, (width, height)).into_drawing_area();
        root.fill(&plot_bg)
            .map_err(|e| AppError::DrawingError(format!("Error filling background: {}", e)))?;
        let layout = preview_chart_layout(width, height, summary.value_max);
        macro_rules! draw_box_plot {
            ($y:expr) => {
                let mut chart = ChartBuilder::on(&root)
                    .margin(layout.margin)
                    .x_label_area_size(layout.x_label_area_size)
                    .y_label_area_size(layout.y_label_area_size)
                    .build_cartesian_2d(0.5..1.5, $y)
                    .map_err(|e| AppError::DrawingError(format!("Error building chart: {}", e)))?;
                chart
                    .configure_mesh()
                    .x_desc("visible window")
                    .y_desc("value")
                    .x_labels(1)
                    .x_label_formatter(&|_| "series".to_string())
                    .y_label_formatter(&|value| axis_label(y_axis_scale, *value))
                    .x_label_style(
                        ("sans-serif", layout.x_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .y_label_style(
                        ("sans-serif", layout.y_label_font_size)
                            .into_font()
                            .color(&axis),
                    )
                    .axis_style(ShapeStyle::from(&axis).stroke_width(2))
                    .light_line_style(grid.mix(0.35))
                    .bold_line_style(grid.mix(0.55))
                    .draw()
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing box-plot mesh: {}", e))
                    })?;
                let value = |value| scale_value(y_axis_scale, value);
                let x = 1.0;
                let half_width = 0.2;
                chart
                    .draw_series(std::iter::once(plotters::element::PathElement::new(
                        vec![(x, value(summary.whisker_low)), (x, value(summary.q1))],
                        ShapeStyle::from(&line).stroke_width(2),
                    )))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing lower whisker: {}", e))
                    })?;
                chart
                    .draw_series(std::iter::once(plotters::element::PathElement::new(
                        vec![(x, value(summary.q3)), (x, value(summary.whisker_high))],
                        ShapeStyle::from(&line).stroke_width(2),
                    )))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing upper whisker: {}", e))
                    })?;
                chart
                    .draw_series(std::iter::once(plotters::prelude::Rectangle::new(
                        [
                            (x - half_width, value(summary.q1)),
                            (x + half_width, value(summary.q3)),
                        ],
                        line.mix(0.25).filled(),
                    )))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing box body: {}", e))
                    })?;
                chart
                    .draw_series(std::iter::once(plotters::prelude::Rectangle::new(
                        [
                            (x - half_width, value(summary.q1)),
                            (x + half_width, value(summary.q3)),
                        ],
                        ShapeStyle::from(&line).stroke_width(2),
                    )))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing box outline: {}", e))
                    })?;
                chart
                    .draw_series(std::iter::once(plotters::element::PathElement::new(
                        vec![
                            (x - half_width, value(summary.median)),
                            (x + half_width, value(summary.median)),
                        ],
                        ShapeStyle::from(&selected).stroke_width(3),
                    )))
                    .map_err(|e| AppError::DrawingError(format!("Error drawing median: {}", e)))?;
                chart
                    .draw_series(summary.outliers.iter().map(|outlier| {
                        plotters::element::Circle::new(
                            (x, value(*outlier)),
                            4,
                            ShapeStyle::from(&selected).filled(),
                        )
                    }))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing outliers: {}", e))
                    })?;
            };
        }
        if y_axis_scale == ChartAxisScale::SymLog {
            draw_box_plot!(symlog(summary.value_min)..symlog(summary.value_max));
        } else if y_axis_scale == ChartAxisScale::Logarithmic {
            let (raw_min, raw_max) = summary.outliers.iter().copied().fold(
                (summary.whisker_low, summary.whisker_high),
                |(min, max), value| (min.min(value), max.max(value)),
            );
            if let Some((min, max)) = normalized_log_axis_bounds(raw_min, raw_max) {
                draw_box_plot!((min..max).log_scale());
            } else {
                draw_box_plot!(summary.value_min..summary.value_max);
            }
        } else {
            draw_box_plot!(summary.value_min..summary.value_max);
        }
        root.present()
            .map_err(|e| AppError::DrawingError(format!("Error presenting chart: {}", e)))?;
        return Ok(());
    }
    let root = BitMapBackend::with_buffer(buffer, (width, height)).into_drawing_area();
    root.margin(10, 10, 10, 10);
    root.fill(&plot_bg)
        .map_err(|e| AppError::DrawingError(format!("Error filling background: {}", e)))?;
    let layout = preview_chart_layout(width, height, data_preview.max);
    let x_log_bounds = (x_axis_scale == ChartAxisScale::Logarithmic)
        .then(|| preview_effective_x_domain(&data_preview, viewport, x_min, true))
        .flatten();

    macro_rules! draw_xy_chart {
        ($x:expr, $y:expr, $log_x:expr, $x_scale:expr, $y_scale:expr) => {{
            let mut chart = ChartBuilder::on(&root)
                .margin(layout.margin)
                .x_label_area_size(layout.x_label_area_size)
                .y_label_area_size(layout.y_label_area_size)
                .build_cartesian_2d($x, $y)
                .map_err(|e| AppError::DrawingError(format!("Error building chart: {}", e)))?;

            chart
                .configure_mesh()
                .x_label_formatter(&|value| axis_label($x_scale, *value))
                .y_label_formatter(&|value| axis_label($y_scale, *value))
                .x_label_style(
                    ("sans-serif", layout.x_label_font_size)
                        .into_font()
                        .color(&axis),
                )
                .y_label_style(
                    ("sans-serif", layout.y_label_font_size)
                        .into_font()
                        .color(&axis),
                )
                .axis_style(ShapeStyle::from(&axis).stroke_width(2))
                .light_line_style(grid.mix(0.35))
                .bold_line_style(grid.mix(0.55))
                .draw()
                .map_err(|e| AppError::DrawingError(format!("Error drawing mesh: {}", e)))?;

            let data = data_preview
                .data
                .iter()
                .map(|(x, y)| {
                    (
                        transformed_scale_value($x_scale, x_min + *x),
                        transformed_scale_value($y_scale, *y),
                    )
                })
                .collect::<Vec<_>>();
            if matches!(mode, PreviewChartMode::Line) {
                let line_series = plotters::prelude::LineSeries::new(
                    data.iter().copied().filter(|(x, _)| !$log_x || *x > 0.0),
                    ShapeStyle::from(&line).stroke_width(3),
                );
                chart.draw_series(line_series).map_err(|e| {
                    AppError::DrawingError(format!("Error drawing line series: {}", e))
                })?;
            }
            if matches!(mode, PreviewChartMode::Scatter) {
                chart
                    .draw_series(
                        data.iter()
                            .copied()
                            .filter(|(x, _)| !$log_x || *x > 0.0)
                            .map(|point| {
                                plotters::prelude::Circle::new(
                                    point,
                                    3,
                                    ShapeStyle::from(&line).filled(),
                                )
                            }),
                    )
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing scatter points: {}", e))
                    })?;
            } else if let Some(points) = preview_visible_points(&data_preview, viewport, x_min) {
                chart
                    .draw_series(points.into_iter().filter(|(x, _)| !$log_x || *x > 0.0).map(
                        |(x, y)| {
                            plotters::prelude::Circle::new(
                                (
                                    transformed_scale_value($x_scale, x),
                                    transformed_scale_value($y_scale, y),
                                ),
                                PREVIEW_POINT_MARKER_RADIUS,
                                ShapeStyle::from(&line).filled(),
                            )
                        },
                    ))
                    .map_err(|e| {
                        AppError::DrawingError(format!("Error drawing point markers: {}", e))
                    })?;
            }
            if let Some(roi) = roi {
                if let Some((start, end)) = preview_roi_range(&data_preview, roi, x_min) {
                    if !roi.precise {
                        if let Some((x0, x1)) =
                            preview_roi_x_bounds(&data_preview, start, end, x_min)
                        {
                            if !$log_x || (x0 > 0.0 && x1 > 0.0) {
                                chart
                                    .draw_series(std::iter::once(
                                        plotters::prelude::Rectangle::new(
                                            [
                                                (
                                                    transformed_scale_value($x_scale, x0),
                                                    transformed_scale_value(
                                                        $y_scale,
                                                        viewport.y_min,
                                                    ),
                                                ),
                                                (
                                                    transformed_scale_value($x_scale, x1),
                                                    transformed_scale_value(
                                                        $y_scale,
                                                        viewport.y_max,
                                                    ),
                                                ),
                                            ],
                                            roi_fill.filled(),
                                        ),
                                    ))
                                    .map_err(|e| {
                                        AppError::DrawingError(format!(
                                            "Error drawing roi fill: {}",
                                            e
                                        ))
                                    })?;
                            }
                        }
                    }
                    let roi_points = data[start..=end]
                        .iter()
                        .copied()
                        .filter(|(x, _)| !$log_x || *x > 0.0)
                        .map(|(x, y)| {
                            (
                                transformed_scale_value($x_scale, x),
                                transformed_scale_value($y_scale, y),
                            )
                        })
                        .collect::<Vec<_>>();
                    if roi.selection_count >= 2 && matches!(mode, PreviewChartMode::Line) {
                        chart
                            .draw_series(plotters::prelude::LineSeries::new(
                                roi_points.iter().copied(),
                                ShapeStyle::from(&roi_line).stroke_width(5),
                            ))
                            .map_err(|e| {
                                AppError::DrawingError(format!("Error drawing roi line: {}", e))
                            })?;
                    }
                    if preview_visible_points(&data_preview, viewport, x_min).is_some()
                        || matches!(mode, PreviewChartMode::Scatter)
                    {
                        chart
                            .draw_series(roi_points.into_iter().map(|point| {
                                plotters::prelude::Circle::new(
                                    point,
                                    PREVIEW_SELECTED_POINT_MARKER_RADIUS,
                                    ShapeStyle::from(&roi_line).filled(),
                                )
                            }))
                            .map_err(|e| {
                                AppError::DrawingError(format!("Error drawing roi points: {}", e))
                            })?;
                    }
                }
            }
        }};
    }
    if x_axis_scale == ChartAxisScale::SymLog || y_axis_scale == ChartAxisScale::SymLog {
        draw_xy_chart!(
            transformed_scale_value(x_axis_scale, viewport.x_min)
                ..transformed_scale_value(x_axis_scale, viewport.x_max),
            transformed_scale_value(y_axis_scale, viewport.y_min)
                ..transformed_scale_value(y_axis_scale, viewport.y_max),
            false,
            x_axis_scale,
            y_axis_scale
        );
    } else {
        match (
            x_log_bounds,
            y_axis_scale == ChartAxisScale::Logarithmic
                && valid_log_range(viewport.y_min, viewport.y_max),
        ) {
            (None, false) => draw_xy_chart!(
                viewport.x_min..viewport.x_max,
                viewport.y_min..viewport.y_max,
                false,
                ChartAxisScale::Linear,
                ChartAxisScale::Linear
            ),
            (Some((x_min, x_max)), false) => draw_xy_chart!(
                (x_min..x_max).log_scale(),
                viewport.y_min..viewport.y_max,
                true,
                ChartAxisScale::Linear,
                ChartAxisScale::Linear
            ),
            (None, true) => draw_xy_chart!(
                viewport.x_min..viewport.x_max,
                (viewport.y_min..viewport.y_max).log_scale(),
                false,
                ChartAxisScale::Linear,
                ChartAxisScale::Linear
            ),
            (Some((x_min, x_max)), true) => draw_xy_chart!(
                (x_min..x_max).log_scale(),
                (viewport.y_min..viewport.y_max).log_scale(),
                true,
                ChartAxisScale::Linear,
                ChartAxisScale::Linear
            ),
        }
    }
    root.present()
        .map_err(|e| AppError::DrawingError(format!("Error presenting chart: {}", e)))?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        preview_chart_plot_area, preview_effective_x_domain, preview_visible_points,
        preview_windowed_values, preview_x_axis_max, render_image_chart,
    };
    use crate::data::DatasetPlotingData;
    use crate::ui::{
        mchart::ChartAxisScale,
        state::{
            PreviewChartMode, PreviewChartViewport, PreviewHistogramRange,
            PREVIEW_CHART_VISIBLE_POINT_LIMIT,
        },
    };
    use ratatui::layout::Rect;

    fn sample_preview(len: usize) -> DatasetPlotingData {
        DatasetPlotingData {
            data: (0..len).map(|i| (i as f64, i as f64)).collect(),
            length: len,
            max: len.saturating_sub(1) as f64,
            min: 0.0,
        }
    }

    #[test]
    fn preview_x_axis_max_uses_last_point_index_for_multiple_points() {
        let preview = DatasetPlotingData {
            data: vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
            length: 3,
            max: 3.0,
            min: 1.0,
        };
        assert_eq!(preview_x_axis_max(&preview), 2.0);
    }

    #[test]
    fn histogram_range_limits_windowed_values() {
        let preview = sample_preview(5);
        let viewport = PreviewChartViewport {
            x_min: 0.0,
            x_max: 4.0,
            y_min: 0.0,
            y_max: 4.0,
        };

        assert_eq!(
            preview_windowed_values(
                &preview,
                viewport,
                0.0,
                Some(PreviewHistogramRange { min: 1.0, max: 3.0 }),
            ),
            vec![1.0, 2.0, 3.0]
        );
    }

    #[test]
    fn preview_log_x_ignores_raw_index_zero() {
        let preview = sample_preview(3);
        let viewport = PreviewChartViewport {
            x_min: 0.0,
            x_max: 2.0,
            y_min: 0.0,
            y_max: 2.0,
        };
        assert_eq!(
            preview_effective_x_domain(&preview, viewport, 0.0, true),
            Some((1.0, 2.0))
        );

        let mut buffer = vec![0; 64 * 64 * 3];
        assert!(render_image_chart(
            &mut buffer,
            64,
            64,
            0.0,
            preview,
            PreviewChartMode::Line,
            ChartAxisScale::Logarithmic,
            ChartAxisScale::Linear,
            Some(viewport),
            None,
            None,
            None,
        )
        .is_ok());
    }

    #[test]
    fn preview_x_axis_max_keeps_single_point_visible() {
        let preview = DatasetPlotingData {
            data: vec![(0.0, 1.0)],
            length: 1,
            max: 1.0,
            min: 1.0,
        };
        assert_eq!(preview_x_axis_max(&preview), 1.0);
    }

    #[test]
    fn preview_x_axis_max_uses_original_length_for_nonconsecutive_points() {
        let preview = DatasetPlotingData {
            data: vec![(0.0, 1.0), (4.0, 2.0), (8.0, 3.0)],
            length: 10,
            max: 3.0,
            min: 1.0,
        };
        assert_eq!(preview_x_axis_max(&preview), 9.0);
    }

    #[test]
    fn preview_visible_points_only_render_under_threshold() {
        let preview = sample_preview(PREVIEW_CHART_VISIBLE_POINT_LIMIT + 5);
        let viewport = PreviewChartViewport {
            x_min: 0.0,
            x_max: (PREVIEW_CHART_VISIBLE_POINT_LIMIT + 4) as f64,
            y_min: 0.0,
            y_max: (PREVIEW_CHART_VISIBLE_POINT_LIMIT + 4) as f64,
        };
        assert!(preview_visible_points(&preview, viewport, 0.0).is_none());

        let zoomed = PreviewChartViewport {
            x_min: 5.0,
            x_max: 5.0 + PREVIEW_CHART_VISIBLE_POINT_LIMIT as f64 - 1.0,
            y_min: 0.0,
            y_max: (PREVIEW_CHART_VISIBLE_POINT_LIMIT + 4) as f64,
        };
        assert_eq!(
            preview_visible_points(&preview, zoomed, 0.0).unwrap().len(),
            PREVIEW_CHART_VISIBLE_POINT_LIMIT
        );
    }

    #[test]
    fn preview_chart_plot_area_accounts_for_axis_offsets() {
        let plot_area =
            preview_chart_plot_area(Rect::new(10, 4, 40, 20), (8, 16), 1234.0).expect("plot area");
        assert!(plot_area.x > 10);
        assert!(plot_area.y >= 4);
        assert!(plot_area.width < 40);
        assert!(plot_area.height < 20);
    }

    #[test]
    fn preview_chart_plot_area_keeps_multiple_rows_in_short_panels() {
        let plot_area =
            preview_chart_plot_area(Rect::new(0, 0, 40, 4), (8, 16), 1234.0).expect("plot area");
        assert!(plot_area.height >= 2);
    }
}
