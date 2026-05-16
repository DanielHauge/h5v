use std::{
    io::BufReader,
    sync::mpsc::{channel, Sender},
    thread,
};

use hdf5_metno::{types::IntSize, Dataset, Selection};
use image::{imageops::FilterType, DynamicImage, ImageBuffer, ImageFormat, Rgb};
use ndarray::{s, Array2, Array3};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
    Resize, StatefulImage,
};

use crate::{
    data::Previewable,
    error::AppError,
    h5f::{plot_projected, H5FNode, ImageType, InterlaceMode, Node},
    ui::{
        app::AppEvent,
        preview::chart::{render_image_chart, MAX_SEGMENT_SIZE},
        segment_scroll::render_position_scroll,
        state::{
            AppState, ChartPreviewKey, ChartPreviewLoadRequest, ChartPreviewSource,
            ClipboardImageData, DatasetImageLoadRequest, ImageLoadKey, ImageWindowAxis,
            ImageWindowState, RawImageLoadRequest, SegmentType, VarLenImageLoadRequest,
        },
    },
};

mod workers;
use super::pipeline::{
    clipboard_image_from_dynamic, send_chart_failure, send_chart_success, send_image_failure,
    send_image_success,
};

pub(crate) use workers::{
    handle_chartpreview_load, handle_chartpreview_resize, handle_image_load, handle_image_resize,
    handle_imagefs_load, handle_imagefsvlen_load, ImageResizeResult,
};
const SMART_IMAGE_WINDOW_MIN_CLIPPED_FRACTION: f32 = 0.5;
const IMAGE_CHROME_SCROLL_WIDTH: u16 = 2;
const IMAGE_CHROME_STACK_HEIGHT: u16 = 1;
const IMAGE_CHROME_WINDOW_HEIGHT: u16 = 4;
pub(crate) const IMAGE_CACHE_CAPACITY: usize = 6;

fn image_text_style() -> Style {
    let mut style =
        Style::default().fg(crate::configure::themed_color(|colors| colors.text.primary));
    if crate::configure::prefers_strong_text() {
        style = style.bold();
    }
    style
}

fn raw_image_frame_count(
    ds: &Dataset,
    typedesc: &hdf5_metno::types::TypeDescriptor,
) -> Option<i32> {
    match typedesc {
        hdf5_metno::types::TypeDescriptor::VarLenArray(arr_type)
            if matches!(
                **arr_type,
                hdf5_metno::types::TypeDescriptor::Unsigned(IntSize::U1)
            ) =>
        {
            Some(ds.shape().first().copied().unwrap_or(1) as i32)
        }
        _ => None,
    }
}

fn render_image_loading_indicator(f: &mut Frame, area: Rect) {
    let indicator = Block::default()
        .title(Span::styled(
            " * ",
            Style::default().fg(crate::configure::themed_color(|colors| {
                colors.help.description
            })),
        ))
        .title_alignment(ratatui::layout::Alignment::Right);
    f.render_widget(indicator, area);
}

pub(crate) fn thread_protocol_from_clipboard_image(
    picker: &Picker,
    tx_resize_img: &Sender<ResizeRequest>,
    clipboard_image: &ClipboardImageData,
) -> Option<ThreadProtocol> {
    let image = ImageBuffer::<image::Rgba<u8>, _>::from_raw(
        clipboard_image.width as u32,
        clipboard_image.height as u32,
        clipboard_image.bytes.clone(),
    )?;
    let dyn_img = DynamicImage::ImageRgba8(image);
    let protocol = picker.new_resize_protocol(dyn_img);
    Some(ThreadProtocol::new(tx_resize_img.clone(), Some(protocol)))
}

fn restore_cached_image(state: &mut AppState<'_>, key: &ImageLoadKey) -> bool {
    let Some(clipboard_image) = state.img_state.touch_cached_image(key) else {
        return false;
    };
    let Some(protocol) = thread_protocol_from_clipboard_image(
        &state.multi_chart.picker,
        &state.img_state.tx_resize_img,
        &clipboard_image,
    ) else {
        state
            .img_state
            .cached_images
            .retain(|entry| entry.key != *key);
        return false;
    };
    state.img_state.protocol = Some(protocol);
    state.img_state.clipboard_image = Some(clipboard_image);
    state.img_state.error = None;
    state.img_state.ds = Some(key.ds_path.clone());
    state.img_state.current_key = Some(key.clone());
    state.img_state.idx_loaded = key.idx;
    true
}

