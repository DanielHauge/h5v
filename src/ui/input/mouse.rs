use ratatui::{
    crossterm::event::{MouseButton, MouseEvent, MouseEventKind},
    layout::Rect,
};

use crate::{
    error::AppError,
    ui::state::{AttributeViewSelection, ContentShowMode, Focus, Mode},
};

use super::{super::state::AppState, EventResult};

pub(super) fn handle_global_mouse_event(
    state: &mut AppState<'_>,
    mouse_event: MouseEvent,
) -> Result<Option<EventResult>, AppError> {
    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if state
                .ui_layout
                .mchart_toggle
                .is_some_and(|rect| point_in_rect(rect, mouse_event.column, mouse_event.row))
            {
                state.mode = if matches!(state.mode, Mode::MultiChart) {
                    Mode::Normal
                } else {
                    Mode::MultiChart
                };
                state.pending_chord = None;
                return Ok(Some(EventResult::Redraw));
            }
            if state
                .ui_layout
                .help_toggle
                .is_some_and(|rect| point_in_rect(rect, mouse_event.column, mouse_event.row))
            {
                if matches!(state.mode, Mode::Help) {
                    state.mode = state.help_return_mode.clone();
                } else {
                    state.help_return_mode = state.mode.clone();
                    state.help.scroll_offset = 0;
                    state.mode = Mode::Help;
                }
                state.pending_chord = None;
                return Ok(Some(EventResult::Redraw));
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

pub(super) fn handle_normal_mouse_event(
    state: &mut AppState<'_>,
    mouse_event: MouseEvent,
) -> Result<EventResult, AppError> {
    state.pending_chord = None;

    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            handle_left_click(state, mouse_event.column, mouse_event.row, false)
        }
        MouseEventKind::Down(MouseButton::Right) => {
            handle_right_mouse_down(state, mouse_event.column, mouse_event.row)
        }
        MouseEventKind::Drag(MouseButton::Right) => {
            handle_right_mouse_drag(state, mouse_event.column, mouse_event.row)
        }
        MouseEventKind::Up(MouseButton::Right) => {
            handle_right_mouse_up(state, mouse_event.column, mouse_event.row)
        }
        MouseEventKind::ScrollUp => {
            handle_heatmap_scroll(state, mouse_event.column, mouse_event.row, true)
        }
        MouseEventKind::ScrollDown => {
            handle_heatmap_scroll(state, mouse_event.column, mouse_event.row, false)
        }
        _ => Ok(EventResult::Continue),
    }
}

