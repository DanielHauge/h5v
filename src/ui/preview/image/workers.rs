use super::*;

pub(crate) fn handle_image_resize(tx_events: Sender<AppEvent>) -> Sender<ResizeRequest> {
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

pub(crate) fn handle_chartpreview_resize(tx_events: Sender<AppEvent>) -> Sender<ResizeRequest> {
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

pub(crate) fn handle_chartpreview_load(
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
                ChartPreviewSource::ProjectedDataset {
                    ds,
                    meta,
                    selection,
                } => match plot_projected(&ds, meta.as_ref(), &selection) {
                    Ok(data_preview) => data_preview,
                    Err(e) => {
                        send_chart_failure(
                            &tx_events,
                            ChartPreviewKey {
                                ds_path: req.ds_path.clone(),
                                selection: req.selection.clone(),
                            },
                            format!("Failed to plot projected data for chart preview: {}", e),
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

pub(crate) fn handle_imagefs_load(
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

pub(crate) fn handle_imagefsvlen_load(
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
pub(crate) enum ImageResizeResult {
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

pub(crate) fn handle_image_load(
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