fn should_skip_image_request(state: &AppState<'_>, key: &ImageLoadKey) -> bool {
    state.img_state.current_request_key().as_ref() == Some(key)
        || state.img_state.pending_keys.contains(key)
        || state.img_state.has_cached_image(key)
}

fn prefetched_window(window: &ImageWindowState) -> Option<ImageWindowState> {
    let start = if window.end() < window.total {
        window.start + window.len
    } else if window.start > 0 {
        window.start.saturating_sub(window.len)
    } else {
        return None;
    };
    Some(ImageWindowState {
        ds_path: window.ds_path.clone(),
        axis: window.axis,
        start,
        len: window.len,
        total: window.total,
    })
}

fn schedule_dataset_image_prefetch(
    state: &mut AppState<'_>,
    ds: &Dataset,
    image_type: &ImageType,
    current_key: &ImageLoadKey,
    window: Option<&ImageWindowState>,
    frame_count: usize,
) -> Result<(), AppError> {
    let (prefetch_key, prefetch_window) = if frame_count > 1 {
        let next_idx = if (current_key.idx as usize) + 1 < frame_count {
            current_key.idx + 1
        } else if current_key.idx > 0 {
            current_key.idx - 1
        } else {
            return Ok(());
        };
        (
            ImageLoadKey {
                ds_path: current_key.ds_path.clone(),
                idx: next_idx,
                window_axis: current_key.window_axis,
                window_start: current_key.window_start,
                window_len: current_key.window_len,
            },
            window.cloned(),
        )
    } else {
        let Some(prefetch_window) = window.and_then(prefetched_window) else {
            return Ok(());
        };
        (
            ImageLoadKey {
                ds_path: current_key.ds_path.clone(),
                idx: current_key.idx,
                window_axis: Some(prefetch_window.axis),
                window_start: prefetch_window.start,
                window_len: prefetch_window.len,
            },
            Some(prefetch_window),
        )
    };

    if should_skip_image_request(state, &prefetch_key) {
        return Ok(());
    }

    state.img_state.pending_keys.insert(prefetch_key.clone());
    state.img_state.tx_load_img.send(DatasetImageLoadRequest {
        key: prefetch_key,
        dataset: ds.clone(),
        image_type: image_type.clone(),
        window: prefetch_window,
    })?;
    Ok(())
}

fn schedule_varlen_image_prefetch(
    state: &mut AppState<'_>,
    ds: &Dataset,
    format: ImageFormat,
    current_key: &ImageLoadKey,
    frame_count: i32,
) -> Result<(), AppError> {
    if frame_count <= 1 {
        return Ok(());
    }
    let next_idx = if current_key.idx + 1 < frame_count {
        current_key.idx + 1
    } else if current_key.idx > 0 {
        current_key.idx - 1
    } else {
        return Ok(());
    };
    let prefetch_key = ImageLoadKey {
        ds_path: current_key.ds_path.clone(),
        idx: next_idx,
        window_axis: None,
        window_start: 0,
        window_len: 0,
    };
    if should_skip_image_request(state, &prefetch_key) {
        return Ok(());
    }
    state.img_state.pending_keys.insert(prefetch_key.clone());
    state
        .img_state
        .tx_load_imgfsvlen
        .send(VarLenImageLoadRequest {
            key: prefetch_key,
            dataset: ds.clone(),
            format,
        })?;
    Ok(())
}

fn dataset_image_dims(image_type: &ImageType, ds: &Dataset) -> Option<(usize, usize, usize)> {
    let shape = ds.shape();
    match image_type {
        ImageType::Grayscale => match shape.len() {
            2 => Some((1, shape[0], shape[1])),
            3 => Some((shape[0], shape[1], shape[2])),
            4 => Some((shape[0], shape[1], shape[2])),
            _ => None,
        },
        ImageType::Bitmap => match shape.len() {
            2 => Some((1, shape[0], shape[1])),
            _ => None,
        },
        ImageType::Truecolor(InterlaceMode::Pixel) => match shape.len() {
            3 => Some((1, shape[0], shape[1])),
            4 => Some((shape[0], shape[1], shape[2])),
            _ => None,
        },
        ImageType::Truecolor(InterlaceMode::Plane) => match shape.len() {
            3 => Some((1, shape[1], shape[2])),
            4 => Some((shape[0], shape[2], shape[3])),
            _ => None,
        },
        _ => None,
    }
}

