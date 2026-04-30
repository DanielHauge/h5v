use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::{
    error::AppError,
    ui::state::{AppState, AppToast, Mode},
};

use super::{
    keymap::{multichart_action, MultiChartAction},
    EventResult,
};

pub(crate) fn handle_mchart_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match multichart_action(&key_event) {
                Some(MultiChartAction::Exit) => {
                    state.mode = Mode::Normal;
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::Quit) => Ok(EventResult::Quit),
                Some(MultiChartAction::ZoomIn) => {
                    state.multi_chart.zoom_in(10.0);
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::ZoomOut) => {
                    state.multi_chart.zoom_out(10.0);
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::PanLeft) => {
                    state.multi_chart.pan_left(10.0);
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::PanRight) => {
                    state.multi_chart.pan_right(10.0);
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::ClearZoom) => {
                    state.multi_chart.clear_zoom();
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::DeleteSelected) => {
                    state.multi_chart.clear_selected();
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::ClearAll) => {
                    state.multi_chart.clear_all();
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::ToggleSelectedVisible) => {
                    state.multi_chart.toggle_selected_visible();
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::ToggleMarkedBase) => {
                    match state.multi_chart.toggle_marked_base() {
                        Ok(_) => Ok(EventResult::Redraw),
                        Err(message) => Ok(EventResult::Toast(AppToast::Warning(message), false)),
                    }
                }
                Some(MultiChartAction::CreateDerived(operation)) => {
                    match state.multi_chart.create_builtin_derived(operation) {
                        Ok(_) => Ok(EventResult::Redraw),
                        Err(message) => Ok(EventResult::Toast(AppToast::Warning(message), false)),
                    }
                }
                Some(MultiChartAction::MoveDown) => {
                    state.multi_chart.move_down();
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::MoveUp) => {
                    state.multi_chart.move_up();
                    Ok(EventResult::Redraw)
                }
                None => Ok(EventResult::Continue),
            },
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
