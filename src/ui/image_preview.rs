use std::{
    io::BufReader,
    sync::mpsc::{channel, Sender},
    thread,
};

use hdf5_metno::{types::IntSize, Dataset, Selection};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
use ndarray::{s, Array2, Array3};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    Frame,
};
use ratatui_image::{
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
    StatefulImage,
};

use crate::{
    data::Previewable,
    error::AppError,
    h5f::{H5FNode, ImageType, InterlaceMode, Node},
    ui::{
        preview_chart::{render_image_chart, MAX_SEGMENT_SIZE},
        state::{ChartPreviewLoadRequest, ChartPreviewSource, IsFromDs},
    },
};

use super::app::{ChartPreviewLoadedResult, ImageLoadedResult};
use super::{
    app::AppEvent,
    segment_scroll::render_segment_scroll,
    state::{
        AppState, ChartPreviewKey, DatasetImageLoadRequest, ImageLoadKey, RawImageLoadRequest,
        SegmentType, VarLenImageLoadRequest,
    },
};

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

fn send_image_success(tx_events: &Sender<AppEvent>, key: ImageLoadKey, protocol: ThreadProtocol) {
    send_event(
        tx_events,
        AppEvent::ImageLoad(ImageLoadedResult::Success { key, protocol }),
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
) {
    send_event(
        tx_events,
        AppEvent::PreviewChartLoad(ChartPreviewLoadedResult::Success { key, protocol }),
    );
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
    let area = if let SegmentType::Image = state.segment_state.segumented {
        let areas_split =
            Layout::horizontal(vec![Constraint::Min(1), Constraint::Length(2)]).split(*area);
        render_segment_scroll(f, &areas_split[1], state)?;
        let areas_split =
            Layout::vertical(vec![Constraint::Length(1), Constraint::Min(2)]).split(areas_split[0]);
        // center styles
        let idx = state.segment_state.idx + 1;
        let segment_count = state.segment_state.segment_count;
        let block = ratatui::widgets::Block::default()
            .title(format!(" Image {}/{} ", idx, segment_count))
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(ratatui::widgets::Borders::TOP)
            .border_type(ratatui::widgets::BorderType::Plain)
            .style(Style::default().fg(ratatui::style::Color::DarkGray));

        f.render_widget(block, areas_split[0]);
        areas_split[1]
    } else {
        *area
    };
    match image_type {
        ImageType::Jpeg => render_raw_img(f, &area, node, state, ImageFormat::Jpeg),
        ImageType::Png => render_raw_img(f, &area, node, state, ImageFormat::Png),
        ImageType::Truecolor(m) => {
            render_ds_img(f, &area, node, state, ImageType::Truecolor(m.clone()))
        }
        ImageType::Grayscale => render_ds_img(f, &area, node, state, ImageType::Grayscale),
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

    let inner_area = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    if state.should_debounce_preview(selected_node) {
        f.render_widget("Loading image preview...", inner_area);
        return Ok(());
    }
    let image_loaded = state.img_state.is_from_ds(selected_node)
        && (!matches!(state.segment_state.segumented, SegmentType::Image)
            || state.img_state.idx_loaded == state.img_state.idx_to_load);

    match image_loaded {
        true => {
            if let Some(e) = &state.img_state.error {
                let error_msg = format!("Error loading image: {}", e);
                f.render_widget(error_msg, inner_area);
            } else if let Some(ref mut protocol) = state.img_state.protocol {
                let image_widget = StatefulImage::default();
                f.render_stateful_widget(image_widget, inner_area, protocol);
            }
        }
        false => {
            state.img_state.protocol = None;
            state.img_state.error = None;
            state.img_state.ds = Some(ds.name());
            if ds.shape().len() == 4 {
                state.segment_state.segumented = SegmentType::Image;
            }
            if let SegmentType::Image = state.segment_state.segumented {
                state.segment_state.segment_count = ds.shape()[0] as i32
            };
            let ds_clone = ds.clone();
            let i = state.img_state.idx_to_load;
            state.img_state.idx_loaded = i;
            state.segment_state.idx = i;
            state.img_state.tx_load_img.send(DatasetImageLoadRequest {
                key: ImageLoadKey {
                    ds_path: ds.name(),
                    idx: state.segment_state.idx,
                },
                dataset: ds_clone,
                image_type: img_type,
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

    let inner_area = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    if state.should_debounce_preview(selected_node) {
        f.render_widget("Loading image preview...", inner_area);
        return Ok(());
    }

    let image_loaded = state.img_state.is_from_ds(selected_node)
        && (!matches!(state.segment_state.segumented, SegmentType::Image)
            || state.img_state.idx_loaded == state.img_state.idx_to_load);

    match image_loaded {
        true => match state.img_state.error {
            Some(ref e) => {
                let error_msg = format!("Error loading image - {}", e);
                f.render_widget(error_msg, inner_area);
            }
            None => {
                if let Some(ref mut protocol) = state.img_state.protocol {
                    let image_widget = StatefulImage::new();
                    f.render_stateful_widget(image_widget, inner_area, protocol);
                }
            }
        },
        false => {
            state.img_state.protocol = None;
            state.img_state.error = None;
            state.img_state.ds = Some(ds.name());
            let typedesc = ds.dtype()?.to_descriptor()?;
            match typedesc {
                hdf5_metno::types::TypeDescriptor::Unsigned(IntSize::U1) => {
                    let ds_reader = ds.as_byte_reader()?;
                    state.segment_state.segumented = SegmentType::NoSegment;
                    state.img_state.idx_loaded = state.img_state.idx_to_load;
                    let ds_buffered = BufReader::new(ds_reader);
                    state.img_state.tx_load_imgfs.send(RawImageLoadRequest {
                        key: ImageLoadKey {
                            ds_path: ds.name(),
                            idx: state.img_state.idx_loaded,
                        },
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
                        state.img_state.idx_loaded = i;
                        state.segment_state.segumented = SegmentType::Image;
                        state.segment_state.segment_count = ds.shape()[0] as i32;
                        state.segment_state.idx = i;
                        state
                            .img_state
                            .tx_load_imgfsvlen
                            .send(VarLenImageLoadRequest {
                                key: ImageLoadKey {
                                    ds_path: ds.name(),
                                    idx: i,
                                },
                                dataset: ds.clone(),
                                format: img_format,
                            })?;
                    }
                }
                _ => {
                    state.img_state.error = Some("Unsupported image format".to_string());
                    let error_msg = format!("Unsupported image format for dataset: {}", ds.name());
                    f.render_widget(error_msg, inner_area);
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

            let stateful_protocol = picker.new_resize_protocol(dyn_img);
            let thread_protocol = ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
            send_chart_success(
                &tx_events,
                ChartPreviewKey {
                    ds_path: req.ds_path,
                    selection: req.selection,
                },
                thread_protocol,
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
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, req.key, thread_protocol);
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
                Ok(d) => d[req.key.idx as usize].as_slice().to_vec(),
                Err(e) => {
                    send_image_failure(&tx_events, req.key, e.to_string());
                    continue;
                }
            };

            let cursor = std::io::Cursor::new(data);
            let data = BufReader::new(cursor);
            match image::load(data, req.format) {
                Ok(dyn_img) => {
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, req.key, thread_protocol);
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
            match req.image_type {
                ImageType::Grayscale => {
                    let shape = req.dataset.shape();
                    let bit_depth = req.dataset.bit_depth();

                    let dyn_img = match bit_depth {
                        BitDepth::Bit8 => {
                            let data: Array2<u8> = match shape.len() {
                                2 => match req.dataset.read_slice::<u8, _, _>(s![.., ..]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        send_image_failure(&tx_events, key.clone(), e.to_string());
                                        continue;
                                    }
                                },
                                3 => {
                                    match req.dataset.read_slice::<u8, _, _>(s![key.idx, .., ..]) {
                                        Ok(d) => d,
                                        Err(e) => {
                                            send_image_failure(
                                                &tx_events,
                                                key.clone(),
                                                e.to_string(),
                                            );
                                            continue;
                                        }
                                    }
                                }
                                4 => {
                                    match req.dataset.read_slice::<u8, _, _>(s![key.idx, .., .., 0])
                                    {
                                        Ok(d) => d,
                                        Err(e) => {
                                            send_image_failure(
                                                &tx_events,
                                                key.clone(),
                                                e.to_string(),
                                            );
                                            continue;
                                        }
                                    }
                                }
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
                                2 => match req.dataset.read_slice::<u16, _, _>(s![.., ..]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        send_image_failure(&tx_events, key.clone(), e.to_string());
                                        continue;
                                    }
                                },
                                3 => match req.dataset.read_slice::<u16, _, _>(s![key.idx, .., ..])
                                {
                                    Ok(d) => d,
                                    Err(e) => {
                                        send_image_failure(&tx_events, key.clone(), e.to_string());
                                        continue;
                                    }
                                },
                                4 => match req.dataset.read_slice::<u16, _, _>(s![
                                    key.idx,
                                    ..,
                                    ..,
                                    0
                                ]) {
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

                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, key, thread_protocol);
                }
                ImageType::Bitmap => {
                    let shape = req.dataset.shape();
                    let data: Array2<bool> = match req.dataset.read_slice::<bool, _, _>(s![.., ..])
                    {
                        Ok(d) => d,
                        Err(e) => {
                            send_image_failure(&tx_events, key.clone(), e.to_string());
                            continue;
                        }
                    };
                    let mut image_buffer = image::GrayImage::new(shape[0] as u32, shape[1] as u32);
                    for i in 0..shape[1] {
                        for j in 0..shape[0] {
                            let pixel = if data[[i, j]] {
                                image::Luma([255])
                            } else {
                                image::Luma([0])
                            };
                            image_buffer.put_pixel(i as u32, j as u32, pixel);
                        }
                    }
                    let dyn_img = image::DynamicImage::ImageLuma8(image_buffer);
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, key, thread_protocol);
                }
                ImageType::Truecolor(interlace) => {
                    let shape = req.dataset.shape();
                    let data: Array3<u8> = match shape.len() {
                        3 => match req.dataset.read_slice::<u8, _, _>(s![.., .., ..]) {
                            Ok(d) => d,
                            Err(e) => {
                                send_image_failure(&tx_events, key.clone(), e.to_string());
                                continue;
                            }
                        },
                        4 => match req.dataset.read_slice::<u8, _, _>(s![key.idx, .., .., ..]) {
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
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    send_image_success(&tx_events, key, thread_protocol);
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