fn compute_image_window(
    ds_path: &str,
    image_type: &ImageType,
    ds: &Dataset,
    viewport_area: &Rect,
    image_cell_size: (u16, u16),
    current: Option<&ImageWindowState>,
) -> Option<ImageWindowState> {
    let (_, height, width) = dataset_image_dims(image_type, ds)?;
    let viewport_width = viewport_area.width.max(1) as f32 * image_cell_size.0.max(1) as f32;
    let viewport_height = viewport_area.height.max(1) as f32 * image_cell_size.1.max(1) as f32;
    let viewport_aspect = viewport_width / viewport_height;

    let candidate = if (width as f32 / height as f32) > viewport_aspect {
        let len = ((height as f32 * viewport_aspect).floor() as usize).clamp(1, width);
        (len < width).then_some((ImageWindowAxis::Cols, width, len))
    } else {
        let len = ((width as f32 / viewport_aspect).floor() as usize).clamp(1, height);
        (len < height).then_some((ImageWindowAxis::Rows, height, len))
    }?;

    let (_, total, len) = candidate;
    let clipped_fraction = 1.0 - (len as f32 / total as f32);
    if clipped_fraction < SMART_IMAGE_WINDOW_MIN_CLIPPED_FRACTION {
        return None;
    }

    let (axis, total, len) = candidate;
    let start = match current {
        Some(existing)
            if existing.ds_path == ds_path && existing.axis == axis && existing.total == total =>
        {
            let center = existing.start + existing.len / 2;
            ImageWindowState::centered_start(total, len, center)
        }
        _ => 0,
    };

    Some(ImageWindowState {
        ds_path: ds_path.to_string(),
        axis,
        start,
        len,
        total,
    })
}

fn window_bounds(window: Option<&ImageWindowState>) -> Option<(usize, usize)> {
    window.map(|window| (window.start, window.end()))
}

fn image_content_area(area: Rect, has_stack: bool, has_window: bool) -> Rect {
    if !has_stack && !has_window {
        return area;
    }

    let chrome_height = if has_window {
        IMAGE_CHROME_WINDOW_HEIGHT
    } else {
        IMAGE_CHROME_STACK_HEIGHT
    };
    let areas_split = Layout::horizontal(vec![
        Constraint::Min(1),
        Constraint::Length(IMAGE_CHROME_SCROLL_WIDTH),
    ])
    .split(area);
    let content_areas =
        Layout::vertical(vec![Constraint::Length(chrome_height), Constraint::Min(2)])
            .split(areas_split[0]);
    content_areas[1]
}

