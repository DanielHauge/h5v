use std::{
    io::BufReader,
    sync::mpsc::{channel, Sender},
    thread,
};

use hdf5_metno::{types::IntSize, ByteReader, Dataset, Selection};
use image::ImageFormat;
use ndarray::{s, Array2, Array3};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    Frame,
};
use ratatui_image::{
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
    StatefulImage,
};

use crate::{
    error::AppError,
    h5f::{H5FNode, ImageType, InterlaceMode, Node},
};

use super::{
    app::AppEvent,
    segment_scroll::render_segment_scroll,
    state::{AppState, SegmentType},
};

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
            Layout::vertical(vec![Constraint::Length(2), Constraint::Min(1)]).split(*area);
        render_segment_scroll(f, &areas_split[0], state)?;
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
    match state.img_state.is_from_ds(selected_node) {
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
            state
                .img_state
                .tx_load_img
                .send((ds_clone, state.segment_state.idx, img_type))?;
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

    match state.img_state.is_from_ds(selected_node) {
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
            state.img_state.ds = Some(ds.name());
            let typedesc = ds
                .dtype()
                .expect("Dataset dtype should be set")
                .to_descriptor()
                .unwrap();
            match typedesc {
                hdf5_metno::types::TypeDescriptor::Unsigned(IntSize::U1) => {
                    let ds_reader = ds.as_byte_reader()?;
                    state.segment_state.segumented = SegmentType::NoSegment;
                    let ds_buffered = BufReader::new(ds_reader);
                    state
                        .img_state
                        .tx_load_imgfs
                        .send((ds_buffered, img_format))
                        .expect("Failed to send image load request");
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
                            .send((ds.clone(), i, img_format))
                            .expect("Failed to send image load request");
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
                Ok(r) => tx_events
                    .send(AppEvent::ImageResized(ImageResizeResult::Success(r)))
                    .expect("Failed to send image redraw event"),
                Err(e) => tx_events
                    .send(AppEvent::ImageResized(ImageResizeResult::Error(
                        e.to_string(),
                    )))
                    .expect("Failed to send image redraw event"),
            }
        }
    });
    tx_worker
}

pub fn handle_imagefs_load(
    tx_events: Sender<AppEvent>,
    tx_worker: Sender<ResizeRequest>,
    picker: Picker,
) -> Sender<(BufReader<ByteReader>, ImageFormat)> {
    let (tx_load, rx_load) = channel::<(BufReader<ByteReader>, ImageFormat)>();

    thread::spawn(move || loop {
        if let Ok((ds_reader, img_format)) = rx_load.recv() {
            if let Ok(dyn_img) = image::load(ds_reader, img_format) {
                let stateful_protocol = picker.new_resize_protocol(dyn_img);
                let thread_protocol =
                    ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                tx_events
                    .send(AppEvent::ImageLoad(ImageLoadedResult::Success(
                        thread_protocol,
                    )))
                    .expect("Failed to send image loaded event");
            }
        }
    });
    tx_load
}

pub fn handle_imagefsvlen_load(
    tx_events: Sender<AppEvent>,
    tx_worker: Sender<ResizeRequest>,
    picker: Picker,
) -> Sender<(Dataset, i32, ImageFormat)> {
    let (tx_load, rx_load) = channel::<(Dataset, i32, ImageFormat)>();

    thread::spawn(move || loop {
        if let Ok((ds, idx, img_format)) = rx_load.recv() {
            let data =
                match ds.read_slice_1d::<hdf5_metno::types::VarLenArray<u8>, _>(Selection::All) {
                    Ok(d) => d[idx as usize].as_slice().to_vec(),
                    Err(e) => {
                        tx_events
                            .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                e.to_string(),
                            )))
                            .expect("Failed to send image loaded event");
                        continue;
                    }
                };

            let cursor = std::io::Cursor::new(data);
            let data = BufReader::new(cursor);
            if let Ok(dyn_img) = image::load(data, img_format) {
                let stateful_protocol = picker.new_resize_protocol(dyn_img);
                let thread_protocol =
                    ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                tx_events
                    .send(AppEvent::ImageLoad(ImageLoadedResult::Success(
                        thread_protocol,
                    )))
                    .expect("Failed to send image loaded event");
            }
        }
    });
    tx_load
}

pub enum ImageResizeResult {
    Success(ResizeResponse),
    Error(String),
}

