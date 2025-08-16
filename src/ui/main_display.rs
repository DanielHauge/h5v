use std::{cell::RefCell, rc::Rc};

use hdf5_metno::types::VarLenUnicode;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Span,
    widgets::{block::Title, Block, Paragraph, Wrap},
    Frame,
};

use crate::{
    color_consts,
    error::AppError,
    h5f::{H5FNode, Node},
    sprint_typedesc::MatrixRenderType,
    ui,
};

use super::{
    attributes::render_info_attributes,
    matrix::{render_matrix, render_not_yet_implemented},
    preview::render_preview,
    state::{AppState, ContentShowMode},
};

fn split_main_display(area: Rect, attributes_count: usize) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                Constraint::Length(attributes_count as u16 + 2),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
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
    let attr_count = node.read_attributes()?.rendered_attributes.len();

    let content_area = if state.show_tree_view {
        let (attr_area, content_area) = split_main_display(*area, attr_count);
        render_info_attributes(f, &attr_area, &mut node, state)?;
        content_area
    } else {
        *area
    };

    let current_display_mode = &state.content_mode;
    let supported_display_modes = node.content_show_modes();
    if supported_display_modes.is_empty() {
        let no_data_message = "Group";
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
        current_display_mode
    } else {
        &supported_display_modes[0]
    };
    let display_index = supported_display_modes
        .iter()
        .position(|x| x == display_mode)
        .expect("Display mode expected to be found in list otherwise not reach this point");

    // Do tab titles:

    let mut tab_titles = vec![];
    for (i, x) in supported_display_modes.iter().enumerate() {
        let title = match x {
            ContentShowMode::Preview => "Preview📈",
            ContentShowMode::Matrix => "Matrix",
        };

        if i == display_index {
            tab_titles.push(Span::styled(title, color_consts::TITLE).bold().underlined());
        } else {
            tab_titles.push(Span::styled(title, color_consts::TITLE));
        }
        if i != supported_modes_count - 1 {
            tab_titles.push(Span::styled(" | ", ui::main_display::Color::Green));
        }
    }

    let title = Title::from(tab_titles);

    let bg_color = match (&state.focus, &state.mode) {
        (ui::state::Focus::Content, ui::state::Mode::Normal) => color_consts::FOCUS_BG_COLOR,
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
                    unreachable!("Should not render matrix for anything other than dataset")
                }
            };
            match attr.matrixable {
                None => {
                    return Ok(());
                }
                Some(x) => match x {
                    MatrixRenderType::Float64 => {
                        render_matrix::<f64>(f, &content_area, &ds, &attr, &mut node, state)?
                    }
                    MatrixRenderType::Uint64 => {
                        render_matrix::<u64>(f, &content_area, &ds, &attr, &mut node, state)?
                    }
                    MatrixRenderType::Int64 => {
                        render_matrix::<i64>(f, &content_area, &ds, &attr, &mut node, state)?
                    }
                    MatrixRenderType::Compound => {
                        render_not_yet_implemented(f, &content_area, "Compound matrix")
                    }
                    MatrixRenderType::Strings => render_matrix::<VarLenUnicode>(
                        f,
                        &content_area,
                        &ds,
                        &attr,
                        &mut node,
                        state,
                    )?,
                },
            }
        }
    }

    Ok(())
}
