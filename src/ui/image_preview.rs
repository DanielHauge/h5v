use std::{
    io::BufReader,
    sync::mpsc::{channel, Sender},
    thread,
};

use hdf5_metno::{ByteReader, Dataset};
use image::ImageFormat;
use ndarray::{s, Array2, Array3};
use ratatui::{layout::Rect, Frame};
use ratatui_image::{
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
    StatefulImage,
};

use crate::{
    error::AppError,
    h5f::{ImageType, InterlaceMode, Node},
};

use super::{app::AppEvent, state::AppState};

pub fn render_img(
    image_type: &ImageType,
    f: &mut Frame,
    area: &Rect,
    node: &Node,
    state: &mut AppState,
) -> Result<(), AppError> {
    match image_type {
        ImageType::Jpeg => render_raw_img(f, area, node, state, ImageFormat::Jpeg),
        ImageType::Png => render_raw_img(f, area, node, state, ImageFormat::Png),
        ImageType::Truecolor(m) => {
            render_ds_img(f, area, node, state, ImageType::Truecolor(m.clone()))
        }
        ImageType::Grayscale => render_ds_img(f, area, node, state, ImageType::Grayscale),
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
            let ds_clone = ds.clone();
            state.img_state.tx_load_img.send((ds_clone, img_type))?;
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
            let ds_reader = ds.as_byte_reader()?;
            let ds_buffered = BufReader::new(ds_reader);
            state
                .img_state
                .tx_load_imgfs
                .send((ds_buffered, img_format))
                .expect("Failed to send image load request");
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
) -> Sender<(BufReader<ByteReader>, ImageFormat)> {
    let (tx_load, rx_load) = channel::<(BufReader<ByteReader>, ImageFormat)>();
    let picker = Picker::from_query_stdio().unwrap_or(Picker::from_fontsize((7, 14)));

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

pub enum ImageResizeResult {
    Success(ResizeResponse),
    Error(String),
}

pub enum ImageLoadedResult {
    Success(ThreadProtocol),
    Failure(String),
}

pub fn handle_image_load(
    tx_events: Sender<AppEvent>,
    tx_worker: Sender<ResizeRequest>,
) -> Sender<(Dataset, ImageType)> {
    let (tx_load, rx_load) = channel::<(Dataset, ImageType)>();
    let picker = Picker::from_query_stdio().unwrap_or(Picker::from_fontsize((7, 14)));
    thread::spawn(move || loop {
        if let Ok((ds_reader, img_format)) = rx_load.recv() {
            match img_format {
                ImageType::Grayscale => {
                    let shape = ds_reader.shape();
                    let data: Array2<u8> = match ds_reader.read_slice::<u8, _, _>(s![.., ..]) {
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
                    let mut image_buffer = image::GrayImage::new(shape[1] as u32, shape[0] as u32);
                    for i in 0..shape[1] {
                        for j in 0..shape[0] {
                            let pixel = image::Luma([data[[j, i]]]);
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
                    let data: Array3<u8> = match ds_reader.read_slice::<u8, _, _>(s![.., .., ..]) {
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
