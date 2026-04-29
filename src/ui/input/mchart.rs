use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::{
    error::AppError,
    ui::state::{AppState, Mode},
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
                Some(MultiChartAction::MoveDown) => {
                    state.multi_chart.idx = state
                        .multi_chart
                        .idx
                        .saturating_add(1)
                        .clamp(0, state.multi_chart.line_series.len().saturating_sub(1));
                    Ok(EventResult::Redraw)
                }
                Some(MultiChartAction::MoveUp) => {
                    state.multi_chart.idx = state.multi_chart.idx.saturating_sub(1);
                    Ok(EventResult::Redraw)
                }
                _ => Ok(EventResult::Continue),
            },
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