fn render_image_chrome(
    f: &mut Frame,
    area: &Rect,
    stack: Option<(i32, i32)>,
    window: Option<&ImageWindowState>,
    loading: bool,
) -> Result<Rect, AppError> {
    if stack.is_none() && window.is_none() {
        return Ok(*area);
    }

    let areas_split = Layout::horizontal(vec![
        Constraint::Min(1),
        Constraint::Length(IMAGE_CHROME_SCROLL_WIDTH),
    ])
    .split(*area);
    let chrome_height = if window.is_some() {
        IMAGE_CHROME_WINDOW_HEIGHT
    } else {
        IMAGE_CHROME_STACK_HEIGHT
    };
    let content_areas =
        Layout::vertical(vec![Constraint::Length(chrome_height), Constraint::Min(2)])
            .split(areas_split[0]);

    if let Some(window) = window {
        render_position_scroll(f, &areas_split[1], window.total, window.start, window.len)?;
    } else if let Some((idx, count)) = stack {
        render_position_scroll(f, &areas_split[1], count as usize, idx as usize, 1)?;
    }

    if let Some(window) = window {
        let title = match stack {
            Some((idx, count)) => format!(
                " Viewport {} | image {}/{}{} ",
                window.label(),
                idx + 1,
                count,
                if loading { " *" } else { "" }
            ),
            None => format!(
                " Viewport {}{} ",
                window.label(),
                if loading { " *" } else { "" }
            ),
        };
        let block = Block::default()
            .title(title)
            .title_style(
                Style::default()
                    .fg(crate::configure::themed_color(|colors| {
                        colors.surface.panel_title
                    }))
                    .bold(),
            )
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(
                Style::default().fg(crate::configure::themed_color(|colors| {
                    colors.surface.break_line
                })),
            )
            .style(
                Style::default().bg(crate::configure::themed_color(|colors| {
                    colors.surface.bg_val3
                })),
            );
        let inner = block.inner(content_areas[0]);
        f.render_widget(block, content_areas[0]);

        let split = Layout::horizontal([Constraint::Min(1), Constraint::Length(2)]).split(inner);
        let [text_area, _scroll_area] = split.as_ref() else {
            return Ok(content_areas[1]);
        };
        let pan_step = (window.len / 4).max(1);
        let start = window.start;
        let end = window.end().saturating_sub(1);
        let total_end = window.total.saturating_sub(1);
        let visible = window.len;
        let start_pct = if window.total == 0 {
            0.0
        } else {
            (start as f64 / window.total as f64) * 100.0
        };
        let end_pct = if window.total == 0 {
            0.0
        } else {
            (window.end() as f64 / window.total as f64) * 100.0
        };
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    "range ",
                    Style::default().fg(crate::configure::themed_color(|colors| {
                        colors.text.type_desc
                    })),
                ),
                Span::styled(
                    format!("{start}..{end} of 0..{total_end} {}", window.label()),
                    image_text_style(),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "cover ",
                    Style::default().fg(crate::configure::themed_color(|colors| {
                        colors.text.type_desc
                    })),
                ),
                Span::styled(
                    format!(
                        "{start_pct:.1}-{end_pct:.1}% | {visible} visible | arrows move {pan_step} {}",
                        window.label()
                    ),
                    image_text_style(),
                ),
            ]),
        ];
        f.render_widget(
            Paragraph::new(lines)
                .style(image_text_style())
                .wrap(Wrap { trim: false }),
            *text_area,
        );
    } else {
        let title = match stack {
            Some((idx, count)) => format!(
                " Image {}/{}{} ",
                idx + 1,
                count,
                if loading { " *" } else { "" }
            ),
            None => format!(" Image{} ", if loading { " *" } else { "" }),
        };
        let block = Block::default()
            .title(title)
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(Borders::TOP)
            .border_type(BorderType::Plain)
            .style(
                Style::default().fg(crate::configure::themed_color(|colors| {
                    colors.surface.image_border
                })),
            );
        f.render_widget(block, content_areas[0]);
    }
    Ok(content_areas[1])
}

pub fn render_img(
    image_type: &ImageType,
    f: &mut Frame,
    area: &Rect,
    // node: &Node,
    selected_node_refc: &mut H5FNode,
    state: &mut AppState,
) -> Result<(), AppError> {
    let node = &selected_node_refc.node;
    match image_type {
        ImageType::Jpeg => {
            state.img_state.window = None;
            render_raw_img(f, area, node, state, ImageFormat::Jpeg)
        }
        ImageType::Png => {
            state.img_state.window = None;
            render_raw_img(f, area, node, state, ImageFormat::Png)
        }
        ImageType::Truecolor(m) => {
            render_ds_img(f, area, node, state, ImageType::Truecolor(m.clone()))
        }
        ImageType::Grayscale => render_ds_img(f, area, node, state, ImageType::Grayscale),
        ImageType::Bitmap => render_ds_img(f, area, node, state, ImageType::Bitmap),
        _ => render_unsupported_image_format(f, area, node),
    }
}

fn render_unsupported_image_format(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Node,
) -> Result<(), AppError> {
    let (ds, _) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => return Ok(()),
    };

    let inner_area = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let unsupported_msg = format!("Unsupported image format for dataset: {}", ds.name());
    f.render_widget(unsupported_msg, inner_area);
    Ok(())
}