pub(super) fn handle_help_mouse_event(
    state: &mut AppState<'_>,
    mouse_event: MouseEvent,
) -> Result<EventResult, AppError> {
    state.pending_chord = None;
    let column = mouse_event.column;
    let row = mouse_event.row;
    match mouse_event.kind {
        MouseEventKind::ScrollUp => {
            let max_scroll = state
                .ui_layout
                .help_scrollbar
                .map(|hitbox| hitbox.content_lines.saturating_sub(hitbox.viewport_lines))
                .unwrap_or(0);
            if state.help_scroll_by(-3, max_scroll) {
                return Ok(EventResult::Redraw);
            }
            return Ok(EventResult::Continue);
        }
        MouseEventKind::ScrollDown => {
            let max_scroll = state
                .ui_layout
                .help_scrollbar
                .map(|hitbox| hitbox.content_lines.saturating_sub(hitbox.viewport_lines))
                .unwrap_or(0);
            if state.help_scroll_by(3, max_scroll) {
                return Ok(EventResult::Redraw);
            }
            return Ok(EventResult::Continue);
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {}
        _ => return Ok(EventResult::Continue),
    }

    if let Some(scrollbar) = state.ui_layout.help_scrollbar {
        if point_in_rect(scrollbar.area, column, row)
            && scrollbar.area.height > 0
            && scrollbar.content_lines > scrollbar.viewport_lines
        {
            let max_scroll = scrollbar
                .content_lines
                .saturating_sub(scrollbar.viewport_lines);
            let row_offset = row.saturating_sub(scrollbar.area.y) as usize;
            let track_len = scrollbar.area.height.saturating_sub(1) as usize;
            let target = if track_len == 0 {
                0
            } else {
                row_offset.saturating_mul(max_scroll) / track_len
            };
            if state.help_set_scroll(target, max_scroll) {
                return Ok(EventResult::Redraw);
            }
            return Ok(EventResult::Continue);
        }
    }

    if state
        .ui_layout
        .help_top_bar
        .is_some_and(|rect| point_in_rect(rect, column, row))
    {
        state.mode = state.help_return_mode.clone();
        return Ok(EventResult::Redraw);
    }

    if let Some(tab_hitbox) = state
        .ui_layout
        .help_tabs
        .iter()
        .find(|tab| point_in_rect(tab.area, column, row))
        .copied()
    {
        state.help.selected_tab = tab_hitbox.tab;
        state.help.scroll_offset = 0;
        return Ok(EventResult::Redraw);
    }

    if let Some(item_hitbox) = state
        .ui_layout
        .help_sidebar_items
        .iter()
        .find(|item| point_in_rect(item.area, column, row))
        .copied()
    {
        match item_hitbox.target {
            super::super::state::HelpSidebarTarget::Keymap(section) => {
                state.help.selected_tab = super::super::state::HelpTab::Keymap;
                state.help.keymap_section = section;
            }
            super::super::state::HelpSidebarTarget::Command(section) => {
                state.help.selected_tab = super::super::state::HelpTab::Commands;
                state.help.command_section = section;
            }
            super::super::state::HelpSidebarTarget::Customization(section) => {
                state.help.selected_tab = super::super::state::HelpTab::Configuration;
                state.help.customization_section = section;
            }
            super::super::state::HelpSidebarTarget::MultiChart(section) => {
                state.help.selected_tab = super::super::state::HelpTab::MultiChart;
                state.help.multichart_section = section;
            }
        }
        state.help.scroll_offset = 0;
        return Ok(EventResult::Redraw);
    }

    Ok(EventResult::Continue)
}

fn handle_left_click(
    state: &mut AppState<'_>,
    column: u16,
    row: u16,
    toggle_if_selected: bool,
) -> Result<EventResult, AppError> {
    if let Some(tab_hitbox) = state
        .ui_layout
        .content_tabs
        .iter()
        .find(|tab| point_in_rect(tab.area, column, row))
        .copied()
    {
        state.set_content_mode(tab_hitbox.mode);
        state.focus = Focus::Content;
        return Ok(EventResult::Redraw);
    }

    if let Some(matrix_cell) = state
        .ui_layout
        .matrix_cells
        .iter()
        .find(|cell| point_in_rect(cell.area, column, row))
        .copied()
    {
        state.focus = Focus::Content;
        if state.active_content_mode() == ContentShowMode::Heatmap {
            state.heatmap_select_cell(matrix_cell.row, matrix_cell.col);
        } else {
            state.matrix_view_state.cursor_row = matrix_cell.row;
            state.matrix_view_state.cursor_col = matrix_cell.col;
        }
        return Ok(EventResult::Redraw);
    }

    if let Some(setting_hitbox) = state
        .ui_layout
        .heatmap_settings
        .iter()
        .find(|hitbox| point_in_rect(hitbox.area, column, row))
        .copied()
    {
        state.focus = Focus::Content;
        state.heatmap_render.selected_setting = setting_hitbox.setting;
        return Ok(EventResult::Redraw);
    }

    if let Some(matrix_row) = state
        .ui_layout
        .matrix_rows
        .iter()
        .find(|row_hitbox| point_in_rect(row_hitbox.area, column, row))
        .copied()
    {
        state.focus = Focus::Content;
        state.matrix_view_state.cursor_row = matrix_row.row;
        return Ok(EventResult::Redraw);
    }

    if let Some(tree) = state.ui_layout.tree {
        if point_in_rect(tree.outer, column, row) {
            state.focus_tree_from_current();
            if point_in_rect(tree.inner, column, row) {
                let clicked_row = row.saturating_sub(tree.inner.y) as usize;
                let clicked_index = tree.row_offset.saturating_add(clicked_row);
                if clicked_row < tree.visible_rows && clicked_index < state.treeview.len() {
                    let was_selected = state.tree_view_cursor == clicked_index;
                    state.tree_view_cursor = clicked_index;
                    if was_selected || toggle_if_selected {
                        let tree_item = &state.treeview[clicked_index];
                        if tree_item.load_more {
                            tree_item.node.borrow_mut().view_loaded += 50;
                            state.compute_tree_view();
                            return Ok(EventResult::Redraw);
                        }
                        if tree_item.node.borrow().is_expandable() {
                            tree_item.node.borrow_mut().expand_toggle()?;
                            state.compute_tree_view();
                            return Ok(EventResult::Redraw);
                        }
                    }
                }
            }
            return Ok(EventResult::Redraw);
        }
    }

    if let Some(attributes) = state.ui_layout.attributes.clone() {
        if point_in_rect(attributes.outer, column, row) {
            state.focus = Focus::Attributes;
            if point_in_rect(attributes.inner, column, row) {
                if let Some(cell) = attributes.cells.iter().find(|cell| {
                    point_in_rect(cell.name_area, column, row)
                        || point_in_rect(cell.value_area, column, row)
                }) {
                    let selection = if point_in_rect(cell.name_area, column, row) {
                        AttributeViewSelection::Name
                    } else {
                        AttributeViewSelection::Value
                    };
                    if let Some(tree_item) = state.treeview.get(state.tree_view_cursor) {
                        let mut node = tree_item.node.borrow_mut();
                        node.attributes_view_cursor.attribute_index = cell.row_index;
                        node.attributes_view_cursor.attribute_view_selection = selection;
                    }
                }
            }
            return Ok(EventResult::Redraw);
        }
    }

    if let Some(content) = state.ui_layout.content {
        if point_in_rect(content, column, row) {
            state.focus = Focus::Content;
            return Ok(EventResult::Redraw);
        }
    }

    Ok(EventResult::Continue)
}

fn handle_heatmap_scroll(
    state: &mut AppState<'_>,
    column: u16,
    row: u16,
    zoom_in: bool,
) -> Result<EventResult, AppError> {
    if state.active_content_mode() != ContentShowMode::Heatmap {
        return Ok(EventResult::Continue);
    }
    let Some(matrix_cell) = state
        .ui_layout
        .matrix_cells
        .iter()
        .find(|cell| point_in_rect(cell.area, column, row))
        .copied()
    else {
        return Ok(EventResult::Continue);
    };
    state.focus = Focus::Content;
    if state.zoom_heatmap_step(Some((matrix_cell.row, matrix_cell.col)), zoom_in) {
        Ok(EventResult::Redraw)
    } else {
        Ok(EventResult::Continue)
    }
}

fn handle_right_mouse_down(
    state: &mut AppState<'_>,
    column: u16,
    row: u16,
) -> Result<EventResult, AppError> {
    if state.active_content_mode() != ContentShowMode::Heatmap {
        return Ok(EventResult::Continue);
    }
    let Some(matrix_cell) = state
        .ui_layout
        .matrix_cells
        .iter()
        .find(|cell| point_in_rect(cell.area, column, row))
        .copied()
    else {
        return Ok(EventResult::Continue);
    };
    state.focus = Focus::Content;
    if state.start_heatmap_drag(matrix_cell.row, matrix_cell.col) {
        Ok(EventResult::Redraw)
    } else {
        Ok(EventResult::Continue)
    }
}

fn handle_right_mouse_drag(
    state: &mut AppState<'_>,
    _column: u16,
    _row: u16,
) -> Result<EventResult, AppError> {
    if state.active_content_mode() == ContentShowMode::Heatmap
        && state.heatmap_render.drag_state.is_some()
    {
        return Ok(EventResult::Continue);
    }
    Ok(EventResult::Continue)
}

fn handle_right_mouse_up(
    state: &mut AppState<'_>,
    column: u16,
    row: u16,
) -> Result<EventResult, AppError> {
    if state.active_content_mode() != ContentShowMode::Heatmap {
        return Ok(EventResult::Continue);
    }
    let Some(drag_state) = state.heatmap_render.drag_state else {
        return Ok(EventResult::Continue);
    };
    let release_cell = state
        .ui_layout
        .matrix_cells
        .iter()
        .find(|cell| point_in_rect(cell.area, column, row))
        .map(|cell| (cell.row, cell.col))
        .unwrap_or((drag_state.anchor_row, drag_state.anchor_col));
    state.focus = Focus::Content;
    if release_cell == (drag_state.anchor_row, drag_state.anchor_col) {
        state.end_heatmap_drag();
        if state.heatmap_render.selected_cells.is_some() && state.zoom_heatmap(None, true) {
            return Ok(EventResult::Redraw);
        }
        return Ok(EventResult::Continue);
    }
    if state.finish_heatmap_drag(release_cell.0, release_cell.1) {
        Ok(EventResult::Redraw)
    } else {
        Ok(EventResult::Continue)
    }
}

fn point_in_rect(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}
