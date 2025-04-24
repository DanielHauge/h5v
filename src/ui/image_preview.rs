use std::{
    cell::OnceCell,
    io::BufReader,
    sync::mpsc::{channel, Sender},
    thread,
};

use hdf5_metno::{ByteReader, Dataset, Error, Reader};
use image::ImageFormat;
use ndarray::{s, Array3};
use ratatui::{layout::Rect, Frame};
use ratatui_image::{
    picker::Picker,
    thread::{ResizeRequest, ThreadImage, ThreadProtocol},
};

use crate::h5f::{ImageType, Node};

use super::app::{AppEvent, AppState};

pub fn render_img(
    image_type: &ImageType,
    f: &mut Frame,
    area: &Rect,
    node: &Node,
    state: &mut AppState,
) -> Result<(), Error> {
    match image_type {
        ImageType::JPEG => render_raw_img(f, area, node, state, ImageFormat::Jpeg),
        ImageType::PNG => render_raw_img(f, area, node, state, ImageFormat::Png),
        ImageType::INDEXED => Ok(()),
        ImageType::TRUECOLOR => render_truecolor(f, area, node, state),
        ImageType::GRAYSCALE => Ok(()),
        ImageType::BITMAP => Ok(()),
    }
}

fn render_truecolor(
    f: &mut Frame,
    area: &Rect,
    selected_node: &Node,
    state: &mut AppState,
) -> Result<(), Error> {
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
            if let Some(ref mut protocol) = state.img_state.protocol {
                let image_widget = ThreadImage::new();
                f.render_stateful_widget(image_widget, inner_area, protocol);
            }
        }
        false => {
            state.img_state.protocol = None;
            state.img_state.ds = Some(ds.name());
            let ds_clone = ds.clone();
            state
                .img_state
                .tx_load_img
                .send((ds_clone, ImageType::TRUECOLOR))
                .unwrap();
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
) -> Result<(), Error> {
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
            if let Some(ref mut protocol) = state.img_state.protocol {
                let image_widget = ThreadImage::new();
                f.render_stateful_widget(image_widget, inner_area, protocol);
            }
        }
        false => {
            state.img_state.protocol = None;
            state.img_state.ds = Some(ds.name());
            let ds_reader = ds.as_byte_reader().unwrap();
            let ds_buffered = BufReader::new(ds_reader);
            state
                .img_state
                .tx_load_imgfs
                .send((ds_buffered, img_format))
                .unwrap();
        }
    }

    Ok(())
}

pub fn handle_image_resize(tx_events: Sender<AppEvent>) -> (Sender<ResizeRequest>, Picker) {
    let (tx_worker, rx_worker) = channel::<ResizeRequest>();
    let picker = Picker::from_query_stdio().expect("Failed to create Picker");

    thread::spawn(move || loop {
        if let Ok(request) = rx_worker.recv() {
            if let Ok(resized) = request.resize_encode() {
                tx_events
                    .send(AppEvent::ImageResized(resized))
                    .expect("Failed to send image redraw event");
            } else {
                panic!("Failed to resize image");
            }
        }
    });
    (tx_worker, picker)
}

pub fn handle_imagefs_load(
    tx_events: Sender<AppEvent>,
    tx_worker: Sender<ResizeRequest>,
) -> Sender<(BufReader<ByteReader>, ImageFormat)> {
    let (tx_load, rx_load) = channel::<(BufReader<ByteReader>, ImageFormat)>();
    let picker = Picker::from_query_stdio().expect("Failed to create Picker");

    thread::spawn(move || loop {
        if let Ok((ds_reader, img_format)) = rx_load.recv() {
            if let Ok(dyn_img) = image::load(ds_reader, img_format) {
                let stateful_protocol = picker.new_resize_protocol(dyn_img);
                let thread_protocol =
                    ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                tx_events.send(AppEvent::ImageLoaded(thread_protocol));
            }
        }
    });
    tx_load
}

pub fn handle_image_load(
    tx_events: Sender<AppEvent>,
    tx_worker: Sender<ResizeRequest>,
) -> Sender<(Dataset, ImageType)> {
    let (tx_load, rx_load) = channel::<(Dataset, ImageType)>();
    let picker = Picker::from_query_stdio().expect("Failed to create Picker");
    thread::spawn(move || loop {
        if let Ok((ds_reader, img_format)) = rx_load.recv() {
            match img_format {
                ImageType::GRAYSCALE => todo!(),
                ImageType::BITMAP => todo!(),
                ImageType::TRUECOLOR => {
                    let shape = ds_reader.shape();
                    let data: Array3<u8> =
                        ds_reader.read_slice::<u8, _, _>(s![.., .., ..]).unwrap();

                    let mut image_buffer = image::RgbaImage::new(shape[0] as u32, shape[1] as u32);
                    for i in 0..shape[0] {
                        for j in 0..shape[1] {
                            let pixel = image::Rgba([
                                data[[i, j, 0]],
                                data[[i, j, 1]],
                                data[[i, j, 2]],
                                255,
                            ]);
                            image_buffer.put_pixel(i as u32, j as u32, pixel);
                        }
                    }
                    let dyn_img = image::DynamicImage::ImageRgba8(image_buffer);
                    let stateful_protocol = picker.new_resize_protocol(dyn_img);
                    let thread_protocol =
                        ThreadProtocol::new(tx_worker.clone(), Some(stateful_protocol));
                    tx_events
                        .send(AppEvent::ImageLoaded(thread_protocol))
                        .expect("Failed to send image loaded event");
                }
                ImageType::INDEXED => todo!(),
                _ => unreachable!("This should never happen"),
            }
        }
    });
    tx_load
}