fn render_ds_img(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Node,
    state: &mut AppState,
    img_type: ImageType,
) -> Result<(), AppError> {
    let (ds, _) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => return Ok(()),
    };
    let ds_path = ds.name();
    let frame_count = dataset_image_dims(&img_type, ds)
        .map(|(frames, _, _)| frames)
        .unwrap_or(1);
    if frame_count > 1 {
        state.segment_state.segumented = SegmentType::Image;
        state.segment_state.segment_count = frame_count as i32;
        state.segment_state.idx = state.img_state.idx_to_load;
    } else {
        state.segment_state.segumented = SegmentType::NoSegment;
        state.segment_state.segment_count = 0;
        state.segment_state.idx = 0;
        state.img_state.idx_to_load = 0;
    }
    let has_stack = frame_count > 1;
    let stack_content_area = image_content_area(*area, has_stack, false);
    let stack_window = compute_image_window(
        &ds_path,
        &img_type,
        ds,
        &stack_content_area,
        state.image_cell_size,
        state.img_state.window.as_ref(),
    );
    let final_content_area = image_content_area(*area, has_stack, stack_window.is_some());
    let desired_window = compute_image_window(
        &ds_path,
        &img_type,
        ds,
        &final_content_area,
        state.image_cell_size,
        state.img_state.window.as_ref(),
    )
    .or(stack_window);
    state.img_state.window = desired_window.clone();

    let desired_key = ImageLoadKey {
        ds_path: ds_path.clone(),
        idx: state.img_state.idx_to_load,
        window_axis: desired_window.as_ref().map(|window| window.axis),
        window_start: desired_window.as_ref().map_or(0, |window| window.start),
        window_len: desired_window.as_ref().map_or(0, |window| window.len),
    };
    let render_area = render_image_chrome(
        f,
        area,
        has_stack.then_some((state.segment_state.idx, state.segment_state.segment_count)),
        desired_window.as_ref(),
        state.img_state.pending_keys.contains(&desired_key),
    )?;
    let mut image_loaded = state.img_state.current_request_key() == Some(desired_key.clone());
    if !image_loaded && restore_cached_image(state, &desired_key) {
        image_loaded = true;
    }
    if state.should_debounce_preview(selected_node) {
        if image_loaded {
            if let Some(ref mut protocol) = state.img_state.protocol {
                let image_widget =
                    StatefulImage::default().resize(Resize::Scale(Some(FilterType::Triangle)));
                f.render_stateful_widget(image_widget, render_area, protocol);
            }
        } else {
            state.img_state.protocol = None;
            state.img_state.clipboard_image = None;
            state.img_state.error = None;
        }
        if !image_loaded || state.img_state.pending_keys.contains(&desired_key) {
            render_image_loading_indicator(f, render_area);
        }
        return Ok(());
    }

    match image_loaded {
        true => {
            if let Some(e) = &state.img_state.error {
                let error_msg = format!("Error loading image: {}", e);
                f.render_widget(
                    Paragraph::new(error_msg).style(
                        Style::default()
                            .fg(crate::configure::themed_color(|colors| colors.text.error)),
                    ),
                    render_area,
                );
            } else if let Some(ref mut protocol) = state.img_state.protocol {
                let image_widget =
                    StatefulImage::default().resize(Resize::Scale(Some(FilterType::Triangle)));
                f.render_stateful_widget(image_widget, render_area, protocol);
                if state.img_state.pending_keys.contains(&desired_key) {
                    render_image_loading_indicator(f, render_area);
                }
                schedule_dataset_image_prefetch(
                    state,
                    ds,
                    &img_type,
                    &desired_key,
                    desired_window.as_ref(),
                    frame_count,
                )?;
            }
        }
        false => {
            state
                .img_state
                .begin_loading(desired_key.clone(), state.img_state.idx_to_load);
            state.img_state.tx_load_img.send(DatasetImageLoadRequest {
                key: desired_key,
                dataset: ds.clone(),
                image_type: img_type,
                window: desired_window,
            })?;
        }
    }

    Ok(())
}

