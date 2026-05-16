use hdf5_metno::types::TypeDescriptor;
use plotters::{
    chart::ChartBuilder,
    prelude::{BitMapBackend, IntoDrawingArea},
    style::{Color as _, IntoFont, RGBColor, ShapeStyle},
};

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    symbols::Marker,
    text::Span,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
    Frame,
};
use ratatui_image::StatefulImage;

use crate::{
    configure,
    data::{DatasetPlotingData, PreviewSelection, Previewable, SliceSelection},
    error::AppError,
    h5f::{
        plot_projected, read_projected_scalar, read_single_value_dataset, H5FNode, HasPath, Node,
    },
    ui::{
        dims::render_dim_selector,
        matrix::{EnumRenderer, RenderIntercept},
        page_scroll::PageDisplayInfo,
        perf,
        preview::image::thread_protocol_from_clipboard_image,
        preview::render_string_preview,
        render::MatrixRenderType,
        state::{
            AppState, ChartPreviewKey, ChartPreviewLoadRequest, ChartPreviewSource, Focus, Mode,
            PageType, PreviewChartViewport,
        },
        std_comp_render::{render_error, render_string, render_unsupported_rendering},
    },
};

pub const MAX_PAGE_SIZE: usize = 2_500_000;

fn render_chart_loading_indicator(f: &mut Frame, area: Rect) {
    let indicator = Block::default()
        .title(Span::styled(
            " * ",
            Style::default().fg(configure::themed_color(|colors| colors.help.description)),
        ))
        .title_alignment(Alignment::Right);
    f.render_widget(indicator, area);
}

fn clear_active_chart_preview(state: &mut AppState<'_>) {
    state.chart_preview_state.ds_loaded = None;
    state.chart_preview_state.protocol = None;
    state.chart_preview_state.clipboard_image = None;
    state.chart_preview_state.error = None;
    state.chart_preview_state.ds_selection = None;
    state.chart_preview_state.rendered_viewport = None;
    state.chart_preview_state.pending_key = None;
    state.chart_preview_state.reset_viewport();
}

fn normalized_axis_bounds(min: f64, max: f64) -> Option<(f64, f64)> {
    if !min.is_finite() || !max.is_finite() || max < min {
        return None;
    }
    if (max - min).abs() < f64::EPSILON {
        let pad = if min == 0.0 {
            1.0
        } else {
            min.abs().max(1.0) * 0.05
        };
        return Some((min - pad, max + pad));
    }
    Some((min, max))
}

fn preview_x_min(page_state: &crate::ui::state::PageState) -> f64 {
    if req_matches_paged_chart(page_state) {
        MAX_PAGE_SIZE as f64 * page_state.idx.max(0) as f64
    } else {
        0.0
    }
}

fn req_matches_paged_chart(page_state: &crate::ui::state::PageState) -> bool {
    matches!(page_state.paged, PageType::Chart) && page_state.idx > 0
}

pub(crate) fn preview_chart_data_bounds(
    data_preview: &DatasetPlotingData,
    x_min: f64,
) -> Option<PreviewChartViewport> {
    let (x_min, x_max) = normalized_axis_bounds(x_min, x_min + preview_x_axis_max(data_preview))?;
    let (y_min, y_max) = normalized_axis_bounds(data_preview.min, data_preview.max)?;
    Some(PreviewChartViewport {
        x_min,
        x_max,
        y_min,
        y_max,
    })
}

fn preview_view_info(state: &AppState, total_items: usize) -> Option<PageDisplayInfo<'static>> {
    if !state.chart_preview_state.has_explicit_viewport() {
        return None;
    }
    let viewport = state.chart_preview_state.viewport?;
    let range_start = viewport.x_min.floor().max(0.0) as usize;
    let range_end = (viewport.x_max.ceil().max(viewport.x_min) as usize)
        .saturating_add(1)
        .min(total_items.max(1));
    Some(PageDisplayInfo {
        title: "View",
        current: 0,
        total: 1,
        range_start,
        range_end: range_end.max(range_start.saturating_add(1).min(total_items.max(1))),
        total_items: total_items.max(1),
        unit: "pts",
    })
}

fn copy_page_display_info(info: &PageDisplayInfo<'static>) -> PageDisplayInfo<'static> {
    PageDisplayInfo {
        title: info.title,
        current: info.current,
        total: info.total,
        range_start: info.range_start,
        range_end: info.range_end,
        total_items: info.total_items,
        unit: info.unit,
    }
}

fn render_chart_protocol_state(
    f: &mut Frame,
    chart_area: Rect,
    state: &mut AppState<'_>,
    is_pending: bool,
) -> Result<(), AppError> {
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
        if is_pending {
            render_chart_loading_indicator(f, chart_area);
        }
    } else if is_pending {
        render_chart_loading_indicator(f, chart_area);
    }
    Ok(())
}

