use ratatui::crossterm::event::{Event, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

use crate::{
    error::AppError,
    ui::mchart::ChartZoomMode,
    ui::state::{AppState, AppToast, Mode},
};

use super::{
    execute_bound_command, execute_bound_lua_callback, execute_bound_script,
    keymap::{
        command_action, global_action, multichart_action, BoundAction, CommandAction,
        MultiChartAction,
    },
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
                    if matches!(
                        key_event.code,
                        ratatui::crossterm::event::KeyCode::Tab
                            | ratatui::crossterm::event::KeyCode::BackTab
                    ) {
                        state.multi_chart.expression_toggle_focus();
                        let file = state.file.clone();
                        state.multi_chart.refresh_expression_prompt(file.as_ref());
                        return Ok(EventResult::Redraw);
                    }
                    return match command_action(&key_event) {
                        Some(CommandAction::Submit) => {
                            if state.multi_chart.expression_has_selected_suggestion() {
                                if state.multi_chart.expression_apply_selected_suggestion() {
                                    let file = state.file.clone();
                                    state.multi_chart.refresh_expression_prompt(file.as_ref());
                                }
                            } else {
                                let file = state.file.clone();
                                let _ = state.multi_chart.submit_expression_prompt(file.as_ref());
                            }
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::Cancel) => {
                            if !state.multi_chart.expression_deselect_suggestion() {
                                state.multi_chart.close_expression_prompt();
                            }
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::Backspace) => {
                            state.multi_chart.expression_backspace();
                            let file = state.file.clone();
                            state.multi_chart.refresh_expression_prompt(file.as_ref());
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::Delete) => {
                            state.multi_chart.expression_delete();
                            let file = state.file.clone();
                            state.multi_chart.refresh_expression_prompt(file.as_ref());
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
                        Some(CommandAction::MoveWordLeft) => {
                            state.multi_chart.expression_move_word_left();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::MoveWordRight) => {
                            state.multi_chart.expression_move_word_right();
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
                            let file = state.file.clone();
                            state.multi_chart.refresh_expression_prompt(file.as_ref());
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::SelectPrevSuggestion) => {
                            state.multi_chart.expression_select_prev_suggestion();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::SelectNextSuggestion) => {
                            state.multi_chart.expression_select_next_suggestion();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::SelectPrevHistory) => {
                            state.multi_chart.expression_select_prev_suggestion();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::SelectNextHistory) => {
                            state.multi_chart.expression_select_next_suggestion();
                            Ok(EventResult::Redraw)
                        }
                        Some(CommandAction::InsertChar(c)) => {
                            state.multi_chart.expression_insert_char(c);
                            let file = state.file.clone();
                            state.multi_chart.refresh_expression_prompt(file.as_ref());
                            Ok(EventResult::Redraw)
                        }
                        _ => Ok(EventResult::Continue),
                    };
                }

                let keymaps = crate::configure::current_keymaps();
                match multichart_action(&key_event, &keymaps) {
                    Some(BoundAction::Action(MultiChartAction::EnterCommand)) => {
                        state.command_return_mode = Mode::MultiChart;
                        state.mode = Mode::Command;
                        state.command_state.begin_new_entry();
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::Exit)) => {
                        state.mode = Mode::Normal;
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::Quit)) => Ok(EventResult::Quit),
                    Some(BoundAction::Action(MultiChartAction::ShowHelp)) => {
                        state.help_return_mode = Mode::MultiChart;
                        state.mode = Mode::Help;
                        state.help.selected_tab = crate::ui::state::HelpTab::MultiChart;
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::CycleViewMode)) => {
                        state.multi_chart.cycle_view_mode();
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::ZoomIn)) => {
                        Ok(if state.multi_chart.zoom_in(10.0) {
                            EventResult::Redraw
                        } else {
                            EventResult::Continue
                        })
                    }
                    Some(BoundAction::Action(MultiChartAction::ZoomOut)) => {
                        Ok(if state.multi_chart.zoom_out(10.0) {
                            EventResult::Redraw
                        } else {
                            EventResult::Continue
                        })
                    }
                    Some(BoundAction::Action(MultiChartAction::PanLeft)) => {
                        Ok(if state.multi_chart.pan_left(10.0) {
                            EventResult::Redraw
                        } else {
                            EventResult::Continue
                        })
                    }
                    Some(BoundAction::Action(MultiChartAction::PanRight)) => {
                        Ok(if state.multi_chart.pan_right(10.0) {
                            EventResult::Redraw
                        } else {
                            EventResult::Continue
                        })
                    }
                    Some(BoundAction::Action(MultiChartAction::ClearZoom)) => {
                        Ok(if state.multi_chart.clear_zoom() {
                            EventResult::Redraw
                        } else {
                            EventResult::Continue
                        })
                    }
                    Some(BoundAction::Action(MultiChartAction::FitAll)) => {
                        Ok(if state.multi_chart.fit_all() {
                            EventResult::Redraw
                        } else {
                            EventResult::Continue
                        })
                    }
                    Some(BoundAction::Action(MultiChartAction::FitSelected)) => {
                        Ok(if state.multi_chart.fit_selected() {
                            EventResult::Redraw
                        } else {
                            EventResult::Continue
                        })
                    }
                    Some(BoundAction::Action(MultiChartAction::DeleteSelected)) => {
                        if let Err(error) = state.multi_chart.clear_selected() {
                            return Ok(EventResult::Toast(AppToast::Warning(error), false));
                        }
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::ClearAll)) => {
                        state.multi_chart.clear_all();
                        state.compute_tree_view();
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::ToggleSelectedVisible)) => {
                        state.multi_chart.toggle_selected_visible();
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::OpenExpressionPrompt)) => {
                        state.multi_chart.open_expression_prompt();
                        let file = state.file.clone();
                        state.multi_chart.refresh_expression_prompt(file.as_ref());
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::EditSelectedExpression)) => {
                        if let Err(error) = state.multi_chart.open_selected_item_for_edit() {
                            return Ok(EventResult::Toast(AppToast::Warning(error), false));
                        }
                        let file = state.file.clone();
                        state.multi_chart.refresh_expression_prompt(file.as_ref());
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::MoveDown)) => {
                        state.multi_chart.move_down();
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Action(MultiChartAction::MoveUp)) => {
                        state.multi_chart.move_up();
                        Ok(EventResult::Redraw)
                    }
                    Some(BoundAction::Command(command)) => execute_bound_command(state, &command),
                    Some(BoundAction::Script(script)) => {
                        execute_bound_script(state, &script, "keybinding script")
                    }
                    Some(BoundAction::LuaCallback(callback_id)) => {
                        execute_bound_lua_callback(state, &callback_id)
                    }
                    None => match global_action(&key_event, &keymaps) {
                        Some(BoundAction::Action(super::keymap::GlobalAction::EnterCommand)) => {
                            state.command_return_mode = Mode::MultiChart;
                            state.mode = Mode::Command;
                            state.command_state.begin_new_entry();
                            Ok(EventResult::Redraw)
                        }
                        Some(BoundAction::Action(action)) => {
                            super::handle_global_action(state, action)
                        }
                        Some(BoundAction::Command(command)) => {
                            execute_bound_command(state, &command)
                        }
                        Some(BoundAction::Script(script)) => {
                            execute_bound_script(state, &script, "keybinding script")
                        }
                        Some(BoundAction::LuaCallback(callback_id)) => {
                            execute_bound_lua_callback(state, &callback_id)
                        }
                        None => Ok(EventResult::Continue),
                    },
                }
            }
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Mouse(mouse_event) => match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if state
                    .multi_chart
                    .click_view_mode_hitbox(mouse_event.column, mouse_event.row)
                {
                    return Ok(EventResult::Redraw);
                }
                if state
                    .multi_chart
                    .click_item_hitbox(mouse_event.column, mouse_event.row)
                {
                    return Ok(EventResult::Redraw);
                }
                let file = state.file.clone();
                match state
                    .multi_chart
                    .click_expression_editor(mouse_event.column, mouse_event.row)
                {
                    Ok(true) => {
                        state.multi_chart.refresh_expression_prompt(file.as_ref());
                        Ok(EventResult::Redraw)
                    }
                    Ok(false) => Ok(EventResult::Continue),
                    Err(error) => Ok(EventResult::Toast(AppToast::Warning(error), false)),
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                state
                    .multi_chart
                    .start_drag_at_position(mouse_event.column, mouse_event.row);
                Ok(EventResult::Continue)
            }
            MouseEventKind::Drag(MouseButton::Right) => {
                if state
                    .multi_chart
                    .drag_to_position(mouse_event.column, mouse_event.row)
                {
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Continue)
                }
            }
            MouseEventKind::Up(MouseButton::Right) => {
                if state
                    .multi_chart
                    .finish_drag_at_position(mouse_event.column, mouse_event.row)
                {
                    Ok(EventResult::Redraw)
                } else {
                    state.multi_chart.end_drag();
                    Ok(EventResult::Continue)
                }
            }
            MouseEventKind::ScrollUp => {
                let zoom_mode = mouse_zoom_mode(mouse_event.modifiers);
                if state.multi_chart.zoom_in_at_position(
                    mouse_event.column,
                    mouse_event.row,
                    10.0,
                    zoom_mode,
                ) {
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Continue)
                }
            }
            MouseEventKind::ScrollDown => {
                let zoom_mode = mouse_zoom_mode(mouse_event.modifiers);
                if state.multi_chart.zoom_out_at_position(
                    mouse_event.column,
                    mouse_event.row,
                    10.0,
                    zoom_mode,
                ) {
                    Ok(EventResult::Redraw)
                } else {
                    Ok(EventResult::Continue)
                }
            }
            _ => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}

fn mouse_zoom_mode(modifiers: KeyModifiers) -> ChartZoomMode {
    if modifiers.contains(KeyModifiers::CONTROL) {
        ChartZoomMode::XOnly
    } else if modifiers.contains(KeyModifiers::SHIFT) {
        ChartZoomMode::YOnly
    } else {
        ChartZoomMode::Uniform
    }
}
