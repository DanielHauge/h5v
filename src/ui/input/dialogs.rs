use ratatui::crossterm::event::{Event, KeyCode};

use crate::{
    error::AppError,
    h5f::FixedStringRewrite,
    ui::state::{AppToast, AttributeCreateField, FixedStringOverflowChoice, Mode},
};

use super::{super::state::AppState, is_handled_key_press, EventResult};

pub(super) fn handle_fixed_string_overflow_dialog(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    let Event::Key(key_event) = event else {
        return Ok(EventResult::Continue);
    };
    if !is_handled_key_press(&key_event) {
        return Ok(EventResult::Continue);
    }
    let Some(dialog) = state.fixed_string_overflow_dialog.as_mut() else {
        state.mode = Mode::Normal;
        return Ok(EventResult::Redraw);
    };

    match key_event.code {
        KeyCode::Esc => {
            state.fixed_string_overflow_dialog = None;
            state.mode = Mode::Normal;
            Ok(EventResult::Redraw)
        }
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h') | KeyCode::Char('k') => {
            dialog.selected_choice = match dialog.selected_choice {
                FixedStringOverflowChoice::Cancel => FixedStringOverflowChoice::ChangeSize,
                FixedStringOverflowChoice::ChangeToVarLen => FixedStringOverflowChoice::Cancel,
                FixedStringOverflowChoice::ChangeSize => FixedStringOverflowChoice::ChangeToVarLen,
            };
            Ok(EventResult::Redraw)
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('l') | KeyCode::Tab => {
            dialog.selected_choice = match dialog.selected_choice {
                FixedStringOverflowChoice::Cancel => FixedStringOverflowChoice::ChangeToVarLen,
                FixedStringOverflowChoice::ChangeToVarLen => FixedStringOverflowChoice::ChangeSize,
                FixedStringOverflowChoice::ChangeSize => FixedStringOverflowChoice::Cancel,
            };
            Ok(EventResult::Redraw)
        }
        KeyCode::Enter => match dialog.selected_choice {
            FixedStringOverflowChoice::Cancel => {
                state.fixed_string_overflow_dialog = None;
                state.mode = Mode::Normal;
                Ok(EventResult::Redraw)
            }
            FixedStringOverflowChoice::ChangeSize => {
                state.mode = Mode::FixedStringResizeDialog;
                Ok(EventResult::Redraw)
            }
            FixedStringOverflowChoice::ChangeToVarLen => {
                let (attr_name, new_value) =
                    (dialog.request.attr_name.clone(), dialog.new_value.clone());
                let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
                match selected_node.rewrite_fixed_string_attribute(
                    &attr_name,
                    &new_value,
                    FixedStringRewrite::ToVarLen,
                ) {
                    Ok(()) => {
                        drop(selected_node);
                        state.fixed_string_overflow_dialog = None;
                        state.mode = Mode::Normal;
                        state.acknowledge_file_write();
                        Ok(EventResult::ReloadFile {
                            write: !state.readonly,
                        })
                    }
                    Err(err) => Ok(EventResult::Toast(AppToast::Error(err.to_string()), false)),
                }
            }
        },
        _ => Ok(EventResult::Continue),
    }
}

