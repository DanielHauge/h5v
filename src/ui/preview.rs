use hdf5_metno::types::{FixedAscii, FixedUnicode, VarLenAscii, VarLenUnicode};
use ratatui::{
    layout::Rect,
    Frame,
};

use super::{
    image_preview::render_img,
    preview_chart::render_chart_preview,
    state::AppState,
    std_comp_render::{render_error, render_string, render_unsupported_rendering},
};
use crate::{
    error::AppError,
    h5f::{Encoding, H5FNode, Node},
};

pub fn render_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node: &mut H5FNode,
    state: &mut AppState,
) {
    let area_inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let node = selected_node.node.clone();

    if let Node::Dataset(_, attr) = node {
        match &attr.image {
            Some(image_type) => {
                match render_img(image_type, f, &area_inner, selected_node, state) {
                    Ok(()) => {}
                    Err(e) => {
                        render_error(f, &area_inner, format!("Render img error: {}", e));
                    }
                }
            }
            None => {
                if attr.matrixable.is_none() {
                    match render_string_preview(f, &area_inner, selected_node) {
                        Ok(()) => {}
                        Err(e) => {
                            render_error(f, &area_inner, format!("Render string error: {}", e));
                        }
                    }
                } else {
                    match render_chart_preview(f, &area_inner, selected_node, state) {
                        Ok(()) => {}
                        Err(e) => {
                            render_error(f, &area_inner, format!("Render chart error: {}", e));
                        }
                    }
                }
            }
        }
    }
}

pub fn render_string_preview(
    f: &mut Frame,
    area: &Rect,
    selected_node_refc: &mut H5FNode,
) -> Result<(), AppError> {
    let selected_node = &selected_node_refc.node;
    let (dataset, meta) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => panic!("Expected a string dataset to preview string data"),
    };

    match meta.encoding {
        Encoding::LittleEndian => {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "LittleEndian not supported for string data",
            );
        }
        Encoding::Unknown => {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Unknown encoding not supported for string data",
            );
        }
        Encoding::Ascii => match dataset.read_scalar::<VarLenAscii>() {
            Ok(x) => render_string(f, area, x),
            Err(e) => render_error(f, area, format!("Error: {}", e)),
        },
        Encoding::UTF8 => match dataset.read_scalar::<VarLenUnicode>() {
            Ok(x) => render_string(f, area, x),
            Err(e) => render_error(f, area, format!("Error: {}", e)),
        },
        Encoding::UTF8Fixed => match dataset.read_scalar::<FixedUnicode<32768>>() {
            Ok(x) => render_string(f, area, x.to_string()),
            Err(e) => render_error(f, area, format!("Error: {}", e)),
        },
        Encoding::AsciiFixed => match dataset.read_scalar::<FixedAscii<32768>>() {
            Ok(x) => render_string(f, area, x.to_string()),
            Err(e) => render_error(f, area, format!("Error: {}", e)),
        },
    }
    Ok(())
}