pub enum ImageLoadedResult {
    Success(ThreadProtocol),
    Failure(String),
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
        let dtype = self
            .dtype()
            .expect("Should get dtype from dataset")
            .to_descriptor()
            .unwrap();
        match dtype {
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
) -> Sender<(Dataset, i32, ImageType)> {
    let (tx_load, rx_load) = channel::<(Dataset, i32, ImageType)>();
    thread::spawn(move || loop {
        if let Ok((ds_reader, idx, img_format)) = rx_load.recv() {
            match img_format {
                ImageType::Grayscale => {
                    let shape = ds_reader.shape();
                    // panic!("idx: {idx}, shape: {shape:?}");
                    let bit_depth = ds_reader.bit_depth();

                    let dyn_img = match bit_depth {
                        BitDepth::Bit8 => {
                            let data: Array2<u8> = match shape.len() {
                                2 => match ds_reader.read_slice::<u8, _, _>(s![.., ..]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        tx_events
                                            .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                                e.to_string(),
                                            )))
                                            .expect("Failed to send image loaded event");
                                        continue;
                                    }
                                },
                                3 => match ds_reader.read_slice::<u8, _, _>(s![idx, .., ..]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        tx_events
                                            .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                                e.to_string(),
                                            )))
                                            .expect("Failed to send image loaded event");
                                        continue;
                                    }
                                },
                                4 => match ds_reader.read_slice::<u8, _, _>(s![idx, .., .., 0]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        tx_events
                                            .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                                e.to_string(),
                                            )))
                                            .expect("Failed to send image loaded event");
                                        continue;
                                    }
                                },
                                _ => {
                                    tx_events
                                        .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                            "Invalid shape for Grayscale image".to_string(),
                                        )))
                                        .expect("Failed to send image loaded event");
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
                                2 => match ds_reader.read_slice::<u16, _, _>(s![.., ..]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        tx_events
                                            .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                                e.to_string(),
                                            )))
                                            .expect("Failed to send image loaded event");
                                        continue;
                                    }
                                },
                                3 => match ds_reader.read_slice::<u16, _, _>(s![idx, .., ..]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        tx_events
                                            .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                                e.to_string(),
                                            )))
                                            .expect("Failed to send image loaded event");
                                        continue;
                                    }
                                },
                                4 => match ds_reader.read_slice::<u16, _, _>(s![idx, .., .., 0]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        tx_events
                                            .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                                e.to_string(),
                                            )))
                                            .expect("Failed to send image loaded event");
                                        continue;
                                    }
                                },
                                _ => {
                                    tx_events
                                        .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                            "Invalid shape for Grayscale image".to_string(),
                                        )))
                                        .expect("Failed to send image loaded event");
                                    continue;
                                }
                            };
                            // Convert 12-bit to 8-bit. We dont want the full 16-bit range, only
                            // up to 4096 (2^12)
                            // first set anything larger than 4095 to 4095
                            let data = data.mapv(|x| if x > 4095 { 4095 } else { x });
                            // then scale to 0-255
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
                            unimplemented!("Unknown bit depth for Grayscale image")
                        }
                    };

                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    tx_events
                        .send(AppEvent::ImageLoad(ImageLoadedResult::Success(
                            thread_protocol,
                        )))
                        .expect("Failed to send image loaded event");
                }
                ImageType::Bitmap => {
                    let shape = ds_reader.shape();
                    let data: Array2<bool> = match ds_reader.read_slice::<bool, _, _>(s![.., ..]) {
                        Ok(d) => d,
                        Err(e) => {
                            tx_events
                                .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                    e.to_string(),
                                )))
                                .expect("Failed to send image loaded event");
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
                    tx_events
                        .send(AppEvent::ImageLoad(ImageLoadedResult::Success(
                            thread_protocol,
                        )))
                        .expect("Failed to send image loaded event");
                }
                ImageType::Truecolor(interlace) => {
                    let shape = ds_reader.shape();
                    let data = match shape.len() {
                        3 => {
                            let data: Array3<u8> =
                                match ds_reader.read_slice::<u8, _, _>(s![.., .., ..]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        tx_events
                                            .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                                e.to_string(),
                                            )))
                                            .expect("Failed to send image loaded event");
                                        continue;
                                    }
                                };
                            data
                        }
                        4 => {
                            let data: Array3<u8> =
                                match ds_reader.read_slice::<u8, _, _>(s![idx, .., .., ..]) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        tx_events
                                            .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                                e.to_string(),
                                            )))
                                            .expect("Failed to send image loaded event");
                                        continue;
                                    }
                                };
                            data
                        }
                        _ => {
                            tx_events
                                .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                                    "Invalid shape for Truecolor image".to_string(),
                                )))
                                .expect("Failed to send image loaded event");

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
                    tx_events
                        .send(AppEvent::ImageLoad(ImageLoadedResult::Success(
                            thread_protocol,
                        )))
                        .expect("Failed to send image loaded event");
                }
                ImageType::Indexed(_interlace) => {
                    tx_events
                        .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                            "Unsupported image format".to_string(),
                        )))
                        .expect("Failed to send image loaded event");
                    continue;
                }
                _ => {
                    tx_events
                        .send(AppEvent::ImageLoad(ImageLoadedResult::Failure(
                            "Unsupported image format".to_string(),
                        )))
                        .expect("Failed to send image loaded event");
                    continue;
                }
            }
        }
    });
    tx_load
}
