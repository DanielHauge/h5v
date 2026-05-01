use ratatui::{layout::Rect, Frame};

use super::{
    image_preview::render_img,
    preview_chart::render_chart_preview,
    state::AppState,
    std_comp_render::{
        render_empty_dataset, render_error, render_string, render_unsupported_rendering,
    },
};
use crate::{
    error::AppError,
    h5f::{read_scalar_string_dataset, Encoding, H5FNode, Node},
    sprint_typedesc::sprint_type_schema,
};

fn compound_schema_preview_text(attr: &crate::h5f::DatasetMeta) -> String {
    let path = attr.virtual_path().unwrap_or(attr.display_name.as_str());
    format!(
        "Compound schema: {path}\n\n{}",
        sprint_type_schema(&attr.type_descriptor)
    )
}

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
        if attr.is_empty() {
            render_empty_dataset(f, &area_inner);
            return;
        }
        if attr.is_compound_container() {
            render_string(
                f,
                &area_inner,
                selected_node,
                compound_schema_preview_text(&attr),
                None,
            );
            return;
        }
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
    node: &mut H5FNode,
) -> Result<(), AppError> {
    let selected_node = &node.node;
    let (dataset, meta) = match selected_node {
        Node::Dataset(ds, attr) => (ds, attr),
        _ => {
            render_unsupported_rendering(
                f,
                area,
                selected_node,
                "Selected node is not a dataset, cannot render string preview",
            );
            return Ok(());
        }
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
        Encoding::Ascii | Encoding::UTF8 | Encoding::UTF8Fixed | Encoding::AsciiFixed => {
            match read_scalar_string_dataset(dataset, &meta.encoding) {
                Ok(x) => render_string(f, area, node, x, meta.hl.clone()),
                Err(e) => render_error(f, area, format!("Error: {}", e)),
            }
        }
    }
    Ok(())
}

pub fn preview_text_for_compound_schema(meta: &crate::h5f::DatasetMeta) -> Option<String> {
    meta.is_compound_container()
        .then(|| compound_schema_preview_text(meta))
}