fn restore_cached_chart_preview(state: &mut AppState<'_>, key: &ChartPreviewKey) -> bool {
    let Some((clipboard_image, data_bounds)) = state.chart_preview_state.touch_cached_preview(key)
    else {
        return false;
    };
    let Some(protocol) = thread_protocol_from_clipboard_image(
        &state.multi_chart.picker,
        &state.chart_preview_state.tx_resize_chartpreview,
        &clipboard_image,
    ) else {
        state
            .chart_preview_state
            .cached_previews
            .retain(|entry| entry.key != *key);
        return false;
    };
    state.chart_preview_state.ds_loaded = Some(key.ds_path.clone());
    state.chart_preview_state.ds_selection = Some(key.selection.clone());
    state.chart_preview_state.rendered_viewport = key.viewport;
    state.chart_preview_state.protocol = Some(protocol);
    state.chart_preview_state.clipboard_image = Some(clipboard_image);
    state.chart_preview_state.error = None;
    state
        .chart_preview_state
        .sync_data_bounds(Some(data_bounds));
    true
}

fn queue_chart_preview_load(
    f: &mut Frame,
    chart_area: Rect,
    state: &mut AppState<'_>,
    node: &Node,
    current_key: ChartPreviewKey,
    source: ChartPreviewSource,
) -> Result<(), AppError> {
    let is_pending = state.chart_preview_state.pending_key.as_ref() == Some(&current_key);
    let chart_loaded =
        state.chart_preview_state.current_request_key().as_ref() == Some(&current_key);

    if state.should_debounce_preview(node) {
        perf::metrics().preview.debounce_skips.increment();
        if !chart_loaded && !restore_cached_chart_preview(state, &current_key) {
            clear_active_chart_preview(state);
        }
        return render_chart_protocol_state(f, chart_area, state, true);
    }

    if chart_loaded {
        perf::metrics().preview.cache_hits.increment();
        return render_chart_protocol_state(f, chart_area, state, is_pending);
    }

    if restore_cached_chart_preview(state, &current_key) {
        perf::metrics().preview.cache_hits.increment();
        return render_chart_protocol_state(f, chart_area, state, false);
    }

    state.chart_preview_state.begin_loading(current_key.clone());
    state
        .chart_preview_state
        .tx_load_chartpreview
        .send(ChartPreviewLoadRequest {
            key: current_key,
            source,
            width: chart_area.width,
            height: chart_area.height,
            page_state: state.page_state.clone(),
        })
        .ok();
    perf::metrics().preview.requests_queued.increment();
    render_chart_protocol_state(f, chart_area, state, true)
}

