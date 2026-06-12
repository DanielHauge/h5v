use image::imageops::FilterType;
use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::Span,
    widgets::{Block, Clear},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

use crate::{
    configure,
    error::AppError,
    h5f::Node,
    ui::{
        perf,
        preview::image::thread_protocol_from_clipboard_image,
        state::{AppState, ChartPreviewKey, ChartPreviewLoadRequest, ChartPreviewSource},
        std_comp_render::render_error,
    },
};

fn render_chart_loading_indicator(f: &mut Frame<'_>, area: Rect) {
    let indicator = Block::default()
        .title(Span::styled(
            configure::configured_symbol(|symbols| symbols.chart.loading_indicator),
            Style::default().fg(configure::themed_color(|colors| colors.help.description)),
        ))
        .title_alignment(Alignment::Right);
    f.render_widget(indicator, area);
}

pub(super) fn clear_active_chart_preview(state: &mut AppState<'_>) {
    state.chart_preview_state.ds_loaded = None;
    state.chart_preview_state.protocol = None;
    state.chart_preview_state.clipboard_image = None;
    state.chart_preview_state.error = None;
    state.chart_preview_state.ds_selection = None;
    state.chart_preview_state.rendered_viewport = None;
    state.chart_preview_state.rendered_roi = None;
    state.chart_preview_state.rendered_size = None;
    state.chart_preview_state.pending_key = None;
    state.chart_preview_state.reset_viewport();
}

pub(super) fn render_chart_protocol_state(
    f: &mut Frame<'_>,
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
        f.render_widget(Clear, chart_area);
        let chart_widget =
            StatefulImage::default().resize(Resize::Scale(Some(FilterType::Triangle)));
        f.render_stateful_widget(chart_widget, chart_area, protocol);
        if is_pending {
            render_chart_loading_indicator(f, chart_area);
        }
    } else if is_pending {
        render_chart_loading_indicator(f, chart_area);
    }
    Ok(())
}

fn restore_cached_chart_preview(state: &mut AppState<'_>, key: &ChartPreviewKey) -> bool {
    let Some((clipboard_image, data_bounds, data_preview)) =
        state.chart_preview_state.touch_cached_preview(key)
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
    state.chart_preview_state.rendered_size = Some((key.width, key.height));
    state.chart_preview_state.protocol = Some(protocol);
    state.chart_preview_state.clipboard_image = Some(clipboard_image);
    state.chart_preview_state.error = None;
    state.chart_preview_state.rendered_roi = key.roi;
    state
        .chart_preview_state
        .set_current_data(Some(data_preview));
    state
        .chart_preview_state
        .sync_data_bounds(Some(data_bounds));
    true
}

pub(super) fn queue_chart_preview_load(
    f: &mut Frame<'_>,
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