fn render_raw_img(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Node,
    state: &mut AppState,
    img_format: ImageFormat,
) -> Result<(), AppError> {
    let (ds, _) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => return Ok(()),
    };
    let typedesc = ds.dtype()?.to_descriptor()?;
    let varlen_frame_count = raw_image_frame_count(ds, &typedesc);
    let has_stack = matches!(varlen_frame_count, Some(frame_count) if frame_count > 1);
    if let Some(frame_count) = varlen_frame_count {
        let clamped_idx = state
            .img_state
            .idx_to_load
            .clamp(0, frame_count.saturating_sub(1));
        state.img_state.idx_to_load = clamped_idx;
        state.img_state.idx_loaded = state
            .img_state
            .idx_loaded
            .clamp(0, frame_count.saturating_sub(1));
        if frame_count > 1 {
            state.segment_state.segumented = SegmentType::Image;
            state.segment_state.segment_count = frame_count;
            state.segment_state.idx = clamped_idx;
        } else {
            state.segment_state.segumented = SegmentType::NoSegment;
            state.segment_state.segment_count = 0;
            state.segment_state.idx = 0;
        }
    } else {
        state.segment_state.segumented = SegmentType::NoSegment;
        state.segment_state.segment_count = 0;
        state.segment_state.idx = 0;
        state.img_state.idx_to_load = 0;
    }

    let desired_key = ImageLoadKey {
        ds_path: ds.name(),
        idx: state.img_state.idx_to_load,
        window_axis: None,
        window_start: 0,
        window_len: 0,
    };
    let mut image_loaded = state.img_state.current_request_key() == Some(desired_key.clone());
    if !image_loaded && restore_cached_image(state, &desired_key) {
        image_loaded = true;
    }
    let show_loading = state.img_state.pending_keys.contains(&desired_key)
        || (state.should_debounce_preview(selected_node) && !image_loaded);
    let render_area = if has_stack {
        render_image_chrome(
            f,
            area,
            Some((state.segment_state.idx, state.segment_state.segment_count)),
            None,
            show_loading,
        )?
    } else {
        *area
    };

    if state.should_debounce_preview(selected_node) {
        if image_loaded {
            if let Some(ref mut protocol) = state.img_state.protocol {
                let image_widget =
                    StatefulImage::new().resize(Resize::Scale(Some(FilterType::Triangle)));
                f.render_stateful_widget(image_widget, render_area, protocol);
            }
        } else {
            state.img_state.protocol = None;
            state.img_state.clipboard_image = None;
            state.img_state.error = None;
        }
        if show_loading {
            render_image_loading_indicator(f, render_area);
        }
        if image_loaded && !state.img_state.pending_keys.contains(&desired_key) {
            if let Some(frame_count) = varlen_frame_count {
                schedule_varlen_image_prefetch(state, ds, img_format, &desired_key, frame_count)?;
            }
        }
        return Ok(());
    }

    match image_loaded {
        true => match state.img_state.error {
            Some(ref e) => {
                let error_msg = format!("Error loading image - {}", e);
                f.render_widget(
                    Paragraph::new(error_msg).style(
                        Style::default()
                            .fg(crate::configure::themed_color(|colors| colors.text.error)),
                    ),
                    render_area,
                );
            }
            None => {
                if let Some(ref mut protocol) = state.img_state.protocol {
                    let image_widget =
                        StatefulImage::new().resize(Resize::Scale(Some(FilterType::Triangle)));
                    f.render_stateful_widget(image_widget, render_area, protocol);
                    if state.img_state.pending_keys.contains(&desired_key) {
                        render_image_loading_indicator(f, render_area);
                    }
                }
                if let Some(frame_count) = varlen_frame_count {
                    schedule_varlen_image_prefetch(
                        state,
                        ds,
                        img_format,
                        &desired_key,
                        frame_count,
                    )?;
                }
            }
        },
        false => {
            state.img_state.error = None;
            state.img_state.ds = Some(ds.name());
            match typedesc {
                hdf5_metno::types::TypeDescriptor::Unsigned(IntSize::U1) => {
                    let ds_reader = ds.as_byte_reader()?;
                    state.segment_state.segumented = SegmentType::NoSegment;
                    state
                        .img_state
                        .begin_loading(desired_key.clone(), state.img_state.idx_to_load);
                    let ds_buffered = BufReader::new(ds_reader);
                    state.img_state.tx_load_imgfs.send(RawImageLoadRequest {
                        key: desired_key,
                        reader: ds_buffered,
                        format: img_format,
                    })?;
                }
                hdf5_metno::types::TypeDescriptor::VarLenArray(arr_type) => {
                    if matches!(
                        *arr_type,
                        hdf5_metno::types::TypeDescriptor::Unsigned(IntSize::U1)
                    ) {
                        state
                            .img_state
                            .begin_loading(desired_key.clone(), state.img_state.idx_to_load);
                        state
                            .img_state
                            .tx_load_imgfsvlen
                            .send(VarLenImageLoadRequest {
                                key: desired_key,
                                dataset: ds.clone(),
                                format: img_format,
                            })?;
                    }
                }
                _ => {
                    state.img_state.error = Some("Unsupported image format".to_string());
                    let error_msg = format!("Unsupported image format for dataset: {}", ds.name());
                    f.render_widget(error_msg, *area);
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}
