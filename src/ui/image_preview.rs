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
    h5f::{H5FNode, ImageType, InterlaceMode, Node},
    ui::{
        preview_chart::{render_image_chart, MAX_SEGMENT_SIZE},
        state::{ChartPreviewLoadRequest, ChartPreviewSource},
    },
};

use super::app::{ChartPreviewLoadedResult, ImageLoadedResult};
use super::{
    app::AppEvent,
    segment_scroll::render_position_scroll,
    state::{
        AppState, ChartPreviewKey, ClipboardImageData, DatasetImageLoadRequest, ImageLoadKey,
        ImageWindowAxis, ImageWindowState, RawImageLoadRequest, SegmentType,
        VarLenImageLoadRequest,
    },
};

const SMART_IMAGE_WINDOW_MIN_CLIPPED_FRACTION: f32 = 0.5;
const IMAGE_CHROME_SCROLL_WIDTH: u16 = 2;
const IMAGE_CHROME_STACK_HEIGHT: u16 = 1;
const IMAGE_CHROME_WINDOW_HEIGHT: u16 = 4;

fn send_event(tx_events: &Sender<AppEvent>, event: AppEvent) {
    let _ = tx_events.send(event);
}

fn send_image_failure(tx_events: &Sender<AppEvent>, key: ImageLoadKey, message: impl Into<String>) {
    send_event(
        tx_events,
        AppEvent::ImageLoad(ImageLoadedResult::Failure {
            key,
            message: message.into(),
        }),
    );
}

fn clipboard_image_from_dynamic(dyn_img: &DynamicImage) -> ClipboardImageData {
    let rgba = dyn_img.to_rgba8();
    ClipboardImageData {
        width: rgba.width() as usize,
        height: rgba.height() as usize,
        bytes: rgba.into_raw(),
    }
}

fn send_image_success(
    tx_events: &Sender<AppEvent>,
    key: ImageLoadKey,
    protocol: ThreadProtocol,
    clipboard_image: ClipboardImageData,
) {
    send_event(
        tx_events,
        AppEvent::ImageLoad(ImageLoadedResult::Success {
            key,
            protocol,
            clipboard_image,
        }),
    );
}

fn send_chart_failure(
    tx_events: &Sender<AppEvent>,
    key: ChartPreviewKey,
    message: impl Into<String>,
) {
    send_event(
        tx_events,
        AppEvent::PreviewChartLoad(ChartPreviewLoadedResult::Failure {
            key,
            message: message.into(),
        }),
    );
}

