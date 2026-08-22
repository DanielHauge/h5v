use std::{cell::RefCell, rc::Rc};

use hdf5_metno::types::{TypeDescriptor, VarLenUnicode};
use ratatui::{
    layout::{Alignment, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::{
    configure,
    error::AppError,
    h5f::{DatasetHandle, DatasetMetaState, H5FNode, HasPath, Node},
    ui::{
        self,
        custom_content::render_custom_content_mode,
        heatmap::render_heatmap,
        matrix::{DefaultMatrixResultRenderIntercept, EnumRenderer},
        render::MatrixRenderType,
    },
};

use super::{
    attributes::{prepare_metadata_layout, render_info_attributes},
    matrix::{
        render_compound_root_matrix, render_matrix, render_not_yet_implemented,
        render_opaque_matrix, render_projected_matrix, render_varlen_u8_matrix,
    },
    preview::render_preview,
    state::{AppState, ContentShowMode},
    std_comp_render::render_empty_dataset,
};

fn split_main_display(
    area: Rect,
    focus: &ui::state::Focus,
    prepared_attributes: Option<&super::attributes::PreparedMetadataLayout>,
) -> (Rect, Rect) {
    let layout = configure::current_auto_layout_settings();
    let focused_attributes_constraint = attribute_constraint(
        &layout.attributes.focused,
        prepared_attributes.map(|layout| layout.preferred_panel_height()),
    );
    let (attributes_constraint, content_constraint) = match super::app::main_content_focus(focus) {
        ui::state::LastFocused::Attributes => (
            focused_attributes_constraint,
            layout.content.unfocused.as_constraint(),
        ),
        ui::state::LastFocused::Content => (
            layout.attributes.unfocused.as_constraint(),
            layout.content.focused.as_constraint(),
        ),
    };
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([attributes_constraint, content_constraint])
        .split(area);
    (chunks[0], chunks[1])
}

fn attribute_constraint(
    size: &configure::LayoutSize,
    preferred_height: Option<u16>,
) -> ratatui::layout::Constraint {
    match (size, preferred_height) {
        (configure::LayoutSize::Max(cap), Some(preferred)) => {
            ratatui::layout::Constraint::Length(preferred.min(*cap).max(3))
        }
        (configure::LayoutSize::Min(floor), Some(preferred)) => {
            ratatui::layout::Constraint::Length(preferred.max(*floor))
        }
        _ => size.as_constraint(),
    }
}

fn content_mode_title(handle: &crate::configure::registry::ContentModeHandle) -> String {
    if let Some(mode) = ContentShowMode::parse_handle(handle.as_str()) {
        return match mode {
            ContentShowMode::Preview => {
                configure::configured_symbol(|symbols| symbols.title.preview)
            }
            ContentShowMode::Matrix => {
                configure::configured_symbol(|symbols| symbols.title.matrix_tab)
            }
            ContentShowMode::Heatmap => "# Heatmap",
        }
        .to_string();
    }

    configure::current_registry_snapshot()
        .content_mode(handle)
        .map(|metadata| metadata.title.clone())
        .unwrap_or_else(|| handle.as_str().to_string())
}

fn metadata_error_message(node: &H5FNode) -> Option<&str> {
    node.metadata_error.as_deref()
}

pub fn render_main_display(
    f: &mut Frame,
    area: &Rect,
    selected_node_no: &Rc<RefCell<H5FNode>>,
    state: &mut AppState,
) -> std::result::Result<(), AppError> {
    let mut node = selected_node_no.borrow_mut();
    let show_metadata = state.show_tree_view
        && !matches!(
            ContentShowMode::parse_handle(state.active_content_mode_handle().as_str()),
            Some(ContentShowMode::Heatmap)
        );
    let prepared_attributes = if show_metadata {
        node.computed_attributes
            .is_some()
            .then(|| prepare_metadata_layout(&mut node, area.width))
            .transpose()?
    } else {
        None
    };

    let content_area = if show_metadata {
        let (attr_area, content_area) =
            split_main_display(*area, &state.focus, prepared_attributes.as_ref());
        render_info_attributes(
            f,
            &attr_area,
            &mut node,
            state,
            prepared_attributes.as_ref(),
        )?;
        content_area
    } else {
        *area
    };
    state.ui_layout.content = Some(content_area);
    state.ui_layout.content_tabs.clear();
    state.ui_layout.matrix_rows.clear();
    state.ui_layout.matrix_cells.clear();

    if show_metadata {
        if let Some(error) = metadata_error_message(&node) {
            f.render_widget(
                Paragraph::new(format!("Dataset metadata unavailable: {error}"))
                    .alignment(Alignment::Center)
                    .style(
                        Style::default().bg(configure::themed_color(|colors| colors.surface.bg)),
                    ),
                content_area,
            );
            return Ok(());
        }
    }

    if show_metadata && matches!(node.node, Node::Dataset(_, DatasetMetaState::Pending(_))) {
        f.render_widget(
            Paragraph::new("Loading dataset metadata...")
                .alignment(Alignment::Center)
                .style(Style::default().bg(configure::themed_color(|colors| colors.surface.bg))),
            content_area,
        );
        return Ok(());
    }

    let selected_path = node.node.path();
    let selected_kind = match &node.node {
        Node::File(_) => "file",
        Node::Group(_, _) => "group",
        Node::Dataset(_, _) => "dataset",
        Node::Broken(_) => "broken",
    };
    let attribute_names: Vec<String> = node
        .computed_attributes
        .as_ref()
        .map(|attributes| {
            attributes
                .attributes
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let available_handles = state.available_content_mode_handles_for_item(
        node.content_show_modes(),
        &selected_path,
        selected_kind,
        &attribute_names,
    );
    let supported_display_modes = configure::ordered_content_mode_handles(&available_handles);
    if supported_display_modes.is_empty() {
        let no_data_message = match &node.node {
            Node::Dataset(_, DatasetMetaState::Loaded(meta)) if meta.is_compound_container() => {
                "Compound"
            }
            _ => "Group",
        };
        let paragraph = Paragraph::new(no_data_message)
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .bg(configure::themed_color(|colors| colors.surface.bg))
                    .fg(configure::themed_color(|colors| colors.content.empty_state)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, content_area);
        return Ok(());
    }
    let current_display_mode = state.active_content_mode_handle();
    let is_supported = supported_display_modes.contains(&current_display_mode);
    let supported_modes_count = supported_display_modes.len();
    let display_mode = if is_supported {
        current_display_mode
    } else {
        supported_display_modes
            .first()
            .cloned()
            .unwrap_or_else(|| ContentShowMode::Preview.handle())
    };
    let display_index = supported_display_modes
        .iter()
        .position(|handle| *handle == display_mode)
        .unwrap_or(0);

    // Do tab titles:

    let mut tab_titles = vec![];
    let mut tab_layout = Vec::new();
    for (i, handle) in supported_display_modes.iter().enumerate() {
        let title = content_mode_title(handle);
        let padded = format!(" {title} ");
        tab_layout.push((
            handle.clone(),
            padded.clone(),
            Line::from(padded.as_str()).width() as u16,
        ));

        if i == display_index {
            tab_titles.push(Span::styled(
                padded,
                Style::default()
                    .fg(configure::themed_color(|colors| colors.accent.selection_fg))
                    .bg(configure::themed_color(|colors| colors.accent.selection_bg))
                    .bold(),
            ));
        } else {
            tab_titles.push(Span::styled(
                padded,
                Style::default()
                    .fg(configure::themed_color(|colors| colors.help.description))
                    .bg(configure::themed_color(|colors| colors.surface.help_key_bg))
                    .bold(),
            ));
        }
        if i != supported_modes_count - 1 {
            tab_titles.push(Span::styled(
                "  ",
                configure::themed_color(|colors| colors.help.muted),
            ));
        }
    }

    let title = Line::from(tab_titles);
    let title_width = title.width() as u16;
    let title_start_x = content_area
        .x
        .saturating_add(content_area.width.saturating_sub(title_width) / 2);
    let mut current_x = title_start_x;
    let separator_width = Line::from("  ").width() as u16;
    for (i, (mode, _, width)) in tab_layout.iter().enumerate() {
        state
            .ui_layout
            .content_tabs
            .push(super::state::ContentTabHitbox {
                area: Rect {
                    x: current_x,
                    y: content_area.y,
                    width: *width,
                    height: 1,
                },
                mode: mode.clone(),
            });
        current_x = current_x.saturating_add(*width);
        if i != supported_modes_count - 1 {
            current_x = current_x.saturating_add(separator_width);
        }
    }

    let bg_color = match (&state.focus, &state.mode) {
        (
            ui::state::Focus::Content,
            ui::state::Mode::Normal
            | ui::state::Mode::AttributeCreateDialog
            | ui::state::Mode::AttributeDeleteDialog
            | ui::state::Mode::FixedStringOverflowDialog
            | ui::state::Mode::FixedStringResizeDialog,
        ) => configure::themed_color(|colors| colors.surface.focus_bg),
        _ => configure::themed_color(|colors| colors.surface.bg),
    };
    f.render_widget(
        Paragraph::new(title)
            .alignment(Alignment::Center)
            .style(Style::default().bg(bg_color)),
        content_area,
    );
    match ContentShowMode::parse_handle(display_mode.as_str()) {
        Some(ContentShowMode::Preview) => render_preview(f, &content_area, &mut node, state),
        Some(ContentShowMode::Matrix) => {
            //
            let (ds, attr) = match node.node.clone() {
                Node::Dataset(DatasetHandle::Loaded(ds), DatasetMetaState::Loaded(attr)) => {
                    (ds, attr)
                }
                _ => {
                    render_not_yet_implemented(
                        f,
                        &content_area,
                        "Matrix mode is only available for datasets",
                    );
                    return Ok(());
                }
            };
            if attr.is_empty() {
                render_empty_dataset(f, &content_area);
                return Ok(());
            }
            if attr.is_compound_container() {
                render_compound_root_matrix(f, &content_area, &ds, &attr, &mut node, state)?;
                return Ok(());
            }
            if attr.is_opaque() {
                render_opaque_matrix(f, &content_area, &ds, &attr, &mut node, state)?;
                return Ok(());
            }
            match attr.matrixable {
                None => {
                    return Ok(());
                }
                Some(ref x) => match x {
                    MatrixRenderType::Float64 => {
                        if attr.is_compound_leaf() {
                            render_projected_matrix::<f64>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                DefaultMatrixResultRenderIntercept,
                            )?
                        } else {
                            render_matrix::<f64>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                DefaultMatrixResultRenderIntercept,
                            )?
                        }
                    }
                    MatrixRenderType::Uint64 => {
                        if attr.is_compound_leaf() {
                            render_projected_matrix::<u64>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                DefaultMatrixResultRenderIntercept,
                            )?
                        } else {
                            render_matrix::<u64>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                DefaultMatrixResultRenderIntercept,
                            )?
                        }
                    }
                    MatrixRenderType::Int64 => {
                        if attr.is_compound_leaf() {
                            render_projected_matrix::<i64>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                DefaultMatrixResultRenderIntercept,
                            )?
                        } else {
                            render_matrix::<i64>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                DefaultMatrixResultRenderIntercept,
                            )?
                        }
                    }
                    MatrixRenderType::Opaque => {
                        render_opaque_matrix(f, &content_area, &ds, &attr, &mut node, state)?
                    }
                    MatrixRenderType::Compound => {
                        render_not_yet_implemented(f, &content_area, "Compound matrix")
                    }
                    MatrixRenderType::Strings => {
                        if attr.is_compound_leaf() {
                            render_projected_matrix::<String>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                DefaultMatrixResultRenderIntercept,
                            )?
                        } else {
                            render_matrix::<VarLenUnicode>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                DefaultMatrixResultRenderIntercept,
                            )?
                        }
                    }
                    MatrixRenderType::Enum => {
                        let TypeDescriptor::Enum(et) = attr.type_descriptor.clone() else {
                            render_not_yet_implemented(
                                f,
                                &content_area,
                                "Matrix enum metadata is inconsistent with the dataset type",
                            );
                            return Ok(());
                        };

                        let enum_mapper =
                            EnumRenderer::with_overrides(et, attr.enum_render_overrides.as_ref());
                        if attr.is_compound_leaf() {
                            render_projected_matrix::<u64>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                enum_mapper,
                            )?
                        } else {
                            render_matrix::<u64>(
                                f,
                                &content_area,
                                &ds,
                                &attr,
                                &mut node,
                                state,
                                enum_mapper,
                            )?
                        }
                    }
                    MatrixRenderType::ByteArray => {
                        render_varlen_u8_matrix(f, &content_area, &ds, &attr, &mut node, state)?
                    }
                },
            }
        }
        Some(ContentShowMode::Heatmap) => render_heatmap(f, &content_area, &mut node, state)?,
        None => render_custom_content_mode(f, &content_area, state, &display_mode)?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::h5f::{H5FNode, Node};

    use super::metadata_error_message;

    #[test]
    fn metadata_worker_failure_is_not_a_loading_state() {
        let mut node = H5FNode::new(Node::Broken("pending".to_string()));
        node.metadata_loading = true;
        node.metadata_error = Some("worker failed".to_string());

        assert_eq!(metadata_error_message(&node), Some("worker failed"));
    }
}