pub fn render_precomputed_chart_preview(
    f: &mut Frame,
    area: &Rect,
    node: &mut H5FNode,
    state: &mut AppState,
    data_preview: DatasetPlotingData,
) -> Result<(), AppError> {
    let _chart_render_timer = perf::metrics().preview.chart_render.start();
    state.chart_preview_state.set_chart_area(None);
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
    state.chart_preview_state.set_chart_area(Some(chart_area));
    let x_min = preview_x_min(&state.page_state);
    if !state.image_protocol_enabled {
        clear_active_chart_preview(state);
        state
            .chart_preview_state
            .sync_data_bounds(preview_chart_data_bounds(&data_preview, x_min));
        perf::metrics().preview.direct_widget_renders.increment();
        render_chart_widget(f, &chart_area, state, data_preview, x_min);
        return Ok(());
    }

    let current_key = ChartPreviewKey {
        ds_path: node.node.path(),
        selection: preview_selection.clone(),
        viewport: state.chart_preview_state.viewport,
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
    state.chart_preview_state.set_chart_area(None);
    let (ds, ds_meta) = match &node.node {
        Node::Dataset(ds, attr) => (ds.clone(), attr.clone()),
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
        return render_string_preview(f, area, node);
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
                    render_string_preview(f, area, node)?;
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
                    render_string_preview(f, area, node)?;
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
    let areas_split =
        Layout::vertical(vec![Constraint::Length(4), Constraint::Min(1)]).split(*area);
    render_dim_selector(
        f,
        &areas_split[0],
        node,
        &shape,
        false,
        selector_info.as_ref(),
    )?;
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
    state.chart_preview_state.set_chart_area(Some(chart_area));
    let x_min = preview_x_min(&state.page_state);
    let current_key = ChartPreviewKey {
        ds_path: node.node.path(),
        selection: data_preview_selection.clone(),
        viewport: state.chart_preview_state.viewport,
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
        state
            .chart_preview_state
            .sync_data_bounds(preview_chart_data_bounds(&data_preview, x_min));
        perf::metrics().preview.direct_widget_renders.increment();
        render_chart_widget(f, &chart_area, state, data_preview, x_min);
    } else {
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
                render_string_preview(f, area, node)?;
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
    let areas_split =
        Layout::vertical(vec![Constraint::Length(4), Constraint::Min(1)]).split(*area);
    render_dim_selector(
        f,
        &areas_split[0],
        node,
        &shape,
        false,
        selector_info.as_ref(),
    )?;
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
    state.chart_preview_state.set_chart_area(Some(chart_area));
    let x_min = preview_x_min(&state.page_state);
    let current_key = ChartPreviewKey {
        ds_path: node.node.path(),
        selection: data_preview_selection.clone(),
        viewport: state.chart_preview_state.viewport,
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
        state
            .chart_preview_state
            .sync_data_bounds(preview_chart_data_bounds(&data_preview, x_min));
        perf::metrics().preview.direct_widget_renders.increment();
        render_chart_widget(f, &chart_area, state, data_preview, x_min);
    } else {
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
    let x_label_count = match chart_area.width {
        0..=7 => 1,
        _ => chart_area.width / 8,
    };
    let x_labels = (0..=x_label_count)
        .map(|i| {
            let x = viewport.x_min
                + (viewport.x_max - viewport.x_min) * (i as f64) / (x_label_count as f64);
            Span::styled(
                format!("{:.1}", x),
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
                format!("{:.1}", y),
                configure::themed_color(|colors| colors.chart.label),
            )
        })
        .collect::<Vec<_>>();

    let data = data_preview
        .data
        .iter()
        .map(|(x, y)| (x_min + *x, *y))
        .collect::<Vec<_>>();
    let ds = Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(
            Style::default()
                .fg(configure::themed_color(|colors| colors.chart.preview_line))
                .bold(),
        )
        .data(&data);
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
    let chart = Chart::new(vec![ds])
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
    viewport: Option<PreviewChartViewport>,
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
    let plot_bg = RGBColor(bg_r, bg_g, bg_b);
    let grid = RGBColor(grid_r, grid_g, grid_b);
    let axis = RGBColor(axis_r, axis_g, axis_b);
    let line = RGBColor(line_r, line_g, line_b);

    let bounds = preview_chart_data_bounds(&data_preview, x_min);
    let viewport = viewport.or(bounds).unwrap_or(PreviewChartViewport {
        x_min,
        x_max: x_min + preview_x_axis_max(&data_preview),
        y_min: data_preview.min,
        y_max: data_preview.max,
    });
    let root = BitMapBackend::with_buffer(buffer, (width, height)).into_drawing_area();
    root.margin(10, 10, 10, 10);
    root.fill(&plot_bg)
        .map_err(|e| AppError::DrawingError(format!("Error filling background: {}", e)))?;
    let max = data_preview.max;
    let y_label_area_size = format!("{max:.4}").len() as u32 * 3 + 30;

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(y_label_area_size)
        .build_cartesian_2d(
            viewport.x_min..viewport.x_max,
            viewport.y_min..viewport.y_max,
        )
        .map_err(|e| AppError::DrawingError(format!("Error building chart: {}", e)))?;

    // Draw the mesh (grid lines)
    chart
        .configure_mesh()
        .x_label_style(("sans-serif", 18).into_font().color(&axis))
        .y_label_style(("sans-serif", 18).into_font().color(&axis))
        .axis_style(ShapeStyle::from(&axis).stroke_width(2))
        .light_line_style(grid.mix(0.35))
        .bold_line_style(grid.mix(0.55))
        .draw()
        .map_err(|e| AppError::DrawingError(format!("Error drawing mesh: {}", e)))?;

    let data = data_preview.data.iter().map(|(x, y)| (x_min + *x, *y));
    let line_series =
        plotters::prelude::LineSeries::new(data, ShapeStyle::from(&line).stroke_width(3));
    chart
        .draw_series(line_series)
        .map_err(|e| AppError::DrawingError(format!("Error drawing line series: {}", e)))?;
    root.present()
        .map_err(|e| AppError::DrawingError(format!("Error presenting chart: {}", e)))?;
    Ok(())
}

fn preview_x_axis_max(data_preview: &DatasetPlotingData) -> f64 {
    match data_preview.length {
        0 | 1 => 1.0,
        len => (len - 1) as f64,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::preview_x_axis_max;
    use crate::data::DatasetPlotingData;

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
}