fn send_chart_success(
    tx_events: &Sender<AppEvent>,
    key: ChartPreviewKey,
    protocol: ThreadProtocol,
    clipboard_image: ClipboardImageData,
) {
    send_event(
        tx_events,
        AppEvent::PreviewChartLoad(ChartPreviewLoadedResult::Success {
            key,
            protocol,
            clipboard_image,
        }),
    );
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
                " Viewport {} | image {}/{} ",
                window.label(),
                idx + 1,
                count
            ),
            None => format!(" Viewport {} ", window.label()),
        };
        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(crate::color_consts::TITLE).bold())
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(crate::color_consts::BREAK_COLOR))
            .style(Style::default().bg(crate::color_consts::BG_VAL3_COLOR));
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
                    Style::default().fg(crate::color_consts::TYPE_DESC_COLOR),
                ),
                Span::raw(format!(
                    "{start}..{end} of 0..{total_end} {}",
                    window.label()
                )),
            ]),
            Line::from(vec![
                Span::styled(
                    "cover ",
                    Style::default().fg(crate::color_consts::TYPE_DESC_COLOR),
                ),
                Span::raw(format!(
                    "{start_pct:.1}-{end_pct:.1}% | {visible} visible | arrows move {pan_step} {}",
                    window.label()
                )),
            ]),
        ];
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), *text_area);
    } else {
        let title = match stack {
            Some((idx, count)) => format!(" Image {}/{} ", idx + 1, count),
            None => " Image ".to_string(),
        };
        let block = Block::default()
            .title(title)
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(Borders::TOP)
            .border_type(BorderType::Plain)
            .style(Style::default().fg(ratatui::style::Color::DarkGray));
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
            let render_area = if let SegmentType::Image = state.segment_state.segumented {
                render_image_chrome(
                    f,
                    area,
                    Some((state.segment_state.idx, state.segment_state.segment_count)),
                    None,
                )?
            } else {
                *area
            };
            render_raw_img(f, &render_area, node, state, ImageFormat::Jpeg)
        }
        ImageType::Png => {
            state.img_state.window = None;
            let render_area = if let SegmentType::Image = state.segment_state.segumented {
                render_image_chrome(
                    f,
                    area,
                    Some((state.segment_state.idx, state.segment_state.segment_count)),
                    None,
                )?
            } else {
                *area
            };
            render_raw_img(f, &render_area, node, state, ImageFormat::Png)
        }
        ImageType::Truecolor(m) => {
            render_ds_img(f, area, node, state, ImageType::Truecolor(m.clone()))
        }
        ImageType::Grayscale => render_ds_img(f, area, node, state, ImageType::Grayscale),
        ImageType::Bitmap => render_ds_img(f, area, node, state, ImageType::Bitmap),
        _ => render_unsupported_image_format(f, &area, node),
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

    let render_area = render_image_chrome(
        f,
        area,
        has_stack.then_some((state.segment_state.idx, state.segment_state.segment_count)),
        desired_window.as_ref(),
    )?;
    if state.should_debounce_preview(selected_node) {
        f.render_widget("Loading image preview...", render_area);
        return Ok(());
    }

    let desired_key = ImageLoadKey {
        ds_path: ds_path.clone(),
        idx: state.img_state.idx_to_load,
        window_axis: desired_window.as_ref().map(|window| window.axis),
        window_start: desired_window.as_ref().map_or(0, |window| window.start),
        window_len: desired_window.as_ref().map_or(0, |window| window.len),
    };
    let image_loaded = state.img_state.current_request_key() == Some(desired_key.clone());

    match image_loaded {
        true => {
            if let Some(e) = &state.img_state.error {
                let error_msg = format!("Error loading image: {}", e);
                f.render_widget(error_msg, render_area);
            } else if let Some(ref mut protocol) = state.img_state.protocol {
                let image_widget =
                    StatefulImage::default().resize(Resize::Scale(Some(FilterType::Triangle)));
                f.render_stateful_widget(image_widget, render_area, protocol);
            }
        }
        false => {
            state.img_state.protocol = None;
            state.img_state.clipboard_image = None;
            state.img_state.error = None;
            state.img_state.ds = Some(ds_path);
            state.img_state.idx_loaded = state.img_state.idx_to_load;
            state.img_state.current_key = Some(desired_key.clone());
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

    if state.should_debounce_preview(selected_node) {
        f.render_widget("Loading image preview...", *area);
        return Ok(());
    }

    let desired_key = ImageLoadKey {
        ds_path: ds.name(),
        idx: state.img_state.idx_to_load,
        window_axis: None,
        window_start: 0,
        window_len: 0,
    };
    let image_loaded = state.img_state.current_request_key() == Some(desired_key.clone());

    match image_loaded {
        true => match state.img_state.error {
            Some(ref e) => {
                let error_msg = format!("Error loading image - {}", e);
                f.render_widget(error_msg, *area);
            }
            None => {
                if let Some(ref mut protocol) = state.img_state.protocol {
                    let image_widget =
                        StatefulImage::new().resize(Resize::Scale(Some(FilterType::Triangle)));
                    f.render_stateful_widget(image_widget, *area, protocol);
                }
            }
        },
        false => {
            state.img_state.protocol = None;
            state.img_state.clipboard_image = None;
            state.img_state.error = None;
            state.img_state.ds = Some(ds.name());
            let typedesc = ds.dtype()?.to_descriptor()?;
            match typedesc {
                hdf5_metno::types::TypeDescriptor::Unsigned(IntSize::U1) => {
                    let ds_reader = ds.as_byte_reader()?;
                    state.segment_state.segumented = SegmentType::NoSegment;
                    state.img_state.idx_loaded = state.img_state.idx_to_load;
                    state.img_state.current_key = Some(desired_key.clone());
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
                        let i = state.img_state.idx_to_load;
                        let frame_count = ds.shape().first().copied().unwrap_or(1) as i32;
                        state.img_state.idx_loaded = i;
                        if frame_count > 1 {
                            state.segment_state.segumented = SegmentType::Image;
                            state.segment_state.segment_count = frame_count;
                            state.segment_state.idx = i.clamp(0, frame_count - 1);
                        } else {
                            state.segment_state.segumented = SegmentType::NoSegment;
                            state.segment_state.segment_count = frame_count.max(0);
                            state.segment_state.idx = 0;
                        }
                        state.img_state.current_key = Some(desired_key.clone());
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

pub fn handle_image_resize(tx_events: Sender<AppEvent>) -> Sender<ResizeRequest> {
    let (tx_worker, rx_worker) = channel::<ResizeRequest>();

    thread::spawn(move || loop {
        if let Ok(request) = rx_worker.recv() {
            match request.resize_encode() {
                Ok(r) => {
                    if let Err(e) =
                        tx_events.send(AppEvent::ImageResized(ImageResizeResult::Success(r)))
                    {
                        eprintln!("Failed to send image redraw event: {}", e);
                    }
                }
                Err(e) => {
                    if let Err(e) = tx_events.send(AppEvent::ImageResized(
                        ImageResizeResult::Error(e.to_string()),
                    )) {
                        eprintln!("Failed to send image redraw event: {}", e);
                    }
                }
            }
        }
    });
    tx_worker
}

pub fn handle_chartpreview_resize(tx_events: Sender<AppEvent>) -> Sender<ResizeRequest> {
    let (tx_worker, rx_worker) = channel::<ResizeRequest>();

    thread::spawn(move || loop {
        if let Ok(request) = rx_worker.recv() {
            match request.resize_encode() {
                Ok(r) => {
                    if let Err(e) =
                        tx_events.send(AppEvent::PreviewChartResized(ImageResizeResult::Success(r)))
                    {
                        eprintln!("Failed to send chart preview redraw event: {}", e);
                    }
                }
                Err(e) => {
                    if let Err(e) = tx_events.send(AppEvent::PreviewChartResized(
                        ImageResizeResult::Error(e.to_string()),
                    )) {
                        eprintln!("Failed to send chart preview redraw event: {}", e);
                    }
                }
            }
        }
    });
    tx_worker
}

pub fn handle_chartpreview_load(
    tx_events: Sender<AppEvent>,
    tx_worker: Sender<ResizeRequest>,
    picker: Picker,
) -> Sender<ChartPreviewLoadRequest> {
    let (tx_load, rx_load) = channel::<ChartPreviewLoadRequest>();

    let (x, y) = picker.font_size();

    thread::spawn(move || loop {
        if let Ok(mut req) = rx_load.recv() {
            while let Ok(queued) = rx_load.try_recv() {
                req = queued;
            }
            let height = req.height as u32 * y as u32;
            let width = req.width as u32 * x as u32;

            let mut buffer = vec![0; (height * width * 3) as usize];
            let x_min = if req.segment_state.idx > 0 {
                MAX_SEGMENT_SIZE as f64 * req.segment_state.idx as f64
            } else {
                0.0
            };

            let data_preview = match req.source {
                ChartPreviewSource::Dataset { ds, selection } => match ds.plot(&selection) {
                    Ok(data_preview) => data_preview,
                    Err(e) => {
                        send_chart_failure(
                            &tx_events,
                            ChartPreviewKey {
                                ds_path: req.ds_path.clone(),
                                selection: req.selection.clone(),
                            },
                            format!("Failed to plot data for chart preview: {}", e),
                        );
                        continue;
                    }
                },
                ChartPreviewSource::Precomputed { data_preview } => data_preview,
            };

            if data_preview.min.is_nan()
                || data_preview.max.is_nan()
                || data_preview.min.is_infinite()
            {
                send_chart_failure(
                    &tx_events,
                    ChartPreviewKey {
                        ds_path: req.ds_path.clone(),
                        selection: req.selection.clone(),
                    },
                    "Data not valid, could not establish min and max bounds for chart\nIt seems the data only contains NaN or infinite values.",
                );
                continue;
            }

            if let Err(e) = render_image_chart(&mut buffer, width, height, x_min, data_preview) {
                send_chart_failure(
                    &tx_events,
                    ChartPreviewKey {
                        ds_path: req.ds_path.clone(),
                        selection: req.selection.clone(),
                    },
                    format!("Failed to render chart preview: {}", e),
                );
                continue;
            }

            let image = ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, buffer);

            let Some(image) = image else {
                send_chart_failure(
                    &tx_events,
                    ChartPreviewKey {
                        ds_path: req.ds_path.clone(),
                        selection: req.selection.clone(),
                    },
                    "Failed to create image buffer for chart preview",
                );
                continue;
            };

            let dyn_img = DynamicImage::ImageRgb8(image);
            let clipboard_image = clipboard_image_from_dynamic(&dyn_img);
            let stateful_protocol = picker.new_resize_protocol(dyn_img);
            let thread_protocol = ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
            send_chart_success(
                &tx_events,
                ChartPreviewKey {
                    ds_path: req.ds_path,
                    selection: req.selection,
                },
                thread_protocol,
                clipboard_image,
            );
        }
    });
    tx_load
}

pub fn handle_imagefs_load(
    tx_events: Sender<AppEvent>,
    tx_worker: Sender<ResizeRequest>,
    picker: Picker,
) -> Sender<RawImageLoadRequest> {
    let (tx_load, rx_load) = channel::<RawImageLoadRequest>();

    thread::spawn(move || loop {
        if let Ok(mut req) = rx_load.recv() {
            // We drain to the latest
            while let Ok(queued) = rx_load.try_recv() {
                req = queued;
            }
            match image::load(req.reader, req.format) {
                Ok(dyn_img) => {
                    let clipboard_image = clipboard_image_from_dynamic(&dyn_img);
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, req.key, thread_protocol, clipboard_image);
                }
                Err(e) => {
                    send_image_failure(&tx_events, req.key, format!("Failed to decode image: {e}"))
                }
            }
        }
    });
    tx_load
}

pub fn handle_imagefsvlen_load(
    tx_events: Sender<AppEvent>,
    tx_worker: Sender<ResizeRequest>,
    picker: Picker,
) -> Sender<VarLenImageLoadRequest> {
    let (tx_load, rx_load) = channel::<VarLenImageLoadRequest>();

    thread::spawn(move || loop {
        if let Ok(mut req) = rx_load.recv() {
            // We drain to the latest
            while let Ok(queued) = rx_load.try_recv() {
                req = queued;
            }
            let data = match req
                .dataset
                .read_slice_1d::<hdf5_metno::types::VarLenArray<u8>, _>(Selection::All)
            {
                Ok(d) => {
                    let frame_idx = req.key.idx;
                    let Some(bytes) = d.get(frame_idx as usize) else {
                        send_image_failure(
                            &tx_events,
                            req.key,
                            format!(
                                "Varlen image index {} is out of bounds for {} frame(s)",
                                frame_idx,
                                d.len()
                            ),
                        );
                        continue;
                    };
                    bytes.as_slice().to_vec()
                }
                Err(e) => {
                    send_image_failure(&tx_events, req.key, e.to_string());
                    continue;
                }
            };

            let cursor = std::io::Cursor::new(data);
            let data = BufReader::new(cursor);
            match image::load(data, req.format) {
                Ok(dyn_img) => {
                    let clipboard_image = clipboard_image_from_dynamic(&dyn_img);
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, req.key, thread_protocol, clipboard_image);
                }
                Err(e) => send_image_failure(
                    &tx_events,
                    req.key,
                    format!("Failed to decode varlen image: {e}"),
                ),
            }
        }
    });
    tx_load
}

#[allow(clippy::large_enum_variant)]
pub enum ImageResizeResult {
    Success(ResizeResponse),
    Error(String),
}

enum BitDepth {
    Bit8,
    Bit12,
    Unknown,
}

trait PixelBitDepth {
    fn bit_depth(&self) -> BitDepth;
}

impl PixelBitDepth for Dataset {
    fn bit_depth(&self) -> BitDepth {
        match match self.dtype() {
            Ok(d) => match d.to_descriptor() {
                Ok(desc) => desc,
                Err(_) => return BitDepth::Unknown,
            },
            Err(_) => return BitDepth::Unknown,
        } {
            hdf5_metno::types::TypeDescriptor::Unsigned(IntSize::U1) => BitDepth::Bit8,
            hdf5_metno::types::TypeDescriptor::Unsigned(IntSize::U2) => BitDepth::Bit12,
            _ => BitDepth::Unknown,
        }
    }
}

pub fn handle_image_load(
    tx_events: Sender<AppEvent>,
    tx_worker: Sender<ResizeRequest>,
    picker: Picker,
) -> Sender<DatasetImageLoadRequest> {
    let (tx_load, rx_load) = channel::<DatasetImageLoadRequest>();
    thread::spawn(move || loop {
        if let Ok(mut req) = rx_load.recv() {
            while let Ok(queued) = rx_load.try_recv() {
                // We drain to the latest
                req = queued;
            }
            let key = req.key.clone();
            let window = req.window.as_ref();
            match req.image_type {
                ImageType::Grayscale => {
                    let shape = req.dataset.shape();
                    let bit_depth = req.dataset.bit_depth();

                    let dyn_img = match bit_depth {
                        BitDepth::Bit8 => {
                            let data: Array2<u8> = match shape.len() {
                                2 => match match window_bounds(window) {
                                    Some((start, end)) => match window.map(|w| w.axis) {
                                        Some(ImageWindowAxis::Cols) => {
                                            req.dataset.read_slice::<u8, _, _>(s![.., start..end])
                                        }
                                        Some(ImageWindowAxis::Rows) => {
                                            req.dataset.read_slice::<u8, _, _>(s![start..end, ..])
                                        }
                                        None => req.dataset.read_slice::<u8, _, _>(s![.., ..]),
                                    },
                                    None => req.dataset.read_slice::<u8, _, _>(s![.., ..]),
                                } {
                                    Ok(d) => d,
                                    Err(e) => {
                                        send_image_failure(&tx_events, key.clone(), e.to_string());
                                        continue;
                                    }
                                },
                                3 => match match window_bounds(window) {
                                    Some((start, end)) => match window.map(|w| w.axis) {
                                        Some(ImageWindowAxis::Cols) => req
                                            .dataset
                                            .read_slice::<u8, _, _>(s![key.idx, .., start..end]),
                                        Some(ImageWindowAxis::Rows) => req
                                            .dataset
                                            .read_slice::<u8, _, _>(s![key.idx, start..end, ..]),
                                        None => {
                                            req.dataset.read_slice::<u8, _, _>(s![key.idx, .., ..])
                                        }
                                    },
                                    None => req.dataset.read_slice::<u8, _, _>(s![key.idx, .., ..]),
                                } {
                                    Ok(d) => d,
                                    Err(e) => {
                                        send_image_failure(&tx_events, key.clone(), e.to_string());
                                        continue;
                                    }
                                },
                                4 => match match window_bounds(window) {
                                    Some((start, end)) => match window.map(|w| w.axis) {
                                        Some(ImageWindowAxis::Cols) => req
                                            .dataset
                                            .read_slice::<u8, _, _>(s![key.idx, .., start..end, 0]),
                                        Some(ImageWindowAxis::Rows) => req
                                            .dataset
                                            .read_slice::<u8, _, _>(s![key.idx, start..end, .., 0]),
                                        None => req.dataset.read_slice::<u8, _, _>(s![
                                            key.idx,
                                            ..,
                                            ..,
                                            0
                                        ]),
                                    },
                                    None => {
                                        req.dataset.read_slice::<u8, _, _>(s![key.idx, .., .., 0])
                                    }
                                } {
                                    Ok(d) => d,
                                    Err(e) => {
                                        send_image_failure(&tx_events, key.clone(), e.to_string());
                                        continue;
                                    }
                                },
                                _ => {
                                    send_image_failure(
                                        &tx_events,
                                        key.clone(),
                                        "Invalid shape for Grayscale image",
                                    );
                                    continue;
                                }
                            };
                            let shape = data.shape();
                            let mut image_buffer =
                                image::GrayImage::new(shape[1] as u32, shape[0] as u32);
                            for i in 0..shape[1] {
                                for j in 0..shape[0] {
                                    let pixel = image::Luma([data[[j, i]]]);
                                    image_buffer.put_pixel(i as u32, j as u32, pixel);
                                }
                            }
                            image::DynamicImage::ImageLuma8(image_buffer)
                        }
                        BitDepth::Bit12 => {
                            let data: Array2<u16> = match shape.len() {
                                2 => match match window_bounds(window) {
                                    Some((start, end)) => match window.map(|w| w.axis) {
                                        Some(ImageWindowAxis::Cols) => {
                                            req.dataset.read_slice::<u16, _, _>(s![.., start..end])
                                        }
                                        Some(ImageWindowAxis::Rows) => {
                                            req.dataset.read_slice::<u16, _, _>(s![start..end, ..])
                                        }
                                        None => req.dataset.read_slice::<u16, _, _>(s![.., ..]),
                                    },
                                    None => req.dataset.read_slice::<u16, _, _>(s![.., ..]),
                                } {
                                    Ok(d) => d,
                                    Err(e) => {
                                        send_image_failure(&tx_events, key.clone(), e.to_string());
                                        continue;
                                    }
                                },
                                3 => match match window_bounds(window) {
                                    Some((start, end)) => match window.map(|w| w.axis) {
                                        Some(ImageWindowAxis::Cols) => req
                                            .dataset
                                            .read_slice::<u16, _, _>(s![key.idx, .., start..end]),
                                        Some(ImageWindowAxis::Rows) => req
                                            .dataset
                                            .read_slice::<u16, _, _>(s![key.idx, start..end, ..]),
                                        None => {
                                            req.dataset.read_slice::<u16, _, _>(s![key.idx, .., ..])
                                        }
                                    },
                                    None => {
                                        req.dataset.read_slice::<u16, _, _>(s![key.idx, .., ..])
                                    }
                                } {
                                    Ok(d) => d,
                                    Err(e) => {
                                        send_image_failure(&tx_events, key.clone(), e.to_string());
                                        continue;
                                    }
                                },
                                4 => match match window_bounds(window) {
                                    Some((start, end)) => match window.map(|w| w.axis) {
                                        Some(ImageWindowAxis::Cols) => {
                                            req.dataset.read_slice::<u16, _, _>(s![
                                                key.idx,
                                                ..,
                                                start..end,
                                                0
                                            ])
                                        }
                                        Some(ImageWindowAxis::Rows) => {
                                            req.dataset.read_slice::<u16, _, _>(s![
                                                key.idx,
                                                start..end,
                                                ..,
                                                0
                                            ])
                                        }
                                        None => req.dataset.read_slice::<u16, _, _>(s![
                                            key.idx,
                                            ..,
                                            ..,
                                            0
                                        ]),
                                    },
                                    None => {
                                        req.dataset.read_slice::<u16, _, _>(s![key.idx, .., .., 0])
                                    }
                                } {
                                    Ok(d) => d,
                                    Err(e) => {
                                        send_image_failure(&tx_events, key.clone(), e.to_string());
                                        continue;
                                    }
                                },
                                _ => {
                                    send_image_failure(
                                        &tx_events,
                                        key.clone(),
                                        "Invalid shape for Grayscale image",
                                    );
                                    continue;
                                }
                            };
                            let data = data.mapv(|x| x.min(4095));
                            let data = data.mapv(|x| ((x as f32 / 4095.0) * 255.0) as u8);
                            let shape = data.shape();
                            let mut image_buffer =
                                image::GrayImage::new(shape[1] as u32, shape[0] as u32);
                            for i in 0..shape[1] {
                                for j in 0..shape[0] {
                                    let pixel = image::Luma([data[[j, i]]]);
                                    image_buffer.put_pixel(i as u32, j as u32, pixel);
                                }
                            }
                            image::DynamicImage::ImageLuma8(image_buffer)
                        }
                        BitDepth::Unknown => {
                            send_image_failure(
                                &tx_events,
                                key.clone(),
                                "Unsupported grayscale bit depth for image rendering",
                            );
                            continue;
                        }
                    };

                    let clipboard_image = clipboard_image_from_dynamic(&dyn_img);
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, key, thread_protocol, clipboard_image);
                }
                ImageType::Bitmap => {
                    let data: Array2<bool> = match req
                        .dataset
                        .dtype()
                        .ok()
                        .and_then(|dtype| dtype.to_descriptor().ok())
                    {
                        Some(hdf5_metno::types::TypeDescriptor::Boolean) => {
                            match match window_bounds(window) {
                                Some((start, end)) => match window.map(|w| w.axis) {
                                    Some(ImageWindowAxis::Cols) => {
                                        req.dataset.read_slice::<bool, _, _>(s![.., start..end])
                                    }
                                    Some(ImageWindowAxis::Rows) => {
                                        req.dataset.read_slice::<bool, _, _>(s![start..end, ..])
                                    }
                                    None => req.dataset.read_slice::<bool, _, _>(s![.., ..]),
                                },
                                None => req.dataset.read_slice::<bool, _, _>(s![.., ..]),
                            } {
                                Ok(d) => d,
                                Err(e) => {
                                    send_image_failure(&tx_events, key.clone(), e.to_string());
                                    continue;
                                }
                            }
                        }
                        Some(hdf5_metno::types::TypeDescriptor::Unsigned(IntSize::U1)) => {
                            match match window_bounds(window) {
                                Some((start, end)) => match window.map(|w| w.axis) {
                                    Some(ImageWindowAxis::Cols) => {
                                        req.dataset.read_slice::<u8, _, _>(s![.., start..end])
                                    }
                                    Some(ImageWindowAxis::Rows) => {
                                        req.dataset.read_slice::<u8, _, _>(s![start..end, ..])
                                    }
                                    None => req.dataset.read_slice::<u8, _, _>(s![.., ..]),
                                },
                                None => req.dataset.read_slice::<u8, _, _>(s![.., ..]),
                            } {
                                Ok(d) => d.mapv(|value| value != 0),
                                Err(e) => {
                                    send_image_failure(&tx_events, key.clone(), e.to_string());
                                    continue;
                                }
                            }
                        }
                        _ => {
                            send_image_failure(
                                &tx_events,
                                key.clone(),
                                "Unsupported bitmap storage type; expected bool or u8",
                            );
                            continue;
                        }
                    };
                    let shape = data.shape();
                    let mut image_buffer = image::GrayImage::new(shape[1] as u32, shape[0] as u32);
                    for i in 0..shape[1] {
                        for j in 0..shape[0] {
                            let pixel = if data[[j, i]] {
                                image::Luma([255])
                            } else {
                                image::Luma([0])
                            };
                            image_buffer.put_pixel(i as u32, j as u32, pixel);
                        }
                    }
                    let dyn_img = image::DynamicImage::ImageLuma8(image_buffer);
                    let clipboard_image = clipboard_image_from_dynamic(&dyn_img);
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, key, thread_protocol, clipboard_image);
                }
                ImageType::Truecolor(interlace) => {
                    let shape = req.dataset.shape();
                    let data: Array3<u8> = match shape.len() {
                        3 => match match interlace {
                            InterlaceMode::Pixel => match window_bounds(window) {
                                Some((start, end)) => match window.map(|w| w.axis) {
                                    Some(ImageWindowAxis::Cols) => {
                                        req.dataset.read_slice::<u8, _, _>(s![.., start..end, ..])
                                    }
                                    Some(ImageWindowAxis::Rows) => {
                                        req.dataset.read_slice::<u8, _, _>(s![start..end, .., ..])
                                    }
                                    None => req.dataset.read_slice::<u8, _, _>(s![.., .., ..]),
                                },
                                None => req.dataset.read_slice::<u8, _, _>(s![.., .., ..]),
                            },
                            InterlaceMode::Plane => match window_bounds(window) {
                                Some((start, end)) => match window.map(|w| w.axis) {
                                    Some(ImageWindowAxis::Cols) => {
                                        req.dataset.read_slice::<u8, _, _>(s![.., .., start..end])
                                    }
                                    Some(ImageWindowAxis::Rows) => {
                                        req.dataset.read_slice::<u8, _, _>(s![.., start..end, ..])
                                    }
                                    None => req.dataset.read_slice::<u8, _, _>(s![.., .., ..]),
                                },
                                None => req.dataset.read_slice::<u8, _, _>(s![.., .., ..]),
                            },
                        } {
                            Ok(d) => d,
                            Err(e) => {
                                send_image_failure(&tx_events, key.clone(), e.to_string());
                                continue;
                            }
                        },
                        4 => match match interlace {
                            InterlaceMode::Pixel => match window_bounds(window) {
                                Some((start, end)) => match window.map(|w| w.axis) {
                                    Some(ImageWindowAxis::Cols) => req
                                        .dataset
                                        .read_slice::<u8, _, _>(s![key.idx, .., start..end, ..]),
                                    Some(ImageWindowAxis::Rows) => req
                                        .dataset
                                        .read_slice::<u8, _, _>(s![key.idx, start..end, .., ..]),
                                    None => {
                                        req.dataset.read_slice::<u8, _, _>(s![key.idx, .., .., ..])
                                    }
                                },
                                None => req.dataset.read_slice::<u8, _, _>(s![key.idx, .., .., ..]),
                            },
                            InterlaceMode::Plane => match window_bounds(window) {
                                Some((start, end)) => match window.map(|w| w.axis) {
                                    Some(ImageWindowAxis::Cols) => req
                                        .dataset
                                        .read_slice::<u8, _, _>(s![key.idx, .., .., start..end]),
                                    Some(ImageWindowAxis::Rows) => req
                                        .dataset
                                        .read_slice::<u8, _, _>(s![key.idx, .., start..end, ..]),
                                    None => {
                                        req.dataset.read_slice::<u8, _, _>(s![key.idx, .., .., ..])
                                    }
                                },
                                None => req.dataset.read_slice::<u8, _, _>(s![key.idx, .., .., ..]),
                            },
                        } {
                            Ok(d) => d,
                            Err(e) => {
                                send_image_failure(&tx_events, key.clone(), e.to_string());
                                continue;
                            }
                        },
                        _ => {
                            send_image_failure(
                                &tx_events,
                                key.clone(),
                                "Invalid shape for Truecolor image",
                            );
                            continue;
                        }
                    };

                    let shape = data.shape();
                    let mut image_buffer = image::RgbaImage::new(shape[1] as u32, shape[0] as u32);
                    match interlace {
                        InterlaceMode::Pixel => {
                            for i in 0..shape[1] {
                                for j in 0..shape[0] {
                                    let pixel = image::Rgba([
                                        data[[j, i, 0]],
                                        data[[j, i, 1]],
                                        data[[j, i, 2]],
                                        255,
                                    ]);
                                    image_buffer.put_pixel(i as u32, j as u32, pixel);
                                }
                            }
                        }
                        InterlaceMode::Plane => {
                            for i in 0..shape[2] {
                                for j in 0..shape[1] {
                                    let pixel = image::Rgba([
                                        data[[0, j, i]],
                                        data[[1, j, i]],
                                        data[[2, j, i]],
                                        255,
                                    ]);
                                    image_buffer.put_pixel(i as u32, j as u32, pixel);
                                }
                            }
                        }
                    }
                    let dyn_img = image::DynamicImage::ImageRgba8(image_buffer);
                    let clipboard_image = clipboard_image_from_dynamic(&dyn_img);
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, key, thread_protocol, clipboard_image);
                }
                ImageType::Indexed(_interlace) => {
                    send_image_failure(&tx_events, key, "Unsupported image format");
                }
                _ => {
                    send_image_failure(&tx_events, key, "Unsupported image format");
                }
            }
        }
    });
    tx_load
}
