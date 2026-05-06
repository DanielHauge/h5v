use std::{cell::RefCell, rc::Rc};

use hdf5_metno::types::{TypeDescriptor, VarLenUnicode};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
    Frame,
};

use crate::{
    color_consts,
    error::AppError,
    h5f::{H5FNode, Node},
    sprint_typedesc::MatrixRenderType,
    ui::{
        self,
        matrix::{DefaultMatrixResultRenderIntercept, EnumRenderer},
    },
};

use super::{
    attributes::{metadata_display_row_count, render_info_attributes},
    matrix::{render_matrix, render_not_yet_implemented, render_projected_matrix},
    preview::render_preview,
    state::{AppState, ContentShowMode},
    std_comp_render::render_empty_dataset,
};

fn split_main_display(area: Rect, attributes_count: usize) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(attributes_count.saturating_add(2).min(10) as u16),
            Constraint::Min(0),
        ])
        .split(area);
    (chunks[0], chunks[1])
}

pub fn render_main_display(
    f: &mut Frame,
    area: &Rect,
    selected_node_no: &Rc<RefCell<H5FNode>>,
    state: &mut AppState,
) -> std::result::Result<(), AppError> {
    let mut node = selected_node_no.borrow_mut();
    let attr_count = metadata_display_row_count(&mut node, area.width)?;

    let content_area = if state.show_tree_view {
        let (attr_area, content_area) = split_main_display(*area, attr_count);
        render_info_attributes(f, &attr_area, &mut node, state)?;
        content_area
    } else {
        *area
    };
    state.ui_layout.content = Some(content_area);
    state.ui_layout.content_tabs.clear();
    state.ui_layout.matrix_rows.clear();
    state.ui_layout.matrix_cells.clear();

    let current_display_mode = &state.content_mode;
    let supported_display_modes = node.content_show_modes();
    if supported_display_modes.is_empty() {
        let no_data_message = match &node.node {
            Node::Dataset(_, meta) if meta.is_compound_container() => "Compound",
            _ => "Group",
        };
        let paragraph = Paragraph::new(no_data_message)
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .bg(color_consts::BG_COLOR)
                    .fg(color_consts::TITLE),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, content_area);
        return Ok(());
    }
    let is_supported = supported_display_modes.contains(current_display_mode);
    let supported_modes_count = supported_display_modes.len();
    let display_mode = if is_supported {
        *current_display_mode
    } else {
        supported_display_modes
            .first()
            .copied()
            .unwrap_or(ContentShowMode::Preview)
    };
    let display_index = supported_display_modes
        .iter()
        .position(|x| *x == display_mode)
        .unwrap_or(0);

    // Do tab titles:

    let mut tab_titles = vec![];
    let mut tab_layout = Vec::new();
    for (i, x) in supported_display_modes.iter().enumerate() {
        let title = match x {
            ContentShowMode::Preview => "Preview📈",
            ContentShowMode::Matrix => "Matrix",
        };
        tab_layout.push((*x, title, Line::from(title).width() as u16));

        if i == display_index {
            tab_titles.push(Span::styled(title, color_consts::TITLE).bold().underlined());
        } else {
            tab_titles.push(Span::styled(title, color_consts::TITLE));
        }
        if i != supported_modes_count - 1 {
            tab_titles.push(Span::styled(" | ", ui::main_display::Color::Green));
        }
    }

    let title = Line::from(tab_titles);
    let title_width = title.width() as u16;
    let title_start_x = content_area
        .x
        .saturating_add(content_area.width.saturating_sub(title_width) / 2);
    let mut current_x = title_start_x;
    let separator_width = Line::from(" | ").width() as u16;
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
                mode: *mode,
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
        ) => color_consts::FOCUS_BG_COLOR,
        _ => color_consts::BG_COLOR,
    };
    let break_line = Block::default()
        .title(title)
        .borders(ratatui::widgets::Borders::TOP)
        .border_style(Style::default().fg(color_consts::BREAK_COLOR))
        .title_alignment(Alignment::Center)
        .title_style(Style::default().fg(color_consts::TITLE))
        .style(Style::default().bg(bg_color));
    f.render_widget(break_line, content_area);
    let available = node.content_show_modes();

    match state.content_show_mode_eval(available) {
        ContentShowMode::Preview => render_preview(f, &content_area, &mut node, state),
        ContentShowMode::Matrix => {
            //
            let (ds, attr) = match node.node.clone() {
                Node::Dataset(ds, attr) => (ds, attr),
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
                },
            }
        }
    }

    Ok(())
}
