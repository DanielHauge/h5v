use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::{
    error::AppError,
    ui::state::{AppState, AppToast, Mode},
};

use super::{
    keymap::{command_action, multichart_action, CommandAction, MultiChartAction},
    EventResult,
};

pub(crate) fn handle_mchart_event(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => {
                if state.multi_chart.is_expression_prompt_active() {
                    return match command_action(&key_event) {
                        Some(CommandAction::Submit) => {
                            let file = state.file.clone();
                            let _ = state.multi_chart.submit_expression_prompt(file.as_ref());
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::Cancel) => {
                            state.multi_chart.close_expression_prompt();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::Backspace) => {
                            state.multi_chart.expression_backspace();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::Delete) => {
                            state.multi_chart.expression_delete();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::MoveLeft) => {
                            state.multi_chart.expression_move_left();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::MoveRight) => {
                            state.multi_chart.expression_move_right();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::MoveToStart) => {
                            state.multi_chart.expression_move_to_start();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::MoveToEnd) => {
                            state.multi_chart.expression_move_to_end();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::Clear) | Some(CommandAction::ClearWord) => {
                            state.multi_chart.expression_clear();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::InsertChar(c)) => {
                            state.multi_chart.expression_insert_char(c);
                            Ok(EventResult::Redraw)
                        }
                        _ => Ok(EventResult::Continue),
                    };
                }

                match multichart_action(&key_event) {
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
                    Some(MultiChartAction::OpenExpressionPrompt) => {
                        state.multi_chart.open_expression_prompt();
                        Ok(EventResult::Redraw)
                    }
                    Some(MultiChartAction::ToggleMarkedBase) => {
                        match state.multi_chart.toggle_marked_base() {
                            Ok(_) => Ok(EventResult::Redraw),
                            Err(message) => {
                                Ok(EventResult::Toast(AppToast::Warning(message), false))
                            }
                        }
                    }
                    Some(MultiChartAction::CreateDerived(operation)) => {
                        match state.multi_chart.create_builtin_derived(operation) {
                            Ok(_) => Ok(EventResult::Redraw),
                            Err(message) => {
                                Ok(EventResult::Toast(AppToast::Warning(message), false))
                            }
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
                }
            }
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