pub(super) fn handle_attribute_create_dialog(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    let Event::Key(key_event) = event else {
        return Ok(EventResult::Continue);
    };
    if !is_handled_key_press(&key_event) {
        return Ok(EventResult::Continue);
    }
    let Some(dialog) = state.attribute_create_dialog.as_mut() else {
        state.mode = Mode::Normal;
        return Ok(EventResult::Toast(AppToast::Empty, true));
    };

    match key_event.code {
        KeyCode::Esc => {
            state.attribute_create_dialog = None;
            state.mode = Mode::Normal;
            Ok(EventResult::Toast(AppToast::Empty, true))
        }
        KeyCode::BackTab | KeyCode::Up => {
            dialog.active_field = match dialog.active_field {
                AttributeCreateField::Name => AttributeCreateField::Value,
                AttributeCreateField::Type => AttributeCreateField::Name,
                AttributeCreateField::Value => AttributeCreateField::Type,
            };
            Ok(EventResult::Redraw)
        }
        KeyCode::Tab | KeyCode::Down => {
            dialog.active_field = match dialog.active_field {
                AttributeCreateField::Name => AttributeCreateField::Type,
                AttributeCreateField::Type => AttributeCreateField::Value,
                AttributeCreateField::Value => AttributeCreateField::Name,
            };
            Ok(EventResult::Redraw)
        }
        KeyCode::Enter => match dialog.active_field {
            AttributeCreateField::Name => {
                dialog.active_field = AttributeCreateField::Type;
                Ok(EventResult::Redraw)
            }
            AttributeCreateField::Type => {
                dialog.active_field = AttributeCreateField::Value;
                Ok(EventResult::Redraw)
            }
            AttributeCreateField::Value => {
                let (name, attr_type, value) =
                    (dialog.name.clone(), dialog.attr_type, dialog.value.clone());
                let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
                let created_type = selected_node.create_attribute(&name, attr_type, &value)?;
                drop(selected_node);
                state.attribute_create_dialog = None;
                state.mode = Mode::Normal;
                state.acknowledge_file_write();
                Ok(EventResult::Toast(
                    AppToast::Info(format!("Created attribute '{}' ({})", name, created_type)),
                    true,
                ))
            }
        },
        KeyCode::Left | KeyCode::Char('h') if dialog.active_field == AttributeCreateField::Type => {
            let idx = crate::h5f::AttributeCreateType::ALL
                .iter()
                .position(|kind| *kind == dialog.attr_type)
                .unwrap_or(0);
            let prev = if idx == 0 {
                crate::h5f::AttributeCreateType::ALL.len() - 1
            } else {
                idx - 1
            };
            dialog.attr_type = crate::h5f::AttributeCreateType::ALL[prev];
            Ok(EventResult::Redraw)
        }
        KeyCode::Right | KeyCode::Char('l')
            if dialog.active_field == AttributeCreateField::Type =>
        {
            let idx = crate::h5f::AttributeCreateType::ALL
                .iter()
                .position(|kind| *kind == dialog.attr_type)
                .unwrap_or(0);
            dialog.attr_type = crate::h5f::AttributeCreateType::ALL
                [(idx + 1) % crate::h5f::AttributeCreateType::ALL.len()];
            Ok(EventResult::Redraw)
        }
        KeyCode::Backspace => {
            let (buffer, cursor) = match dialog.active_field {
                AttributeCreateField::Name => (&mut dialog.name, &mut dialog.name_cursor),
                AttributeCreateField::Type => return Ok(EventResult::Continue),
                AttributeCreateField::Value => (&mut dialog.value, &mut dialog.value_cursor),
            };
            if *cursor > 0 {
                *cursor -= 1;
                buffer.remove(*cursor);
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Delete => {
            let (buffer, cursor) = match dialog.active_field {
                AttributeCreateField::Name => (&mut dialog.name, &mut dialog.name_cursor),
                AttributeCreateField::Type => return Ok(EventResult::Continue),
                AttributeCreateField::Value => (&mut dialog.value, &mut dialog.value_cursor),
            };
            if *cursor < buffer.len() {
                buffer.remove(*cursor);
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Home => {
            match dialog.active_field {
                AttributeCreateField::Name => dialog.name_cursor = 0,
                AttributeCreateField::Type => {}
                AttributeCreateField::Value => dialog.value_cursor = 0,
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::End => {
            match dialog.active_field {
                AttributeCreateField::Name => dialog.name_cursor = dialog.name.len(),
                AttributeCreateField::Type => {}
                AttributeCreateField::Value => dialog.value_cursor = dialog.value.len(),
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Left if dialog.active_field != AttributeCreateField::Type => {
            match dialog.active_field {
                AttributeCreateField::Name => {
                    dialog.name_cursor = dialog.name_cursor.saturating_sub(1)
                }
                AttributeCreateField::Type => {}
                AttributeCreateField::Value => {
                    dialog.value_cursor = dialog.value_cursor.saturating_sub(1)
                }
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Right if dialog.active_field != AttributeCreateField::Type => {
            match dialog.active_field {
                AttributeCreateField::Name => {
                    dialog.name_cursor = (dialog.name_cursor + 1).min(dialog.name.len())
                }
                AttributeCreateField::Type => {}
                AttributeCreateField::Value => {
                    dialog.value_cursor = (dialog.value_cursor + 1).min(dialog.value.len())
                }
            }
            Ok(EventResult::Redraw)
        }
        KeyCode::Char(c) if c.is_ascii() && !c.is_ascii_control() => {
            let (buffer, cursor) = match dialog.active_field {
                AttributeCreateField::Name => (&mut dialog.name, &mut dialog.name_cursor),
                AttributeCreateField::Type => return Ok(EventResult::Continue),
                AttributeCreateField::Value => (&mut dialog.value, &mut dialog.value_cursor),
            };
            buffer.insert(*cursor, c);
            *cursor += 1;
            Ok(EventResult::Redraw)
        }
        _ => Ok(EventResult::Continue),
    }
}

pub(super) fn handle_attribute_delete_dialog(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    let Event::Key(key_event) = event else {
        return Ok(EventResult::Continue);
    };
    if !is_handled_key_press(&key_event) {
        return Ok(EventResult::Continue);
    }
    let Some(dialog) = state.attribute_delete_dialog.as_ref() else {
        state.mode = Mode::Normal;
        return Ok(EventResult::Toast(AppToast::Empty, true));
    };

    match key_event.code {
        KeyCode::Esc => {
            state.attribute_delete_dialog = None;
            state.mode = Mode::Normal;
            Ok(EventResult::Toast(AppToast::Empty, true))
        }
        KeyCode::Enter => {
            let attr_name = dialog.attr_name.clone();
            let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
            selected_node.delete_attribute(&attr_name)?;
            drop(selected_node);
            state.attribute_delete_dialog = None;
            state.mode = Mode::Normal;
            state.acknowledge_file_write();
            Ok(EventResult::Toast(
                AppToast::Info(format!("Deleted attribute '{}'", attr_name)),
                true,
            ))
        }
        _ => Ok(EventResult::Continue),
    }
}

pub(super) fn handle_fixed_string_resize_dialog(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    let Event::Key(key_event) = event else {
        return Ok(EventResult::Continue);
    };
    if !is_handled_key_press(&key_event) {
        return Ok(EventResult::Continue);
    }
    let Some(dialog) = state.fixed_string_overflow_dialog.as_mut() else {
        state.mode = Mode::Normal;
        return Ok(EventResult::Redraw);
    };

    match key_event.code {
        KeyCode::Esc => {
            state.mode = Mode::FixedStringOverflowDialog;
            Ok(EventResult::Redraw)
        }
        KeyCode::Backspace => {
            dialog.size_input.pop();
            Ok(EventResult::Redraw)
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            dialog.size_input.push(c);
            Ok(EventResult::Redraw)
        }
        KeyCode::Enter => {
            let new_size: usize = match dialog.size_input.parse() {
                Ok(size) => size,
                Err(_) => {
                    return Ok(EventResult::Toast(
                        AppToast::Error("Invalid fixed string size".to_string()),
                        false,
                    ))
                }
            };
            if new_size < dialog.overflow.required_size {
                return Ok(EventResult::Toast(
                    AppToast::Error(format!(
                        "New size must be at least {} bytes",
                        dialog.overflow.required_size
                    )),
                    false,
                ));
            }

            let (attr_name, new_value) =
                (dialog.request.attr_name.clone(), dialog.new_value.clone());
            let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
            match selected_node.rewrite_fixed_string_attribute(
                &attr_name,
                &new_value,
                FixedStringRewrite::Resize(new_size),
            ) {
                Ok(()) => {
                    drop(selected_node);
                    state.fixed_string_overflow_dialog = None;
                    state.mode = Mode::Normal;
                    state.acknowledge_file_write();
                    Ok(EventResult::ReloadFile {
                        write: !state.readonly,
                    })
                }
                Err(err) => Ok(EventResult::Toast(AppToast::Error(err.to_string()), false)),
            }
        }
        _ => Ok(EventResult::Continue),
    }
}
